//! The desktop's Recently deleted (Tasks API-F2.15 and API-F2.30; API-S-019, DE-S-014, ADR
//! 041 Decisions 8 to 11): the library service the Recently deleted and settings routes run,
//! over the library catalogs, with the installation's settings in the operational store
//! (`filesquid.sqlite`); and the expiry of deleted files as a durable job, run at launch and
//! then at the installation's stored expiry interval until the app exits.
//!
//! The [`ExpiryMonitor`] reads the stored interval at every tick, so an interval the owner
//! changes takes effect for the next run without a relaunch: that run falls due one new
//! interval after the start of the last run, or at once when that time has already passed.
//!
//! The desktop only wires: what a command, a setting, an expiry or its schedule means is
//! `filesquid-api`'s ([`LibraryService`], [`ExpirySchedule`]), and how an expired file's
//! object is deleted or rewritten as a tombstone is the catalog's ([`SqliteCatalog`]).

use filesquid_api::jobs::{Job, JobError, JobRunner, Jobs};
use filesquid_api::library::{
    ExpirySchedule, LibraryError, LibraryService, DEFAULT_EXPIRY_INTERVAL_MINUTES,
};
use filesquid_api::navigation::NavigationPublisher;
use filesquid_api::ApiState;
use filesquid_core::InstallationId;
use filesquid_native::workers::expiry::run_expiry_job;
use filesquid_native::{HostMonitor, SqliteCatalog, SqliteRetentionSettings, JOURNAL_FILE};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

/// How often the expiry monitor checks whether a run is due and whether the app is exiting.
/// It bounds how late a run starts after it falls due, and how long a changed interval waits
/// to be read.
pub const EXPIRY_TICK: Duration = Duration::from_secs(1);

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

    /// The installation's stored expiry interval, in minutes. Database I/O.
    ///
    /// # Errors
    /// The settings store cannot be read.
    pub fn interval_minutes(&self) -> Result<u32, LibraryError> {
        let settings = self.service.installation_settings()?;
        Ok(settings.expiry_interval_minutes)
    }

    /// Run one expiry job now, `resumed` when the job routes resumed that job; a failure is
    /// logged and the next run tries again. Database, filesystem and journal I/O.
    pub fn run_once(&self, resumed: Option<&str>, stop: &AtomicBool) {
        let (jobs, catalog) = (&self.jobs, &self.catalog);
        let run = run_expiry_job(jobs, &self.service, catalog, &self.source, resumed, stop);
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
    /// The job the job routes resumed last, which the next run starts.
    resumed: Mutex<Option<String>>,
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
            resumed: Mutex::new(None),
            me: me.clone(),
        })
    }

    /// Run once now unless a run is already in progress, starting the job resumed last if
    /// any; answer whether this call ran. The in-progress flag is cleared on every exit, a
    /// panic included, so a failed run never blocks the later ones. Database, filesystem and
    /// journal I/O.
    pub fn run_now(&self) -> bool {
        if self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        let _clear = RunningFlag(&self.running);
        let resumed = self.take_resumed();
        self.task.run_once(resumed.as_deref(), &self.stop);
        true
    }

    /// Take the job the job routes resumed last, if any.
    fn take_resumed(&self) -> Option<String> {
        self.resumed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Whether a run is in progress.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Whether the app is exiting.
    pub fn is_stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// The installation's stored expiry interval, in minutes. Database I/O.
    ///
    /// # Errors
    /// The settings store cannot be read.
    pub fn interval_minutes(&self) -> Result<u32, LibraryError> {
        self.task.interval_minutes()
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

/// Clears a run's in-progress flag when dropped, on every exit of the run.
struct RunningFlag<'a>(&'a AtomicBool);

impl Drop for RunningFlag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl JobRunner for ExpiryHandle {
    /// Run the resumed expiry job `job` on its own thread; while a run is in progress, the
    /// job is kept and started by the next run.
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        *self.resumed.lock().unwrap_or_else(PoisonError::into_inner) = Some(job.id.clone());
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

/// The clock an [`ExpiryMonitor`] schedules by: milliseconds that never go backwards.
pub type MonitorClock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The expiry monitor of this host (DE-S-014): it runs the expiry at launch and then at the
/// installation's stored expiry interval, read again at every tick, so a changed interval is
/// taken for the next run without a relaunch.
pub struct ExpiryMonitor {
    /// What a run does, and the app's exit flag.
    handle: Arc<ExpiryHandle>,
    /// The start of the last run this monitor started.
    schedule: Mutex<ExpirySchedule>,
    /// The monitor's clock.
    clock: MonitorClock,
    /// The interval read last, used while the settings store cannot be read.
    last_interval: AtomicU32,
}

impl ExpiryMonitor {
    /// A monitor of `handle` whose clock is the time since now, which a change of the
    /// system's clock does not move.
    pub fn new(handle: Arc<ExpiryHandle>) -> Self {
        let launched = Instant::now();
        let elapsed = move || i64::try_from(launched.elapsed().as_millis()).unwrap_or(i64::MAX);
        Self::with_clock(handle, Arc::new(elapsed))
    }

    /// A monitor of `handle` that reads `clock`.
    pub fn with_clock(handle: Arc<ExpiryHandle>, clock: MonitorClock) -> Self {
        Self {
            handle,
            schedule: Mutex::new(ExpirySchedule::new()),
            clock,
            last_interval: AtomicU32::new(DEFAULT_EXPIRY_INTERVAL_MINUTES),
        }
    }

    /// The stored expiry interval in minutes, or the interval read last when the settings
    /// store cannot be read now (the failure is logged). Database I/O.
    pub fn interval_minutes(&self) -> u32 {
        match self.handle.interval_minutes() {
            Ok(minutes) => {
                self.last_interval.store(minutes, Ordering::SeqCst);
                minutes
            }
            Err(err) => {
                tracing::warn!(
                    operation = filesquid_api::library::EXPIRY_JOB_KIND,
                    error_code = err.error_code(),
                    "the expiry interval could not be read; the last one read is kept"
                );
                self.last_interval.load(Ordering::SeqCst)
            }
        }
    }

    /// When the last run this monitor started began, on its clock; `None` before the first.
    pub fn last_started_ms(&self) -> Option<i64> {
        self.schedule().last_started_ms()
    }

    /// When the next run falls due under the interval stored now, on the monitor's clock;
    /// `None` before the run at launch. Database I/O.
    pub fn next_due_ms(&self) -> Option<i64> {
        let interval = self.interval_minutes();
        self.schedule().next_due_ms(interval)
    }

    /// Run the expiry now if a run is due under the interval stored now; answer whether this
    /// call ran it. A run already in progress (a resumed job) is left alone and the due run
    /// starts at a later tick. Database, filesystem and journal I/O.
    pub fn tick(&self) -> bool {
        let interval = self.interval_minutes();
        let now = (self.clock)();
        if !self.schedule().is_due(interval, now) {
            return false;
        }
        if !self.handle.run_now() {
            return false;
        }
        self.schedule().record_start(now);
        true
    }

    /// The schedule; a poisoned lock still holds a whole value.
    fn schedule(&self) -> std::sync::MutexGuard<'_, ExpirySchedule> {
        self.schedule.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Tick `monitor` every [`EXPIRY_TICK`] on a blocking thread, so the expiry runs at launch and
/// then at the stored interval, until the app's exit sets the stop flag (bounded by the app's
/// lifetime).
pub fn spawn_expiry(monitor: Arc<ExpiryMonitor>) {
    tauri::async_runtime::spawn(async move {
        while !monitor.handle.is_stopping() {
            let tick = monitor.clone();
            let ticked = tauri::async_runtime::spawn_blocking(move || tick.tick()).await;
            if ticked.is_err() {
                tracing::error!(operation = "library.expiry", "the expiry run panicked");
            }
            tokio::time::sleep(EXPIRY_TICK).await;
        }
    });
}
