//! The durable journal: sealing the activity journal into OCFL objects (API-S-032;
//! [ADR 033](../../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)
//! Decision 7).
//!
//! The journal store is the live record; it is not durable by itself, because the operational
//! database can be lost. Two mechanisms make the journal recoverable from files:
//!
//! - **Per-commit entries.** Every committed operation writes its journal entries as
//!   `provenance/<version>.journal.jsonl` beside its PROV-O record, in the same OCFL commit
//!   ([`crate::history::CommitRecorder`]).
//! - **Sealed segments** (this module). When the host asks (hourly, on app close, on library
//!   close), [`SealService`] writes every entry not yet sealed as one JSON Lines segment into
//!   a journal OCFL object: the library's (at [`JOURNAL_OBJECT_ROOT`] in its storage root) for
//!   that library's events, the installation's (in the host's data directory) for every
//!   other event. A segment is a new version of that object; the object's own segments say
//!   how far it is sealed, so the seal cursor advances with the commit and with nothing
//!   else, and a failed seal leaves its entries for the next run.
//!
//! **Accepted bound:** while seals succeed, losing the database loses at most one sealing
//! interval ([`SEAL_INTERVAL_MS`]) of entries that changed nothing; everything that changed
//! files or metadata is in its commit. An entry left unsealed for more than two intervals
//! turns health `degraded` with the reason `journal_unsealed` ([`SealStatus`]).
//!
//! A seal's own bookkeeping (its process entries, its `journal.seal` entries and the
//! `ocfl.commit` of its segment) is sealed with the next real entries; a run that would seal
//! nothing else commits nothing and journals nothing (no activity took place).

mod object;
mod service;

pub use object::{
    journal_object_id, read_sealed_entries, segment_path, JournalObject, JournalRoot,
    OcflJournalObject, SealError, SealFuture, Segment, JOURNAL_OBJECT_PREFIX, JOURNAL_OBJECT_ROOT,
    MAX_SEGMENT_BYTES, SEGMENT_DIR,
};
pub use service::{ScopeReport, SealConfig, SealOutcome, SealReport, SealService, SealStatus};

use super::JournalEntry;

/// How often the host seals: every hour (DE-S-013).
pub const SEAL_INTERVAL_MS: i64 = 3_600_000;

/// The registry kind of a seal run.
pub const SEAL_PROCESS_KIND: &str = "journal.seal";

/// Journal operation of one scope's seal: `succeeded` with the entry count and the segment's
/// version, or `failed` with the error code.
pub const SEAL_OPERATION: &str = "journal.seal";

/// The scope key of the installation's journal object.
pub const INSTALLATION_SCOPE: &str = "installation";

/// The scope key of a library's journal object: `library:<library-id>`.
pub fn library_scope(library_id: &str) -> String {
    format!("library:{library_id}")
}

/// True for an entry a seal itself produced: its process entries, its `journal.seal`
/// entries, and the `ocfl.commit` of a journal object. Such entries are sealed only together
/// with other entries, so an idle host writes no new journal version every hour. Pure.
pub fn is_seal_bookkeeping(entry: &JournalEntry) -> bool {
    let process = entry
        .process_id
        .as_deref()
        .is_some_and(|p| p.starts_with(&format!("{SEAL_PROCESS_KIND}-")));
    let journal_object = entry
        .object_ref
        .as_deref()
        .is_some_and(|r| r.starts_with(JOURNAL_OBJECT_PREFIX));
    process || journal_object || entry.operation == SEAL_OPERATION
}
