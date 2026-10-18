//! The library catalog port that every host injects (API-S-006, API-S-008, API-S-019).
//!
//! `filesquid-api` decides what a library command means; the host's [`LibraryCatalog`]
//! stores it. A host implementation (SQLite on the desktop, PostgreSQL on the server, the
//! encrypted replica in the browser) reads records and applies a [`CatalogCommit`]
//! atomically: it checks the expected revision, applies every [`CatalogChange`], commits the
//! change as one OCFL version through the `filesquid-ocfl` change model (API-S-008), stores
//! the commit's journal entry in the same transaction, records the operation ID for replay,
//! and answers the library's new revision. Listing methods page by [`PageKey`] in ascending
//! (order, ID) order. No method takes or answers a filesystem path or SQL.

use super::policy::{FileState, VisibilityFilter};
use crate::journal::JournalEntry;
use crate::operation::{ExpectedRevision, OperationId};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Library, LibraryId, RepositoryId, VirtualCollection,
    VirtualCollectionId, VirtualMembership,
};
use serde::{Deserialize, Serialize};

/// A library and its name. The name is shown to people and never journaled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryRecord {
    /// The library and its repository.
    pub library: Library,
    /// Name shown to people.
    pub name: String,
}

/// A library record with its current revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryState {
    /// The library record.
    pub record: LibraryRecord,
    /// Opaque revision token; the expected revision of the next command.
    pub revision: String,
}

/// A file with its hide and delete state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// The file (identifier, library and home collection).
    pub file: File,
    /// Hidden flag and recoverable deletion.
    pub state: FileState,
}

/// Position of an item in a listing: ascending `order`, then ascending `id`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageKey {
    /// Primary sort value (creation sequence or membership order key).
    pub order: i64,
    /// Opaque identifier of the item; breaks ties.
    pub id: String,
}

/// A listed item with its position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyed<T> {
    /// Position of the item in its listing.
    pub key: PageKey,
    /// The item.
    pub item: T,
}

/// Which files a file listing covers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum FileScope {
    /// Every file of the library.
    Library,
    /// Files whose bytes live in one collection.
    Collection {
        /// The home collection.
        collection_id: CollectionId,
    },
    /// Members of one virtual collection, in membership order.
    VirtualCollection {
        /// The virtual collection.
        virtual_collection_id: VirtualCollectionId,
    },
    /// Files whose collection was removed (`is:unfiled` of F-063).
    Unfiled,
}

/// A file listing: library, scope and view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileListing {
    /// The library.
    pub library_id: LibraryId,
    /// Which files.
    pub scope: FileScope,
    /// Ordinary, hidden or deleted view.
    pub visibility: VisibilityFilter,
}

/// One change of a [`CatalogCommit`]. A host applies all of a commit's changes or none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogChange {
    /// Store a new library record.
    CreateLibrary(LibraryRecord),
    /// Store a collection, replacing one with the same identifier.
    PutCollection(Collection),
    /// Remove a collection record; its files stay (they become unfiled).
    RemoveCollection(CollectionId),
    /// Store a virtual collection, replacing one with the same identifier.
    PutVirtualCollection(VirtualCollection),
    /// Remove a virtual collection and its memberships; the files stay.
    RemoveVirtualCollection(VirtualCollectionId),
    /// Add one virtual-collection membership.
    AddVirtualMember(VirtualMembership),
    /// Remove one virtual-collection membership.
    RemoveVirtualMember {
        /// The virtual collection.
        virtual_collection_id: VirtualCollectionId,
        /// The member file.
        file_id: FileId,
    },
    /// Replace a file's hide and delete state.
    SetFileState {
        /// The file.
        file_id: FileId,
        /// Its new state.
        state: FileState,
    },
    /// Remove a recoverably deleted file and its memberships for good.
    PurgeFile(FileId),
}

/// Everything one command commits, applied by the host in one transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogCommit {
    /// Idempotency key; a host answers a repeated key with its first receipt.
    pub operation_id: OperationId,
    /// The library changed.
    pub library_id: LibraryId,
    /// The revision the caller read; a mismatch is [`CatalogError::Conflict`].
    pub expected_revision: ExpectedRevision,
    /// The changes, in order.
    pub changes: Vec<CatalogChange>,
    /// The journal entry stored in the same transaction (opaque IDs only).
    pub journal: JournalEntry,
    /// Identifier the command created, kept in the receipt for replays.
    pub created_id: Option<String>,
}

/// What a committed operation answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitReceipt {
    /// The library changed.
    pub library_id: LibraryId,
    /// The library's revision after the commit.
    pub revision: String,
    /// Identifier the command created, if any.
    pub created_id: Option<String>,
}

/// A failure of the injected catalog. Details go to stdout only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    /// The expected revision did not match at commit time.
    Conflict,
    /// The store is unreachable now; a retry may succeed.
    Unavailable(String),
    /// The store failed; a retry will not help.
    Failed(String),
}

/// The library catalog a host injects. Reads see only committed state.
pub trait LibraryCatalog: Send + Sync {
    /// The receipt of an operation already committed, if any.
    fn receipt(&self, operation_id: &OperationId) -> Result<Option<CommitReceipt>, CatalogError>;

    /// One library and its revision.
    fn library(&self, library_id: &LibraryId) -> Result<Option<LibraryState>, CatalogError>;

    /// Libraries of `repository_id` after `after`, at most `limit`, in creation order.
    fn libraries(
        &self,
        repository_id: &RepositoryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<LibraryRecord>>, CatalogError>;

    /// One collection.
    fn collection(&self, collection_id: &CollectionId) -> Result<Option<Collection>, CatalogError>;

    /// Collections of `library_id` after `after`, at most `limit`, by (order key, ID).
    fn collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<Collection>>, CatalogError>;

    /// One virtual collection.
    fn virtual_collection(
        &self,
        virtual_collection_id: &VirtualCollectionId,
    ) -> Result<Option<VirtualCollection>, CatalogError>;

    /// Virtual collections of `library_id` after `after`, at most `limit`, by (order key, ID).
    fn virtual_collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError>;

    /// Whether `file_id` is a member of `virtual_collection_id`.
    fn is_virtual_member(
        &self,
        virtual_collection_id: &VirtualCollectionId,
        file_id: &FileId,
    ) -> Result<bool, CatalogError>;

    /// One file with its state.
    fn file(&self, file_id: &FileId) -> Result<Option<FileEntry>, CatalogError>;

    /// Files of `listing` after `after`, at most `limit`, by (order, ID).
    fn files(
        &self,
        listing: &FileListing,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError>;

    /// Apply `commit` atomically with its journal entry and answer the receipt.
    fn commit(&self, commit: CatalogCommit) -> Result<CommitReceipt, CatalogError>;
}
