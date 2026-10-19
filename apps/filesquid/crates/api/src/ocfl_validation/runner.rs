//! The runs of a [`ValidationService`] and its takeover check (API-S-022).
//!
//! A run registers in the process registry as `ocfl.validation`, with progress and
//! heartbeat. Its outcome (with the objects it checked, also when it stopped on an outage) is
//! recorded as the library's last run and journaled as `ocfl.validation.run`. Any change in
//! the open findings publishes the health report on `health.change`.

use super::error::ValidationError;
use super::run::{check_object, index, pass, read_error, Indexed, PassResult, RunContext};
use super::service::{RunLock, ValidationService};
use super::store::{RunOutcome, RunRecord, RunTrigger};
use super::{LOG_TARGET, RUN_OPERATION};
use crate::journal::{JournalEntry, Outcome};
use crate::registry::{ProcessHandle, ProcessSpec};
use filesquid_core::LibraryId;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use std::sync::atomic::AtomicBool;

/// Longest silence of a running validation before it counts as stalled: one object can take
/// long to check when its content is large.
const HEARTBEAT_TIMEOUT_MS: i64 = 30 * 60 * 1000;

impl<B: StorageBackend + 'static> ValidationService<B> {
    /// Validate `library_id` now for `actor`: one pass from the cursor (or from the start),
    /// up to the configured cap. A pass that stops on a storage or store outage is recorded
    /// and answered as a run with outcome `failed`, with the objects it had checked.
    ///
    /// # Errors
    ///
    /// `not_found`, `already_running`, or `unavailable` after shutdown; each is journaled and
    /// nothing is checked.
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
            objects: &lock.objects,
        };
        let cap = self.config().object_cap;
        let passed = match index(lock.backend.as_ref()).await {
            Ok(objects) => pass(&ctx, lock.backend.as_ref(), &objects, from_start, cap).await,
            Err(err) => PassResult::failed(err),
        };
        drop(lock);
        let record = self.record(&library_id, trigger, started_ms, &passed);
        self.finish(&process, actor, &record, passed.changed);
        record
    }

    /// The record of a pass.
    fn record(
        &self,
        library_id: &LibraryId,
        trigger: RunTrigger,
        started_ms: i64,
        passed: &PassResult,
    ) -> RunRecord {
        RunRecord {
            library_id: library_id.clone(),
            trigger,
            started_ms,
            ended_ms: (self.inner.clock)(),
            outcome: passed.outcome,
            checked: passed.checked,
            not_checked: passed.total.saturating_sub(passed.checked),
            error_code: passed.error.as_ref().map(|e| e.error_code().to_string()),
        }
    }

    /// Store and journal the record, move the schedule, end the process, and publish health
    /// when findings changed.
    fn finish(&self, process: &ProcessHandle, actor: &str, record: &RunRecord, changed: bool) {
        if let Err(err) = self.inner.store.record_run(record) {
            tracing::warn!(target: LOG_TARGET, error = %err, "validation run not recorded");
        }
        let due = record.started_ms.saturating_add(self.config().interval_ms);
        self.remember_next_due(&record.library_id, due);
        let code = record.error_code.as_deref().unwrap_or("internal");
        let entry = match record.outcome {
            RunOutcome::Completed | RunOutcome::Capped => {
                JournalEntry::new(RUN_OPERATION, Outcome::Succeeded)
            }
            RunOutcome::Cancelled => JournalEntry::new(RUN_OPERATION, Outcome::Cancelled),
            RunOutcome::Failed => {
                JournalEntry::new(RUN_OPERATION, Outcome::Failed).error_code(code)
            }
        };
        let duration = u64::try_from(record.ended_ms - record.started_ms).unwrap_or(0);
        self.journal(
            entry
                .actor(actor)
                .target(record.library_id.as_str())
                .process(process.id())
                .duration_ms(duration),
        );
        match record.outcome {
            RunOutcome::Failed => process.fail(code),
            RunOutcome::Cancelled => process.cancel(),
            RunOutcome::Completed | RunOutcome::Capped => process.complete(),
        }
        if changed {
            self.inner.monitor.publish_health();
        }
    }

    /// Validate the object `object_id` at `object_root` of `library_id` after a commit on
    /// it, when that commit took over an expired or lost reservation (API-S-022). Any other
    /// commit is not validated here. This is not a run: it takes no run lock and does not
    /// move the cursor, but it holds the library's object lock, so it never interleaves with
    /// a run on the same object. Only that object is read. An object at that root with
    /// another identifier is left alone.
    ///
    /// # Errors
    ///
    /// `not_found` when the library is not open; `unavailable` (or `internal`) for a storage
    /// or store failure.
    pub async fn validate_after_commit(
        &self,
        library_id: &LibraryId,
        object_root: &str,
        object_id: &str,
        took_over: bool,
    ) -> Result<(), ValidationError> {
        if !took_over {
            return Ok(());
        }
        let access = self.access(library_id)?;
        let object = Object::open(access.backend.as_ref(), object_root)
            .await
            .map_err(|e| read_error(&e))?;
        if object.id() != object_id {
            tracing::info!(target: LOG_TARGET, "takeover check skipped: another object at the root");
            return Ok(());
        }
        let indexed = Indexed::of(&object, object_root);
        let journal = self.inner.monitor.journal();
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            store: self.inner.store.as_ref(),
            journal: &journal,
            library_id,
            actor: "system",
            cancel: &cancel,
            process: None,
            objects: &access.objects,
        };
        if !check_object(&ctx, access.backend.as_ref(), &indexed)
            .await?
            .is_empty()
        {
            self.inner.monitor.publish_health();
        }
        Ok(())
    }
}
