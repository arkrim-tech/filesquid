//! The per-library SQLite catalog (Task DE-0.2): the desktop's native storage behind the
//! `filesquid_api::library::LibraryCatalog` port
//! ([ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md),
//! [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//!
//! `filesquid-api` decides what a library command or query means ([`LibraryService`]); this
//! module only stores it. Each library has its own SQLite file, `<root>/<library-id>.sqlite`
//! (the identifier is a canonical UUID v4, so it never names another path), opened in WAL
//! mode with foreign keys on and an owner-only file mode. A commit runs in one transaction of
//! that file: the stored receipt of its operation ID is answered first (looked up in every
//! library), then the create or revision rule, every change, the new revision, the receipt,
//! the removal of the operation's pending intent and its journal entry (into the file's
//! journal outbox). The outbox is then forwarded to the host's activity journal store; what
//! the store refuses is forwarded later, so nothing is dropped.
//!
//! The catalog is an index, never the only copy of accepted data: committed state is
//! canonical in the library's OCFL objects, and the catalog is rebuilt from them (DE-0.3,
//! DE-1.1). Copying a live catalog file is not a supported backup; [`SqliteCatalog::backup`]
//! writes a consistent copy with SQLite's `VACUUM INTO`. No method answers a filesystem path
//! or SQL to the API, and no error it returns names a path.
//!
//! [`LibraryService`]: filesquid_api::library::LibraryService

mod intent;
mod outbox;
mod rows;
mod schema;
mod write;

pub use intent::PendingIntent;

use filesquid_api::journal::{JournalEntry, JournalStore, Outcome};
use filesquid_api::library::{
    CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, Keyed, LibraryCatalog,
    LibraryRecord, LibraryState, PageKey,
};
use filesquid_api::operation::OperationId;
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId,
};
use rows::sql;
use rusqlite::{Connection, Transaction};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Directory of the library catalogs inside a host's data directory.
pub const CATALOG_DIR: &str = "libraries";

/// File extension of a library's catalog file (`<library-id>.sqlite`).
pub const CATALOG_FILE_EXTENSION: &str = "sqlite";

/// Version of the catalog schema this host writes (`PRAGMA user_version`). A file with a
/// newer version is refused rather than misread.
pub const CATALOG_SCHEMA_VERSION: i64 = 1;

/// Most library catalogs one catalog directory may hold; opening more fails (AGENTS.md §22).
pub const MAX_LIBRARIES: usize = 256;

/// `tracing` target of the catalog's stdout diagnostics.
const LOG_TARGET: &str = "filesquid::catalog";

/// A [`CatalogError::Failed`] with `detail` (stdout only; never a path).
pub(crate) fn failed(detail: impl Into<String>) -> CatalogError {
    CatalogError::Failed(detail.into())
}

/// The journal error code of a catalog failure. Pure.
fn error_code(err: &CatalogError) -> &'static str {
    match err {
        CatalogError::Conflict => "conflict",
        CatalogError::Unavailable(_) => "unavailable",
        CatalogError::Failed(_) => "internal",
    }
}

/// One open library catalog file.
struct OpenLibrary {
    id: LibraryId,
    conn: Connection,
}

/// The library catalogs of one catalog directory, one SQLite file per library. One mutex
/// serialises every read and commit, so a commit's receipt check across libraries and its
/// transaction see the same state.
pub struct SqliteCatalog {
    root: PathBuf,
    journal: Arc<dyn JournalStore>,
    libraries: Mutex<BTreeMap<String, OpenLibrary>>,
}

impl std::fmt::Debug for SqliteCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteCatalog").finish_non_exhaustive()
    }
}

impl SqliteCatalog {
    /// Open the catalog directory `root` (created owner-only if missing), open every library
    /// catalog file in it, migrate each to [`CATALOG_SCHEMA_VERSION`], and forward journal
    /// entries a crash left in their outboxes to `journal`, the host's activity journal store.
    /// A file without a library row (a create that never committed) is left closed.
    /// Filesystem and database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Unavailable`] when the directory or a file cannot be opened;
    /// [`CatalogError::Failed`] when it holds more than [`MAX_LIBRARIES`] catalogs or a file's
    /// schema is newer than this host.
    pub fn open(root: &Path, journal: Arc<dyn JournalStore>) -> Result<Self, CatalogError> {
        create_root(root)?;
        let libraries = open_existing(root)?;
        let catalog = Self {
            root: root.to_path_buf(),
            journal,
            libraries: Mutex::new(libraries),
        };
        catalog.forward_all()?;
        Ok(catalog)
    }

    /// Store `files` (each with its listing order) in `library_id`'s catalog, as an import or
    /// a rebuild from OCFL does, with `journal` in the same transaction. Files already stored
    /// keep their hide and delete state. The revision is not advanced: the canonical change
    /// is the OCFL version the caller committed. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library, a file of another library or a
    /// database failure; nothing is stored.
    pub fn index_files(
        &self,
        library_id: &LibraryId,
        files: &[(File, i64)],
        journal: JournalEntry,
    ) -> Result<(), CatalogError> {
        self.write_journaled(library_id, journal, |tx| {
            write::index_files(tx, library_id, files)
        })
    }

    /// Store `folders` in `library_id`'s catalog with `journal` in the same transaction, like
    /// [`Self::index_files`]. Database I/O.
    ///
    /// # Errors
    /// As [`Self::index_files`].
    pub fn index_folders(
        &self,
        library_id: &LibraryId,
        folders: &[Folder],
        journal: JournalEntry,
    ) -> Result<(), CatalogError> {
        self.write_journaled(library_id, journal, |tx| {
            write::index_folders(tx, library_id, folders)
        })
    }

    /// Record `intent` in its library's catalog before canonical work starts; the commit of
    /// its operation ID removes it. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    pub fn record_intent(&self, intent: &PendingIntent) -> Result<(), CatalogError> {
        let libraries = self.lock()?;
        let library = open_library(&libraries, &intent.library_id)?;
        intent::record(&library.conn, intent)
    }

    /// The oldest `limit` pending intents of `library_id` (work a crash left to reconcile).
    /// Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    pub fn pending_intents(
        &self,
        library_id: &LibraryId,
        limit: usize,
    ) -> Result<Vec<PendingIntent>, CatalogError> {
        let libraries = self.lock()?;
        intent::list(
            &open_library(&libraries, library_id)?.conn,
            library_id,
            limit,
        )
    }

    /// Number of journal entries of `library_id` not yet in the host journal. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    pub fn outbox_len(&self, library_id: &LibraryId) -> Result<u64, CatalogError> {
        let libraries = self.lock()?;
        outbox::len(&open_library(&libraries, library_id)?.conn)
    }

    /// Write a consistent copy of `library_id`'s catalog to `dest` with SQLite's
    /// `VACUUM INTO` (the supported backup; a live-file copy is not) and journal it as
    /// `library.catalog.backup`. `dest` must not exist or be empty; it is created owner-only.
    /// Filesystem and database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library, a non-empty destination or a failed
    /// copy (journaled as `failed`; a destination this call created is removed).
    pub fn backup(&self, library_id: &LibraryId, dest: &Path) -> Result<(), CatalogError> {
        let mut libraries = self.lock()?;
        let library = open_library_mut(&mut libraries, library_id)?;
        let result = vacuum_into(&library.conn, dest);
        let entry = match &result {
            Ok(()) => JournalEntry::new("library.catalog.backup", Outcome::Succeeded),
            Err(err) => JournalEntry::new("library.catalog.backup", Outcome::Failed)
                .error_code(error_code(err)),
        };
        journal_alone(&mut library.conn, &entry.target(library_id.as_str()))?;
        self.forward_logged(&library.conn);
        result
    }

    /// Run `write` and push `journal` into `library_id`'s outbox in one transaction, then
    /// forward the outbox. Database I/O.
    fn write_journaled(
        &self,
        library_id: &LibraryId,
        journal: JournalEntry,
        write: impl FnOnce(&Transaction<'_>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        let mut libraries = self.lock()?;
        let library = open_library_mut(&mut libraries, library_id)?;
        let tx = library.conn.transaction().map_err(sql)?;
        write(&tx)?;
        outbox::push(&tx, &journal)?;
        tx.commit().map_err(sql)?;
        self.forward_logged(&library.conn);
        Ok(())
    }

    /// Forward every library's outbox (at open). Database and journal-store I/O.
    fn forward_all(&self) -> Result<(), CatalogError> {
        let libraries = self.lock()?;
        libraries
            .values()
            .for_each(|library| self.forward_logged(&library.conn));
        Ok(())
    }

    /// Forward `conn`'s outbox to the host journal. A refusal keeps the entries for the next
    /// call and is logged as a warning (stdout; the journal layer journals warnings).
    fn forward_logged(&self, conn: &Connection) {
        if let Err(err) = outbox::forward(conn, self.journal.as_ref()) {
            tracing::warn!(
                target: LOG_TARGET,
                operation = "library.catalog.journal_forward",
                error_code = error_code(&err),
                "journal outbox kept for the next forward"
            );
        }
    }

    /// Create `commit`'s library in a new catalog file and apply the commit there. A file this
    /// call created is removed when the commit fails. Filesystem and database I/O.
    fn create(
        &self,
        libraries: &mut BTreeMap<String, OpenLibrary>,
        commit: &CatalogCommit,
    ) -> Result<CommitReceipt, CatalogError> {
        let path = schema::file_path(&self.root, &commit.library_id)?;
        let existed = path.exists();
        let opened = schema::open_catalog_db(&path)
            .and_then(|mut conn| write::apply_commit(&mut conn, commit, true).map(|r| (conn, r)));
        let (conn, receipt) = match opened {
            Ok(done) => done,
            Err(err) => {
                if !existed {
                    remove_catalog_file(&path);
                }
                return Err(err);
            }
        };
        self.forward_logged(&conn);
        let id = commit.library_id.clone();
        libraries.insert(id.as_str().to_string(), OpenLibrary { id, conn });
        Ok(receipt)
    }

    /// Lock the open libraries, mapping a poisoned lock to a store failure.
    fn lock(&self) -> Result<MutexGuard<'_, BTreeMap<String, OpenLibrary>>, CatalogError> {
        self.libraries
            .lock()
            .map_err(|_| failed("library catalog lock poisoned"))
    }

    /// The first `Some` that `read` answers over the open libraries, for lookups by an
    /// identifier that does not name its library. Bounded by [`MAX_LIBRARIES`]. Database I/O.
    fn find<T>(
        &self,
        read: impl Fn(&LibraryId, &Connection) -> Result<Option<T>, CatalogError>,
    ) -> Result<Option<T>, CatalogError> {
        let libraries = self.lock()?;
        find_in(&libraries, read)
    }
}

/// The first `Some` that `read` answers over `libraries`. Database I/O.
fn find_in<T>(
    libraries: &BTreeMap<String, OpenLibrary>,
    read: impl Fn(&LibraryId, &Connection) -> Result<Option<T>, CatalogError>,
) -> Result<Option<T>, CatalogError> {
    for library in libraries.values() {
        if let Some(found) = read(&library.id, &library.conn)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The open library `library_id`.
fn open_library<'a>(
    libraries: &'a BTreeMap<String, OpenLibrary>,
    library_id: &LibraryId,
) -> Result<&'a OpenLibrary, CatalogError> {
    libraries
        .get(library_id.as_str())
        .ok_or_else(|| failed("unknown library"))
}

/// The open library `library_id`, mutably (for a transaction).
fn open_library_mut<'a>(
    libraries: &'a mut BTreeMap<String, OpenLibrary>,
    library_id: &LibraryId,
) -> Result<&'a mut OpenLibrary, CatalogError> {
    libraries
        .get_mut(library_id.as_str())
        .ok_or_else(|| failed("unknown library"))
}

/// Push `entry` into `conn`'s outbox in a transaction of its own. Database I/O.
fn journal_alone(conn: &mut Connection, entry: &JournalEntry) -> Result<(), CatalogError> {
    let tx = conn.transaction().map_err(sql)?;
    outbox::push(&tx, entry)?;
    tx.commit().map_err(sql)
}

/// Create the catalog directory `root` if missing and make it owner-only (0700) on Unix.
/// Filesystem I/O; the error names no path.
fn create_root(root: &Path) -> Result<(), CatalogError> {
    let unavailable =
        |e: std::io::Error| CatalogError::Unavailable(format!("catalog directory: {e}"));
    std::fs::create_dir_all(root).map_err(unavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
            .map_err(unavailable)?;
    }
    Ok(())
}

/// The libraries of the catalog files in `root`, at most [`MAX_LIBRARIES`]. Files that are
/// not catalog files are ignored. Filesystem and database I/O.
fn open_existing(root: &Path) -> Result<BTreeMap<String, OpenLibrary>, CatalogError> {
    let names = catalog_file_names(root)?;
    if names.len() > MAX_LIBRARIES {
        return Err(failed(format!(
            "catalog directory holds more than {MAX_LIBRARIES} libraries"
        )));
    }
    let mut libraries = BTreeMap::new();
    for (name, id) in names {
        let conn = schema::open_catalog_db(&root.join(&name))?;
        if rows::library(&conn)?.is_some() {
            libraries.insert(id.as_str().to_string(), OpenLibrary { id, conn });
        }
    }
    Ok(libraries)
}

/// The catalog file names in `root` with their libraries. Filesystem I/O.
fn catalog_file_names(root: &Path) -> Result<Vec<(String, LibraryId)>, CatalogError> {
    let unavailable =
        |e: std::io::Error| CatalogError::Unavailable(format!("read catalog directory: {e}"));
    let mut names = Vec::new();
    for entry in std::fs::read_dir(root).map_err(unavailable)? {
        let name = entry.map_err(unavailable)?.file_name();
        let name = name.to_string_lossy();
        if let Some(id) = schema::library_of(&name) {
            names.push((name.into_owned(), id));
        }
    }
    Ok(names)
}

/// Remove a catalog file this process created and SQLite's `-wal` and `-shm` files beside
/// it. A failure is logged to stdout (it leaves an empty catalog, which open ignores).
/// Filesystem I/O.
fn remove_catalog_file(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        match std::fs::remove_file(&name) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(
                target: LOG_TARGET,
                operation = "library.catalog.remove",
                error_kind = ?err.kind(),
                "an uncommitted catalog file could not be removed"
            ),
        }
    }
}

/// Copy `conn`'s database consistently to `dest` (`VACUUM INTO`), which must not exist or be
/// empty. A destination this call created is removed when the copy fails. Filesystem and
/// database I/O.
fn vacuum_into(conn: &Connection, dest: &Path) -> Result<(), CatalogError> {
    let existing = std::fs::metadata(dest).map(|m| m.len()).ok();
    if existing.is_some_and(|len| len > 0) {
        return Err(failed("backup destination is not empty"));
    }
    let dest_text = dest
        .to_str()
        .ok_or_else(|| failed("backup destination is not valid UTF-8"))?;
    crate::journal_sqlite::create_owner_only(dest)
        .map_err(|e| CatalogError::Unavailable(format!("create backup file: {e}")))?;
    let copied = conn
        .execute("VACUUM INTO ?1", rusqlite::params![dest_text])
        .map(drop)
        .map_err(sql);
    if copied.is_err() && existing.is_none() {
        remove_catalog_file(dest);
    }
    copied
}

impl LibraryCatalog for SqliteCatalog {
    fn receipt(&self, operation_id: &OperationId) -> Result<Option<CommitReceipt>, CatalogError> {
        self.find(|id, conn| rows::receipt(conn, id, operation_id.as_str()))
    }

    fn library(&self, library_id: &LibraryId) -> Result<Option<LibraryState>, CatalogError> {
        let libraries = self.lock()?;
        let Some(library) = libraries.get(library_id.as_str()) else {
            return Ok(None);
        };
        Ok(
            rows::library(&library.conn)?.map(|(record, revision, _)| LibraryState {
                record,
                revision: revision.to_string(),
            }),
        )
    }

    fn libraries(
        &self,
        repository_id: &RepositoryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<LibraryRecord>>, CatalogError> {
        let libraries = self.lock()?;
        let mut items = Vec::new();
        for library in libraries.values() {
            let Some((record, _, order)) = rows::library(&library.conn)? else {
                continue;
            };
            if record.library.repository_id() == repository_id {
                let id = library.id.as_str().to_string();
                items.push(Keyed {
                    key: PageKey { order, id },
                    item: record,
                });
            }
        }
        items.retain(|k| after.is_none_or(|a| k.key > *a));
        items.sort_by(|a, b| a.key.cmp(&b.key));
        items.truncate(limit);
        Ok(items)
    }

    fn collection(&self, collection_id: &CollectionId) -> Result<Option<Collection>, CatalogError> {
        self.find(|id, conn| rows::collection(conn, id, collection_id))
    }

    fn collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<Collection>>, CatalogError> {
        let libraries = self.lock()?;
        match libraries.get(library_id.as_str()) {
            Some(library) => rows::collections(&library.conn, library_id, after, limit),
            None => Ok(Vec::new()),
        }
    }

    fn virtual_collection(
        &self,
        virtual_collection_id: &VirtualCollectionId,
    ) -> Result<Option<VirtualCollection>, CatalogError> {
        self.find(|id, conn| rows::virtual_collection(conn, id, virtual_collection_id))
    }

    fn virtual_collections(
        &self,
        library_id: &LibraryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError> {
        let libraries = self.lock()?;
        match libraries.get(library_id.as_str()) {
            Some(library) => rows::virtual_collections(&library.conn, library_id, after, limit),
            None => Ok(Vec::new()),
        }
    }

    fn is_virtual_member(
        &self,
        virtual_collection_id: &VirtualCollectionId,
        file_id: &FileId,
    ) -> Result<bool, CatalogError> {
        let found = self.find(|_, conn| rows::is_member(conn, virtual_collection_id, file_id))?;
        Ok(found.is_some())
    }

    fn last_member_order(
        &self,
        virtual_collection_id: &VirtualCollectionId,
    ) -> Result<Option<i64>, CatalogError> {
        self.find(|_, conn| rows::last_member_order(conn, virtual_collection_id))
    }

    fn folder(&self, folder_id: &FolderId) -> Result<Option<Folder>, CatalogError> {
        self.find(|id, conn| rows::folder(conn, id, folder_id))
    }

    fn file(&self, file_id: &FileId) -> Result<Option<FileEntry>, CatalogError> {
        self.find(|id, conn| rows::file(conn, id, file_id))
    }

    fn files(
        &self,
        listing: &FileListing,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError> {
        let libraries = self.lock()?;
        match libraries.get(listing.library_id.as_str()) {
            Some(library) => rows::files(&library.conn, listing, after, limit),
            None => Ok(Vec::new()),
        }
    }

    /// Answer a stored receipt of the operation ID from any library first, then create the
    /// library's file or check that the library exists, and apply the commit in one
    /// transaction of its file.
    fn commit(&self, commit: CatalogCommit) -> Result<CommitReceipt, CatalogError> {
        let mut libraries = self.lock()?;
        let op = commit.operation_id.as_str();
        if let Some(done) = find_in(&libraries, |id, conn| rows::receipt(conn, id, op))? {
            return Ok(done);
        }
        let creates = write::creates_library(&commit)?;
        let present = libraries.contains_key(commit.library_id.as_str());
        if creates == present {
            return Err(CatalogError::Conflict);
        }
        if creates {
            return self.create(&mut libraries, &commit);
        }
        let library = open_library_mut(&mut libraries, &commit.library_id)?;
        let receipt = write::apply_commit(&mut library.conn, &commit, false)?;
        self.forward_logged(&library.conn);
        Ok(receipt)
    }
}
