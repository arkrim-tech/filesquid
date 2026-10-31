//! The desktop's Recently deleted (Task API-F2.15; API-S-019, DE-S-014, ADR 041 Decisions 8
//! to 11): the library service the Recently deleted and settings routes run, over the
//! library catalogs, with the installation's settings in the operational store
//! (`filesquid.sqlite`); and the expiry of deleted files as a durable job, run at launch and
//! then every [`EXPIRY_INTERVAL`] until the app exits.
//!
//! The desktop only wires: what a command, a setting or an expiry means is `filesquid-api`'s
//! ([`LibraryService`]), and how an expired file's object is deleted or rewritten as a
//! tombstone is the catalog's ([`SqliteCatalog`]).

use filesquid_api::jobs::Jobs;
use filesquid_api::library::LibraryService;
use filesquid_api::navigation::NavigationPublisher;
use filesquid_api::ApiState;
use filesquid_core::InstallationId;
use filesquid_native::workers::expiry::run_expiry_job;
use filesquid_native::{HostMonitor, SqliteCatalog, SqliteRetentionSettings, JOURNAL_FILE};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    let settings = SqliteRetentionSettings::open(&data_dir.join(JOURNAL_FILE), host.monitor().store())
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

/// Run `task` at launch and then every [`EXPIRY_INTERVAL`] on a blocking thread, until `stop`
/// is set at app exit (bounded by the app's lifetime).
pub fn spawn_expiry(task: ExpiryTask, stop: Arc<AtomicBool>) {
    let task = Arc::new(task);
    tauri::async_runtime::spawn(async move {
        while !stop.load(Ordering::SeqCst) {
            let (run, flag) = (task.clone(), stop.clone());
            let ran = tauri::async_runtime::spawn_blocking(move || run.run_once(&flag)).await;
            if ran.is_err() {
                tracing::error!(operation = "library.expiry", "the expiry run panicked");
            }
            wait_or_stop(&stop).await;
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
