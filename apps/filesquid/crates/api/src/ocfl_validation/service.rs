//! [`ValidationService`]: the open libraries, the schedule, the run lock and the object lock
//! of each library (API-S-022).
//!
//! The service is generic over the host's storage backend type `B`. The host opens each
//! library it has open with [`ValidationService::open_library`] and installs the service in
//! [`crate::ApiState`] as an [`super::OcflValidation`], which the routes and the host's
//! ticker use. The runs themselves are in [`super::runner`].

use super::error::ValidationError;
use super::store::FindingsStore;
use super::{CONFIGURE_OPERATION, LOG_TARGET, RUN_OPERATION};
use crate::journal::{now_ms, JournalEntry, Level, Outcome};
use crate::monitor::Monitor;
use crate::registry::Clock;
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::StorageBackend;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Default time between scheduled runs of a library: 7 days.
pub const DEFAULT_INTERVAL_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Default object cap of one run.
pub const DEFAULT_OBJECT_CAP: u64 = 100_000;

/// Shortest schedule an owner may set: one hour.
const MIN_INTERVAL_MS: i64 = 60 * 60 * 1000;

/// Longest schedule an owner may set: 366 days.
const MAX_INTERVAL_MS: i64 = 366 * 24 * 60 * 60 * 1000;

/// Largest object cap an owner may set.
const MAX_OBJECT_CAP: u64 = 10_000_000;

/// The schedule and the cap, which the desktop owner or a server administrator may change
/// (`PUT /api/v1/admin/ocfl/validation-settings`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationConfig {
    /// Time between scheduled runs of a library, in milliseconds (one hour to 366 days).
    pub interval_ms: i64,
    /// Most objects one run checks (1 to 10,000,000).
    pub object_cap: u64,
}

impl Default for ValidationConfig {
    /// Every 7 days, at most 100,000 objects per run.
    fn default() -> Self {
        Self {
            interval_ms: DEFAULT_INTERVAL_MS,
            object_cap: DEFAULT_OBJECT_CAP,
        }
    }
}

/// One open library.
struct OpenLibrary<B> {
    library_id: LibraryId,
    backend: Arc<B>,
    opened_ms: i64,
    /// The cancel flag of the run going now; `None` when no run is going.
    running: Option<Arc<AtomicBool>>,
    /// Held while one object is validated, by runs and takeover checks alike.
    objects: Arc<tokio::sync::Mutex<()>>,
    /// When the next scheduled run is due, once known (read from the store at most once,
    /// then kept current by the runs and the configuration).
    next_due_ms: Option<i64>,
}

/// Shared state of the service.
pub(super) struct Inner<B> {
    libraries: Mutex<BTreeMap<String, OpenLibrary<B>>>,
    pub(super) store: Arc<dyn FindingsStore>,
    pub(super) monitor: Monitor,
    config: Mutex<ValidationConfig>,
    pub(super) clock: Clock,
    pub(super) runtime: Option<tokio::runtime::Handle>,
    /// Set by [`ValidationService::shut_down`]; no run or takeover check starts after it.
    pub(super) stopped: AtomicBool,
    /// Takeover checks going now, which [`ValidationService::shut_down`] waits for.
    checks: AtomicUsize,
}

/// Runs the `filesquid-ocfl` validator over the libraries a host has open.
pub struct ValidationService<B> {
    pub(super) inner: Arc<Inner<B>>,
}

impl<B> Clone for ValidationService<B> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<B> std::fmt::Debug for ValidationService<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidationService").finish_non_exhaustive()
    }
}

/// Holds a library's run lock; releases it when dropped, on every path. It releases only the
/// run it took, so a library closed and opened again during the run keeps its new state.
pub(super) struct RunLock<B> {
    service: ValidationService<B>,
    pub(super) library_id: LibraryId,
    pub(super) backend: Arc<B>,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) objects: Arc<tokio::sync::Mutex<()>>,
}

impl<B> Drop for RunLock<B> {
    fn drop(&mut self) {
        if let Ok(mut libraries) = self.service.inner.libraries.lock() {
            if let Some(library) = libraries.get_mut(self.library_id.as_str()) {
                let ours = library
                    .running
                    .as_ref()
                    .is_some_and(|flag| Arc::ptr_eq(flag, &self.cancel));
                if ours {
                    library.running = None;
                }
            }
        }
    }
}

/// What a takeover check needs of an open library. While it lives, the check counts as
/// going, so [`ValidationService::shut_down`] waits for it.
pub(super) struct ObjectAccess<B> {
    pub(super) backend: Arc<B>,
    pub(super) objects: Arc<tokio::sync::Mutex<()>>,
    _going: CheckGoing<B>,
}

/// Counts one takeover check as going until dropped, on every path.
struct CheckGoing<B> {
    inner: Arc<Inner<B>>,
}

impl<B> CheckGoing<B> {
    /// Count a check as going in `inner`.
    fn start(inner: &Arc<Inner<B>>) -> Self {
        inner.checks.fetch_add(1, Ordering::SeqCst);
        Self {
            inner: inner.clone(),
        }
    }
}

impl<B> Drop for CheckGoing<B> {
    fn drop(&mut self) {
        self.inner.checks.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<B: StorageBackend + 'static> ValidationService<B> {
    /// A service over `store`, reporting through `monitor`, with the default schedule and
    /// cap and the system clock (see [`with_clock`](Self::with_clock)).
    pub fn new(store: Arc<dyn FindingsStore>, monitor: Monitor) -> Self {
        Self::with_clock(store, monitor, Arc::new(now_ms))
    }

    /// A service over `store`, reporting through `monitor`, timed by `clock` (Unix
    /// milliseconds, UTC). Installs `store` as the monitor's findings source, so health reads
    /// the open findings; when the monitor already has one, it keeps it and this is logged.
    /// Background runs use the Tokio runtime current at creation, or the one current when a
    /// run starts.
    pub fn with_clock(store: Arc<dyn FindingsStore>, monitor: Monitor, clock: Clock) -> Self {
        if !monitor.install_findings(store.clone()) {
            tracing::warn!(target: LOG_TARGET, "the monitor already reads another findings store");
        }
        Self {
            inner: Arc::new(Inner {
                libraries: Mutex::new(BTreeMap::new()),
                store,
                monitor,
                config: Mutex::new(ValidationConfig::default()),
                clock,
                runtime: tokio::runtime::Handle::try_current().ok(),
                stopped: AtomicBool::new(false),
                checks: AtomicUsize::new(0),
            }),
        }
    }

    /// Open `library_id` for validation over `backend`. Its first scheduled run is one
    /// interval after now, unless it has a stored last run. Opening a library that is already
    /// open only replaces its backend: a run going on it keeps its run lock and object lock.
    pub fn open_library(&self, library_id: LibraryId, backend: Arc<B>) {
        let opened_ms = (self.inner.clock)();
        let Ok(mut libraries) = self.inner.libraries.lock() else {
            return;
        };
        if let Some(open) = libraries.get_mut(library_id.as_str()) {
            open.backend = backend;
            return;
        }
        let library = OpenLibrary {
            library_id: library_id.clone(),
            backend,
            opened_ms,
            running: None,
            objects: Arc::new(tokio::sync::Mutex::new(())),
            next_due_ms: None,
        };
        libraries.insert(library_id.as_str().to_string(), library);
    }

    /// Stop validating `library_id`; a run going now is cancelled. The library is removed
    /// and its run cancelled under one lock, so no run can start in between. Answers whether
    /// it was open.
    pub fn close_library(&self, library_id: &LibraryId) -> bool {
        let Ok(mut libraries) = self.inner.libraries.lock() else {
            return false;
        };
        let Some(removed) = libraries.remove(library_id.as_str()) else {
            return false;
        };
        if let Some(flag) = &removed.running {
            flag.store(true, Ordering::SeqCst);
        }
        true
    }

    /// The schedule and cap now in force.
    pub fn config(&self) -> ValidationConfig {
        self.inner.config.lock().map(|c| *c).unwrap_or_default()
    }

    /// Change the schedule and cap for `actor` (the desktop owner or a server
    /// administrator), journaled as `ocfl.validation.configure`. The next scheduled run of
    /// each library moves with the new interval.
    ///
    /// # Errors
    ///
    /// `invalid_config` for an interval outside one hour to 366 days or a cap outside 1 to
    /// 10,000,000; nothing changes.
    pub fn configure(&self, actor: &str, config: ValidationConfig) -> Result<(), ValidationError> {
        let result = check_config(&config).and_then(|()| self.set_config(config));
        let entry = match &result {
            Ok(()) => JournalEntry::new(CONFIGURE_OPERATION, Outcome::Succeeded),
            Err(err) => failed(CONFIGURE_OPERATION, err),
        };
        self.journal(entry.actor(actor));
        result
    }

    /// Store `config` and forget the cached due times, which depend on the interval.
    fn set_config(&self, config: ValidationConfig) -> Result<(), ValidationError> {
        let mut current = self
            .inner
            .config
            .lock()
            .map_err(|_| ValidationError::Internal)?;
        *current = config;
        drop(current);
        let mut libraries = self
            .inner
            .libraries
            .lock()
            .map_err(|_| ValidationError::Internal)?;
        libraries.values_mut().for_each(|l| l.next_due_ms = None);
        Ok(())
    }

    /// Ask the run of `library_id` going now to stop after the object it is checking.
    /// Answers whether a run was going.
    pub fn cancel(&self, library_id: &LibraryId) -> bool {
        let Ok(libraries) = self.inner.libraries.lock() else {
            return false;
        };
        let running = libraries
            .get(library_id.as_str())
            .and_then(|l| l.running.clone());
        if let Some(flag) = &running {
            flag.store(true, Ordering::SeqCst);
        }
        running.is_some()
    }

    /// Take the run lock of `library_id`.
    ///
    /// # Errors
    ///
    /// `unavailable` once the service is shut down; `not_found` when the library is not
    /// open; `already_running` while a run holds it.
    pub(super) fn lock(&self, library_id: &LibraryId) -> Result<RunLock<B>, ValidationError> {
        if self.inner.stopped.load(Ordering::SeqCst) {
            return Err(ValidationError::Unavailable);
        }
        let mut libraries = self
            .inner
            .libraries
            .lock()
            .map_err(|_| ValidationError::Internal)?;
        let library = libraries
            .get_mut(library_id.as_str())
            .ok_or(ValidationError::NotFound)?;
        if library.running.is_some() {
            return Err(ValidationError::AlreadyRunning);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        library.running = Some(cancel.clone());
        Ok(RunLock {
            service: self.clone(),
            library_id: library_id.clone(),
            backend: library.backend.clone(),
            cancel,
            objects: library.objects.clone(),
        })
    }

    /// The backend and object lock of the open library `library_id`, counted as a going
    /// check until dropped.
    ///
    /// # Errors
    ///
    /// `unavailable` once the service is shut down; `not_found` when the library is not open.
    pub(super) fn access(
        &self,
        library_id: &LibraryId,
    ) -> Result<ObjectAccess<B>, ValidationError> {
        // Counted before the stop flag is read, so shut_down either sees this check or this
        // check sees the flag.
        let going = CheckGoing::start(&self.inner);
        if self.inner.stopped.load(Ordering::SeqCst) {
            return Err(ValidationError::Unavailable);
        }
        let libraries = self
            .inner
            .libraries
            .lock()
            .map_err(|_| ValidationError::Internal)?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or(ValidationError::NotFound)?;
        Ok(ObjectAccess {
            backend: library.backend.clone(),
            objects: library.objects.clone(),
            _going: going,
        })
    }

    /// The open libraries, with whether a run is going and when the next scheduled run is
    /// due (`None` when the store cannot say).
    pub(super) fn libraries(&self) -> Vec<(LibraryId, bool, Option<i64>)> {
        let snapshot: Vec<(LibraryId, bool, Option<i64>, i64)> = match self.inner.libraries.lock() {
            Ok(libraries) => libraries
                .values()
                .map(|l| {
                    (
                        l.library_id.clone(),
                        l.running.is_some(),
                        l.next_due_ms,
                        l.opened_ms,
                    )
                })
                .collect(),
            Err(_) => return Vec::new(),
        };
        snapshot
            .into_iter()
            .map(|(id, running, cached, opened)| {
                let due = cached.or_else(|| self.load_next_due(&id, opened));
                (id, running, due)
            })
            .collect()
    }

    /// Read when the next scheduled run of a library opened at `opened_ms` is due (one
    /// interval after its last run started, or after it was opened) and cache it.
    fn load_next_due(&self, library_id: &LibraryId, opened_ms: i64) -> Option<i64> {
        let last = self.inner.store.last_run(library_id).ok()?;
        let base = last.map_or(opened_ms, |run| run.started_ms);
        let due = base.saturating_add(self.config().interval_ms);
        self.remember_next_due(library_id, due);
        Some(due)
    }

    /// Keep `due` as the next scheduled run of `library_id`.
    pub(super) fn remember_next_due(&self, library_id: &LibraryId, due: i64) {
        if let Ok(mut libraries) = self.inner.libraries.lock() {
            if let Some(library) = libraries.get_mut(library_id.as_str()) {
                library.next_due_ms = Some(due);
            }
        }
    }

    /// The open libraries whose scheduled run is due now and that no run holds. The store is
    /// read only for a library whose due time is not yet known.
    pub fn due_libraries(&self) -> Vec<LibraryId> {
        let now = (self.inner.clock)();
        self.libraries()
            .into_iter()
            .filter(|(_, running, due)| !running && due.is_some_and(|due| due <= now))
            .map(|(id, _, _)| id)
            .collect()
    }

    /// Stop starting runs and takeover checks, cancel every run going now, and wait up to
    /// `wait` for the runs and the takeover checks to end (checked every 20 milliseconds), so
    /// none writes after the host stops. Answers whether every one ended in time.
    pub fn shut_down(&self, wait: std::time::Duration) -> bool {
        self.inner.stopped.store(true, Ordering::SeqCst);
        let step = std::time::Duration::from_millis(20);
        let rounds = wait.as_millis() / step.as_millis() + 1;
        for _ in 0..rounds {
            let ids: Vec<LibraryId> = self
                .libraries()
                .into_iter()
                .filter(|(_, running, _)| *running)
                .map(|(id, _, _)| id)
                .collect();
            let checks = self.inner.checks.load(Ordering::SeqCst);
            if ids.is_empty() && checks == 0 {
                return true;
            }
            ids.iter().for_each(|id| {
                self.cancel(id);
            });
            std::thread::sleep(step);
        }
        tracing::warn!(target: LOG_TARGET, "OCFL validation runs or checks still going at shutdown");
        false
    }

    /// Journal a refused run of `library_id` for `actor`.
    pub(super) fn refused(&self, actor: &str, library_id: &LibraryId, err: &ValidationError) {
        self.journal(
            failed(RUN_OPERATION, err)
                .actor(actor)
                .target(library_id.as_str()),
        );
    }

    /// Send `entry`, stamped with the service clock, through the monitor's bounded journal
    /// queue.
    pub(super) fn journal(&self, entry: JournalEntry) {
        self.inner
            .monitor
            .journal()
            .send(entry.at_ms((self.inner.clock)()));
    }
}

/// Check `config` against the bounds.
fn check_config(config: &ValidationConfig) -> Result<(), ValidationError> {
    let interval_ok = (MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(&config.interval_ms);
    let cap_ok = (1..=MAX_OBJECT_CAP).contains(&config.object_cap);
    if interval_ok && cap_ok {
        return Ok(());
    }
    Err(ValidationError::Invalid {
        code: "invalid_config",
        message: "Choose a schedule between one hour and a year and a cap between 1 and 10,000,000 objects.",
    })
}

/// The failed entry of `operation` for `err`: warn for a refusal, error for a fault.
pub(super) fn failed(operation: &str, err: &ValidationError) -> JournalEntry {
    let level = match err {
        ValidationError::Unavailable | ValidationError::Internal => Level::Error,
        _ => Level::Warn,
    };
    tracing::info!(target: LOG_TARGET, operation, error_code = err.error_code(), "refused");
    JournalEntry::new(operation, Outcome::Failed)
        .error_code(err.error_code())
        .level(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::MemoryJournal;
    use crate::monitor::{MonitorConfig, StaticHostChecks};
    use crate::ocfl_validation::MemoryFindingsStore;
    use filesquid_ocfl::storage::MemoryBackend;

    /// A run lock taken before a library was closed and opened again never frees the run
    /// lock of the reopened library.
    #[test]
    fn an_old_run_lock_never_frees_a_reopened_library() {
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("test")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("test"),
            crate::EventBus::new(),
        );
        let service: ValidationService<MemoryBackend> =
            ValidationService::new(Arc::new(MemoryFindingsStore::new()), monitor.clone());
        let id = LibraryId::new("lib-1").expect("id");
        service.open_library(id.clone(), Arc::new(MemoryBackend::new()));
        let old = service.lock(&id).expect("first run");
        assert!(service.close_library(&id));
        service.open_library(id.clone(), Arc::new(MemoryBackend::new()));
        let new = service.lock(&id).expect("a run of the reopened library");
        drop(old);
        assert_eq!(
            service.lock(&id).err(),
            Some(ValidationError::AlreadyRunning)
        );
        drop(new);
        assert!(service.lock(&id).is_ok(), "free once its own run ends");
        monitor.shutdown();
    }

    /// A service over a memory store, its monitor, and one open library.
    fn open_service() -> (ValidationService<MemoryBackend>, Monitor, LibraryId) {
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("test")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("test"),
            crate::EventBus::new(),
        );
        let service: ValidationService<MemoryBackend> =
            ValidationService::new(Arc::new(MemoryFindingsStore::new()), monitor.clone());
        let id = LibraryId::new("lib-1").expect("id");
        service.open_library(id.clone(), Arc::new(MemoryBackend::new()));
        (service, monitor, id)
    }

    /// Opening a library that is already open keeps the run lock and the object lock of the
    /// run going on it.
    #[test]
    fn reopening_an_open_library_keeps_its_locks() {
        let (service, monitor, id) = open_service();
        let run = service.lock(&id).expect("run");
        let objects = run.objects.clone();
        service.open_library(id.clone(), Arc::new(MemoryBackend::new()));
        assert_eq!(
            service.lock(&id).err(),
            Some(ValidationError::AlreadyRunning)
        );
        let access = service.access(&id).expect("access");
        assert!(Arc::ptr_eq(&access.objects, &objects), "one object lock");
        drop((run, access));
        monitor.shutdown();
    }

    /// Closing a library cancels the run going on it, and no run can start on it after.
    #[test]
    fn closing_a_library_cancels_its_run() {
        let (service, monitor, id) = open_service();
        let run = service.lock(&id).expect("run");
        assert!(service.close_library(&id));
        assert!(run.cancel.load(Ordering::SeqCst), "the run is told to stop");
        assert_eq!(service.lock(&id).err(), Some(ValidationError::NotFound));
        assert!(!service.close_library(&id));
        monitor.shutdown();
    }

    /// After shutdown no takeover check starts, and shutdown waits for one going.
    #[test]
    fn shutdown_waits_for_takeover_checks_and_refuses_new_ones() {
        let (service, monitor, id) = open_service();
        let going = service.access(&id).expect("access");
        let wait = std::time::Duration::from_millis(40);
        assert!(!service.shut_down(wait), "a check is going");
        assert_eq!(
            service.access(&id).err(),
            Some(ValidationError::Unavailable)
        );
        drop(going);
        assert!(service.shut_down(wait), "nothing left");
        monitor.shutdown();
    }
}
