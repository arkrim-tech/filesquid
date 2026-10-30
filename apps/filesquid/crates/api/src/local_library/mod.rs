//! Local library operations on an owner-chosen OCFL storage root (Task CL-1.1; F-001,
//! F-042, API-S-023).
//!
//! The host opens the folder the owner chose and injects it as a `filesquid-ocfl`
//! [`StorageBackend`](filesquid_ocfl::storage::StorageBackend); this module decides what each
//! operation means and does no storage I/O of its own ([API-S-006](../../resources/srs.md)).
//! Every host runs the same meaning through [`LocalLibraryService`]:
//!
//! - **Create** a library's storage root in an empty folder: only the OCFL 1.1 storage-root
//!   declaration is written, never over an existing file. No account or connection is needed.
//! - **Open** a storage root through the engine's guards (Git metadata, links, declaration).
//! - **Copy a completed snapshot**: the committed files of every object, each written only
//!   where nothing exists, into an empty destination, which is then validated, so the copy
//!   is a checked OCFL root that ordinary tools can read. A head being published refuses the
//!   copy until it completes.
//! - **Rebuild from files**: every object's committed head and validator findings, from the
//!   OCFL files alone, so a host can rebuild its catalog without its database. External
//!   edits are reported, never adopted or repaired. With a [`JournalRestore`] installed it
//!   also restores the library's journal into the host's fresh journal store from the
//!   per-commit entries and the sealed segments (Task CL-1.9, API-S-032).
//! - **List repair candidates**: objects whose committed head has no version inventory
//!   (W010), found by checking each head for that file.
//! - **Repair a version inventory**: the one command that adds a file to an existing version
//!   directory, for named objects, by the owner, after recorded consent, journaled per object
//!   in the same transaction as its receipt (API-S-023, OCFL-S-020).
//!
//! The versioned routes of these operations (API-S-025, Task API-F1.12) are in
//! `routes`: the host registers each folder the owner chose under an opaque location ID
//! and injects them through [`LibraryLocations`]; [`LibraryCommands`] binds libraries to
//! locations and calls [`LocalLibraryService`], and the host installs it as the
//! [`LibraryOperations`] the routes call. The owner picks each folder in the host's own dialog
//! through `POST /api/v1/libraries/location-choices` ([`choice`]), which answers the opaque
//! location ID only.
//!
//! Open, copy, rebuild and the candidate query never write to the source root, and none of
//! them repairs (F-001: direct inspection and copy MUST NOT authorize in-place edits of OCFL
//! inventories). Answers and journal entries carry opaque IDs and codes only: no filesystem
//! path, storage key, name or content (API-S-005).

/// `tracing` target of this module's stdout-only diagnostics, outside `filesquid::activity`
/// so the journal layer does not record them; each outcome reaches the journal once, through
/// the service.
const LOG_TARGET: &str = "filesquid::local_library";

mod candidates;
pub mod choice;
mod commands;
mod error;
mod facts;
mod journal_rebuild;
mod ledger;
mod locations;
mod operations;
mod provenance;
mod rebuild;
mod rebuild_job;
mod repair;
mod root;
pub(crate) mod routes;
mod service;
mod snapshot;
mod transfer;

pub use commands::{LibraryCommands, MAX_OPEN_LIBRARIES};
pub use error::LocalLibraryError;
pub use facts::{
    check_name, FreeBytesReason, FreeSpace, LibraryFacts, LibraryRegistry, MemoryLibraryRegistry,
    RecentLibraries, RecentLibrary, RegistryBook, RegistryEntry, RenameLibraryRequest,
    MAX_LIBRARY_NAME_BYTES, MAX_RECENT_LIBRARIES, MAX_REMEMBERED_LIBRARIES, UNTITLED_LIBRARY,
};
pub use journal_rebuild::{JournalRebuild, JournalRestore, MAX_RESTORED_ENTRIES, RESTORE_BATCH};
pub use ledger::{LedgerError, MemoryRepairLedger, RecordedRepair, RepairLedger, RepairRecord};
pub use locations::{
    LibraryLocations, LibraryRequest, LibraryRoot, LibraryView, LocationRequest, Opened,
    SnapshotCopyRequest, INITIAL_REVISION, MAX_LOCATION_ID_BYTES,
};
pub use operations::{LibraryFuture, LibraryOperations, Readable};
pub use provenance::{
    JOURNAL_IDS_UNAVAILABLE, PROVENANCE_COMMIT_FAILED, PROVENANCE_NOT_CONFIGURED,
};
pub use rebuild::{
    RebuildEnd, RebuildEntry, RebuildProgress, RebuildReport, RebuildStep, RebuiltObject,
    UnreadableObject,
};
pub use rebuild_job::{
    rebuild_source, submit_rebuild, RebuildJobAnswer, RebuildOutcome, RebuildRunner, REBUILD_BATCH,
    REBUILD_JOB_KIND,
};
pub use repair::{
    ObjectRepair, RepairAnswer, RepairCandidate, RepairCandidates, RepairConsent, RepairRequest,
    RepairResult, CONSENT_STATEMENT, CONSENT_STATEMENT_ID, MAX_CANDIDATES, MAX_REPAIR_OBJECTS,
    REPAIR_OPERATION,
};
pub use root::RootSummary;
pub use service::{
    LocalLibraryService, CANDIDATES_OPERATION, COPY_OPERATION, CREATE_OPERATION, FACTS_OPERATION,
    OPEN_OPERATION, REBUILD_OPERATION, RECENT_OPERATION, RENAME_OPERATION,
};
pub use snapshot::{CopyReport, ObjectFindings};
