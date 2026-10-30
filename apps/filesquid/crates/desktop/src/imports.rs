//! The desktop's import routes (Tasks DE-1.2, DE-1.8; API-S-036, DE-S-019): the host's
//! [`ImportStarter`] behind `POST /api/v1/imports` and its review routes.
//!
//! A request whose operation ID already planned or started an import answers that plan or
//! job. Otherwise the owner picks files or a folder in a native dialog the host shows from
//! Rust ([`RfdSources`], the `rfd` crate, so the webview keeps no dialog permission,
//! DE-S-002), and the host reviews what was picked without staging a byte
//! ([`filesquid_native::import::review`]): the manifest the job will run, the files the
//! library already holds by digest, the bytes to copy, the numbered names and the free bytes
//! of the library's volume after the copy, kept in the API's [`PlanBook`] for an hour. Only
//! when the owner confirms the plan (`POST /api/v1/imports/{plan_id}/start`), and its source
//! is unchanged, does the host submit one durable import job, keep its plan (with the
//! destination the request named, Task DE-1.7), its manifest and its per-file items (the
//! files already held as `skipped`) in its operational store (a job whose plan cannot be kept
//! is failed at once), and queue it on the native import worker. The check for an existing
//! job and the submission run under one lock, so two requests can never make two jobs for one
//! operation ID. Only opaque IDs and base names leave the host; the picked paths never do
//! (DE-S-006).

use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportSource, ImportStartError, ImportStarter, PlanBook,
    PlanItemPage,
};
use filesquid_api::jobs::{JobItemState, Jobs};
use filesquid_api::journal::now_ms;
use filesquid_api::library::mint_uuid_v4;
use filesquid_api::local_library::{FreeSpace, LibraryRegistry};
use filesquid_core::LibraryId;
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::index::ImportIndex;
use filesquid_native::import::items::{items_of_manifest, SqliteJobItems};
use filesquid_native::import::job::submit_import;
use filesquid_native::import::plans::{ImportPlan, SqliteImportPlans};
use filesquid_native::import::review::{fingerprint_of_plan, review, Reviewed};
use filesquid_native::import::worker::ImportWorker;
use filesquid_native::HostFiles;
use std::collections::HashMap;
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
    /// Each job's per-file progress and the digests of committed imports.
    pub items: Arc<SqliteJobItems>,
    /// The library index a plan's manifest is placed in.
    pub index: Arc<dyn ImportIndex>,
    /// The libraries this host opened, for the free bytes of their volume.
    pub registry: Arc<dyn LibraryRegistry>,
    /// The worker the jobs run on.
    pub worker: Arc<ImportWorker>,
    /// The native dialog.
    pub dialog: Box<dyn SourceDialog>,
}

/// A reviewed plan the owner has not started yet: what was picked and what its job will run.
#[derive(Clone)]
struct Pending {
    plan: ImportPlan,
    reviewed: Reviewed,
}

/// The desktop's import starter: one dialog at a time, one start at a time.
pub struct DesktopImports {
    starting: ImportStarting,
    open: AtomicBool,
    starts: Mutex<()>,
    book: PlanBook,
    pending: Mutex<HashMap<String, Pending>>,
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
            book: PlanBook::new(),
            pending: Mutex::new(HashMap::new()),
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

    /// Submit and queue the job of the reviewed `pending` plan for `operation_id`, keeping its
    /// plan, manifest and items; a job whose plan cannot be kept is failed. Writes the job
    /// store, the plans and the items.
    fn submit(&self, operation_id: &str, pending: &Pending) -> Result<String, ImportStartError> {
        let job = submit_import(&self.starting.jobs, &pending.plan, operation_id)
            .map_err(|_| ImportStartError::Unavailable)?;
        if self.keep(&job.id, operation_id, pending).is_err() {
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

    /// Keep the plan, the manifest and the items of job `job_id`, the files the library
    /// already holds as `skipped`. Writes the plans and the items.
    fn keep(&self, job_id: &str, operation_id: &str, pending: &Pending) -> Result<(), String> {
        let starting = &self.starting;
        starting.plans.save(job_id, operation_id, &pending.plan)?;
        let manifest = &pending.reviewed.manifest;
        starting.plans.save_manifest(job_id, manifest)?;
        let items = items_of_manifest(manifest);
        starting
            .items
            .keep(job_id, &items)
            .map_err(|err| err.to_string())?;
        for index in &pending.reviewed.present {
            let skipped = JobItemState::Skipped;
            let set = starting.items.set(job_id, *index, skipped, 0, None);
            set.map_err(|err| err.to_string())?;
        }
        Ok(())
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

    /// The job of `operation_id`: the one already started, or a new one for the reviewed
    /// `pending` plan, decided under the start lock so two requests never make two jobs.
    /// Writes the job store, the plans and the items.
    fn start_once(
        &self,
        operation_id: &str,
        pending: &Pending,
    ) -> Result<String, ImportStartError> {
        let _one_start = self
            .starts
            .lock()
            .map_err(|_| ImportStartError::Unavailable)?;
        match self.known_job(operation_id)? {
            Some(job_id) => Ok(job_id),
            None => self.submit(operation_id, pending),
        }
    }

    /// The free bytes of the volume of `library`'s folder, when the host can read them. Reads
    /// the registry and the volume.
    fn free_bytes(&self, library: &LibraryId) -> Option<u64> {
        let registry = &self.starting.registry;
        let entry = registry.entry(library)?;
        match registry.free_space(&entry.location_id) {
            FreeSpace::Bytes(bytes) => Some(bytes),
            FreeSpace::VolumeUnreadable | FreeSpace::NotLocal => None,
        }
    }

    /// Review what the owner picked for `request` into a new plan and remember it; answer
    /// `planned`. Reads the filesystem, the index and the digests; stages nothing.
    fn review_picked(
        &self,
        request: &ImportRequest,
        library: &LibraryId,
        sources: Vec<PathBuf>,
    ) -> Result<ImportAnswer, ImportStartError> {
        let plan = ImportPlan {
            library_id: request.library_id.clone(),
            collection_id: request.collection_id.clone(),
            folder: request.folder.clone(),
            sources,
            method: method_of(request.source),
        };
        let starting = &self.starting;
        let free = self.free_bytes(library);
        let reviewed =
            review(&plan, starting.index.as_ref(), &starting.items, free).map_err(unreviewed)?;
        let plan_id = mint_uuid_v4().map_err(|_| ImportStartError::Unavailable)?;
        let (facts, items) = (reviewed.facts.clone(), reviewed.items.clone());
        let fingerprint = reviewed.fingerprint.clone();
        self.remember(&plan_id, Pending { plan, reviewed });
        Ok(self
            .book
            .open(&plan_id, request, facts, items, &fingerprint, now_ms()))
    }

    /// Remember the reviewed plan `plan_id`, forgetting every plan the book no longer holds
    /// open, so at most the book's bound is kept.
    fn remember(&self, plan_id: &str, pending: Pending) {
        let mut kept = self.lock_pending();
        let now = now_ms();
        kept.retain(|id, _| self.book.is_open(id, now));
        kept.insert(plan_id.to_string(), pending);
    }

    /// The reviewed plans, locked; a poisoned lock is recovered (plain data).
    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<String, Pending>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The refusal of a pick that cannot be reviewed, logged with its code (never a path). Pure
/// but for the log.
fn unreviewed(code: &str) -> ImportStartError {
    tracing::warn!(
        operation = filesquid_api::imports::IMPORT_START_OPERATION,
        error_code = code,
        "a picked source could not be reviewed"
    );
    ImportStartError::Unavailable
}

impl ImportStarter for DesktopImports {
    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        match self.book.replay(request, now_ms())? {
            Some(answer) => Ok(Some(answer)),
            None => self.answer_known(request),
        }
    }

    fn plan(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        if let Some(answer) = self.answer_known(request)? {
            return Ok(answer);
        }
        let library =
            LibraryId::new(&request.library_id).map_err(|_| ImportStartError::LibraryNotFound)?;
        if !self.starting.files.is_open(&library) {
            return Err(ImportStartError::LibraryNotFound);
        }
        match self.pick(request.source)? {
            Some(sources) => self.review_picked(request, &library, sources),
            None => Ok(self.book.cancelled(request)),
        }
    }

    fn start_plan(&self, _actor: &str, plan_id: &str) -> Result<ImportAnswer, ImportStartError> {
        let pending = self.lock_pending().get(plan_id).cloned();
        let pending = pending.ok_or(ImportStartError::PlanNotFound)?;
        let fingerprint = fingerprint_of_plan(&pending.plan).map_err(unreviewed)?;
        self.book
            .start(plan_id, &fingerprint, now_ms(), |operation, _| {
                self.start_once(operation, &pending)
            })
    }

    fn discard(&self, _actor: &str, plan_id: &str) -> Result<(), ImportStartError> {
        self.book.discard(plan_id, now_ms())?;
        self.lock_pending().remove(plan_id);
        Ok(())
    }

    fn plan_items(
        &self,
        plan_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<PlanItemPage, ImportStartError> {
        self.book.items(plan_id, cursor, limit, now_ms())
    }
}
