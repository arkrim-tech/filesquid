//! In-memory [`LibraryCatalog`]: the reference implementation of the port for tests and for
//! hosts without a database. Everything is lost when the process ends. One mutex guards the
//! whole state, so a commit's changes, its journal entry and its receipt land together.

use super::catalog::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, FileScope,
    Keyed, LibraryCatalog, LibraryRecord, LibraryState, PageKey,
};
use crate::journal::JournalEntry;
use crate::operation::OperationId;
use filesquid_core::{
    Collection, CollectionId, File, FileId, LibraryId, RepositoryId, VirtualCollection,
    VirtualCollectionId,
};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

/// A stored library: record, revision counter and creation sequence.
#[derive(Clone)]
struct StoredLibrary {
    record: LibraryRecord,
    revision: u64,
    sequence: i64,
}

/// A stored file: the entry and its listing order.
#[derive(Clone)]
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
    receipts: BTreeMap<String, CommitReceipt>,
    journal: Vec<JournalEntry>,
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
        let entry = FileEntry {
            file,
            state: Default::default(),
        };
        let id = entry.file.id().as_str().to_string();
        self.lock()?.files.insert(id, StoredFile { entry, order });
        Ok(())
    }

    /// Every journal entry committed with a change, oldest first. Reads through a poisoned
    /// lock, because a commit never leaves the state half applied before it panics.
    pub fn journal_entries(&self) -> Vec<JournalEntry> {
        self.read().journal.clone()
    }

    /// Number of commits applied (read like [`Self::journal_entries`]).
    pub fn commit_count(&self) -> u64 {
        self.read().commits
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

/// The key of an item listed by (order, ID).
fn key(order: i64, id: &str) -> PageKey {
    PageKey {
        order,
        id: id.to_string(),
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
            FileScope::Unfiled => (!self
                .collections
                .contains_key(file.home_collection_id().as_str()))
            .then_some(stored.order),
        }
    }

    /// Apply one change.
    fn apply(&mut self, change: CatalogChange) {
        match change {
            CatalogChange::CreateLibrary(record) => self.create_library(record),
            CatalogChange::PutCollection(c) => {
                self.collections.insert(c.id().as_str().to_string(), c);
            }
            CatalogChange::RemoveCollection(id) => {
                self.collections.remove(id.as_str());
            }
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
        }
    }

    /// Store a new library at revision 0 (the commit bumps it to 1).
    fn create_library(&mut self, record: LibraryRecord) {
        let sequence = i64::try_from(self.libraries.len()).unwrap_or(i64::MAX);
        let id = record.library.id().as_str().to_string();
        let stored = StoredLibrary {
            record,
            revision: 0,
            sequence,
        };
        self.libraries.insert(id, stored);
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

    /// Whether `commit` may apply: a new library must not exist; otherwise the revision
    /// must match.
    fn check_revision(&self, commit: &CatalogCommit) -> Result<(), CatalogError> {
        let creates = matches!(
            commit.changes.first(),
            Some(CatalogChange::CreateLibrary(_))
        );
        let current = self.libraries.get(commit.library_id.as_str());
        let ok = match (creates, current) {
            (true, None) => true,
            (false, Some(lib)) => lib.revision.to_string() == commit.expected_revision.as_str(),
            _ => false,
        };
        ok.then_some(()).ok_or(CatalogError::Conflict)
    }

    /// Bump the library's revision and answer it.
    fn bump(&mut self, library_id: &LibraryId) -> Result<u64, CatalogError> {
        let library = self
            .libraries
            .get_mut(library_id.as_str())
            .ok_or_else(|| CatalogError::Failed("committed library vanished".into()))?;
        library.revision += 1;
        Ok(library.revision)
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

    fn commit(&self, commit: CatalogCommit) -> Result<CommitReceipt, CatalogError> {
        let mut state = self.lock()?;
        if let Some(done) = state.receipts.get(commit.operation_id.as_str()) {
            return Ok(done.clone());
        }
        state.check_revision(&commit)?;
        commit.changes.into_iter().for_each(|c| state.apply(c));
        let revision = state.bump(&commit.library_id)?;
        state.journal.push(commit.journal.redacted());
        state.commits += 1;
        let receipt = CommitReceipt {
            library_id: commit.library_id,
            revision: revision.to_string(),
            created_id: commit.created_id,
        };
        state
            .receipts
            .insert(commit.operation_id.as_str().to_string(), receipt.clone());
        Ok(receipt)
    }
}
