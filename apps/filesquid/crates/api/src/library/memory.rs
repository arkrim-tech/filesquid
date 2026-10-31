//! In-memory [`LibraryCatalog`]: the reference implementation of the port for tests and for
//! hosts without a database. Everything is lost when the process ends. One mutex guards the
//! whole state. A commit runs every check that can fail first, then applies its changes,
//! its journal entry and its receipt in place with infallible map writes, so they land
//! together or not at all. A commit with no change stores nothing. Nothing is copied, so a commit no longer costs a copy of the
//! catalog. A purge or virtual-collection removal still scans every stored membership.

use super::catalog::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, FileScope,
    Keyed, LibraryCatalog, LibraryRecord, LibraryState, PageKey, COLLECTION_HOLDS_FILES,
};
use super::expiry::{ObjectPurger, PendingPurge};
use super::policy::{Deletion, ExpiryMode, MAX_RETENTION_DAYS};
use crate::journal::JournalEntry;
use crate::operation::OperationId;
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId,
};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

/// A stored library: record, revision counter and creation sequence.
struct StoredLibrary {
    record: LibraryRecord,
    revision: u64,
    sequence: i64,
    retention_days: Option<u32>,
}

/// A stored file: the entry and its listing order.
struct StoredFile {
    entry: FileEntry,
    order: i64,
}

/// Everything the catalog holds.
#[derive(Default)]
struct State {
    libraries: BTreeMap<String, StoredLibrary>,
    collections: BTreeMap<String, Collection>,
    virtuals: BTreeMap<String, VirtualCollection>,
    /// (virtual collection ID, file ID) → membership order key.
    members: BTreeMap<(String, String), i64>,
    files: BTreeMap<String, StoredFile>,
    folders: BTreeMap<String, Folder>,
    receipts: BTreeMap<String, CommitReceipt>,
    /// Recorded object works of expired files, by file ID: (operation ID, mode, library).
    pending: BTreeMap<String, (String, ExpiryMode, String)>,
    /// Object works done, in order.
    purged: Vec<(FileId, ExpiryMode)>,
    /// Object works still to refuse (failure injection).
    failing: u32,
    journal: Vec<JournalEntry>,
    /// Commits that changed something (one OCFL version each on a real host).
    commits: u64,
}

/// The in-memory library catalog.
#[derive(Default)]
pub struct MemoryCatalog {
    state: Mutex<State>,
}

impl MemoryCatalog {
    /// An empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `file` with listing `order`, as an import would (imports are API-F1.3). Its
    /// state starts visible and not deleted.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Failed`] when the state lock is poisoned.
    pub fn insert_file(&self, file: File, order: i64) -> Result<(), CatalogError> {
        let entry = FileEntry::new(file, Default::default());
        let id = entry.file.id().as_str().to_string();
        self.lock()?.files.insert(id, StoredFile { entry, order });
        Ok(())
    }

    /// Store `folder` of a library's collection hierarchy, as a host would.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Failed`] when the state lock is poisoned.
    /// Store `collection` directly, outside any command (test seeding).
    ///
    /// # Errors
    /// [`CatalogError::Failed`] when the lock is poisoned.
    pub fn insert_collection(&self, collection: Collection) -> Result<(), CatalogError> {
        let id = collection.id().as_str().to_string();
        self.lock()?.collections.insert(id, collection);
        Ok(())
    }

    pub fn insert_folder(&self, folder: Folder) -> Result<(), CatalogError> {
        let id = folder.id().as_str().to_string();
        self.lock()?.folders.insert(id, folder);
        Ok(())
    }

    /// Every journal entry stored with a commit, oldest first. A commit that changed nothing
    /// stores none. Reads through a poisoned lock, because a commit checks everything that can
    /// fail before its first write and never leaves the state half applied.
    pub fn journal_entries(&self) -> Vec<JournalEntry> {
        self.read().journal.clone()
    }

    /// Number of commits that changed something, each a new revision (read like
    /// [`Self::journal_entries`]).
    pub fn commit_count(&self) -> u64 {
        self.read().commits
    }

    /// Set the retention period of `library_id`, as the settings command of API-F2.15 will
    /// (a test seam until then); an unknown library is left alone.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for a period outside 1 to [`MAX_RETENTION_DAYS`] days; nothing
    /// is stored.
    pub fn set_retention_days(
        &self,
        library_id: &LibraryId,
        days: Option<u32>,
    ) -> Result<(), CatalogError> {
        if days.is_some_and(|days| !(1..=MAX_RETENTION_DAYS).contains(&days)) {
            return Err(CatalogError::Failed("retention period out of range".into()));
        }
        if let Some(library) = self.read().libraries.get_mut(library_id.as_str()) {
            library.retention_days = days;
        }
        Ok(())
    }

    /// Number of operations recorded with a receipt; an operation that changed nothing has
    /// none (read like [`Self::journal_entries`]).
    /// The object works of expired files done so far, in order.
    pub fn purged_objects(&self) -> Vec<(FileId, ExpiryMode)> {
        self.read().purged.clone()
    }

    /// Refuse the next `count` object works (failure injection for tests).
    pub fn fail_next_purges(&self, count: u32) {
        self.read().failing = count;
    }

    pub fn operation_count(&self) -> u64 {
        u64::try_from(self.read().receipts.len()).unwrap_or(u64::MAX)
    }

    /// Lock the state for an inspection read, recovering it from a poisoned lock.
    fn read(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Lock the state, mapping a poisoned lock to a store failure.
    fn lock(&self) -> Result<MutexGuard<'_, State>, CatalogError> {
        self.state
            .lock()
            .map_err(|_| CatalogError::Failed("memory catalog lock poisoned".into()))
    }
}

/// Items of `items` after `after`, sorted by key, at most `limit`.
fn page<T>(mut items: Vec<Keyed<T>>, after: Option<&PageKey>, limit: usize) -> Vec<Keyed<T>> {
    items.retain(|k| after.is_none_or(|a| k.key > *a));
    items.sort_by(|a, b| a.key.cmp(&b.key));
    items.truncate(limit);
    items
}

/// The answer to a checked commit with no change, at the library's `revision`: nothing is
/// stored, neither its journal entry nor a receipt (owner decision, 2026-10-19). Pure.
fn unstored_receipt(commit: CatalogCommit, revision: u64) -> CommitReceipt {
    CommitReceipt {
        library_id: commit.library_id,
        revision: revision.to_string(),
        created_id: commit.created_id,
        deletion: commit.deletion,
    }
}

/// The key of an item listed by (order, ID).
fn key(order: i64, id: &str) -> PageKey {
    PageKey {
        order,
        id: id.to_string(),
    }
}

/// Whether `commit` creates its library. A create that is not the first change, is repeated,
/// or names another library makes the commit malformed.
fn creates_library(commit: &CatalogCommit) -> Result<bool, CatalogError> {
    let creates: Vec<(usize, &LibraryRecord)> = commit
        .changes
        .iter()
        .enumerate()
        .filter_map(|(index, change)| match change {
            CatalogChange::CreateLibrary(record) => Some((index, record)),
            _ => None,
        })
        .collect();
    match creates.as_slice() {
        [] => Ok(false),
        [(0, record)] if record.library.id() == &commit.library_id => Ok(true),
        _ => Err(CatalogError::Failed(
            "malformed commit: misplaced create-library change".into(),
        )),
    }
}

impl State {
    /// The listing order of `stored` within `scope`, or `None` when it is outside the scope.
    fn order_in(&self, scope: &FileScope, stored: &StoredFile) -> Option<i64> {
        let file = &stored.entry.file;
        match scope {
            FileScope::Library => Some(stored.order),
            FileScope::Collection { collection_id } => {
                (file.home_collection_id() == collection_id).then_some(stored.order)
            }
            FileScope::VirtualCollection {
                virtual_collection_id,
            } => self
                .members
                .get(&(
                    virtual_collection_id.as_str().to_string(),
                    file.id().as_str().to_string(),
                ))
                .copied(),
        }
    }

    /// Apply one change.
    fn apply(&mut self, commit_library: &LibraryId, operation: &str, change: CatalogChange) {
        match change {
            CatalogChange::CreateLibrary(record) => self.create_library(record),
            CatalogChange::PutCollection(c) => {
                self.collections.insert(c.id().as_str().to_string(), c);
            }
            CatalogChange::RemoveCollection(id) => {
                self.collections.remove(id.as_str());
            }
            CatalogChange::DeleteCollectionFiles {
                collection_id,
                deletion,
                title,
            } => self.delete_collection_files(&collection_id, deletion, &title),
            CatalogChange::RehomeFile {
                file_id,
                collection_id,
            } => self.rehome(&file_id, collection_id),
            CatalogChange::SetRetention(days) => self.set_retention(commit_library, days),
            CatalogChange::PutVirtualCollection(v) => {
                self.virtuals.insert(v.id().as_str().to_string(), v);
            }
            CatalogChange::RemoveVirtualCollection(id) => self.remove_virtual(&id),
            CatalogChange::AddVirtualMember(m) => {
                let pair = (
                    m.virtual_collection_id().as_str().to_string(),
                    m.file_id().as_str().to_string(),
                );
                self.members.insert(pair, m.order_key());
            }
            CatalogChange::RemoveVirtualMember {
                virtual_collection_id,
                file_id,
            } => {
                let pair = (
                    virtual_collection_id.as_str().to_string(),
                    file_id.as_str().to_string(),
                );
                self.members.remove(&pair);
            }
            CatalogChange::SetFileState { file_id, state } => {
                if let Some(stored) = self.files.get_mut(file_id.as_str()) {
                    stored.entry.state = state;
                }
            }
            CatalogChange::PurgeFile(id) => self.purge(&id),
            CatalogChange::ExpireFile { file_id, mode } => {
                self.purge(&file_id);
                let library = commit_library.as_str().to_string();
                self.pending.insert(
                    file_id.as_str().to_string(),
                    (operation.to_string(), mode, library),
                );
            }
        }
    }

    /// Store a new library at revision 0 (the commit then sets its revision to 1).
    fn create_library(&mut self, record: LibraryRecord) {
        let sequence = i64::try_from(self.libraries.len()).unwrap_or(i64::MAX);
        let id = record.library.id().as_str().to_string();
        let stored = StoredLibrary {
            record,
            revision: 0,
            sequence,
            retention_days: None,
        };
        self.libraries.insert(id, stored);
    }

    /// Mark every file of `collection_id` that is not deleted yet as deleted with `deletion`,
    /// in place.
    fn delete_collection_files(
        &mut self,
        collection_id: &CollectionId,
        deletion: Deletion,
        title: &str,
    ) {
        self.files
            .values_mut()
            .filter(|stored| stored.entry.file.home_collection_id() == collection_id)
            .for_each(|stored| {
                let state = &mut stored.entry.state;
                state.deletion = state.deletion.or(Some(deletion));
                stored.entry.removed_home_title = Some(title.to_string());
            });
    }

    /// Give `file_id` the home `collection_id`, clearing its removed home title.
    fn rehome(&mut self, file_id: &FileId, collection_id: CollectionId) {
        if let Some(stored) = self.files.get_mut(file_id.as_str()) {
            let file = &stored.entry.file;
            stored.entry.file =
                File::new(file.id().clone(), file.library_id().clone(), collection_id);
            stored.entry.removed_home_title = None;
        }
    }

    /// Set the retention period of `library_id`.
    fn set_retention(&mut self, library_id: &LibraryId, days: Option<u32>) {
        if let Some(library) = self.libraries.get_mut(library_id.as_str()) {
            library.retention_days = days;
        }
    }

    /// Refuse a commit that removes a collection still holding a file that is not deleted,
    /// unless the same commit deletes that collection's files before removing it, so no
    /// file is left with a removed home and no deleted state (ADR 041 Decision 8). Pure.
    fn check_removals(&self, commit: &CatalogCommit) -> Result<(), CatalogError> {
        let mut deleted: Vec<&CollectionId> = Vec::new();
        for change in &commit.changes {
            match change {
                CatalogChange::DeleteCollectionFiles { collection_id, .. } => {
                    deleted.push(collection_id)
                }
                CatalogChange::RemoveCollection(id) if !deleted.contains(&id) => {
                    self.check_empty(id)?
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Refuse when `collection_id` is the home of a file that is not deleted. Pure.
    fn check_empty(&self, collection_id: &CollectionId) -> Result<(), CatalogError> {
        let holds = self.files.values().any(|stored| {
            stored.entry.file.home_collection_id() == collection_id
                && stored.entry.state.deletion.is_none()
        });
        match holds {
            true => Err(CatalogError::Failed(COLLECTION_HOLDS_FILES.into())),
            false => Ok(()),
        }
    }

    /// Remove a virtual collection and its memberships.
    fn remove_virtual(&mut self, id: &VirtualCollectionId) {
        self.virtuals.remove(id.as_str());
        self.members.retain(|(vc, _), _| vc != id.as_str());
    }

    /// Remove a file and its memberships.
    fn purge(&mut self, id: &FileId) {
        self.files.remove(id.as_str());
        self.members.retain(|(_, file), _| file != id.as_str());
    }

    /// The library's revision once `commit` applies, or why it may not apply. A library the
    /// commit creates must not exist and starts at revision 1. Otherwise the library must
    /// exist at the expected revision, which advances by one when the commit changes
    /// something and is kept when it changes nothing.
    fn next_revision(&self, commit: &CatalogCommit) -> Result<u64, CatalogError> {
        let creates = creates_library(commit)?;
        let current = self.libraries.get(commit.library_id.as_str());
        let changed = u64::from(!commit.changes.is_empty());
        match (creates, current) {
            (true, None) => Ok(1),
            (false, Some(lib)) if lib.revision.to_string() == commit.expected_revision.as_str() => {
                Ok(lib.revision + changed)
            }
            _ => Err(CatalogError::Conflict),
        }
    }

    /// Apply a checked `commit` that has at least one change in place: its changes, the
    /// library's `revision`, its journal entry and its receipt. Every step is an infallible
    /// map write, so the commit lands whole.
    fn apply_checked(&mut self, commit: CatalogCommit, revision: u64) -> CommitReceipt {
        let library_id = commit.library_id.clone();
        let operation = commit.operation_id.as_str().to_string();
        commit
            .changes
            .into_iter()
            .for_each(|c| self.apply(&library_id, &operation, c));
        if let Some(library) = self.libraries.get_mut(commit.library_id.as_str()) {
            library.revision = revision;
        }
        self.commits += 1;
        let receipt = CommitReceipt {
            library_id: commit.library_id,
            revision: revision.to_string(),
            created_id: commit.created_id,
            deletion: commit.deletion,
        };
        self.journal.push(commit.journal.redacted());
        self.receipts
            .insert(commit.operation_id.as_str().to_string(), receipt.clone());
        receipt
    }
}

impl LibraryCatalog for MemoryCatalog {
    fn receipt(&self, operation_id: &OperationId) -> Result<Option<CommitReceipt>, CatalogError> {
        Ok(self.lock()?.receipts.get(operation_id.as_str()).cloned())
    }

    fn library(&self, library_id: &LibraryId) -> Result<Option<LibraryState>, CatalogError> {
        let state = self.lock()?;
        Ok(state
            .libraries
            .get(library_id.as_str())
            .map(|l| LibraryState {
                record: l.record.clone(),
                revision: l.revision.to_string(),
                retention_days: l.retention_days,
            }))
    }

    fn libraries(
        &self,
        repository_id: &RepositoryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<LibraryRecord>>, CatalogError> {
        let state = self.lock()?;
        let items = state
            .libraries
            .values()
            .filter(|l| l.record.library.repository_id() == repository_id)
            .map(|l| Keyed {
                key: key(l.sequence, l.record.library.id().as_str()),
                item: l.record.clone(),
            })
            .collect();
        Ok(page(items, after, limit))
    }

    fn collection(&self, id: &CollectionId) -> Result<Option<Collection>, CatalogError> {
        Ok(self.lock()?.collections.get(id.as_str()).cloned())
    }

    fn collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<Collection>>, CatalogError> {
        let state = self.lock()?;
        let items = state
            .collections
            .values()
            .filter(|c| c.library_id() == library_id)
            .map(|c| Keyed {
                key: key(c.order_key(), c.id().as_str()),
                item: c.clone(),
            })
            .collect();
        Ok(page(items, after, limit))
    }

    fn virtual_collection(
        &self,
        id: &VirtualCollectionId,
    ) -> Result<Option<VirtualCollection>, CatalogError> {
        Ok(self.lock()?.virtuals.get(id.as_str()).cloned())
    }

    fn virtual_collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError> {
        let state = self.lock()?;
        let items = state
            .virtuals
            .values()
            .filter(|v| v.library_id() == library_id)
            .map(|v| Keyed {
                key: key(v.order_key(), v.id().as_str()),
                item: v.clone(),
            })
            .collect();
        Ok(page(items, after, limit))
    }

    fn is_virtual_member(
        &self,
        virtual_collection_id: &VirtualCollectionId,
        file_id: &FileId,
    ) -> Result<bool, CatalogError> {
        let pair = (
            virtual_collection_id.as_str().to_string(),
            file_id.as_str().to_string(),
        );
        Ok(self.lock()?.members.contains_key(&pair))
    }

    fn last_member_order(
        &self,
        virtual_collection_id: &VirtualCollectionId,
    ) -> Result<Option<i64>, CatalogError> {
        let state = self.lock()?;
        Ok(state
            .members
            .iter()
            .filter(|((vc, _), _)| vc == virtual_collection_id.as_str())
            .map(|(_, order)| *order)
            .max())
    }

    fn folder(&self, folder_id: &FolderId) -> Result<Option<Folder>, CatalogError> {
        Ok(self.lock()?.folders.get(folder_id.as_str()).cloned())
    }

    fn file(&self, file_id: &FileId) -> Result<Option<FileEntry>, CatalogError> {
        Ok(self
            .lock()?
            .files
            .get(file_id.as_str())
            .map(|s| s.entry.clone()))
    }

    fn files(
        &self,
        listing: &FileListing,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError> {
        let state = self.lock()?;
        let items = state
            .files
            .values()
            .filter(|s| s.entry.file.library_id() == &listing.library_id)
            .filter(|s| listing.visibility.admits(&s.entry.state))
            .filter_map(|s| {
                let order = state.order_in(&listing.scope, s)?;
                Some(Keyed {
                    key: key(order, s.entry.file.id().as_str()),
                    item: s.entry.clone(),
                })
            })
            .collect();
        Ok(page(items, after, limit))
    }

    /// Answer a stored receipt first, then check the commit. A commit with no change stores
    /// nothing and answers the current revision; any other commit is applied in place.
    fn commit(&self, commit: CatalogCommit) -> Result<CommitReceipt, CatalogError> {
        let mut state = self.lock()?;
        if let Some(done) = state.receipts.get(commit.operation_id.as_str()) {
            return Ok(done.clone());
        }
        let revision = state.next_revision(&commit)?;
        state.check_removals(&commit)?;
        if commit.changes.is_empty() {
            return Ok(unstored_receipt(commit, revision));
        }
        Ok(state.apply_checked(commit, revision))
    }
}

impl ObjectPurger for MemoryCatalog {
    fn pending_purges(
        &self,
        library_id: &LibraryId,
        limit: usize,
    ) -> Result<Vec<PendingPurge>, CatalogError> {
        let state = self.lock()?;
        state
            .pending
            .iter()
            .filter(|(_, (_, _, library))| library == library_id.as_str())
            .take(limit)
            .map(|(file, (operation_id, mode, _))| {
                FileId::new(file.clone())
                    .map(|file_id| PendingPurge {
                        file_id,
                        operation_id: operation_id.clone(),
                        mode: *mode,
                    })
                    .map_err(|_| CatalogError::Failed("empty file ID".into()))
            })
            .collect()
    }

    fn purge_object(
        &self,
        _library_id: &LibraryId,
        purge: &PendingPurge,
        _actor: &str,
    ) -> Result<(), CatalogError> {
        let mut state = self.lock()?;
        if state.failing > 0 {
            state.failing -= 1;
            return Err(CatalogError::Unavailable("injected object failure".into()));
        }
        if state.pending.remove(purge.file_id.as_str()).is_some() {
            state.purged.push((purge.file_id.clone(), purge.mode));
        }
        Ok(())
    }
}
