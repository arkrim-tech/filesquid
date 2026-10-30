//! [`MemoryHomeCollections`]: the reference [`HomeCollections`] port, held in memory, for
//! tests and hosts without a catalog. It applies the same rules as the SQLite catalog: every
//! place is scoped to its library; a path is taken by a file at it, below it (the path is then
//! a folder) or above it (a file where a folder would be); a registered file is answered
//! again, never registered twice; a reservation belongs to one command's input.

use super::{HomeCollections, HomeRequest, Placed, Reserved};
use crate::canonical::collection_path::{free_path, ItemKind, PathError};
use crate::journal::JournalEntry;
use crate::library::CatalogError;
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

/// Where a registered file is: its library, home collection and logical path.
type Place = (LibraryId, CollectionId, String);

/// The state of the memory port.
#[derive(Debug, Default)]
struct State {
    /// The collections of each library.
    collections: HashSet<(LibraryId, CollectionId)>,
    /// Each registered file's place.
    files: HashMap<FileId, Place>,
    /// The fingerprint and file of each unfinished operation, by library and operation ID.
    reserved: HashMap<(LibraryId, String), (String, FileId)>,
    /// Every registration's journal entry, in order.
    journal: Vec<JournalEntry>,
    /// Whether the next registration fails as the store being unreachable (a crash between a
    /// file's object and its registration, as tests stage it).
    fail_next: bool,
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

    /// Register `file_id` at `path` in `collection_id` (of the library that holds it)
    /// directly, as an earlier file (a recoverably deleted one included) that holds its path.
    ///
    /// # Panics
    /// When no library holds `collection_id` (a test set-up error).
    pub fn place(&self, file_id: &FileId, collection_id: &CollectionId, path: &str) {
        let mut state = self.lock();
        let library = state.library_of(collection_id).expect("a held collection");
        let place = (library, collection_id.clone(), path.to_string());
        state.files.insert(file_id.clone(), place);
    }

    /// The home collection and logical path of `file_id`, if registered.
    pub fn placed(&self, file_id: &FileId) -> Option<(CollectionId, String)> {
        let state = self.lock();
        let (_, collection, path) = state.files.get(file_id)?;
        Some((collection.clone(), path.clone()))
    }

    /// Make the next registration fail as an unreachable store, leaving the committed object
    /// unregistered and its reservation in place, as a crash between the two would.
    pub fn fail_next_registration(&self) {
        self.lock().fail_next = true;
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
    /// The library that holds `collection`, if any. Pure over the state.
    fn library_of(&self, collection: &CollectionId) -> Option<LibraryId> {
        let held = self.collections.iter().find(|(_, held)| held == collection);
        held.map(|(library, _)| library.clone())
    }

    /// Whether `path` is taken in `collection` of `library`. Pure over the state.
    fn taken(&self, library: &LibraryId, collection: &CollectionId, path: &str) -> bool {
        self.files
            .values()
            .filter(|(lib, home, _)| lib == library && home == collection)
            .any(|(_, _, held)| overlaps(held, path))
    }

    /// The first free path for `path` in `collection` of `library`. Pure over the state.
    fn free(
        &self,
        library: &LibraryId,
        collection: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<String, PathError> {
        let mut taken =
            |candidate: &str| Ok::<_, CatalogError>(self.taken(library, collection, candidate));
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
        Ok(self.lock().library_of(collection_id))
    }

    fn free_path(
        &self,
        library_id: &LibraryId,
        collection_id: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<Result<String, PathError>, CatalogError> {
        Ok(self.lock().free(library_id, collection_id, path, kind))
    }

    fn reserve(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        fingerprint: &str,
        minted: &FileId,
    ) -> Result<Reserved, CatalogError> {
        let mut state = self.lock();
        let key = (library_id.clone(), operation_id.to_string());
        let held = state
            .reserved
            .entry(key)
            .or_insert_with(|| (fingerprint.to_string(), minted.clone()));
        Ok(match held.0 == fingerprint {
            true => Reserved::File(held.1.clone()),
            false => Reserved::Reused,
        })
    }

    fn release(&self, library_id: &LibraryId, operation_id: &str) -> Result<(), CatalogError> {
        let key = (library_id.clone(), operation_id.to_string());
        self.lock().reserved.remove(&key);
        Ok(())
    }

    fn register(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError> {
        let mut state = self.lock();
        if std::mem::take(&mut state.fail_next) {
            return Err(CatalogError::Unavailable("staged crash".into()));
        }
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

/// Place `request`'s file in `state`: a file already placed in `library_id` is answered with
/// its place; a collection `library_id` does not hold is refused; else the file takes the
/// first free path.
fn place_new(state: &mut State, library_id: &LibraryId, request: &HomeRequest) -> Placed {
    if let Some((lib, collection, path)) = state.files.get(&request.file_id) {
        if lib == library_id {
            return Placed::AlreadyRegistered {
                collection_id: collection.clone(),
                path: path.clone(),
            };
        }
    }
    let key = (library_id.clone(), request.collection_id.clone());
    if !state.collections.contains(&key) {
        return Placed::Refused("collection_not_found");
    }
    let found = state.free(
        library_id,
        &request.collection_id,
        &request.path,
        ItemKind::File,
    );
    let path = match found {
        Ok(path) => path,
        Err(refused) => return Placed::Refused(refused.error_code()),
    };
    let place = (
        library_id.clone(),
        request.collection_id.clone(),
        path.clone(),
    );
    state.files.insert(request.file_id.clone(), place);
    Placed::Registered { path }
}
