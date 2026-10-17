//! The host's monitor: activity journal, process registry, metrics and health in one handle
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Each host starts one [`Monitor`], installs it in [`crate::ApiState`], and calls
//! [`Monitor::tick`] about once a second.

use crate::events::topic;
use crate::health::{evaluate, HealthInputs, HealthReport, HealthStatus, ERROR_WINDOW_MS};
use crate::journal::{
    JournalEntry, JournalError, JournalQuery, JournalStore, Level, Outcome, RetentionPolicy,
};
use crate::journal_writer::{JournalSender, JournalWriter, WriterConfig};
use crate::metrics::Metrics;
use crate::registry::{system_clock, Clock, ProcessRegistry, ProcessState};
use crate::{ApiEvent, EventBus};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Monitor settings for one host.
#[derive(Clone)]
pub struct MonitorConfig {
    /// Host name recorded on journal entries.
    pub host: String,
    /// Directory holding the operational store; checked for writability and free space.
    pub data_dir: PathBuf,
    /// Journal writer queue settings.
    pub writer: WriterConfig,
    /// Journal retention (default: keep everything).
    pub retention: RetentionPolicy,
    /// Clock for the registry and health window.
    pub clock: Clock,
}

impl MonitorConfig {
    /// Defaults: 1 024-entry queue, keep everything, system clock. Who may read the monitor
    /// is decided by the host's [`crate::auth::Authoriser`], not here.
    pub fn new(host: impl Into<String>, data_dir: impl Into<PathBuf>) -> Self {
        Self {
            host: host.into(),
            data_dir: data_dir.into(),
            writer: WriterConfig::default(),
            retention: RetentionPolicy::KeepAll,
            clock: system_clock(),
        }
    }
}

struct Inner {
    store: Arc<dyn JournalStore>,
    writer: JournalWriter,
    registry: ProcessRegistry,
    metrics: Arc<Metrics>,
    config: MonitorConfig,
    events: EventBus,
    last_status: Mutex<Option<HealthStatus>>,
}

/// Cloneable handle to a host's monitor.
#[derive(Clone)]
pub struct Monitor {
    inner: Arc<Inner>,
}

impl Monitor {
    /// Start the journal writer and the registry for `store`.
    pub fn start(store: Arc<dyn JournalStore>, config: MonitorConfig, events: EventBus) -> Self {
        let metrics = Arc::new(Metrics::default());
        let writer = JournalWriter::start(
            store.clone(),
            config.writer,
            events.clone(),
            metrics.clone(),
        );
        let registry = ProcessRegistry::new(
            writer.sender(),
            events.clone(),
            metrics.clone(),
            config.clock.clone(),
        );
        Self {
            inner: Arc::new(Inner {
                store,
                writer,
                registry,
                metrics,
                config,
                events,
                last_status: Mutex::new(None),
            }),
        }
    }

    /// Sender for journal entries.
    pub fn journal(&self) -> JournalSender {
        self.inner.writer.sender()
    }

    /// The process registry.
    pub fn registry(&self) -> &ProcessRegistry {
        &self.inner.registry
    }

    /// The shared counters and gauges.
    pub fn metrics(&self) -> &Metrics {
        &self.inner.metrics
    }

    /// The journal store (for queries).
    pub fn store(&self) -> Arc<dyn JournalStore> {
        self.inner.store.clone()
    }

    /// Host name.
    pub fn host(&self) -> &str {
        &self.inner.config.host
    }

    /// Gather host checks and evaluate health. Performs I/O (store ping, a probe write in the
    /// data directory, a free-space query, an error count).
    pub fn health_report(&self) -> HealthReport {
        let report = evaluate(&self.gather_inputs());
        self.inner.metrics.set_health(report.status.gauge());
        report
    }

    /// Periodic work: mark stalled processes, re-evaluate health, and journal and publish a
    /// health change. Performs I/O through [`health_report`](Self::health_report).
    pub fn tick(&self) -> HealthReport {
        self.inner.registry.check_stalled();
        let report = self.health_report();
        let previous = self
            .inner
            .last_status
            .lock()
            .map(|mut s| s.replace(report.status))
            .unwrap_or(None);
        if previous != Some(report.status) {
            let level = match report.status {
                HealthStatus::Ok => Level::Info,
                HealthStatus::Degraded => Level::Warn,
                HealthStatus::Failing => Level::Error,
            };
            let mut entry = JournalEntry::new("health.changed", Outcome::Info).level(level);
            entry.error_code = report.reasons.first().map(|r| r.code.clone());
            self.journal().send(entry);
            if let Ok(data) = serde_json::to_value(&report) {
                self.inner
                    .events
                    .publish(ApiEvent::new(topic::HEALTH_CHANGE, data));
            }
        }
        report
    }

    /// Apply the configured retention policy to the journal store (I/O).
    pub fn apply_retention(&self) -> Result<u64, JournalError> {
        self.inner
            .store
            .apply_retention(self.inner.config.retention, (self.inner.config.clock)())
    }

    /// Write everything queued and stop the journal writer.
    pub fn shutdown(&self) {
        self.inner.writer.shutdown();
    }

    /// Collect the facts the health rules need (I/O).
    fn gather_inputs(&self) -> HealthInputs {
        let now = (self.inner.config.clock)();
        let stalled = self
            .inner
            .registry
            .snapshot()
            .iter()
            .filter(|p| p.state == ProcessState::Stalled)
            .count();
        let errors = JournalQuery {
            level: Some(Level::Error),
            from_ms: Some(now - ERROR_WINDOW_MS),
            ..Default::default()
        };
        HealthInputs {
            stalled,
            store_ok: self.inner.store.ping().is_ok(),
            data_dir_writable: probe_write(&self.inner.config.data_dir),
            free_bytes: fs4::available_space(&self.inner.config.data_dir).ok(),
            queue_fill: self.journal().queue_fill(),
            errors_last_5m: self.inner.store.count(&errors).unwrap_or(0),
        }
    }
}

/// Distinguishes concurrent probes in one process.
static PROBE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Prove the directory is writable by exclusively creating a uniquely named probe file and
/// removing it (I/O). `create_new` means an existing file is never truncated or removed.
fn probe_write(dir: &std::path::Path) -> bool {
    use std::io::Write;
    let probe = dir.join(format!(
        ".filesquid-write-probe-{}-{}",
        std::process::id(),
        PROBE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    else {
        return false;
    };
    let ok = file.write_all(b"ok").is_ok();
    drop(file);
    if let Err(err) = std::fs::remove_file(&probe) {
        tracing::warn!(%err, error_code = "probe_cleanup_failed", "write probe not removed");
    }
    ok
}

/// A host's running monitor plus its own `host` process, created by
/// [`HostMonitor::open`]. Both FileSquid hosts use this so their startup, ticking and
/// shutdown behave the same.
#[cfg(feature = "sqlite")]
pub struct HostMonitor {
    monitor: Monitor,
    host: crate::registry::ProcessHandle,
    /// Held by [`tick`](Self::tick) and [`stop`](Self::stop); `true` once stopped. An
    /// in-flight tick finishes before stop shuts the journal writer, and no tick runs after.
    stopped: Mutex<bool>,
}

/// Heartbeat timeout for the long-lived `host` process.
#[cfg(feature = "sqlite")]
pub const HOST_HEARTBEAT_TIMEOUT_MS: i64 = 30_000;

#[cfg(feature = "sqlite")]
impl HostMonitor {
    /// Open (creating if needed) `<data_dir>/filesquid.sqlite`, start the monitor, install it
    /// in `state`, attach `layer` (if given) so warnings and errors are journaled, register the
    /// `host` process and journal `host.started`. Performs filesystem and database I/O.
    pub fn open(
        data_dir: &std::path::Path,
        host: &str,
        state: &crate::ApiState,
        layer: Option<&crate::trace_layer::JournalLayer>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("create data dir {}: {e}", data_dir.display()))?;
        let store = crate::journal::SqliteJournal::open(&data_dir.join("filesquid.sqlite"), host)
            .map_err(|e| e.to_string())?;
        let monitor = Monitor::start(
            Arc::new(store),
            MonitorConfig::new(host, data_dir),
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
            .register(crate::registry::ProcessSpec::new(
                "host",
                HOST_HEARTBEAT_TIMEOUT_MS,
            ));
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
