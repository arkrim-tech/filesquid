//! [`ValidationService`]: the open libraries, the schedule and the one-run-per-library lock,
//! and the runs themselves (API-S-022).
//!
//! The service is generic over the host's storage backend type `B`. The host opens each
//! library it has open with [`ValidationService::open_library`] and installs the service in
//! [`crate::ApiState`] as an [`super::OcflValidation`], which the routes and the host's
//! ticker use. A run registers in the process registry as `ocfl.validation`, with progress
//! and heartbeat, and its outcome is journaled as `ocfl.validation.run`. Any change in the
//! open findings publishes the health report on `health.change`.

use super::error::ValidationError;
use super::run::{check_object, index, pass, PassResult, RunContext};
use super::store::{FindingsStore, RunOutcome, RunRecord, RunTrigger};
use super::{CONFIGURE_OPERATION, LOG_TARGET, RUN_OPERATION};
use crate::journal::{now_ms, JournalEntry, Level, Outcome};
use crate::monitor::Monitor;
use crate::registry::{Clock, ProcessSpec};
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::StorageBackend;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
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

/// Longest silence of a running validation before it counts as stalled: one object can take
/// long to check when its content is large.
const HEARTBEAT_TIMEOUT_MS: i64 = 30 * 60 * 1000;

/// The schedule and the cap, which the desktop owner or a server administrator may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationConfig {
    /// Time between scheduled runs of a library, in milliseconds.
    pub interval_ms: i64,
    /// Most objects one run checks.
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
}

/// Shared state of the service.
pub(super) struct Inner<B> {
    libraries: Mutex<BTreeMap<String, OpenLibrary<B>>>,
    pub(super) store: Arc<dyn FindingsStore>,
    pub(super) monitor: Monitor,
    config: Mutex<ValidationConfig>,
    pub(super) clock: Clock,
    pub(super) runtime: Option<tokio::runtime::Handle>,
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

/// Holds a library's run lock; releases it when dropped, on every path.
pub(super) struct RunLock<B> {
    service: ValidationService<B>,
    library_id: LibraryId,
    pub(super) backend: Arc<B>,
    pub(super) cancel: Arc<AtomicBool>,
}

impl<B> Drop for RunLock<B> {
    fn drop(&mut self) {
        if let Ok(mut libraries) = self.service.inner.libraries.lock() {
            if let Some(library) = libraries.get_mut(self.library_id.as_str()) {
                library.running = None;
            }
        }
    }
}

impl<B: StorageBackend + 'static> ValidationService<B> {
    /// A service over `store`, reporting through `monitor`, with the default schedule and
    /// cap and the system clock. Installs `store` as the monitor's findings source, so health
    /// reads the open findings. Background runs use the Tokio runtime current at creation.
    pub fn new(store: Arc<dyn FindingsStore>, monitor: Monitor) -> Self {
        monitor.install_findings(store.clone());
        Self {
            inner: Arc::new(Inner {
                libraries: Mutex::new(BTreeMap::new()),
                store,
                monitor,
                config: Mutex::new(ValidationConfig::default()),
                clock: Arc::new(now_ms),
                runtime: tokio::runtime::Handle::try_current().ok(),
            }),
        }
    }

    /// Use `clock` (Unix milliseconds, UTC) for run times and the schedule. Call before the
    /// service is shared.
    pub fn with_clock(self, clock: Clock) -> Self {
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|_| {
            panic!("with_clock must be called before the service is shared")
        });
        Self {
            inner: Arc::new(Inner { clock, ..inner }),
        }
    }

    /// Open `library_id` for validation over `backend`. Its first scheduled run is one
    /// interval after now, unless it has a stored last run.
    pub fn open_library(&self, library_id: LibraryId, backend: Arc<B>) {
        let opened_ms = (self.inner.clock)();
        if let Ok(mut libraries) = self.inner.libraries.lock() {
            libraries.insert(
                library_id.as_str().to_string(),
                OpenLibrary {
                    library_id,
                    backend,
                    opened_ms,
                    running: None,
                },
            );
        }
    }

    /// Stop validating `library_id`; a run going now is cancelled. Answers whether it was
    /// open.
    pub fn close_library(&self, library_id: &LibraryId) -> bool {
        self.cancel(library_id);
        self.inner
            .libraries
            .lock()
            .map(|mut libraries| libraries.remove(library_id.as_str()).is_some())
            .unwrap_or(false)
    }

    /// The schedule and cap now in force.
    pub fn config(&self) -> ValidationConfig {
        self.inner.config.lock().map(|c| *c).unwrap_or_default()
    }

    /// Change the schedule and cap for `actor` (the desktop owner or a server
    /// administrator), journaled as `ocfl.validation.configure`.
    ///
    /// # Errors
    ///
    /// `invalid_config` for an interval outside one hour to 366 days or a cap outside 1 to
    /// 10,000,000; nothing changes.
    pub fn configure(&self, actor: &str, config: ValidationConfig) -> Result<(), ValidationError> {
        let interval_ok = (MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(&config.interval_ms);
        let cap_ok = (1..=MAX_OBJECT_CAP).contains(&config.object_cap);
        let result = match (interval_ok, cap_ok) {
            (true, true) => self
                .inner
                .config
                .lock()
                .map(|mut c| *c = config)
                .map_err(|_| ValidationError::Internal),
            _ => Err(ValidationError::Invalid {
                code: "invalid_config",
                message: "Choose a schedule between one hour and a year and a cap between 1 and 10,000,000 objects.",
            }),
        };
        let entry = match &result {
            Ok(()) => JournalEntry::new(CONFIGURE_OPERATION, Outcome::Succeeded),
            Err(err) => failed(CONFIGURE_OPERATION, err),
        };
        self.journal(entry.actor(actor));
        result
    }

    /// Ask the run of `library_id` going now to stop after the object it is checking.
    /// Answers whether a run was going.
    pub fn cancel(&self, library_id: &LibraryId) -> bool {
        let libraries = match self.inner.libraries.lock() {
            Ok(libraries) => libraries,
            Err(_) => return false,
        };
        let running = libraries
            .get(library_id.as_str())
            .and_then(|l| l.running.clone());
        if let Some(flag) = &running {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        running.is_some()
    }

    /// Take the run lock of `library_id`.
    ///
    /// # Errors
    ///
    /// `not_found` when the library is not open; `already_running` while a run holds it.
    pub(super) fn lock(&self, library_id: &LibraryId) -> Result<RunLock<B>, ValidationError> {
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
        })
    }

    /// Validate `library_id` now for `actor`: one pass from the cursor (or from the start),
    /// up to the configured cap. A failed pass (storage or store outage) is recorded and
    /// answered as a run with outcome `failed`.
    ///
    /// # Errors
    ///
    /// `not_found` or `already_running`; both are journaled and nothing is checked.
    pub async fn run(
        &self,
        library_id: &LibraryId,
        trigger: RunTrigger,
        from_start: bool,
        actor: &str,
    ) -> Result<RunRecord, ValidationError> {
        let lock = self
            .lock(library_id)
            .inspect_err(|err| self.refused(actor, library_id, err))?;
        Ok(self.run_locked(lock, trigger, from_start, actor).await)
    }

    /// Run one pass under `lock`, register it as a process, record and journal it.
    pub(super) async fn run_locked(
        &self,
        lock: RunLock<B>,
        trigger: RunTrigger,
        from_start: bool,
        actor: &str,
    ) -> RunRecord {
        let library_id = lock.library_id.clone();
        let process = self
            .inner
            .monitor
            .registry()
            .register(ProcessSpec::new("ocfl.validation", HEARTBEAT_TIMEOUT_MS));
        let started_ms = (self.inner.clock)();
        let journal = self.inner.monitor.journal();
        let ctx = RunContext {
            store: self.inner.store.as_ref(),
            journal: &journal,
            library_id: &library_id,
            actor,
            cancel: &lock.cancel,
            process: Some(&process),
        };
        let cap = self.config().object_cap;
        let passed = match index(lock.backend.as_ref()).await {
            Ok(objects) => pass(&ctx, lock.backend.as_ref(), &objects, from_start, cap).await,
            Err(err) => Err(err),
        };
        drop(lock);
        let record = self.record(&library_id, trigger, started_ms, &passed);
        self.finish(&process, actor, &record, &passed);
        record
    }

    /// The record of a pass.
    fn record(
        &self,
        library_id: &LibraryId,
        trigger: RunTrigger,
        started_ms: i64,
        passed: &Result<PassResult, ValidationError>,
    ) -> RunRecord {
        let (outcome, checked, not_checked, error_code) = match passed {
            Ok(p) => (p.outcome, p.checked, p.total.saturating_sub(p.checked), None),
            Err(err) => (RunOutcome::Failed, 0, 0, Some(err.error_code().to_string())),
        };
        RunRecord {
            library_id: library_id.clone(),
            trigger,
            started_ms,
            ended_ms: (self.inner.clock)(),
            outcome,
            checked,
            not_checked,
            error_code,
        }
    }

    /// Store and journal the record, end the process, and publish health when findings
    /// changed.
    fn finish(
        &self,
        process: &crate::registry::ProcessHandle,
        actor: &str,
        record: &RunRecord,
        passed: &Result<PassResult, ValidationError>,
    ) {
        if let Err(err) = self.inner.store.record_run(record) {
            tracing::warn!(target: LOG_TARGET, error = %err, "validation run not recorded");
        }
        let entry = match record.outcome {
            RunOutcome::Completed | RunOutcome::Capped => {
                JournalEntry::new(RUN_OPERATION, Outcome::Succeeded)
            }
            RunOutcome::Cancelled => JournalEntry::new(RUN_OPERATION, Outcome::Cancelled),
            RunOutcome::Failed => JournalEntry::new(RUN_OPERATION, Outcome::Failed)
                .error_code(record.error_code.as_deref().unwrap_or("internal")),
        };
        self.journal(
            entry
                .actor(actor)
                .target(record.library_id.as_str())
                .process(process.id())
                .duration_ms(u64::try_from(record.ended_ms - record.started_ms).unwrap_or(0)),
        );
        match record.outcome {
            RunOutcome::Failed => process.fail(record.error_code.as_deref().unwrap_or("internal")),
            RunOutcome::Cancelled => process.cancel(),
            _ => process.complete(),
        }
        if passed.as_ref().is_ok_and(|p| p.changed) {
            self.inner.monitor.publish_health();
        }
    }

    /// Validate `object_id` of `library_id` after a commit on it, when that commit took
    /// over an expired or lost reservation (API-S-022). Any other commit is not validated
    /// here. This is not a run: it takes no run lock and does not move the cursor.
    ///
    /// # Errors
    ///
    /// `not_found` when the library is not open; `unavailable` for a storage or store
    /// outage.
    pub async fn validate_after_commit(
        &self,
        library_id: &LibraryId,
        object_id: &str,
        took_over: bool,
    ) -> Result<(), ValidationError> {
        if !took_over {
            return Ok(());
        }
        let backend = self.backend(library_id)?;
        let objects = index(backend.as_ref()).await?;
        let Some(object) = objects.iter().find(|o| o.id == object_id) else {
            return Ok(());
        };
        let journal = self.inner.monitor.journal();
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            store: self.inner.store.as_ref(),
            journal: &journal,
            library_id,
            actor: "system",
            cancel: &cancel,
            process: None,
        };
        if !check_object(&ctx, backend.as_ref(), object).await?.is_empty() {
            self.inner.monitor.publish_health();
        }
        Ok(())
    }

    /// The backend of the open library `library_id`.
    fn backend(&self, library_id: &LibraryId) -> Result<Arc<B>, ValidationError> {
        let libraries = self
            .inner
            .libraries
            .lock()
            .map_err(|_| ValidationError::Internal)?;
        libraries
            .get(library_id.as_str())
            .map(|l| l.backend.clone())
            .ok_or(ValidationError::NotFound)
    }

    /// The open libraries, with whether a run is going and when each was opened.
    pub(super) fn libraries(&self) -> Vec<(LibraryId, bool, i64)> {
        self.inner
            .libraries
            .lock()
            .map(|libraries| {
                libraries
                    .values()
                    .map(|l| (l.library_id.clone(), l.running.is_some(), l.opened_ms))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// When the next scheduled run of a library opened at `opened_ms` is due: one interval
    /// after its last run started, or after it was opened.
    pub(super) fn next_run_ms(&self, library_id: &LibraryId, opened_ms: i64) -> Option<i64> {
        let last = self.inner.store.last_run(library_id).ok()?;
        let base = last.map_or(opened_ms, |run| run.started_ms);
        Some(base.saturating_add(self.config().interval_ms))
    }

    /// The open libraries whose scheduled run is due now and that no run holds.
    pub fn due_libraries(&self) -> Vec<LibraryId> {
        let now = (self.inner.clock)();
        self.libraries()
            .into_iter()
            .filter(|(_, running, _)| !running)
            .filter(|(id, _, opened)| self.next_run_ms(id, *opened).is_some_and(|due| due <= now))
            .map(|(id, _, _)| id)
            .collect()
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
        self.inner.monitor.journal().send(entry.at_ms((self.inner.clock)()));
    }
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
