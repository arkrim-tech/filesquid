//! [`MemoryHomeCollections`]: the reference [`HomeCollections`] port, held in memory, for
//! tests and hosts without a catalog. It applies the same rules as the SQLite catalog: a path
//! is taken by a file at it, below it (the path is then a folder) or above it (a file where a
//! folder would be); a registered file is answered again, never registered twice.

use super::{HomeCollections, HomeRequest, Placed};
use crate::canonical::collection_path::{free_path, ItemKind, PathError};
use crate::journal::JournalEntry;
use crate::library::CatalogError;
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

/// The state of the memory port.
#[derive(Debug, Default)]
struct State {
    /// The collections of each library.
    collections: HashSet<(LibraryId, CollectionId)>,
    /// Each registered file's home collection and logical path.
    files: HashMap<FileId, (CollectionId, String)>,
    /// The file each unfinished operation reserved, by library and operation ID.
    reserved: HashMap<(LibraryId, String), FileId>,
    /// Every registration's journal entry, in order.
    journal: Vec<JournalEntry>,
}

/// Home collections held in memory.
#[derive(Debug, Default)]
pub struct MemoryHomeCollections {
    state: Mutex<State>,
}

impl MemoryHomeCollections {
    /// An empty port with no collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold the collection `collection_id` in `library_id`.
    pub fn add_collection(&self, library_id: &LibraryId, collection_id: &CollectionId) {
        self.lock()
            .collections
            .insert((library_id.clone(), collection_id.clone()));
    }

    /// Register `file_id` at `path` in `collection_id` directly, as an earlier file (a
    /// recoverably deleted one included) that holds its path.
    pub fn place(&self, file_id: &FileId, collection_id: &CollectionId, path: &str) {
        self.lock()
            .files
            .insert(file_id.clone(), (collection_id.clone(), path.to_string()));
    }

    /// The home collection and logical path of `file_id`, if registered.
    pub fn placement(&self, file_id: &FileId) -> Option<(CollectionId, String)> {
        self.lock().files.get(file_id).cloned()
    }

    /// The number of unfinished reservations.
    pub fn reserved(&self) -> usize {
        self.lock().reserved.len()
    }

    /// Every registration's journal entry, in order.
    pub fn journal(&self) -> Vec<JournalEntry> {
        self.lock().journal.clone()
    }

    /// The state, locked; a poisoned lock is recovered (plain data).
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl State {
    /// Whether `path` is taken in `collection`. Pure over the state.
    fn taken(&self, collection: &CollectionId, path: &str) -> bool {
        self.files
            .values()
            .filter(|(home, _)| home == collection)
            .any(|(_, held)| overlaps(held, path))
    }

    /// The first free path for `path` in `collection`. Pure over the state.
    fn free(
        &self,
        collection: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<String, PathError> {
        let mut taken = |candidate: &str| Ok::<_, CatalogError>(self.taken(collection, candidate));
        free_path(path, kind, &mut taken).unwrap_or(Err(PathError::Invalid))
    }
}

/// Whether a file held at `held` takes `path`: at it, below it or above it. Pure.
fn overlaps(held: &str, path: &str) -> bool {
    held == path || held.starts_with(&format!("{path}/")) || path.starts_with(&format!("{held}/"))
}

impl HomeCollections for MemoryHomeCollections {
    fn collection_library(
        &self,
        collection_id: &CollectionId,
    ) -> Result<Option<LibraryId>, CatalogError> {
        let state = self.lock();
        let held = state
            .collections
            .iter()
            .find(|(_, held)| held == collection_id);
        Ok(held.map(|(library, _)| library.clone()))
    }

    fn free_path(
        &self,
        _library_id: &LibraryId,
        collection_id: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<Result<String, PathError>, CatalogError> {
        Ok(self.lock().free(collection_id, path, kind))
    }

    fn reserve(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        minted: &FileId,
    ) -> Result<FileId, CatalogError> {
        let mut state = self.lock();
        let key = (library_id.clone(), operation_id.to_string());
        Ok(state
            .reserved
            .entry(key)
            .or_insert_with(|| minted.clone())
            .clone())
    }

    fn register(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError> {
        let mut state = self.lock();
        let placed = place_new(&mut state, library_id, request);
        if let Placed::Registered { .. } = placed {
            let entry = journal
                .target(request.file_id.as_str())
                .target(request.collection_id.as_str());
            state.journal.push(entry);
        }
        if !matches!(placed, Placed::Refused(_)) {
            let key = (library_id.clone(), operation_id.to_string());
            state.reserved.remove(&key);
        }
        Ok(placed)
    }
}

/// Place `request`'s file in `state`: a file already placed is answered as it is; a
/// collection `library_id` does not hold is refused; else the file takes the first free path.
fn place_new(state: &mut State, library_id: &LibraryId, request: &HomeRequest) -> Placed {
    if let Some((_, path)) = state.files.get(&request.file_id) {
        return Placed::AlreadyRegistered { path: path.clone() };
    }
    let key = (library_id.clone(), request.collection_id.clone());
    if !state.collections.contains(&key) {
        return Placed::Refused("collection_not_found");
    }
    let path = match state.free(&request.collection_id, &request.path, ItemKind::File) {
        Ok(path) => path,
        Err(refused) => return Placed::Refused(refused.error_code()),
    };
    let home = (request.collection_id.clone(), path.clone());
    state.files.insert(request.file_id.clone(), home);
    Placed::Registered { path }
}
