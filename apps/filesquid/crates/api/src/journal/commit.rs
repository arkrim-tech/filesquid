//! Journal entries of OCFL commits (API-S-028; ADR 040 Decision 6).
//!
//! Every commit any writer makes is journaled as [`OCFL_COMMIT`], naming the
//! operation ID and the object version it published, so journal rows join
//! PROV-O records and OCFL versions by identifier, not by timestamp. A
//! commit whose version reservation took over a lease that was still held
//! adds a [`LEASE_TAKEOVER`] warning naming the same object version. Failed
//! and refused attempts are journaled by their callers as before and never
//! reach PROV-O.

use filesquid_core::{OperationId, VersionRef};

use super::{JournalEntry, Level, Outcome};

/// Operation name of a published OCFL version.
pub const OCFL_COMMIT: &str = "ocfl.commit";

/// Operation name of a commit that took over a version reservation.
pub const LEASE_TAKEOVER: &str = "ocfl.lease.takeover";

/// What one published commit is journaled with.
#[derive(Debug, Clone, Copy)]
pub struct CommitFacts<'a> {
    /// The operation the commit belongs to.
    pub operation_id: &'a OperationId,
    /// The object version the commit published.
    pub version: &'a VersionRef,
    /// Whether the commit's reservation took over a lease that was still
    /// held (`CommitReceipt::took_over`).
    pub took_over: bool,
}

/// The journal entries of one published commit: one [`OCFL_COMMIT`] entry,
/// and a [`LEASE_TAKEOVER`] warning when the reservation took over a lease.
/// Every entry names the operation ID, the object reference
/// `<object-id>@vN` and, as its target, the object ID.
pub fn ocfl_commit_entries(facts: &CommitFacts<'_>) -> Vec<JournalEntry> {
    let commit = linked(JournalEntry::new(OCFL_COMMIT, Outcome::Succeeded), facts);
    if !facts.took_over {
        return vec![commit];
    }
    let takeover = linked(JournalEntry::new(LEASE_TAKEOVER, Outcome::Info), facts)
        .level(Level::Warn)
        .error_code("lease_taken_over");
    vec![commit, takeover]
}

/// `entry` with the commit's operation ID, object reference and target.
fn linked(entry: JournalEntry, facts: &CommitFacts<'_>) -> JournalEntry {
    entry
        .operation_id(facts.operation_id.as_str())
        .object_ref(facts.version.object_ref())
        .target(facts.version.object_id())
}
