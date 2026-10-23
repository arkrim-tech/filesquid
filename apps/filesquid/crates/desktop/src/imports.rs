//! The desktop's import route (Task DE-1.2; API-S-036, DE-S-019): the host's
//! [`ImportStarter`] behind `POST /api/v1/imports`.
//!
//! A request whose operation ID already started a job answers that job. Otherwise the owner
//! picks files or a folder in a native dialog the host shows from Rust ([`RfdSources`], the
//! `rfd` crate, so the webview keeps no dialog permission, DE-S-002), the host submits one
//! durable import job, keeps its plan (with the destination collection and folder the
//! request named, which the API checked before the dialog, Task DE-1.7) in its operational
//! store (a job whose plan cannot be kept is failed at once, so none is left without one) and
//! queues it on the native import worker. The check for an existing job and the submission run under one lock, so two
//! requests can never make two jobs for one operation ID. Only opaque IDs leave the host;
//! the picked paths never do (DE-S-006).

use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportSource, ImportStartError, ImportStarter,
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
use std::sync::{Arc, Mutex};
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

/// The desktop's import starter: one dialog at a time, one start at a time.
pub struct DesktopImports {
    starting: ImportStarting,
    open: AtomicBool,
    starts: Mutex<()>,
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
            starts: Mutex::new(()),
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

    /// Submit and queue the job of `plan` for `operation_id`; a job whose plan cannot be
    /// kept is failed. Writes the job store and the plans.
    fn submit(&self, operation_id: &str, plan: &ImportPlan) -> Result<String, ImportStartError> {
        let job = submit_import(&self.starting.jobs, plan, operation_id)
            .map_err(|_| ImportStartError::Unavailable)?;
        if self
            .starting
            .plans
            .save(&job.id, operation_id, plan)
            .is_err()
        {
            self.fail_unplanned(&job.id, &job.source);
            return Err(ImportStartError::Unavailable);
        }
        self.starting
            .worker
            .enqueue(&job.id, &job.source)
            .map_err(|code| {
                tracing::warn!(
                    operation = filesquid_api::imports::IMPORT_START_OPERATION,
                    error_code = code,
                    "the import job waits for the next launch"
                );
                ImportStartError::Unavailable
            })?;
        Ok(job.id)
    }

    /// Fail the job `job_id` (source identity `source`) whose plan could not be kept, logging
    /// a failure to do so.
    fn fail_unplanned(&self, job_id: &str, source: &str) {
        let failed = self
            .starting
            .jobs
            .start(job_id, source)
            .and_then(|running| running.fail("plan_unsaved"));
        if failed.is_err() {
            tracing::warn!(
                operation = filesquid_api::imports::IMPORT_START_OPERATION,
                error_code = "plan_unsaved",
                "an import job whose plan could not be kept was left pending"
            );
        }
    }
}

/// Whether `plan` is the import `request` asks for: the same library, destination collection
/// and folder, and kind of source. Pure.
fn same_import(plan: &ImportPlan, request: &ImportRequest) -> bool {
    plan.library_id == request.library_id
        && plan.collection_id == request.collection_id
        && plan.folder == request.folder
        && plan.method == method_of(request.source)
}

/// The import method of a picked source. Pure.
fn method_of(source: ImportSource) -> ImportMethod {
    match source {
        ImportSource::Files => ImportMethod::FilePicker,
        ImportSource::Folder => ImportMethod::Folder,
    }
}

impl DesktopImports {
    /// The job `operation_id` already started, if any. Reads the plans.
    fn known_job(&self, operation_id: &str) -> Result<Option<String>, ImportStartError> {
        self.starting
            .plans
            .job_of_operation(operation_id)
            .map_err(|_| ImportStartError::Unavailable)
    }

    /// The answer of the job `request`'s operation ID already started, if any: that job when
    /// its plan names the request's library, destination and kind of source, else a refusal,
    /// so an operation ID reused for another import is never answered with the first one's
    /// job. Reads the plans.
    fn answer_known(
        &self,
        request: &ImportRequest,
    ) -> Result<Option<ImportAnswer>, ImportStartError> {
        let operation = request.envelope.operation_id();
        let Some(job_id) = self.known_job(operation.as_str())? else {
            return Ok(None);
        };
        let plan = self
            .starting
            .plans
            .load(&job_id)
            .map_err(|_| ImportStartError::Unavailable)?
            .ok_or(ImportStartError::Unavailable)?;
        match same_import(&plan, request) {
            true => Ok(Some(ImportAnswer::started(operation, job_id))),
            false => Err(ImportStartError::OperationReused),
        }
    }

    /// The job of `operation_id`: the one already started, or a new one for `plan`, decided
    /// under the start lock so two requests never make two jobs. Writes the job store and
    /// the plans.
    fn start_once(
        &self,
        operation_id: &str,
        plan: &ImportPlan,
    ) -> Result<String, ImportStartError> {
        let _one_start = self
            .starts
            .lock()
            .map_err(|_| ImportStartError::Unavailable)?;
        match self.known_job(operation_id)? {
            Some(job_id) => Ok(job_id),
            None => self.submit(operation_id, plan),
        }
    }
}

impl ImportStarter for DesktopImports {
    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        self.answer_known(request)
    }

    fn start(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        let operation = request.envelope.operation_id();
        let operation_id = operation.as_str();
        if let Some(answer) = self.answer_known(request)? {
            return Ok(answer);
        }
        let library =
            LibraryId::new(&request.library_id).map_err(|_| ImportStartError::LibraryNotFound)?;
        if !self.starting.files.is_open(&library) {
            return Err(ImportStartError::LibraryNotFound);
        }
        let Some(sources) = self.pick(request.source)? else {
            return Ok(ImportAnswer::cancelled(operation));
        };
        let plan = ImportPlan {
            library_id: request.library_id.clone(),
            collection_id: request.collection_id.clone(),
            folder: request.folder.clone(),
            sources,
            method: method_of(request.source),
        };
        self.start_once(operation_id, &plan)
            .map(|job_id| ImportAnswer::started(operation, job_id))
    }
}
