//! The desktop's PROV-O history backfill (API-S-029, DE-S-011, DE-S-014; ADR 040 Decision 7):
//! one runner for the run at launch and for `POST /api/v1/admin/history/backfill`
//! ([`DesktopBackfill`], the host's [`BackfillLauncher`]).
//!
//! A run is the durable backfill job over the library catalogs
//! ([`history_backfill::run_backfill_job`]): it resumes the job a quit suspended, skipping the
//! libraries its checkpoint lists, and otherwise visits every library (versions that have a
//! record are skipped). It then backfills each library opened in a folder the owner chose
//! ([`OpenedRoots::backfill`], Task DE-1.1). Each run registers in the process registry and
//! journals itself. At most one run goes at a time: a request while one runs is refused, so
//! the launch run and a requested run never race for the job's lease. Setting the host's stop
//! flag (at app exit) cancels a run between objects and suspends the job with its checkpoint;
//! the exit waits for it, bounded ([`DesktopBackfill::wait`]).

use filesquid_api::history::backfill::{Backfill, BACKFILL_OPERATION};
use filesquid_api::history::launch::{BackfillLauncher, LaunchError};
use filesquid_api::jobs::{JobError, Jobs};
use filesquid_native::workers::history_backfill::{self, BackfillJobRun};
use filesquid_native::{OpenedRoots, SqliteCatalog};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Longest the app's exit waits for a cancelled history backfill to end before the journal is
/// flushed.
pub const BACKFILL_STOP_WAIT: Duration = Duration::from_secs(10);

/// What one backfill run needs: the job service, the backfill job of this host, the
/// catalogs, the job's source identity and the folders the owner opened.
pub struct BackfillTask {
    /// The durable job service.
    pub jobs: Jobs,
    /// The backfill of this host ([`crate::history_backfill`]).
    pub backfill: Backfill,
    /// The library catalogs.
    pub catalog: Arc<SqliteCatalog>,
    /// The job's source identity ([`history_backfill::backfill_source`]).
    pub source: String,
    /// The libraries opened in folders the owner chose.
    pub opened: Arc<OpenedRoots>,
}

/// The desktop's backfill runner: one run at a time, on its own thread.
pub struct DesktopBackfill {
    task: Arc<BackfillTask>,
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for DesktopBackfill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopBackfill").finish_non_exhaustive()
    }
}

impl DesktopBackfill {
    /// A runner of `task` that `stop` cancels. Starts nothing.
    pub fn new(task: BackfillTask, stop: Arc<AtomicBool>) -> Self {
        Self {
            task: Arc::new(task),
            stop,
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    /// Start one run on its own thread, unless one is running.
    ///
    /// # Errors
    /// [`LaunchError::Running`] while a run goes; [`LaunchError::Unavailable`] when no
    /// thread could be started (logged as an error, which the journal layer journals).
    pub fn start(&self) -> Result<(), LaunchError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(LaunchError::Running);
        }
        let (task, stop, running) = (self.task.clone(), self.stop.clone(), self.running.clone());
        let spawned = std::thread::Builder::new()
            .name("filesquid-history-backfill".into())
            .spawn(move || {
                run_and_report(&task, &stop);
                running.store(false, Ordering::SeqCst);
            });
        let handle = spawned.map_err(|err| {
            self.running.store(false, Ordering::SeqCst);
            tracing::error!(
                operation = BACKFILL_OPERATION,
                error_code = "thread_unavailable",
                error = %err,
                "the history backfill could not be started"
            );
            LaunchError::Unavailable
        })?;
        self.keep(handle);
        Ok(())
    }

    /// Whether a run is going now.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Keep the thread of the run just started, so the exit can wait for it.
    fn keep(&self, handle: JoinHandle<()>) {
        match self.thread.lock() {
            Ok(mut slot) => *slot = Some(handle),
            Err(_) => tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = "internal",
                "the history backfill thread cannot be awaited at exit"
            ),
        }
    }

    /// Wait, at most `limit`, for the run's thread to end (after the stop flag was set). A
    /// thread still running then is logged and left to end with the process.
    pub fn wait(&self, limit: Duration) {
        let handle = self.thread.lock().ok().and_then(|mut slot| slot.take());
        let Some(handle) = handle else {
            return;
        };
        let deadline = Instant::now() + limit;
        while !handle.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !handle.is_finished() {
            tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = "stop_timeout",
                "the history backfill did not stop in time"
            );
            return;
        }
        if handle.join().is_err() {
            tracing::error!(
                operation = BACKFILL_OPERATION,
                error_code = "panicked",
                "the history backfill thread panicked"
            );
        }
    }
}

impl BackfillLauncher for DesktopBackfill {
    fn launch(&self, _actor: &str) -> Result<(), LaunchError> {
        self.start()
    }
}

/// One run: the durable job over the catalogs, then the opened folders, each reported.
/// Filesystem, database and journal I/O.
fn run_and_report(task: &BackfillTask, stop: &AtomicBool) {
    report_job(history_backfill::run_backfill_job(
        &task.jobs,
        &task.catalog,
        &task.backfill,
        &task.source,
        stop,
    ));
    report_opened(task.opened.backfill(&task.backfill, stop));
}

/// Log a backfill job that could not run, and each library whose backfill could not run, as
/// warnings with error codes only (the journal layer journals warnings; the job and each run
/// that started journaled themselves).
fn report_job(result: Result<BackfillJobRun, JobError>) {
    let run = match result {
        Ok(run) => run,
        Err(err) => {
            tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = err.error_code(),
                "the history backfill job could not run; the next launch retries"
            );
            return;
        }
    };
    for library in run.ran.iter().filter(|library| library.error.is_some()) {
        warn_library(library.library_id.as_str());
    }
}

/// Log each opened folder whose backfill could not run, as a warning with its code only.
fn report_opened(result: Result<Vec<filesquid_native::OpenedBackfill>, String>) {
    let done = match result {
        Ok(done) => done,
        Err(_) => {
            tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = "unavailable",
                "the opened libraries could not be listed for the history backfill"
            );
            return;
        }
    };
    for library in done.iter().filter(|library| library.report.is_err()) {
        warn_library(library.library_id.as_str());
    }
}

/// Log that the backfill could not run for `library_id`.
fn warn_library(library_id: &str) {
    tracing::warn!(
        operation = BACKFILL_OPERATION,
        library_id,
        error_code = "unavailable",
        "the history backfill could not run for a library; the next request retries"
    );
}
