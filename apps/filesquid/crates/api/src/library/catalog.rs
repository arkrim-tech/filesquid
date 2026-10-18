//! The library catalog port that every host injects (API-S-006, API-S-008, API-S-019).
//!
//! `filesquid-api` decides what a library command means; the host's [`LibraryCatalog`]
//! stores it. A host implementation (SQLite on the desktop, PostgreSQL on the server, the
//! encrypted replica in the browser) reads records and applies a [`CatalogCommit`]
//! atomically, in the order [`LibraryCatalog::commit`] states: answer a stored receipt of the
//! operation ID first, then check the create or revision rule, apply every [`CatalogChange`],
//! commit the change as one OCFL version through the `filesquid-ocfl` change model
//! (API-S-008), store the commit's journal entry and receipt in the same transaction, and
//! answer the library's new revision. A commit with no change (a command that changed
//! nothing) stores only its journal entry and receipt, at the current revision. Listing
//! methods page by [`PageKey`] in ascending (order, ID) order. No method takes or answers a
//! filesystem path or SQL.

use super::policy::{Deletion, FileState, VisibilityFilter};
use crate::journal::JournalEntry;
use crate::operation::{ExpectedRevision, OperationId};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId, VirtualMembership,
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
/// [`CatalogChange::CreateLibrary`] may appear only as the first change of a commit, at
/// most once, and only for the commit's own library.
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
    /// Idempotency key; a host answers a repeated key with its first receipt, before any
    /// other check.
    pub operation_id: OperationId,
    /// The library changed.
    pub library_id: LibraryId,
    /// The revision the caller read. For a commit whose first change is
    /// [`CatalogChange::CreateLibrary`] it is [`crate::library::NEW_LIBRARY_REVISION`] and is
    /// not compared: the library MUST be absent, or the commit is
    /// [`CatalogError::Conflict`]. For every other commit the library MUST exist at exactly
    /// this revision, or the commit is [`CatalogError::Conflict`].
    pub expected_revision: ExpectedRevision,
    /// The changes, in order; empty when the command changed nothing.
    pub changes: Vec<CatalogChange>,
    /// The journal entry stored in the same transaction (opaque IDs only).
    pub journal: JournalEntry,
    /// Identifier the command created, kept in the receipt for replays.
    pub created_id: Option<String>,
    /// Deletion and purge-after times of a recoverable deletion, kept in the receipt so a
    /// replay answers the first times.
    pub deletion: Option<Deletion>,
}

/// What a committed operation answered; a replay of its operation ID answers it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitReceipt {
    /// The library changed.
    pub library_id: LibraryId,
    /// The library's revision after the commit (unchanged for a commit with no change).
    pub revision: String,
    /// Identifier the command created, if any.
    pub created_id: Option<String>,
    /// Deletion and purge-after times of a recoverable deletion, if any.
    pub deletion: Option<Deletion>,
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

    /// The highest membership order key of `virtual_collection_id`, or `None` when it has no
    /// member. New members are ordered after it.
    fn last_member_order(
        &self,
        virtual_collection_id: &VirtualCollectionId,
    ) -> Result<Option<i64>, CatalogError>;

    /// One folder of a library's collection hierarchy.
    fn folder(&self, folder_id: &FolderId) -> Result<Option<Folder>, CatalogError>;

    /// One file with its state.
    fn file(&self, file_id: &FileId) -> Result<Option<FileEntry>, CatalogError>;

    /// Files of `listing` after `after`, at most `limit`, by (order, ID).
    fn files(
        &self,
        listing: &FileListing,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError>;

    /// Apply `commit` atomically with its journal entry and answer the receipt. A host MUST
    /// work in this order, all in one transaction:
    ///
    /// 1. If a receipt is stored for `commit.operation_id`, answer it and change nothing.
    ///    This comes first because a retry carries the revision read before the first
    ///    attempt, which the first attempt has since advanced.
    /// 2. Check the create or revision rule of [`CatalogCommit::expected_revision`]; a
    ///    mismatch is [`CatalogError::Conflict`] with no change.
    /// 3. Apply every change; when there is at least one, commit one OCFL version and
    ///    advance the revision. A commit with no change keeps the revision and writes no
    ///    version.
    /// 4. Store the journal entry and the receipt, then answer the receipt.
    ///
    /// A failure at any step MUST leave the catalog as it was.
    fn commit(&self, commit: CatalogCommit) -> Result<CommitReceipt, CatalogError>;
}
