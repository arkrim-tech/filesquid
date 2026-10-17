//! The process registry: every long-running FileSquid process (job, worker, import, upload,
//! migration, C2PA scan, sync session, host task) registers here with state, progress and a
//! heartbeat. A missed heartbeat marks it `stalled` (FileSquid T-057). Every state change is
//! journaled and pushed on the event channel as `process.update`.

use crate::events::topic;
use crate::journal::{format_rfc3339_ms, now_ms, JournalEntry, Outcome};
use crate::journal_writer::JournalSender;
use crate::metrics::Metrics;
use crate::{ApiEvent, EventBus};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Source of "now" in Unix milliseconds; injectable so tests control time.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Finished processes kept for display before the oldest are pruned.
const KEEP_FINISHED: usize = 200;

/// The system clock.
pub fn system_clock() -> Clock {
    Arc::new(now_ms)
}

/// Lifecycle state of a process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
    /// Working and reporting heartbeats.
    Running,
    /// Missed its heartbeat deadline.
    Stalled,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Ended before finishing.
    Cancelled,
}

impl ProcessState {
    fn finished(self) -> bool {
        matches!(
            self,
            ProcessState::Completed | ProcessState::Failed | ProcessState::Cancelled
        )
    }
}

/// What a process declares when it registers.
#[derive(Clone, Debug)]
pub struct ProcessSpec {
    /// Process type, for example `import`, `c2pa.scan` or `host`.
    pub kind: String,
    /// Longest silence before the process counts as stalled.
    pub heartbeat_timeout_ms: i64,
}

impl ProcessSpec {
    /// Describe a process of `kind` that must report at least every `heartbeat_timeout_ms`.
    pub fn new(kind: impl Into<String>, heartbeat_timeout_ms: i64) -> Self {
        Self {
            kind: kind.into(),
            heartbeat_timeout_ms,
        }
    }
}

/// Read-only view of a process for the API and the Monitor dashboard.
#[derive(Clone, Debug, Serialize)]
pub struct ProcessView {
    /// Opaque process identifier.
    pub id: String,
    /// Process type.
    pub kind: String,
    /// Current state.
    pub state: ProcessState,
    /// Units done.
    pub done: u64,
    /// Units in total, when known.
    pub total: Option<u64>,
    /// Start time (RFC 3339 UTC).
    pub started: String,
    /// Last heartbeat (RFC 3339 UTC).
    pub last_heartbeat: String,
    /// Heartbeat timeout in milliseconds.
    pub heartbeat_timeout_ms: i64,
    /// Units per second since start, when measurable.
    pub rate_per_second: Option<f64>,
    /// Estimated seconds to finish, when measurable.
    pub eta_seconds: Option<u64>,
    /// Error code for failed processes.
    pub error_code: Option<String>,
}

struct Record {
    id: String,
    kind: String,
    state: ProcessState,
    done: u64,
    total: Option<u64>,
    started_ms: i64,
    heartbeat_ms: i64,
    timeout_ms: i64,
    error_code: Option<String>,
}

impl Record {
    fn view(&self, now: i64) -> ProcessView {
        let elapsed_s = (now - self.started_ms) as f64 / 1000.0;
        let rate = (elapsed_s > 0.0 && self.done > 0).then(|| self.done as f64 / elapsed_s);
        let eta = match (rate, self.total) {
            (Some(r), Some(t)) if r > 0.0 && t >= self.done => {
                Some(((t - self.done) as f64 / r).round() as u64)
            }
            _ => None,
        };
        ProcessView {
            id: self.id.clone(),
            kind: self.kind.clone(),
            state: self.state,
            done: self.done,
            total: self.total,
            started: format_rfc3339_ms(self.started_ms),
            last_heartbeat: format_rfc3339_ms(self.heartbeat_ms),
            heartbeat_timeout_ms: self.timeout_ms,
            rate_per_second: rate,
            eta_seconds: eta,
            error_code: self.error_code.clone(),
        }
    }
}

struct Inner {
    records: Mutex<Vec<Record>>,
    next: AtomicU64,
    journal: JournalSender,
    events: EventBus,
    metrics: Arc<Metrics>,
    clock: Clock,
}

/// Shared registry of live and recently finished processes.
#[derive(Clone)]
pub struct ProcessRegistry {
    inner: Arc<Inner>,
}

impl ProcessRegistry {
    /// Create a registry that journals to `journal` and publishes on `events`.
    pub fn new(
        journal: JournalSender,
        events: EventBus,
        metrics: Arc<Metrics>,
        clock: Clock,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                records: Mutex::new(Vec::new()),
                next: AtomicU64::new(1),
                journal,
                events,
                metrics,
                clock,
            }),
        }
    }

    /// Register a running process and return its handle.
    pub fn register(&self, spec: ProcessSpec) -> ProcessHandle {
        let now = (self.inner.clock)();
        let id = format!(
            "{}-{}",
            spec.kind,
            self.inner.next.fetch_add(1, Ordering::SeqCst)
        );
        let record = Record {
            id: id.clone(),
            kind: spec.kind.clone(),
            state: ProcessState::Running,
            done: 0,
            total: None,
            started_ms: now,
            heartbeat_ms: now,
            timeout_ms: spec.heartbeat_timeout_ms,
            error_code: None,
        };
        let view = record.view(now);
        self.with_records(|records| {
            records.push(record);
            prune(records);
            self.publish(records, view);
        });
        self.inner.metrics.process_started();
        self.journal(&id, "process.started", Outcome::Started, None, now);
        ProcessHandle {
            registry: self.clone(),
            id,
            finished: AtomicBool::new(false),
        }
    }

    /// Views of all live and recently finished processes, in registration order.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        let now = (self.inner.clock)();
        self.with_records(|records| records.iter().map(|r| r.view(now)).collect())
    }

    /// Mark processes whose heartbeat is overdue as stalled. Each is journaled once.
    /// Returns the ids that became stalled in this call.
    pub fn check_stalled(&self) -> Vec<String> {
        let now = (self.inner.clock)();
        let newly: Vec<ProcessView> = self.with_records(|records| {
            let newly: Vec<ProcessView> = records
                .iter_mut()
                .filter(|r| r.state == ProcessState::Running && now - r.heartbeat_ms > r.timeout_ms)
                .map(|r| {
                    r.state = ProcessState::Stalled;
                    r.view(now)
                })
                .collect();
            newly
                .iter()
                .for_each(|view| self.publish(records, view.clone()));
            set_gauges(&self.inner.metrics, records);
            newly
        });
        for view in &newly {
            self.journal(
                &view.id,
                "process.stalled",
                Outcome::Stalled,
                Some("heartbeat_timeout"),
                now,
            );
        }
        newly.into_iter().map(|v| v.id).collect()
    }

    fn with_records<T>(&self, f: impl FnOnce(&mut Vec<Record>) -> T) -> T {
        let mut guard = self.inner.records.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    /// Apply `change` to one record that exists and is not finished, then prune and publish
    /// its new view while still holding the lock, so events leave in mutation order.
    /// Returns whether the record was stalled before the change, or `None` when not found.
    fn update(&self, id: &str, change: impl FnOnce(&mut Record)) -> Option<bool> {
        let now = (self.inner.clock)();
        self.with_records(|records| {
            let record = records
                .iter_mut()
                .find(|r| r.id == id && !r.state.finished())?;
            let was_stalled = record.state == ProcessState::Stalled;
            record.heartbeat_ms = now;
            change(record);
            let view = record.view(now);
            prune(records);
            self.publish(records, view);
            Some(was_stalled)
        })
    }

    fn heartbeat_update(&self, id: &str, change: impl FnOnce(&mut Record)) {
        let Some(was_stalled) = self.update(id, |r| {
            if r.state == ProcessState::Stalled {
                r.state = ProcessState::Running;
            }
            change(r);
        }) else {
            return;
        };
        if was_stalled {
            self.journal(
                id,
                "process.resumed",
                Outcome::Resumed,
                None,
                (self.inner.clock)(),
            );
        }
    }

    fn finish(&self, id: &str, state: ProcessState, error_code: Option<&str>) {
        if self
            .update(id, |r| {
                r.state = state;
                r.error_code = error_code.map(str::to_string);
            })
            .is_none()
        {
            return;
        }
        let (operation, outcome) = match state {
            ProcessState::Completed => {
                self.inner.metrics.process_completed();
                ("process.completed", Outcome::Succeeded)
            }
            ProcessState::Failed => {
                self.inner.metrics.process_failed();
                ("process.failed", Outcome::Failed)
            }
            _ => {
                self.inner.metrics.process_cancelled();
                ("process.cancelled", Outcome::Cancelled)
            }
        };
        self.journal(id, operation, outcome, error_code, (self.inner.clock)());
    }

    fn journal(
        &self,
        id: &str,
        operation: &str,
        outcome: Outcome,
        error_code: Option<&str>,
        at: i64,
    ) {
        let mut entry = JournalEntry::new(operation, outcome).process(id).at_ms(at);
        entry.error_code = error_code.map(str::to_string);
        self.inner.journal.send(entry);
    }

    /// Push `view` as `process.update` and refresh the gauges from `records`. Callers hold
    /// the records lock, so a later mutation can never publish before an earlier one.
    fn publish(&self, records: &[Record], view: ProcessView) {
        if let Ok(data) = serde_json::to_value(view) {
            self.inner
                .events
                .publish(ApiEvent::new(topic::PROCESS_UPDATE, data));
        }
        set_gauges(&self.inner.metrics, records);
    }
}

/// Store the running and stalled counts of `records`; called under the records lock so an
/// older count can never overwrite a newer one.
fn set_gauges(metrics: &Metrics, records: &[Record]) {
    let count = |s| records.iter().filter(|r| r.state == s).count() as u64;
    metrics.set_process_gauges(count(ProcessState::Running), count(ProcessState::Stalled));
}

/// Keep at most [`KEEP_FINISHED`] finished records (oldest removed first).
fn prune(records: &mut Vec<Record>) {
    let finished = records.iter().filter(|r| r.state.finished()).count();
    let mut excess = finished.saturating_sub(KEEP_FINISHED);
    records.retain(|r| {
        if excess > 0 && r.state.finished() {
            excess -= 1;
            return false;
        }
        true
    });
}

/// Handle a process uses to report progress and its end. Dropping an unfinished handle
/// cancels the process.
pub struct ProcessHandle {
    registry: ProcessRegistry,
    id: String,
    finished: AtomicBool,
}

impl ProcessHandle {
    /// The process identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Report progress (also counts as a heartbeat).
    pub fn progress(&self, done: u64, total: u64) {
        self.registry.heartbeat_update(&self.id, |r| {
            r.done = done;
            r.total = Some(total);
        });
    }

    /// Report that the process is alive.
    pub fn heartbeat(&self) {
        self.registry.heartbeat_update(&self.id, |_| {});
    }

    /// Finish successfully.
    pub fn complete(&self) {
        self.end(ProcessState::Completed, None);
    }

    /// Finish with an error code.
    pub fn fail(&self, error_code: &str) {
        self.end(ProcessState::Failed, Some(error_code));
    }

    /// End before finishing.
    pub fn cancel(&self) {
        self.end(ProcessState::Cancelled, None);
    }

    fn end(&self, state: ProcessState, error_code: Option<&str>) {
        if !self.finished.swap(true, Ordering::SeqCst) {
            self.registry.finish(&self.id, state, error_code);
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}
