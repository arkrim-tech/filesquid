//! PROV-O history of OCFL versions (API-S-028, API-S-029, API-S-032; PRD F-067, F-046;
//! ADR 040 Decisions 3 to 9).
//!
//! Every commit the API makes or drives writes, in the same OCFL version, the PROV-O record
//! of that version (`provenance/<version>.jsonld`, built by `filesquid-prov`) and the
//! journal entries of the commit (`provenance/<version>.journal.jsonl`). The version `user`
//! names the installation (`urn:uuid:<installation-uuid>`, never a person) and the version
//! `message` is the structured `filesquid.operation.v1` message.
//!
//! - [`ProvenanceContext`], [`version_user`] and [`commit_meta`]: who commits and the
//!   version metadata a commit sets.
//! - [`CommitRecorder`]: the change model's version-bound file function that writes both
//!   files for the reserved version, again after a rebase, and keeps the journal entries the
//!   caller appends once the commit is published.
//! - [`backfill`]: the bounded, resumable job that reconstructs records for versions
//!   committed before the history existed.
//! - [`HistorySource`], [`HistorySnapshot`], [`MemoryHistory`], [`OcflHistory`] and
//!   [`OcflRootsHistory`] (every storage root a host's [`HistoryRoots`] lists): what the
//!   history routes read, injected by the host; any index is rebuilt from the OCFL records
//!   and flagged when a cap cut it short.
//! - [`redact_record`] and [`Redactor`]: a record with agent identities, version messages
//!   and logical paths removed, for shared or published outputs (PRD F-046).
//!
//! The routes (`GET /api/v1/history`, `GET /api/v1/files/{id}/history`, its JSON-LD export
//! and `GET /api/v1/files/{id}/versions/{version}/prov`) perform
//! [`crate::auth::Action::ReadHistory`].

pub mod backfill;
mod recorder;
mod redact;
pub(crate) mod routes;
mod source;

pub use filesquid_prov::HostFacts;
pub use recorder::{
    commit_meta, journal_path, version_user, CommitOperation, CommitRecorder, ProvenanceContext,
    INSTALLATION_USER_NAME, JOURNAL_SUFFIX,
};
pub use redact::{redact_record, RedactError, Redactor, REDACTED_INSTALLATION};
pub use routes::{HistoryPage, JSON_LD, MAX_EXPORT_BYTES, TRUNCATED_HEADER};
pub use source::{
    entry_of, HistoryError, HistoryRoots, HistorySnapshot, HistorySource, MemoryHistory,
    OcflHistory, OcflRootsHistory, RootResult, MAX_HISTORY_OBJECTS, MAX_HISTORY_RECORDS,
    MAX_HISTORY_ROOTS,
};

/// Journal operation of a provenance record that could not be written, or was not written
/// because the host configured no provenance context. Never silent (API-S-028).
pub const PROV_RECORD_OPERATION: &str = "prov.record";
