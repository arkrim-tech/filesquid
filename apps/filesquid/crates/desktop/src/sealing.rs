//! Journal sealing and the refill of a lost journal on the desktop (Task DE-0.11; DE-S-013,
//! API-S-032, [ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)
//! Decision 7).
//!
//! The host runs the API's [`SealService`]:
//!
//! - **at launch**, once, which catches up a seal a crash or a forced quit missed
//!   ([`catch_up`]); a run that finds only seal bookkeeping commits and journals nothing;
//! - **every hour** ([`SEAL_INTERVAL_MS`]) while the app runs ([`spawn_hourly`]), bounded by
//!   the app's stop flag;
//! - **at app close**, waiting at most [`CLOSE_SEAL_WAIT`] ([`seal_before_close`]). The
//!   desktop has no separate library close: a library stays open until the app exits, so this
//!   seal is also the close-time seal of every open library.
//!
//! Each run is a registered `journal.seal` process and journals `journal.seal` per scope
//! (API-S-032). Host events go into the installation's journal object at
//! `<data dir>/journal.ocfl/`; a library's events into its own storage root, opened for
//! sealing when the library is served ([`open_library`]). The service is the source of the
//! `journal_unsealed` health reason.
//!
//! When the launch found no operational store (`filesquid.sqlite` deleted or lost), the host
//! refills the new one from the installation's sealed segments before its monitor opens it
//! ([`refill_before_open`]), and journals the refill once the monitor is open; a
//! library's own journal comes back with that library's rebuild from files, through the
//! SQLite journal store the library service restores into (CL-1.9).

use filesquid_api::journal::seal::{SealConfig, SealReport, SealService, SEAL_INTERVAL_MS};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_core::{InstallationId, LibraryId};
use filesquid_native::journal_objects::{
    library_journal, open_installation_journal, restore_installation_journal, Refill,
    INSTALLATION_JOURNAL_DIR,
};
use filesquid_native::{HostMonitor, HostRoot, SqliteJournal, JOURNAL_FILE};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Longest the app's exit waits for its close-time seal.
pub const CLOSE_SEAL_WAIT: Duration = Duration::from_secs(30);

/// How often the hourly loop looks at the stop flag.
const STOP_POLL: Duration = Duration::from_secs(1);

/// Journal operation of the refill of a lost operational store.
pub const REFILL_OPERATION: &str = "journal.refill";

/// Journal operation of a journal object that could not be opened for sealing.
pub const SEAL_OPEN_OPERATION: &str = "journal.seal.open";

/// The desktop's sealing: the service and who its library objects are sealed as.
#[derive(Clone)]
pub struct DesktopSealing {
    /// The API's sealing service.
    pub service: Arc<SealService>,
    /// The installation every journal object is sealed as.
    pub installation: InstallationId,
}

impl std::fmt::Debug for DesktopSealing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopSealing").finish_non_exhaustive()
    }
}

/// Build the sealing service of `host` over the installation's journal object in
/// `data_dir`, sealed as `installation`, and install it as the monitor's seal status.
/// Filesystem I/O.
///
/// # Errors
/// The installation's journal object could not be opened (journaled `journal.seal.open`
/// failed, with its error code only), or a seal status was already installed.
pub async fn start_sealing(
    host: &HostMonitor,
    data_dir: &Path,
    installation: &InstallationId,
) -> Result<DesktopSealing, String> {
    let object = open_installation_journal(data_dir, installation)
        .await
        .inspect_err(|_| journal_open_failure(host, "installation"))?;
    let service = Arc::new(SealService::new(
        host.monitor().clone(),
        object,
        SealConfig::default(),
    ));
    if !host.monitor().install_seal_status(service.clone()) {
        return Err("a seal status was already installed".to_string());
    }
    Ok(DesktopSealing {
        service,
        installation: installation.clone(),
    })
}

/// Seal whatever an earlier launch left unsealed (a missed hourly or close-time seal).
/// Journal and storage I/O.
pub async fn catch_up(sealing: &DesktopSealing) -> SealReport {
    sealing.service.seal_all().await
}

/// Open the journal object of `library_id`, served from `root`, for sealing; false when it
/// was already open or could not be opened (logged as a warning with its error code only,
/// which the journal layer journals). Creates directories.
pub fn open_library(
    sealing: &DesktopSealing,
    (library_id, root): (&LibraryId, &HostRoot),
    data_dir: &Path,
) -> bool {
    if sealing.service.is_open(library_id.as_str()) {
        return false;
    }
    match library_journal(root, library_id, data_dir, &sealing.installation) {
        Ok(object) => sealing.service.open_library(library_id.as_str(), object),
        Err(_) => {
            tracing::warn!(
                operation = SEAL_OPEN_OPERATION,
                library_id = library_id.as_str(),
                error_code = "unavailable",
                "a library's journal object could not be opened for sealing"
            );
            false
        }
    }
}

/// Seal every scope each `interval` until `stop` is set (the app's exit), looking at `stop`
/// at least every second: bounded by the app's lifetime. Runs on `runtime`.
pub fn spawn_hourly(
    runtime: &tokio::runtime::Handle,
    sealing: DesktopSealing,
    stop: Arc<AtomicBool>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    runtime.spawn(async move {
        while wait_unless_stopped(interval, &stop).await {
            let report = sealing.service.seal_all().await;
            log_report("hourly", &report);
        }
    })
}

/// The hourly interval of [`spawn_hourly`]: [`SEAL_INTERVAL_MS`].
pub fn hourly() -> Duration {
    Duration::from_millis(SEAL_INTERVAL_MS as u64)
}

/// Wait `interval` in steps of at most [`STOP_POLL`]; false as soon as `stop` is set.
async fn wait_unless_stopped(interval: Duration, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + interval;
    let steps = interval.as_millis() / STOP_POLL.as_millis().max(1) + 1;
    for _ in 0..=steps {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        tokio::time::sleep(left.min(STOP_POLL)).await;
    }
    !stop.load(Ordering::SeqCst)
}

/// Seal every scope at app close, waiting at most `limit`; `None` (logged) when the seal did
/// not finish in time, its entries then wait for the next launch's catch-up. Journal and
/// storage I/O.
pub async fn seal_before_close(sealing: &DesktopSealing, limit: Duration) -> Option<SealReport> {
    match tokio::time::timeout(limit, sealing.service.seal_all()).await {
        Ok(report) => {
            log_report("close", &report);
            Some(report)
        }
        Err(_) => {
            tracing::warn!(
                operation = "journal.seal",
                error_code = "timeout",
                "the close-time seal did not finish; the next launch catches it up"
            );
            None
        }
    }
}

/// When the launch found no operational store (`store_lost`) but the data directory holds the
/// installation's sealed journal, recreate the store and refill it from the sealed segments
/// under their own journal IDs, reserving the ids the seals read ([`restore_installation_journal`]).
/// It runs before the host monitor opens the store, so no entry of this launch can take an id
/// still to be restored. `None` when there was nothing to refill. Filesystem and database I/O.
pub async fn refill_before_open(
    data_dir: &Path,
    store_lost: bool,
) -> Option<Result<Refill, String>> {
    if !store_lost || !data_dir.join(INSTALLATION_JOURNAL_DIR).is_dir() {
        return None;
    }
    let store = match SqliteJournal::open(&data_dir.join(JOURNAL_FILE), "filesquid-desktop") {
        Ok(store) => store,
        Err(_) => {
            return Some(Err(
                "the operational store could not be recreated".to_string()
            ))
        }
    };
    Some(restore_installation_journal(data_dir, &store).await)
}

/// Journal the refill [`refill_before_open`] did, as [`REFILL_OPERATION`] `succeeded` or
/// `failed` with its error code (the counts go to stdout only); nothing when there was none.
pub fn journal_refill(host: &HostMonitor, refill: Option<Result<Refill, String>>) {
    let entry = match refill {
        None => return,
        Some(Ok(refill)) => {
            tracing::info!(
                restored = refill.restored,
                already_present = refill.already_present,
                unreadable = refill.unreadable,
                "the operational store was refilled from the sealed journal"
            );
            JournalEntry::new(REFILL_OPERATION, Outcome::Succeeded)
        }
        Some(Err(_)) => JournalEntry::new(REFILL_OPERATION, Outcome::Failed)
            .level(Level::Error)
            .error_code("unavailable"),
    };
    host.monitor().journal().send(entry);
}

/// Journal that the journal object of `scope` could not be opened for sealing (the scope is
/// `installation` or an opaque library ID).
fn journal_open_failure(host: &HostMonitor, scope: &str) {
    let entry = JournalEntry::new(SEAL_OPEN_OPERATION, Outcome::Failed)
        .level(Level::Error)
        .target(scope)
        .error_code("unavailable");
    host.monitor().journal().send(entry);
}

/// Log what a seal run did (stdout; every scope is journaled by the service itself).
fn log_report(when: &str, report: &SealReport) {
    tracing::info!(
        when,
        scopes = report.scopes.len(),
        all_sealed = report.all_sealed(),
        "journal seal run"
    );
}

/// Everything the launch does for the journal once the monitor is open (DE-S-013): journal the
/// `refill` [`refill_before_open`] did, start the sealing service, catch up a missed seal and
/// start the hourly seal on `runtime`, bounded by `stop`. Filesystem, database and storage
/// I/O.
///
/// # Errors
/// As [`start_sealing`].
pub async fn launch_sealing(
    host: &HostMonitor,
    data_dir: &Path,
    installation: &InstallationId,
    (runtime, stop, refill): (
        &tokio::runtime::Handle,
        Arc<AtomicBool>,
        Option<Result<Refill, String>>,
    ),
) -> Result<DesktopSealing, String> {
    journal_refill(host, refill);
    let sealing = start_sealing(host, data_dir, installation).await?;
    log_report("launch", &catch_up(&sealing).await);
    spawn_hourly(runtime, sealing.clone(), stop, hourly());
    Ok(sealing)
}
