//! The provenance follow-up of a repair (API-S-028; ADR 040 Decision 6).
//!
//! The engine repair adds a version inventory to an existing version directory and makes
//! no version, so it carries no PROV-O record. After an object is repaired the service
//! commits one provenance-only version (`ChangeModel::commit_bound_only`) whose record is of
//! kind `repair` and whose operation ID is the repair run's ID, the same ID the consent and
//! outcome entries carry. The commit stages nothing, so its staging area is in memory.

use std::sync::Arc;

use filesquid_core::{OperationId, OperationKind};
use filesquid_ocfl::change::{ChangeModel, StagingArea};
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{rfc3339_utc, VersionReservations};
use tracing::Instrument;

use super::LOG_TARGET;
use crate::history::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use crate::journal::{JournalEntry, JournalStore};

/// Why a repair's provenance record was not written: no context was configured.
pub const PROVENANCE_NOT_CONFIGURED: &str = "provenance_not_configured";

/// Why a repair's provenance record was not written: no journal ID could be allocated.
pub const JOURNAL_IDS_UNAVAILABLE: &str = "journal_ids_unavailable";

/// Why a repair's provenance record was not written: the commit was refused.
pub const PROVENANCE_COMMIT_FAILED: &str = "provenance_commit_failed";

/// What a service records provenance with: who commits, and the journal store that hands
/// out the commit's entry IDs before it.
#[derive(Clone)]
pub(super) struct Provenance {
    pub(super) context: ProvenanceContext,
    pub(super) ids: Arc<dyn JournalStore>,
}

/// One repaired object whose record is committed.
pub(super) struct Repaired<'a> {
    /// The object's root in the storage root.
    pub(super) object_root: &'a str,
    /// The object ID.
    pub(super) object_id: &'a str,
    /// The repair run ID: the operation ID of the record.
    pub(super) run_id: &'a str,
    /// The journal actor.
    pub(super) actor: &'a str,
    /// The reservation holder of the run.
    pub(super) holder: &'a str,
    /// Now, in Unix milliseconds (UTC).
    pub(super) now_ms: i64,
}

/// Commit the provenance-only version that records the repair of `repaired` (storage I/O),
/// and return its version name with the journal entries to append.
///
/// # Errors
///
/// [`JOURNAL_IDS_UNAVAILABLE`] or [`PROVENANCE_COMMIT_FAILED`]; nothing is published.
pub(super) async fn commit_repair_record<B: StorageBackend, R: VersionReservations>(
    provenance: &Provenance,
    root: &B,
    reservations: &R,
    repaired: &Repaired<'_>,
) -> Result<(String, Vec<JournalEntry>), &'static str> {
    let ids = provenance
        .ids
        .allocate_ids(CommitRecorder::JOURNAL_IDS)
        .map_err(|err| {
            tracing::warn!(target: LOG_TARGET, error = %err, "no journal id for the repair record");
            JOURNAL_IDS_UNAVAILABLE
        })?;
    let operation_id = OperationId::new(repaired.run_id).map_err(|_| PROVENANCE_COMMIT_FAILED)?;
    let now = rfc3339_utc(u64::try_from(repaired.now_ms).unwrap_or_default());
    let operation = CommitOperation::new(operation_id, OperationKind::Repair, now.as_str())
        .with_actor(repaired.actor);
    let recorder = CommitRecorder::new(provenance.context.clone(), operation.clone(), ids[0]);
    let staging = MemoryBackend::new();
    let area =
        StagingArea::new(&staging, "changes/repair").map_err(|_| PROVENANCE_COMMIT_FAILED)?;
    let model = ChangeModel::new(
        root,
        reservations,
        repaired.holder,
        repaired.object_root,
        repaired.object_id,
        area,
    );
    let meta = commit_meta(&provenance.context, &operation, now);
    let span = crate::trace_layer::activity_span(operation.operation_id.as_str(), None);
    let receipt = model
        .commit_bound_only(meta, &recorder)
        .instrument(span)
        .await
        .map_err(|err| {
            tracing::warn!(target: LOG_TARGET, error = %err, "the repair record was not committed");
            PROVENANCE_COMMIT_FAILED
        })?;
    Ok((
        receipt.version.to_string(),
        recorder.journal_entries(&receipt),
    ))
}
