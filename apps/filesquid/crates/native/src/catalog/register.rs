//! Registering imported files in their home collection, and the folder reads of a collection
//! (Task DE-1.7; DE-S-018, DE-S-019, API-S-011, API-S-026, API-S-036,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 3, 4, 5 and 12).
//!
//! After an import commits a file's object, the job registers the file: it gets its home
//! collection and its collection logical path, which the snapshot records with its object
//! under the import's operation ID, published as one snapshot version and then indexed in the
//! same catalog transaction. A path that is already taken in the collection, by a file of any
//! state, gets the first free number beside it (`photo-1.jpg`); after 9,999 numbers, or when a
//! numbered name would exceed 255 bytes, the registration is refused with that rule's code and
//! changes nothing. A file registered already is answered with its path, never registered
//! twice, so a resumed import adds no second version.

use super::{placement, rows, write, SqliteCatalog};
use filesquid_api::canonical::collection_path::{free_path, ItemKind};
use filesquid_api::canonical::SnapshotKey;
use filesquid_api::files::{ScopeAfter, ScopeEntry, ScopePage};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::library::CatalogError;
use filesquid_core::{CollectionId, File, FileId, LibraryId};
use rusqlite::{Connection, Transaction};

/// Journal operation of registering imported files in their home collection.
pub const REGISTER_OPERATION: &str = "library.files.register";

/// One file to register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterFile {
    /// The file, whose object `urn:uuid:<file-id>` the import committed.
    pub file_id: FileId,
    /// Its home collection, a collection of the library.
    pub collection_id: CollectionId,
    /// Its collection logical path: the manifest entry's path beneath the destination folder.
    pub path: String,
}

/// What became of a registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Registration {
    /// Registered at `path`: the one asked for, or the first free numbered path beside it.
    Registered {
        /// The collection logical path it got.
        path: String,
    },
    /// The file was registered before (a resumed import); nothing changed.
    AlreadyRegistered {
        /// Its collection logical path.
        path: String,
    },
    /// Refused with the error code, changing nothing: `collection_not_found`,
    /// `path_suffix_exhausted`, `name_too_long` or `invalid_path`.
    Refused(&'static str),
}

impl SqliteCatalog {
    /// Register `request`'s file in `library_id` as the module documentation says, publishing
    /// its entity under the operation ID `journal` names and storing `journal` (with the file
    /// and collection as targets, never a path or a name) with its version. A registration
    /// that changes nothing publishes no version and stores no entry. Database, filesystem and
    /// journal I/O.
    ///
    /// # Errors
    /// As [`SqliteCatalog::index_files`]; a refused registration is an `Ok` answer.
    pub fn register_file(
        &self,
        library_id: &LibraryId,
        request: &RegisterFile,
        journal: JournalEntry,
    ) -> Result<Registration, CatalogError> {
        let journal = journal
            .target(request.file_id.as_str())
            .target(request.collection_id.as_str());
        let keys = vec![SnapshotKey::File(request.file_id.clone())];
        let mut outcome = Registration::Refused("internal");
        self.write_journaled(library_id, journal.clone(), keys, |tx| {
            outcome = place_new(tx, library_id, request)?;
            Ok(())
        })?;
        if let Registration::Refused(code) = outcome {
            self.journal_refused(library_id, refused_entry(journal, code))?;
        }
        Ok(outcome)
    }

    /// Store the `failed` entry of a refused registration of `library_id` in its outbox and
    /// forward it. Database and journal I/O.
    fn journal_refused(
        &self,
        library_id: &LibraryId,
        entry: JournalEntry,
    ) -> Result<(), CatalogError> {
        let mut libraries = self.lock()?;
        let library = super::open_library_mut(&mut libraries, library_id)?;
        super::journal_alone(&mut library.conn, &entry)?;
        self.forward_logged(&library.conn);
        Ok(())
    }

    /// Whether `path` is taken in the collection `collection_id`, by a file of any state or
    /// as a folder of one; `false` for a collection no attached library holds. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for a database failure.
    pub fn path_taken(
        &self,
        collection_id: &CollectionId,
        path: &str,
    ) -> Result<bool, CatalogError> {
        let taken = self.in_collection(collection_id, |_, conn| {
            placement::taken(conn, collection_id, path)
        })?;
        Ok(taken.unwrap_or(false))
    }

    /// The folders directly below `parent` in `collection_id` that ordinary files imply, by
    /// name in code-point order, after `after`, at most `limit`; none for an unknown
    /// collection. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for a database failure.
    pub fn subfolders(
        &self,
        collection_id: &CollectionId,
        parent: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, CatalogError> {
        let names = self.in_collection(collection_id, |_, conn| {
            placement::subfolders(conn, collection_id, parent, after, limit)
        })?;
        Ok(names.unwrap_or_default())
    }

    /// Whether ordinary files of `collection_id` imply the folder `path`. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for a database failure.
    pub fn folder_exists(
        &self,
        collection_id: &CollectionId,
        path: &str,
    ) -> Result<bool, CatalogError> {
        let exists = self.in_collection(collection_id, |_, conn| {
            placement::folder_exists(conn, collection_id, path)
        })?;
        Ok(exists.unwrap_or(false))
    }

    /// One page of the folder `folder` of `collection_id`: its subfolders, then its ordinary
    /// files, at most `limit` entries after `after`; `None` for a collection no attached
    /// library holds. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for a database failure.
    pub fn scope_entries(
        &self,
        collection_id: &CollectionId,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, CatalogError> {
        self.in_collection(collection_id, |library_id, conn| {
            let entries: Vec<ScopeEntry> =
                placement::entries(conn, collection_id, folder, after, limit)?;
            Ok(ScopePage {
                library_id: library_id.as_str().to_string(),
                entries,
            })
        })
    }

    /// `read` over the attached library that holds the collection `collection_id`, or `None`
    /// when none does. Bounded by [`super::MAX_LIBRARIES`]. Database I/O.
    fn in_collection<T>(
        &self,
        collection_id: &CollectionId,
        read: impl Fn(&LibraryId, &Connection) -> Result<T, CatalogError>,
    ) -> Result<Option<T>, CatalogError> {
        let libraries = self.lock()?;
        for library in libraries.values() {
            if rows::collection(&library.conn, &library.id, collection_id)?.is_some() {
                return read(&library.id, &library.conn).map(Some);
            }
        }
        Ok(None)
    }
}

/// The `failed` entry of a registration journaled as `journal` that was refused with `code`.
/// Pure.
fn refused_entry(journal: JournalEntry, code: &str) -> JournalEntry {
    let mut entry = journal.level(Level::Warn).error_code(code);
    entry.outcome = Outcome::Failed;
    entry
}

/// Place `request`'s file inside `tx`: answer a file that already has a path as it is; refuse
/// a collection the library does not hold; else store the file (a new one, or one of the
/// earlier snapshot form that has none) at the first free path. Database I/O.
fn place_new(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    request: &RegisterFile,
) -> Result<Registration, CatalogError> {
    if let (Some(path), _) = placement::placement_of(tx, &request.file_id)? {
        return Ok(Registration::AlreadyRegistered { path });
    }
    if rows::collection(tx, library_id, &request.collection_id)?.is_none() {
        return Ok(Registration::Refused("collection_not_found"));
    }
    let mut taken = |path: &str| placement::taken(tx, &request.collection_id, path);
    let path = match free_path(&request.path, ItemKind::File, &mut taken)? {
        Ok(path) => path,
        Err(refused) => return Ok(Registration::Refused(refused.error_code())),
    };
    let file = File::new(
        request.file_id.clone(),
        library_id.clone(),
        request.collection_id.clone(),
    );
    write::index_files(tx, library_id, &[(file, placement::next_order(tx)?)])?;
    placement::place(tx, &request.file_id, &path)?;
    Ok(Registration::Registered { path })
}
