//! A host's running monitor over its native operational store
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Both FileSquid hosts use [`HostMonitor::open`], so their startup, ticking and shutdown
//! behave the same.

use crate::{DataDirChecks, SqliteJournal};
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::registry::{ProcessHandle, ProcessSpec};
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Heartbeat timeout for the long-lived `host` process.
pub const HOST_HEARTBEAT_TIMEOUT_MS: i64 = 30_000;

/// File name of the SQLite operational store inside the data directory.
pub const JOURNAL_FILE: &str = "filesquid.sqlite";

/// A host's running monitor plus its own `host` process, created by [`HostMonitor::open`].
pub struct HostMonitor {
    monitor: Monitor,
    host: ProcessHandle,
    /// Held by [`tick`](Self::tick) and [`stop`](Self::stop); `true` once stopped. An
    /// in-flight tick finishes before stop shuts the journal writer, and no tick runs after.
    stopped: Mutex<bool>,
}

impl HostMonitor {
    /// Open (creating if needed) `<data_dir>/filesquid.sqlite`, start the monitor with the
    /// data-directory checks, install it in `state`, attach `layer` (if given) so warnings and
    /// errors are journaled, register the `host` process and journal `host.started`.
    /// Performs filesystem and database I/O.
    pub fn open(
        data_dir: &Path,
        host: &str,
        state: &ApiState,
        layer: Option<&JournalLayer>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("create data dir {}: {e}", data_dir.display()))?;
        let store =
            SqliteJournal::open(&data_dir.join(JOURNAL_FILE), host).map_err(|e| e.to_string())?;
        let monitor = Monitor::start(
            Arc::new(store),
            Arc::new(DataDirChecks::new(data_dir)),
            MonitorConfig::new(host),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
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
        })
    }

    /// The monitor.
    pub fn monitor(&self) -> &Monitor {
        &self.monitor
    }

    /// Periodic work (call about once a second): host heartbeat, stalled checks, health.
    /// Does nothing after [`stop`](Self::stop).
    pub fn tick(&self) {
        let stopped = self.stopped.lock().unwrap_or_else(|p| p.into_inner());
        if *stopped {
            return;
        }
        self.host.heartbeat();
        self.monitor.tick();
    }

    /// Wait for an in-flight tick, then journal `host.stopped`, finish the host process and
    /// flush and stop the journal writer. Later calls do nothing.
    pub fn stop(&self) {
        let mut stopped = self.stopped.lock().unwrap_or_else(|p| p.into_inner());
        if *stopped {
            return;
        }
        *stopped = true;
        self.monitor.journal().send(
            JournalEntry::new("host.stopped", Outcome::Succeeded).target(self.monitor.host()),
        );
        self.host.complete();
        self.monitor.shutdown();
    }
}
