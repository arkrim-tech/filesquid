//! The PROV-O history of catalog commits (Task DE-0.10; ADR 040 Decisions 3, 4 and 6).
//!
//! Every snapshot version a catalog commit publishes carries the installation as its OCFL
//! version `user`, a `filesquid.operation.v1` message naming the operation and its kind, and,
//! when the host configured a [`ProvenanceContext`], the operation's PROV-O record and journal
//! entries (`provenance/<version>.jsonld` and `.journal.jsonl`) written in that same version.
//! The journal entry IDs are allocated from the host journal before the commit, so the
//! entries the outbox forwards afterwards and the ones a reconcile replays from the version
//! are stored once. Everything here is pure apart from that allocation.

use filesquid_api::canonical::{operation_kind, operation_message, OperationRecord};
use filesquid_api::history::{
    version_user, CommitOperation, CommitRecorder, ProvenanceContext, PROV_RECORD_OPERATION,
};
use filesquid_api::journal::commit::{ocfl_commit_entries, CommitFacts};
use filesquid_api::journal::{JournalEntry, JournalStore, Level, Outcome};
use filesquid_core::{OperationId, VersionRef};
use filesquid_ocfl::change::VersionMeta;
use filesquid_ocfl::write::{rfc3339_utc, CommitReceipt, VersionBoundFiles};

/// Journal IDs one recorded commit allocates: its `ocfl.commit` entry and the operation's.
const RECORDED_IDS: usize = CommitRecorder::JOURNAL_IDS + 1;

/// Who records the catalog's commits and where their journal IDs come from.
pub(crate) struct History<'a> {
    /// The installation and host of every record.
    pub context: &'a ProvenanceContext,
    /// The host journal that allocates the entry IDs.
    pub journal: &'a dyn JournalStore,
}

/// How one catalog commit is recorded: its version metadata, its recorder (when the host
/// configured provenance and IDs could be allocated) and the operation's journal entry.
pub(crate) struct CommitPlan {
    meta: VersionMeta,
    recorder: Option<CommitRecorder>,
    entry: JournalEntry,
    operation_id: Option<OperationId>,
    /// Entries that report why the commit carries no record (journaled with it).
    warnings: Vec<JournalEntry>,
}

/// The plan of the commit that publishes `record`, journaled as `entry`. Allocates the
/// commit's journal IDs from `history`'s journal (journal-store I/O); without a history, or
/// when the IDs cannot be allocated, the version still names the operation and (with a
/// history) the installation, and the plan carries a `prov.record` warning instead.
pub(crate) fn plan_commit(
    history: Option<&History<'_>>,
    record: &OperationRecord,
    entry: &JournalEntry,
) -> CommitPlan {
    let operation_id = OperationId::new(record.operation_id.clone()).ok();
    let mut meta = VersionMeta::new(started(entry)).with_message(operation_message(record));
    if let Some(history) = history {
        meta = meta.with_user(version_user(&history.context.installation));
    }
    let mut plan = CommitPlan {
        meta,
        recorder: None,
        entry: entry.clone(),
        operation_id: operation_id.clone(),
        warnings: Vec::new(),
    };
    let (Some(history), Some(operation_id)) = (history, operation_id) else {
        return plan;
    };
    match history.journal.allocate_ids(RECORDED_IDS) {
        Ok(ids) if ids.len() == RECORDED_IDS => {
            plan.recorder = Some(recorder(history, entry, operation_id, &ids));
        }
        _ => plan
            .warnings
            .push(unrecorded(entry, "journal_ids_unavailable")),
    }
    plan
}

/// The recorder of a commit, its `ocfl.commit` entry under `ids[0]` and the operation's
/// entry under `ids[1]`. Pure.
fn recorder(
    history: &History<'_>,
    entry: &JournalEntry,
    operation_id: OperationId,
    ids: &[i64],
) -> CommitRecorder {
    let kind = operation_kind(&entry.operation);
    let operation =
        CommitOperation::new(operation_id, kind, started(entry)).with_actor(entry.actor.clone());
    let mut own = entry.clone();
    own.id = Some(ids[1]);
    CommitRecorder::new(history.context.clone(), operation, ids[0]).with_entries(vec![own])
}

/// When the operation started: its journal entry's time, RFC 3339 UTC. Pure.
fn started(entry: &JournalEntry) -> String {
    rfc3339_utc(u64::try_from(entry.time_ms).unwrap_or(0))
}

/// A `prov.record` warning that the commit of `entry` carries no record, for `code`. Pure.
fn unrecorded(entry: &JournalEntry, code: &str) -> JournalEntry {
    JournalEntry::new(PROV_RECORD_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .error_code(code)
        .actor(entry.actor.clone())
        .at_ms(entry.time_ms)
}

impl CommitPlan {
    /// The version metadata: installation `user` and operation message.
    pub(crate) fn meta(&self) -> VersionMeta {
        self.meta.clone()
    }

    /// The version-bound files function, when the commit is recorded.
    pub(crate) fn bound(&self) -> Option<&dyn VersionBoundFiles> {
        self.recorder.as_ref().map(|r| r as &dyn VersionBoundFiles)
    }

    /// The journal entries of the published commit `receipt`, in outbox order: with a
    /// recorder, exactly the entries the version carries (same IDs); otherwise the
    /// `ocfl.commit` entry and the operation's entry linked to the version, then any warning.
    /// Pure.
    pub(crate) fn published(&self, receipt: &CommitReceipt) -> Vec<JournalEntry> {
        if let Some(recorder) = &self.recorder {
            return recorder.journal_entries(receipt);
        }
        let mut entries = self.linked(receipt);
        entries.extend(self.warnings.iter().cloned());
        entries
    }

    /// The journal entries of a commit that published no version: the operation's entry
    /// with its operation ID. Pure.
    pub(crate) fn unpublished(&self) -> Vec<JournalEntry> {
        let entry = self.entry.clone();
        vec![match &self.operation_id {
            Some(id) => entry.operation_id(id.as_str()),
            None => entry,
        }]
    }

    /// `ocfl.commit` and the operation's entry linked to the version of `receipt`. Pure.
    fn linked(&self, receipt: &CommitReceipt) -> Vec<JournalEntry> {
        let reference = VersionRef::new(receipt.object_id.clone(), receipt.version.to_string());
        let (Some(id), Ok(reference)) = (&self.operation_id, reference) else {
            return self.unpublished();
        };
        let facts = CommitFacts {
            operation_id: id,
            version: &reference,
            took_over: receipt.took_over,
        };
        let stamp = |e: JournalEntry| e.actor(self.entry.actor.clone()).at_ms(self.entry.time_ms);
        let mut entries: Vec<JournalEntry> =
            ocfl_commit_entries(&facts).into_iter().map(stamp).collect();
        entries.push(
            self.entry
                .clone()
                .operation_id(id.as_str())
                .object_ref(reference.object_ref()),
        );
        entries
    }
}
