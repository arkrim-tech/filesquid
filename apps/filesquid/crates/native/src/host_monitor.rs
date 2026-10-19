//! A host's running monitor over its native operational store
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Both FileSquid hosts use [`HostMonitor::open`], so their startup, ticking and shutdown
//! behave the same.

use crate::{DataDirChecks, SqliteFindings, SqliteJournal};
use filesquid_api::journal::{JournalEntry, JournalStore, Outcome};
use filesquid_api::monitor::{HostChecks, Monitor, MonitorConfig};
use filesquid_api::ocfl_validation::{
    OcflValidation, ValidationError, ValidationService, START_OPERATION,
};
use filesquid_api::registry::{ProcessHandle, ProcessSpec};
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_ocfl::storage::StorageBackend;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Heartbeat timeout for the long-lived `host` process.
pub const HOST_HEARTBEAT_TIMEOUT_MS: i64 = 30_000;

/// File name of the SQLite operational store inside the data directory.
pub const JOURNAL_FILE: &str = "filesquid.sqlite";

/// Error returned when OCFL validation was already started for this host or state.
pub const VALIDATION_ALREADY_STARTED: &str = "OCFL validation is already started";

/// Longest wait at stop for OCFL validation runs to end after they are cancelled.
pub const VALIDATION_STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Error returned when the API state already has a monitor installed.
pub const ALREADY_INSTALLED: &str = "a monitor is already installed in this API state";

/// A host's running monitor plus its own `host` process, created by [`HostMonitor::open`].
pub struct HostMonitor {
    monitor: Monitor,
    host: ProcessHandle,
    /// Held by [`tick`](Self::tick) and [`stop`](Self::stop); `true` once stopped. An
    /// in-flight tick finishes before stop shuts the journal writer, and no tick runs after.
    stopped: Mutex<bool>,
    /// The OCFL validation [`tick`](Self::tick) drives, once started.
    validation: OnceLock<Arc<dyn OcflValidation>>,
}

impl HostMonitor {
    /// Open (creating if needed) `<data_dir>/filesquid.sqlite` and start the host's monitor
    /// over it with the data-directory checks (see [`start`](Self::start)). Performs
    /// filesystem and database I/O.
    ///
    /// # Errors
    /// The data directory or the journal could not be opened, or `state` already has a
    /// monitor (checked before any file is created).
    pub fn open(
        data_dir: &Path,
        host: &str,
        state: &ApiState,
        layer: Option<&JournalLayer>,
    ) -> Result<Self, String> {
        if state.monitor().is_some() {
            return Err(ALREADY_INSTALLED.to_string());
        }
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("create data dir {}: {e}", data_dir.display()))?;
        let store =
            SqliteJournal::open(&data_dir.join(JOURNAL_FILE), host).map_err(|e| e.to_string())?;
        let checks = DataDirChecks::new(data_dir);
        Self::start(Arc::new(store), Arc::new(checks), host, state, layer)
    }

    /// Start the monitor over the host-injected `store` and `checks`, install it in `state`,
    /// attach `layer` (if given) so warnings and errors are journaled, register the `host`
    /// process and journal `host.started`. A host with another journal store (such as the
    /// server's PostgreSQL store) uses this directly.
    ///
    /// # Errors
    /// `state` already has a monitor. The monitor started here is then shut down (its journal
    /// writer stopped) and nothing is installed or journaled.
    pub fn start(
        store: Arc<dyn JournalStore>,
        checks: Arc<dyn HostChecks>,
        host: &str,
        state: &ApiState,
        layer: Option<&JournalLayer>,
    ) -> Result<Self, String> {
        let monitor = Monitor::start(
            store,
            checks,
            MonitorConfig::new(host),
            state.events.clone(),
        );
        if !state.install_monitor(monitor.clone()) {
            monitor.shutdown();
            return Err(ALREADY_INSTALLED.to_string());
        }
        if let Some(layer) = layer {
            layer.attach(monitor.journal());
        }
        monitor
            .journal()
            .send(JournalEntry::new("host.started", Outcome::Succeeded).target(host));
        let host_process = monitor
            .registry()
            .register(ProcessSpec::new("host", HOST_HEARTBEAT_TIMEOUT_MS));
        Ok(Self {
            monitor,
            host: host_process,
            stopped: Mutex::new(false),
            validation: OnceLock::new(),
        })
    }

    /// Start OCFL validation (API-S-022) over the findings store in
    /// `<data_dir>/filesquid.sqlite`: install it in `state` for the validation routes and
    /// let [`tick`](Self::tick) start the scheduled runs. The host opens each library with
    /// [`ValidationService::open_library`] on the service answered. Performs database I/O.
    ///
    /// # Errors
    /// The findings store could not be opened (journaled as `ocfl.validation.start` with
    /// `unavailable` or `internal`), or validation was already started for this host or
    /// installed in `state` (journaled with `conflict`).
    pub fn start_validation<B: StorageBackend + 'static>(
        &self,
        data_dir: &Path,
        state: &ApiState,
    ) -> Result<ValidationService<B>, String> {
        self.install_validation(data_dir, state)
            .map_err(|(code, message)| {
                let entry = JournalEntry::new(START_OPERATION, Outcome::Failed).error_code(code);
                self.monitor.journal().send(entry);
                message
            })
    }

    /// The body of [`start_validation`](Self::start_validation): the error carries its
    /// journal error code.
    fn install_validation<B: StorageBackend + 'static>(
        &self,
        data_dir: &Path,
        state: &ApiState,
    ) -> Result<ValidationService<B>, (&'static str, String)> {
        let already = || ("conflict", VALIDATION_ALREADY_STARTED.to_string());
        if self.validation.get().is_some() || state.validation().is_some() {
            return Err(already());
        }
        let store = SqliteFindings::open(&data_dir.join(JOURNAL_FILE)).map_err(|err| {
            let message = err.to_string();
            (ValidationError::from(err).error_code(), message)
        })?;
        let service = ValidationService::new(Arc::new(store), self.monitor.clone());
        let handle: Arc<dyn OcflValidation> = Arc::new(service.clone());
        if self.validation.set(handle.clone()).is_err() || !state.install_validation(handle) {
            return Err(already());
        }
        Ok(service)
    }

    /// The monitor.
    pub fn monitor(&self) -> &Monitor {
        &self.monitor
    }

    /// Periodic work (call about once a second): host heartbeat, stalled checks, health, and
    /// the OCFL validation runs that are due.
    /// Does nothing after [`stop`](Self::stop).
    pub fn tick(&self) {
        let stopped = self.stopped.lock().unwrap_or_else(|p| p.into_inner());
        if *stopped {
            return;
        }
        self.host.heartbeat();
        self.monitor.tick();
        if let Some(validation) = self.validation.get() {
            validation.tick();
        }
    }

    /// Wait for an in-flight tick, cancel the OCFL validation runs and wait for them (at most
    /// [`VALIDATION_STOP_WAIT`]), then journal `host.stopped`, finish the host process and
    /// flush and stop the journal writer. Later calls do nothing.
    pub fn stop(&self) {
        let mut stopped = self.stopped.lock().unwrap_or_else(|p| p.into_inner());
        if *stopped {
            return;
        }
        *stopped = true;
        if let Some(validation) = self.validation.get() {
            validation.shut_down(VALIDATION_STOP_WAIT);
        }
        self.monitor.journal().send(
            JournalEntry::new("host.stopped", Outcome::Succeeded).target(self.monitor.host()),
        );
        self.host.complete();
        self.monitor.shutdown();
    }
}
