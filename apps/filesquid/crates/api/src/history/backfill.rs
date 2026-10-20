//! The history backfill job (API-S-029; ADR 040 Decision 7).
//!
//! Versions committed before the history existed have no PROV-O record. A [`Backfill`] run
//! walks every object of a storage root and, for each object with versions that lack one,
//! stages the records `filesquid-prov` reconstructs from the inventory (marked
//! `reconstructed`, inventing no agent) and commits them as one provenance-only version that
//! carries the run's own record (kind `backfill`) and journal entry. The run:
//!
//! - is **bounded**: at most [`Backfill::with_max_versions`] records per run (default
//!   [`DEFAULT_MAX_VERSIONS`]), and the object walk ends with the storage root's listing;
//! - is **resumable and idempotent**: what is recorded is read back from OCFL, so a version
//!   that has a record is skipped and a second run commits nothing;
//! - is **cancellable** between objects through a flag the caller holds;
//! - registers in the [`ProcessRegistry`] with progress per object and a heartbeat between
//!   staging an object's records and committing them, and journals its start, end, every
//!   failure (`prov.backfill`) and every commit (`ocfl.commit`).
//!
//! A failure on one object is journaled and reported, and the run goes on with the next.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use filesquid_core::{OperationId, OperationKind};
use filesquid_ocfl::change::{ChangeModel, StagingArea};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{rfc3339_utc, VersionReservations};
use filesquid_prov::ocfl::{missing_records, stage_reconstructed};
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use super::recorder::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use crate::journal::{now_ms, JournalEntry, JournalStore, Level, Outcome};
use crate::journal_writer::JournalSender;
use crate::library::mint_uuid_v4;
use crate::registry::{Clock, ProcessHandle, ProcessRegistry, ProcessSpec};

/// Journal operation and process kind of a backfill run.
pub const BACKFILL_OPERATION: &str = "prov.backfill";

/// Records one run reconstructs unless configured otherwise.
pub const DEFAULT_MAX_VERSIONS: usize = 1_000;

/// Longest silence before the registry counts a run as stalled.
pub const HEARTBEAT_TIMEOUT_MS: i64 = 60_000;

/// `tracing` target of this module's stdout-only diagnostics.
const LOG_TARGET: &str = "filesquid::history";

/// One object the run committed records for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfilledObject {
    /// The object ID.
    pub object_id: String,
    /// The provenance-only version the run committed.
    pub version: String,
    /// The versions whose reconstructed records it holds, oldest first.
    pub reconstructed: Vec<String>,
}

/// One object the run could not complete; nothing was published for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillFailure {
    /// The object root's object ID, when the object could be opened.
    pub object_id: Option<String>,
    /// Why: `object_unreadable`, `journal_ids_unavailable`, `stage_failed` or
    /// `commit_failed`.
    pub error_code: String,
}

/// What one run did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillReport {
    /// The run ID: the operation ID of every commit and journal entry of the run.
    pub run_id: String,
    /// The process ID the registry gave the run.
    pub process_id: String,
    /// Objects the run committed records for.
    pub objects: Vec<BackfilledObject>,
    /// Objects the run could not complete.
    pub failures: Vec<BackfillFailure>,
    /// True when the run stopped at its cap: running again continues.
    pub reached_cap: bool,
    /// True when the caller cancelled the run.
    pub cancelled: bool,
}

/// Why a run could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackfillError {
    /// No run ID could be minted, or the storage root cannot be opened.
    Unavailable,
}

impl std::fmt::Display for BackfillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the history backfill cannot run now")
    }
}

impl std::error::Error for BackfillError {}

/// The backfill job of one host.
#[derive(Clone)]
pub struct Backfill {
    context: ProvenanceContext,
    ids: Arc<dyn JournalStore>,
    journal: JournalSender,
    registry: ProcessRegistry,
    clock: Clock,
    max_versions: usize,
}

impl std::fmt::Debug for Backfill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backfill")
            .field("max_versions", &self.max_versions)
            .finish_non_exhaustive()
    }
}

/// What one run carries from object to object.
struct Run<'r> {
    run_id: String,
    holder: String,
    cancel: &'r AtomicBool,
    handle: &'r ProcessHandle,
    budget: usize,
    report: BackfillReport,
}

impl Backfill {
    /// A job that commits as `context`, allocates journal IDs from `ids`, journals through
    /// `journal` and registers its runs in `registry`.
    pub fn new(
        context: ProvenanceContext,
        ids: Arc<dyn JournalStore>,
        journal: JournalSender,
        registry: ProcessRegistry,
    ) -> Self {
        Self {
            context,
            ids,
            journal,
            registry,
            clock: Arc::new(now_ms),
            max_versions: DEFAULT_MAX_VERSIONS,
        }
    }

    /// Use `clock` (Unix milliseconds, UTC) for commit and journal times.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Reconstruct at most `max` records per run (at least one).
    #[must_use]
    pub fn with_max_versions(mut self, max: usize) -> Self {
        self.max_versions = max.max(1);
        self
    }

    /// Run once over the storage root on `root`, writing under `reservations`, until every
    /// object is done, the cap is reached or `cancel` is set (storage I/O).
    ///
    /// # Errors
    ///
    /// [`BackfillError::Unavailable`] when no run ID can be minted or the storage root
    /// cannot be opened; the latter is journaled and fails the registered process.
    pub async fn run<B: StorageBackend, R: VersionReservations>(
        &self,
        root: &B,
        reservations: &R,
        cancel: &AtomicBool,
    ) -> Result<BackfillReport, BackfillError> {
        let run_id = mint_uuid_v4().map_err(|_| BackfillError::Unavailable)?;
        let handle = self
            .registry
            .register(ProcessSpec::new(BACKFILL_OPERATION, HEARTBEAT_TIMEOUT_MS));
        self.journal_run(&run_id, &handle, Outcome::Started, None);
        let Ok(storage) = StorageRoot::open(root).await else {
            self.journal_run(
                &run_id,
                &handle,
                Outcome::Failed,
                Some("storage_unavailable"),
            );
            handle.fail("storage_unavailable");
            return Err(BackfillError::Unavailable);
        };
        let mut run = Run {
            holder: format!("filesquid-backfill-{run_id}"),
            report: BackfillReport::empty(&run_id, handle.id()),
            run_id,
            cancel,
            handle: &handle,
            budget: self.max_versions,
        };
        self.walk(root, reservations, &storage, &mut run).await;
        self.finish(&run, &handle);
        Ok(run.report)
    }

    /// Visit the objects in storage-root order until done, capped or cancelled.
    async fn walk<B: StorageBackend, R: VersionReservations>(
        &self,
        root: &B,
        reservations: &R,
        storage: &StorageRoot<'_, B>,
        run: &mut Run<'_>,
    ) {
        let roots = storage.objects();
        let total = roots.len() as u64;
        for (done, object_root) in roots.iter().enumerate() {
            if run.cancel.load(Ordering::SeqCst) {
                run.report.cancelled = true;
                return;
            }
            if run.budget == 0 {
                run.report.reached_cap = true;
                return;
            }
            match storage.open_object(object_root).await {
                Ok(object) => {
                    self.object(root, reservations, object_root, &object, run)
                        .await
                }
                Err(err) => {
                    tracing::warn!(target: LOG_TARGET, error = %err, "backfill skipped an object");
                    self.fail(run, None, "object_unreadable");
                }
            }
            run.handle.progress(done as u64 + 1, total);
        }
    }

    /// Reconstruct and commit the missing records of one object, within the run's budget;
    /// report the object, or journal and report why it failed. The run reaches its cap only
    /// when a commit used up the budget with records of this object still missing.
    async fn object<B: StorageBackend, R: VersionReservations, O: StorageBackend>(
        &self,
        root: &B,
        reservations: &R,
        object_root: &str,
        object: &Object<'_, O>,
        run: &mut Run<'_>,
    ) {
        let missing = missing_records(object).len();
        if missing == 0 {
            return;
        }
        let target = (root, reservations, object_root);
        match self.commit_object(target, object, run).await {
            Ok(done) => {
                run.budget = run.budget.saturating_sub(done.reconstructed.len());
                run.report.reached_cap |= missing > done.reconstructed.len();
                run.report.objects.push(done);
            }
            Err(code) => self.fail(run, Some(object.id().to_string()), code),
        }
    }

    /// Stage the reconstructed records of `object` (at most the run's budget), heartbeat,
    /// and commit them with the run's own record; journal the commit (storage I/O).
    ///
    /// # Errors
    ///
    /// `journal_ids_unavailable`, `stage_failed` or `commit_failed`; nothing is published.
    async fn commit_object<B: StorageBackend, R: VersionReservations, O: StorageBackend>(
        &self,
        (root, reservations, object_root): (&B, &R, &str),
        object: &Object<'_, O>,
        run: &Run<'_>,
    ) -> Result<BackfilledObject, &'static str> {
        let now = rfc3339_utc(u64::try_from((self.clock)()).unwrap_or_default());
        let operation_id = OperationId::new(run.run_id.as_str()).map_err(|_| "commit_failed")?;
        let operation = CommitOperation::new(operation_id, OperationKind::Backfill, now.as_str());
        let ids = self
            .ids
            .allocate_ids(CommitRecorder::JOURNAL_IDS)
            .map_err(|_| "journal_ids_unavailable")?;
        let staging = MemoryBackend::new();
        let area = StagingArea::new(&staging, "changes/backfill").map_err(|_| "stage_failed")?;
        let model = ChangeModel::new(
            root,
            reservations,
            &run.holder,
            object_root,
            object.id(),
            area,
        );
        let staged = stage_reconstructed(&model, object, &now, run.budget)
            .await
            .map_err(|err| {
                tracing::warn!(target: LOG_TARGET, error = %err, "backfill could not stage");
                "stage_failed"
            })?;
        run.handle.heartbeat();
        let recorder = CommitRecorder::new(self.context.clone(), operation.clone(), ids[0]);
        let meta = commit_meta(&self.context, &operation, now);
        let span = crate::trace_layer::activity_span(operation.operation_id.as_str(), None);
        let receipt = model
            .commit_with(meta, Some(&recorder))
            .instrument(span)
            .await
            .map_err(|err| {
                tracing::warn!(target: LOG_TARGET, error = %err, "backfill could not commit");
                "commit_failed"
            })?;
        recorder
            .journal_entries(&receipt)
            .into_iter()
            .for_each(|entry| self.journal.send(entry));
        Ok(BackfilledObject {
            object_id: object.id().to_string(),
            version: receipt.version.to_string(),
            reconstructed: staged.iter().map(ToString::to_string).collect(),
        })
    }

    /// Report and journal a failure on one object.
    fn fail(&self, run: &mut Run<'_>, object_id: Option<String>, code: &'static str) {
        let entry = JournalEntry::new(BACKFILL_OPERATION, Outcome::Failed)
            .level(Level::Warn)
            .error_code(code)
            .operation_id(&run.run_id)
            .process(&run.report.process_id)
            .at_ms((self.clock)());
        let entry = match &object_id {
            Some(id) => entry.target(id.as_str()),
            None => entry,
        };
        self.journal.send(entry);
        run.report.failures.push(BackfillFailure {
            object_id,
            error_code: code.to_string(),
        });
    }

    /// End the registered process and journal how the run ended.
    fn finish(&self, run: &Run<'_>, handle: &ProcessHandle) {
        if run.report.cancelled {
            self.journal_run(&run.run_id, handle, Outcome::Cancelled, None);
            handle.cancel();
            return;
        }
        self.journal_run(&run.run_id, handle, Outcome::Succeeded, None);
        handle.complete();
    }

    /// Journal the run's start or end.
    fn journal_run(
        &self,
        run_id: &str,
        handle: &ProcessHandle,
        outcome: Outcome,
        error_code: Option<&str>,
    ) {
        let entry = JournalEntry::new(BACKFILL_OPERATION, outcome)
            .operation_id(run_id)
            .process(handle.id())
            .at_ms((self.clock)());
        self.journal.send(match error_code {
            Some(code) => entry.error_code(code),
            None => entry,
        });
    }
}

impl BackfillReport {
    /// The report of run `run_id`, registered as `process_id`, before any object.
    fn empty(run_id: &str, process_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            process_id: process_id.to_string(),
            objects: Vec::new(),
            failures: Vec::new(),
            reached_cap: false,
            cancelled: false,
        }
    }
}
