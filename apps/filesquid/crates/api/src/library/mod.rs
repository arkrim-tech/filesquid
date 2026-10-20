//! Library domain commands and queries: FileSquid's application meaning for libraries,
//! collections, virtual collections, membership, the hide and delete policy and paged
//! listing (API-S-019, Task CL-0.2).
//!
//! Every host runs the same meaning through [`LibraryService`]
//! ([API-S-006](../../resources/srs.md)): the service checks and plans each versioned
//! [`CommandRequest`] or [`QueryRequest`] and hands storage to the host's injected
//! [`LibraryCatalog`]. This module does no storage I/O and holds no filesystem path or SQL;
//! [`MemoryCatalog`] is the in-memory reference catalog for tests and for hosts without a
//! database. The vocabulary is F-049's: library, collection, virtual collection and **file**.
//!
//! Rules every host gets from here:
//!
//! - New identifiers are UUID v4 from the OS CSPRNG ([`mint_uuid_v4`], CORE-S-001).
//! - A command names its library; anything it references must belong to that library, or
//!   the command is `not_found`. A stale expected revision is `conflict` and changes
//!   nothing; a new library is created at [`NEW_LIBRARY_REVISION`].
//! - A repeated operation ID of a command that changed something answers its first outcome
//!   without a second change; a command that changes nothing stores no receipt, so its retry
//!   is planned again.
//! - Deleting a collection or virtual collection keeps its files; removing a file from a
//!   virtual collection keeps its collection and other memberships.
//! - A deleted file is recoverable until its purge-after time, [`RECOVERABLE_DAYS`] days by
//!   default; an earlier permanent deletion needs a [`PurgeConfirmation`] of the exact scope. Ordinary listings never show hidden or
//!   deleted files.
//! - Every commit that changes something carries one redacted journal entry for the host to
//!   store in the same transaction; a command that changes nothing stores no journal entry,
//!   receipt or OCFL version (owner decision, 2026-10-19); refused commands are journaled
//!   once as `failed` through the bounded journal queue that every [`LibraryService`] holds.

/// `tracing` target of this module's stdout-only diagnostics. They are logged at INFO under a
/// target outside `filesquid::activity`, so the journal layer does not record them and each
/// failure reaches the activity journal exactly once, through the service.
const LOG_TARGET: &str = "filesquid::library";

mod announce;
mod catalog;
mod command;
mod error;
mod ids;
mod memory;
mod plan;
mod policy;
mod query;
mod service;

pub(crate) use announce::{navigation_parents, NavigationParents};
pub use catalog::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, FileScope,
    Keyed, LibraryCatalog, LibraryRecord, LibraryState, PageKey,
};
pub use command::{
    CommandRequest, CommandResult, LibraryCommand, MAX_EXPRESSION_BYTES, MAX_FILES_PER_COMMAND,
    MAX_NAME_BYTES, NEW_LIBRARY_REVISION,
};
pub use error::LibraryError;
pub use ids::{is_uuid_v4, mint_uuid_v4};
pub use memory::MemoryCatalog;
pub use policy::{
    Deletion, DeletionPolicy, FileState, PurgeConfirmation, VisibilityFilter, RECOVERABLE_DAYS,
};
pub use query::{Cursor, LibraryQuery, Page, QueryAnswer, QueryRequest};
pub use service::LibraryService;
