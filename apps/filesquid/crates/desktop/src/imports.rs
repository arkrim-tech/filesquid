//! The desktop's import route (Task DE-1.2; API-S-036, DE-S-019): the host's
//! [`ImportStarter`] behind `POST /api/v1/imports`.
//!
//! A request whose operation ID already started a job answers that job. Otherwise the owner
//! picks files or a folder in a native dialog the host shows from Rust ([`RfdSources`], the
//! `rfd` crate, so the webview keeps no dialog permission, DE-S-002), the host keeps the plan
//! in its operational store, submits one durable import job and queues it on the native import
//! worker. Only opaque IDs leave the host; the picked paths never do (DE-S-006).

use filesquid_api::imports::{
    ImportAnswer, ImportOutcome, ImportRequest, ImportSource, ImportStartError, ImportStarter,
};
use filesquid_api::jobs::Jobs;
use filesquid_core::LibraryId;
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::job::submit_import;
use filesquid_native::import::plans::{ImportPlan, SqliteImportPlans};
use filesquid_native::import::worker::ImportWorker;
use filesquid_native::HostFiles;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// A native dialog in which the owner picks what to import.
pub trait SourceDialog: Send + Sync {
    /// Show the dialog for `source` and answer what the owner picked, or `None` when they
    /// cancelled. Blocks until the dialog closes.
    ///
    /// # Errors
    /// [`ImportStartError::Unavailable`] when no dialog can be shown.
    fn pick(&self, source: ImportSource) -> Result<Option<Vec<PathBuf>>, ImportStartError>;
}

/// The platform's native file or folder dialog through `rfd`, parented to the main window
/// where the backend supports a parent (DE-S-017's rule for the folder choice).
#[derive(Clone)]
pub struct RfdSources {
    app: tauri::AppHandle,
}

impl std::fmt::Debug for RfdSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RfdSources").finish_non_exhaustive()
    }
}

impl RfdSources {
    /// The dialog of the app `app`. Pure.
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl SourceDialog for RfdSources {
    fn pick(&self, source: ImportSource) -> Result<Option<Vec<PathBuf>>, ImportStartError> {
        let mut dialog = rfd::AsyncFileDialog::new();
        if let Some(window) = self.app.get_webview_window(crate::app_window::MAIN_WINDOW) {
            dialog = dialog.set_parent(&window);
        }
        let picked = match source {
            ImportSource::Files => {
                let dialog = dialog.set_title("Choose files to import");
                tauri::async_runtime::block_on(dialog.pick_files())
            }
            ImportSource::Folder => {
                let dialog = dialog.set_title("Choose a folder to import");
                tauri::async_runtime::block_on(dialog.pick_folder()).map(|folder| vec![folder])
            }
        };
        Ok(picked.map(|handles| {
            handles
                .iter()
                .map(|handle| handle.path().to_path_buf())
                .collect()
        }))
    }
}

/// What the desktop's import starter works with.
pub struct ImportStarting {
    /// The file service the libraries are open in.
    pub files: HostFiles,
    /// The job service.
    pub jobs: Jobs,
    /// Each job's plan.
    pub plans: Arc<SqliteImportPlans>,
    /// The worker the jobs run on.
    pub worker: Arc<ImportWorker>,
    /// The native dialog.
    pub dialog: Box<dyn SourceDialog>,
}

/// The desktop's import starter: one dialog at a time.
pub struct DesktopImports {
    starting: ImportStarting,
    open: AtomicBool,
}

impl std::fmt::Debug for DesktopImports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopImports").finish_non_exhaustive()
    }
}

impl DesktopImports {
    /// The starter over `starting`. Pure.
    pub fn new(starting: ImportStarting) -> Self {
        Self {
            starting,
            open: AtomicBool::new(false),
        }
    }

    /// Show the dialog, one at a time.
    fn pick(&self, source: ImportSource) -> Result<Option<Vec<PathBuf>>, ImportStartError> {
        if self.open.swap(true, Ordering::SeqCst) {
            return Err(ImportStartError::Busy);
        }
        let picked = self.starting.dialog.pick(source);
        self.open.store(false, Ordering::SeqCst);
        picked
    }

    /// Submit and queue the job of `plan` for `operation_id`. Writes the job store and the
    /// plans.
    fn submit(&self, operation_id: &str, plan: &ImportPlan) -> Result<String, ImportStartError> {
        let job = submit_import(&self.starting.jobs, plan, operation_id)
            .map_err(|_| ImportStartError::Unavailable)?;
        self.starting
            .plans
            .save(&job.id, operation_id, plan)
            .map_err(|_| ImportStartError::Unavailable)?;
        self.starting.worker.enqueue(&job.id).map_err(|code| {
            tracing::warn!(
                operation = filesquid_api::imports::IMPORT_START_OPERATION,
                error_code = code,
                "the import job waits for the next launch"
            );
            ImportStartError::Unavailable
        })?;
        Ok(job.id)
    }
}

/// The answer naming `job_id` for `operation_id`. Pure.
fn started(operation_id: &str, job_id: String) -> ImportAnswer {
    ImportAnswer {
        operation_id: operation_id.to_string(),
        outcome: ImportOutcome::Started,
        job_id: Some(job_id),
    }
}

/// The import method of a picked source. Pure.
fn method_of(source: ImportSource) -> ImportMethod {
    match source {
        ImportSource::Files => ImportMethod::FilePicker,
        ImportSource::Folder => ImportMethod::Folder,
    }
}

impl ImportStarter for DesktopImports {
    fn start(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        let operation_id = request.envelope.operation_id().as_str();
        let known = self.starting.plans.job_of_operation(operation_id);
        if let Some(job_id) = known.map_err(|_| ImportStartError::Unavailable)? {
            return Ok(started(operation_id, job_id));
        }
        let library =
            LibraryId::new(&request.library_id).map_err(|_| ImportStartError::LibraryNotFound)?;
        if !self.starting.files.is_open(&library) {
            return Err(ImportStartError::LibraryNotFound);
        }
        let Some(sources) = self.pick(request.source)? else {
            return Ok(ImportAnswer {
                operation_id: operation_id.to_string(),
                outcome: ImportOutcome::Cancelled,
                job_id: None,
            });
        };
        let plan = ImportPlan {
            library_id: request.library_id.clone(),
            collection_id: request.collection_id.clone(),
            sources,
            method: method_of(request.source),
        };
        self.submit(operation_id, &plan)
            .map(|job_id| started(operation_id, job_id))
    }
}
