//! The desktop's Recently deleted (Task API-F2.15; API-S-019, DE-S-014, ADR 041 Decisions 8
//! to 11): the library service the Recently deleted and settings routes run, over the
//! library catalogs, with the installation's settings in the operational store
//! (`filesquid.sqlite`); and the expiry of deleted files as a durable job, run at launch and
//! then every [`EXPIRY_INTERVAL`] until the app exits.
//!
//! The desktop only wires: what a command, a setting or an expiry means is `filesquid-api`'s
//! ([`LibraryService`]), and how an expired file's object is deleted or rewritten as a
//! tombstone is the catalog's ([`SqliteCatalog`]).

use filesquid_api::jobs::{Job, JobError, JobRunner, Jobs};
use filesquid_api::library::LibraryService;
use filesquid_api::navigation::NavigationPublisher;
use filesquid_api::ApiState;
use filesquid_core::InstallationId;
use filesquid_native::workers::expiry::run_expiry_job;
use filesquid_native::{HostMonitor, SqliteCatalog, SqliteRetentionSettings, JOURNAL_FILE};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// Time between two expiry runs.
pub const EXPIRY_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How often a waiting expiry loop checks whether the app is exiting.
const STOP_POLL: Duration = Duration::from_secs(1);

/// Install the library service of the Recently deleted and settings routes over `catalog`,
/// with the installation settings in `data_dir`'s operational store, its refusals journaled
/// through the host's bounded queue and its changes announced on the event channel.
/// Database I/O.
///
/// # Errors
/// The settings store cannot be opened, or a library service was already installed.
pub fn install_recently_deleted(
    state: &ApiState,
    host: &HostMonitor,
    catalog: Arc<SqliteCatalog>,
    data_dir: &Path,
) -> Result<Arc<LibraryService>, String> {
    let settings =
        SqliteRetentionSettings::open(&data_dir.join(JOURNAL_FILE), host.monitor().store())
            .map_err(|_| "the Recently deleted settings could not be opened".to_string())?;
    let service = LibraryService::new(catalog, host.monitor().journal())
        .with_settings(Arc::new(settings))
        .with_navigation(NavigationPublisher::new(state.events.clone()));
    let service = Arc::new(service);
    if !state.install_library_service(service.clone()) {
        return Err("a library service was already installed".to_string());
    }
    Ok(service)
}

/// What one expiry loop needs.
pub struct ExpiryTask {
    /// The library service.
    pub service: Arc<LibraryService>,
    /// The library catalogs, the object purger.
    pub catalog: Arc<SqliteCatalog>,
    /// The host's durable jobs.
    pub jobs: Jobs,
    /// The job source: this installation.
    pub source: String,
}

impl ExpiryTask {
    /// The task of `installation`.
    pub fn new(
        service: Arc<LibraryService>,
        catalog: Arc<SqliteCatalog>,
        jobs: Jobs,
        installation: &InstallationId,
    ) -> Self {
        Self {
            service,
            catalog,
            jobs,
            source: format!("installation:{}", installation.as_str()),
        }
    }

    /// Run one expiry job now; a failure is logged and the next run tries again. Database,
    /// filesystem and journal I/O.
    pub fn run_once(&self, stop: &AtomicBool) {
        let run = run_expiry_job(&self.jobs, &self.service, &self.catalog, &self.source, stop);
        if let Err(err) = run {
            tracing::warn!(
                operation = filesquid_api::library::EXPIRY_JOB_KIND,
                error_code = err.error_code(),
                "the expiry job could not run; the next run tries again"
            );
        }
    }
}

/// Longest the app's exit waits for an expiry run in progress.
pub const EXPIRY_STOP_WAIT: Duration = Duration::from_secs(10);

/// The running expiry of this host: its task, whether a run is in progress, and the app's
/// exit flag. It is also the `library.expiry` job's [`JobRunner`], so a resumed job runs.
pub struct ExpiryHandle {
    /// What a run does.
    task: ExpiryTask,
    /// True while a run is in progress.
    running: AtomicBool,
    /// The app's exit flag.
    stop: Arc<AtomicBool>,
    /// This handle, for the thread a resumed job runs on.
    me: Weak<ExpiryHandle>,
}

impl ExpiryHandle {
    /// A handle of `task` that stops when `stop` is set.
    pub fn new(task: ExpiryTask, stop: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            task,
            running: AtomicBool::new(false),
            stop,
            me: me.clone(),
        })
    }

    /// Run once now unless a run is already in progress; answer whether this call ran.
    /// Database, filesystem and journal I/O.
    pub fn run_now(&self) -> bool {
        if self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.task.run_once(&self.stop);
        self.running.store(false, Ordering::SeqCst);
        true
    }

    /// Whether a run is in progress.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Wait at most `limit` for a run in progress to end (the exit sets the stop flag first,
    /// so the run suspends its job between libraries).
    pub fn wait(&self, limit: Duration) {
        let deadline = std::time::Instant::now() + limit;
        while self.is_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.is_running() {
            tracing::warn!(
                operation = filesquid_api::library::EXPIRY_JOB_KIND,
                error_code = "stop_timeout",
                "the expiry run did not stop in time"
            );
        }
    }
}

impl JobRunner for ExpiryHandle {
    /// Run the resumed expiry job on its own thread; a run already in progress takes it.
    fn resume(&self, _job: &Job) -> Result<(), JobError> {
        if self.is_running() {
            return Ok(());
        }
        let me = self
            .me
            .upgrade()
            .ok_or_else(|| JobError::Unavailable("the expiry is stopping".into()))?;
        std::thread::Builder::new()
            .name("filesquid-expiry".into())
            .spawn(move || {
                me.run_now();
            })
            .map(drop)
            .map_err(|err| JobError::Unavailable(err.to_string()))
    }
}

/// Run `handle` at launch and then every [`EXPIRY_INTERVAL`] on a blocking thread, until its
/// stop flag is set at app exit (bounded by the app's lifetime).
pub fn spawn_expiry(handle: Arc<ExpiryHandle>) {
    tauri::async_runtime::spawn(async move {
        while !handle.stop.load(Ordering::SeqCst) {
            let run = handle.clone();
            let ran = tauri::async_runtime::spawn_blocking(move || run.run_now()).await;
            if ran.is_err() {
                tracing::error!(operation = "library.expiry", "the expiry run panicked");
            }
            wait_or_stop(&handle.stop).await;
        }
    });
}

/// Wait [`EXPIRY_INTERVAL`], waking every [`STOP_POLL`] to end early when `stop` is set.
async fn wait_or_stop(stop: &AtomicBool) {
    let steps = EXPIRY_INTERVAL.as_secs() / STOP_POLL.as_secs().max(1);
    for _ in 0..steps {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(STOP_POLL).await;
    }
}
