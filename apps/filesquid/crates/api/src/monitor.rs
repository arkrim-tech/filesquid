//! The host's monitor: activity journal, process registry, metrics and health in one handle
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Each host starts one [`Monitor`], installs it in [`crate::ApiState`], and calls
//! [`Monitor::tick`] about once a second.

use crate::events::topic;
use crate::health::{evaluate, HealthInputs, HealthReport, HealthStatus, ERROR_WINDOW_MS};
use crate::journal::{
    JournalEntry, JournalError, JournalQuery, JournalStore, Level, Outcome, RetentionPolicy,
};
use crate::journal_writer::{AnnouncingJournal, JournalSender, JournalWriter, WriterConfig};
use crate::metrics::Metrics;
use crate::registry::{system_clock, Clock, ProcessRegistry, ProcessState};
use crate::{ApiEvent, EventBus};
use std::sync::{Arc, Mutex};

/// The host checks the health rules need that require storage I/O (SRS-F-061 Health: data
/// directory writable, free space against thresholds). Hosts inject an implementation, as
/// they inject the [`JournalStore`], so this crate performs no storage I/O itself
/// ([API-S-006](../resources/srs.md)); the native one is `filesquid_native::DataDirChecks`.
pub trait HostChecks: Send + Sync {
    /// Whether the directory holding the operational store accepts a write (may do I/O).
    fn data_dir_writable(&self) -> bool;
    /// Free bytes on that directory's volume, when measurable (may do I/O).
    fn free_bytes(&self) -> Option<u64>;
}

/// Fixed host-check answers, for hosts with no data directory (such as a future browser
/// host, whose store is not a local directory) and for tests. Pure: performs no I/O.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticHostChecks {
    /// Answer for [`HostChecks::data_dir_writable`].
    pub data_dir_writable: bool,
    /// Answer for [`HostChecks::free_bytes`].
    pub free_bytes: Option<u64>,
}

impl StaticHostChecks {
    /// Writable, with free space not measured (no free-space reason is raised).
    pub const fn healthy() -> Self {
        Self {
            data_dir_writable: true,
            free_bytes: None,
        }
    }
}

impl HostChecks for StaticHostChecks {
    fn data_dir_writable(&self) -> bool {
        self.data_dir_writable
    }

    fn free_bytes(&self) -> Option<u64> {
        self.free_bytes
    }
}

/// Monitor settings for one host.
#[derive(Clone)]
pub struct MonitorConfig {
    /// Host name recorded on journal entries.
    pub host: String,
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
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            writer: WriterConfig::default(),
            retention: RetentionPolicy::KeepAll,
            clock: system_clock(),
        }
    }
}

struct Inner {
    store: Arc<dyn JournalStore>,
    checks: Arc<dyn HostChecks>,
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
    /// Start the journal writer and the registry for `store`; health uses the host-injected
    /// `checks`.
    pub fn start(
        store: Arc<dyn JournalStore>,
        checks: Arc<dyn HostChecks>,
        config: MonitorConfig,
        events: EventBus,
    ) -> Self {
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
                checks,
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

    /// The journal store for a host component that appends to it directly
    /// (the catalog outbox, DE-0.10): every accepted append is announced as
    /// `journal.entry` events and counted in this monitor's metrics, like
    /// the writer's entries.
    pub fn announcing_store(&self) -> Arc<dyn JournalStore> {
        Arc::new(AnnouncingJournal::new(
            self.inner.store.clone(),
            self.inner.events.clone(),
            self.inner.metrics.clone(),
        ))
    }

    /// Host name.
    pub fn host(&self) -> &str {
        &self.inner.config.host
    }

    /// Gather host checks and evaluate health. Calls the injected store (ping, an error count)
    /// and host checks, whose host implementations may do I/O.
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
            data_dir_writable: self.inner.checks.data_dir_writable(),
            free_bytes: self.inner.checks.free_bytes(),
            queue_fill: self.journal().queue_fill(),
            errors_last_5m: self.inner.store.count(&errors).unwrap_or(0),
        }
    }
}
