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
//! journal outbox). A commit with no change stores nothing but the removal of its pending
//! intent: no receipt, no journal entry and no new revision; an index refresh
//! ([`SqliteCatalog::index_files`], [`SqliteCatalog::index_folders`]) that changes nothing
//! likewise publishes no version and stores no journal entry. The outbox is then forwarded
//! to the host's activity journal store; what the store refuses is forwarded later, so
//! nothing is dropped.
//!
//! The catalog is an index, never the only copy of accepted data: committed state is
//! canonical in the library's OCFL objects, and the catalog is rebuilt from them (DE-0.3,
//! DE-1.1). Copying a live catalog file is not a supported backup; [`SqliteCatalog::backup`]
//! writes a consistent copy with SQLite's `VACUUM INTO`. No method answers a filesystem path
//! or SQL to the API, and no error it returns names a path.
//!
//! [`LibraryService`]: filesquid_api::library::LibraryService

mod canonical;
mod history;
mod intent;
mod outbox;
mod project;
mod provenance;
mod reconcile;
mod rows;
mod schema;
mod write;

pub use canonical::{library_ocfl_root, OCFL_ROOT_SUFFIX, WORK_DIR_SUFFIX};
pub use intent::{intent_entry, PendingIntent, INTENT_RECORDED_OPERATION};
pub use provenance::{CatalogRoots, HostRoots, LibraryBackfill, MAX_BACKFILL_RUNS};
pub use reconcile::RECONCILE_OPERATION;

use filesquid_api::canonical::{operation_kind, OperationRecord, SnapshotKey};
use filesquid_api::history::ProvenanceContext;
use filesquid_api::journal::{JournalEntry, JournalStore, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
use filesquid_api::library::mint_uuid_v4;
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
pub const CATALOG_SCHEMA_VERSION: i64 = 2;

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

/// Where a commit can be stopped as a crash would stop it: just before the snapshot version
/// is published, and just after. The host runs with no probe ([`NoProbe`]); crash-injection
/// tests return an error from one stop, which ends the commit there with nothing more
/// written, and then reopen the catalog files as a restarted process would.
pub trait CommitProbe: Send + Sync {
    /// Called once the intent is durable and the changes are applied, before any OCFL byte
    /// of the version is written.
    ///
    /// # Errors
    /// An error stops the commit here.
    fn before_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        let _ = operation_id;
        Ok(())
    }

    /// Called once the version is published, before the catalog transaction commits.
    ///
    /// # Errors
    /// An error stops the commit here.
    fn after_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        let _ = operation_id;
        Ok(())
    }
}

/// The host's probe: it never stops a commit.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoProbe;

impl CommitProbe for NoProbe {}

/// One open library: its catalog file and its canonical OCFL storage.
struct OpenLibrary {
    id: LibraryId,
    conn: Connection,
    canonical: canonical::CanonicalRoot,
}

/// The library catalogs of one catalog directory, one SQLite file per library. One mutex
/// serialises every read and commit, so a commit's receipt check across libraries and its
/// transaction see the same state.
pub struct SqliteCatalog {
    root: PathBuf,
    journal: Arc<dyn JournalStore>,
    probe: Arc<dyn CommitProbe>,
    provenance: Option<ProvenanceContext>,
    failures: Option<JournalSender>,
    libraries: Mutex<BTreeMap<String, OpenLibrary>>,
}

impl std::fmt::Debug for SqliteCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteCatalog").finish_non_exhaustive()
    }
}

impl SqliteCatalog {
    /// Open the catalog directory `root` (created owner-only if missing): every library that
    /// has a catalog file or an OCFL root in it is opened, its catalog migrated to
    /// [`CATALOG_SCHEMA_VERSION`] and reconciled with its snapshot head (rebuilt from it when
    /// behind or missing); then journal entries a crash left in the outboxes are forwarded to
    /// `journal`, the host's activity journal store. A catalog file without a library row and
    /// without a snapshot (a create that never committed), or whose row names another library
    /// than its file name, is left closed. Filesystem and database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Unavailable`] when the directory or a file cannot be opened;
    /// [`CatalogError::Failed`] when it holds more than [`MAX_LIBRARIES`] catalogs or a file's
    /// schema is newer than this host.
    pub fn open(root: &Path, journal: Arc<dyn JournalStore>) -> Result<Self, CatalogError> {
        Self::open_with_probe(root, journal, Arc::new(NoProbe))
    }

    /// [`Self::open`] with a probe that can stop commits as a crash would (crash-injection
    /// tests). Filesystem and database I/O.
    ///
    /// # Errors
    /// As [`Self::open`].
    pub fn open_with_probe(
        root: &Path,
        journal: Arc<dyn JournalStore>,
        probe: Arc<dyn CommitProbe>,
    ) -> Result<Self, CatalogError> {
        create_root(root)?;
        let libraries = open_existing(root)?;
        let catalog = Self {
            root: root.to_path_buf(),
            journal,
            probe,
            provenance: None,
            failures: None,
            libraries: Mutex::new(libraries),
        };
        catalog.forward_all()?;
        Ok(catalog)
    }

    /// The same catalog recording the PROV-O history of every commit it publishes (Task
    /// DE-0.10): each snapshot version names `context`'s installation as its OCFL `user` and
    /// carries the operation's PROV-O record and journal entries, whose IDs the host journal
    /// allocates before the commit. Without it, versions still name their operation and kind,
    /// and the outbox still stores the `ocfl.commit` entry linked to each version.
    #[must_use]
    pub fn with_provenance(mut self, context: ProvenanceContext) -> Self {
        self.provenance = Some(context);
        self
    }

    /// The same catalog journaling the refusals that come before any library is open (an
    /// unknown library, a poisoned lock), which have no outbox to hold them, through
    /// `failures`: the host's bounded journal queue, whose producers wait and whose entries
    /// are never dropped. Without it, such a refusal is appended straight to the journal
    /// store, and one the store refuses is reported on stdout only.
    #[must_use]
    pub fn with_failure_journal(mut self, failures: JournalSender) -> Self {
        self.failures = Some(failures);
        self
    }

    /// Store `files` (each with its listing order) in `library_id`'s catalog, as an import
    /// does, publishing their entities as one snapshot version and storing `journal` in the
    /// same transaction. Files already stored keep their hide and delete state. The revision
    /// (the command token) is not advanced. A refresh that changes nothing (the same files
    /// again, or none) publishes no version and stores no journal entry, like a command that
    /// changes nothing (owner decision 2026-10-19). Database and filesystem I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library, a file of another library or a
    /// database or engine failure; [`CatalogError::Unavailable`] for storage trouble; nothing
    /// is stored.
    pub fn index_files(
        &self,
        library_id: &LibraryId,
        files: &[(File, i64)],
        journal: JournalEntry,
    ) -> Result<(), CatalogError> {
        let keys = files
            .iter()
            .map(|(file, _)| SnapshotKey::File(file.id().clone()))
            .collect();
        self.write_journaled(library_id, journal, keys, |tx| {
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
        let keys = folders
            .iter()
            .map(|folder| SnapshotKey::Folder(folder.id().clone()))
            .collect();
        self.write_journaled(library_id, journal, keys, |tx| {
            write::index_folders(tx, library_id, folders)
        })
    }

    /// Record `intent` in its library's catalog before canonical work starts; the commit of
    /// its operation ID removes it. An intent whose operation is already committed (its
    /// receipt is stored) is not stored, so no later commit is needed to clear it.
    /// Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    pub fn record_intent(&self, intent: &PendingIntent) -> Result<(), CatalogError> {
        let libraries = self.lock()?;
        let library = open_library(&libraries, &intent.library_id)?;
        let committed = rows::receipt(
            &library.conn,
            &intent.library_id,
            intent.operation_id.as_str(),
        )?;
        if committed.is_some() {
            return Ok(());
        }
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

    /// Write an index refresh of `library_id` with [`Self::write_indexed`], then forward the
    /// outbox. A refresh that fails is journaled as `failed` with its error code (in the
    /// library's outbox, in a transaction of its own, or through the host's failure journal
    /// when the library is not open) and its error returned; a refresh that changes nothing stores
    /// no journal entry (owner decision 2026-10-19, DE-S-010). Database, filesystem and
    /// journal-store I/O.
    fn write_journaled(
        &self,
        library_id: &LibraryId,
        journal: JournalEntry,
        keys: Vec<SnapshotKey>,
        write: impl FnOnce(&Transaction<'_>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        let refused = |err| self.journal_refused_refresh(library_id, &journal, err);
        let mut libraries = self.lock().map_err(refused)?;
        let library = open_library_mut(&mut libraries, library_id).map_err(refused)?;
        let result = self.write_indexed(library, library_id, &journal, keys, write);
        if let Err(err) = &result {
            journal_failed_refresh(&mut library.conn, library_id, &journal, err);
        }
        self.forward_logged(&library.conn);
        result
    }

    /// Journal the `failed` entry of an index refresh of `library_id` that was refused before
    /// its library was open (no outbox to hold it), and answer `err`: through the host's
    /// bounded failure queue ([`Self::with_failure_journal`]; it blocks while the queue is
    /// full and never drops), or, without one, straight to the journal store, logging to
    /// stdout an entry the store refuses. Journal-queue or journal-store I/O.
    fn journal_refused_refresh(
        &self,
        library_id: &LibraryId,
        journal: &JournalEntry,
        err: CatalogError,
    ) -> CatalogError {
        let entry = failed_refresh(library_id, journal, &err);
        if let Some(failures) = &self.failures {
            failures.send(entry);
            return err;
        }
        if let Err(store) = self.journal.append(&[entry]) {
            tracing::warn!(
                target: LOG_TARGET,
                operation = %journal.operation,
                error = ?store,
                "the refusal of an index refresh could not be journaled"
            );
        }
        err
    }

    /// Run `write`, publish the snapshot version of the entities `keys` (every entity when
    /// the library has no snapshot yet), and store its version and `journal` in one
    /// transaction. A refresh that leaves the entities unchanged publishes no version and
    /// stores no journal entry. A failure after the version is published leaves the catalog
    /// behind its head, which reconcile brings forward at the next open. Database and
    /// filesystem I/O.
    fn write_indexed(
        &self,
        library: &mut OpenLibrary,
        library_id: &LibraryId,
        journal: &JournalEntry,
        keys: Vec<SnapshotKey>,
        write: impl FnOnce(&Transaction<'_>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        let tx = library.conn.transaction().map_err(sql)?;
        write(&tx)?;
        let delta = write::delta_of_keys(&tx, library_id, keys)?;
        let record = index_record(&tx, &journal.operation)?;
        let plan = history::plan_commit(self.history().as_ref(), &record, journal);
        let published = library
            .canonical
            .commit(&delta, plan.meta(), plan.bound())?;
        let entries = match &published {
            Some(published) => plan.published(published),
            None => Vec::new(),
        };
        if let Some(published) = published {
            reconcile::set_snapshot_version(&tx, &published.version.to_string())?;
        }
        for entry in &entries {
            outbox::push(&tx, entry)?;
        }
        tx.commit().map_err(sql)
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

    /// Create `commit`'s library: its catalog file and OCFL root, then apply the commit there.
    /// When the commit fails before anything could be published, the files and directories
    /// this call created are removed; otherwise they stay for reconcile. Filesystem and
    /// database I/O.
    fn create(
        &self,
        libraries: &mut BTreeMap<String, OpenLibrary>,
        commit: &CatalogCommit,
    ) -> Result<CommitReceipt, CatalogError> {
        if libraries.len() >= MAX_LIBRARIES {
            return Err(failed(format!(
                "a catalog directory holds at most {MAX_LIBRARIES} libraries"
            )));
        }
        let id = commit.library_id.clone();
        let path = schema::file_path(&self.root, &id)?;
        let existed = path.symlink_metadata().is_ok()
            || canonical::library_ocfl_root(&self.root, &id)?
                .symlink_metadata()
                .is_ok();
        match self.create_files(&path, commit) {
            Ok((conn, canonical, receipt)) => {
                self.forward_logged(&conn);
                libraries.insert(
                    id.as_str().to_string(),
                    OpenLibrary {
                        id,
                        conn,
                        canonical,
                    },
                );
                Ok(receipt)
            }
            Err(stopped) => {
                if !existed && !stopped.keep_intent {
                    remove_catalog_file(&path);
                    canonical::remove_canonical_dirs(&self.root, &id);
                }
                Err(stopped.error)
            }
        }
    }

    /// Open the new library's catalog file and OCFL root and apply `commit`. Filesystem and
    /// database I/O.
    fn create_files(
        &self,
        path: &Path,
        commit: &CatalogCommit,
    ) -> Result<(Connection, canonical::CanonicalRoot, CommitReceipt), write::Stopped> {
        let mut conn = schema::open_catalog_db(path).map_err(write::Stopped::discard)?;
        let canonical = canonical::CanonicalRoot::open(&self.root, &commit.library_id, true)
            .map_err(write::Stopped::discard)?;
        let receipt = write::apply_commit(&mut conn, commit, true, &self.canonical(&canonical))?;
        Ok((conn, canonical, receipt))
    }

    /// The canonical side of a commit to `root`, with this catalog's probe.
    fn canonical<'a>(&'a self, root: &'a canonical::CanonicalRoot) -> write::Canonical<'a> {
        write::Canonical {
            root,
            probe: self.probe.as_ref(),
            history: self.history(),
        }
    }

    /// Who records this catalog's commits, when the host configured provenance.
    fn history(&self) -> Option<history::History<'_>> {
        self.provenance.as_ref().map(|context| history::History {
            context,
            journal: self.journal.as_ref(),
        })
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

/// The operation record of an index write journaled as `operation`: a fresh operation ID at
/// the current revision.
/// Database I/O.
fn index_record(tx: &Transaction<'_>, operation: &str) -> Result<OperationRecord, CatalogError> {
    let operation_id = mint_uuid_v4()
        .map_err(|_| CatalogError::Unavailable("random source unavailable".into()))?;
    let revision = rows::library(tx)?
        .map(|(_, revision, _)| revision.to_string())
        .ok_or_else(|| failed("unknown library"))?;
    Ok(OperationRecord {
        operation_id,
        kind: Some(operation_kind(operation).as_str().to_string()),
        revision,
        created_id: None,
        deletion: None,
    })
}

/// The `failed` journal entry of the index refresh `journal` of `library_id` that stopped
/// with `err`: same operation, actor and time, targeting the library, with the error's code.
/// Pure.
fn failed_refresh(
    library_id: &LibraryId,
    journal: &JournalEntry,
    err: &CatalogError,
) -> JournalEntry {
    let level = match err {
        CatalogError::Conflict => Level::Warn,
        CatalogError::Unavailable(_) | CatalogError::Failed(_) => Level::Error,
    };
    JournalEntry::new(journal.operation.clone(), Outcome::Failed)
        .actor(journal.actor.clone())
        .at_ms(journal.time_ms)
        .level(level)
        .error_code(error_code(err))
        .target(library_id.as_str())
}

/// Push the `failed` entry of the index refresh `journal` of `library_id` into `conn`'s outbox; when even
/// that fails, log it to stdout (the refresh's own error is still returned). Database I/O.
fn journal_failed_refresh(
    conn: &mut Connection,
    library_id: &LibraryId,
    journal: &JournalEntry,
    err: &CatalogError,
) {
    if let Err(store) = journal_alone(conn, &failed_refresh(library_id, journal, err)) {
        tracing::warn!(
            target: LOG_TARGET,
            operation = %journal.operation,
            error_code = error_code(&store),
            "the failure of an index refresh could not be journaled"
        );
    }
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

/// The libraries of the catalog directory `root`, at most [`MAX_LIBRARIES`]: each one with a
/// catalog file or an OCFL root. Other files are ignored. Filesystem and database I/O.
fn open_existing(root: &Path) -> Result<BTreeMap<String, OpenLibrary>, CatalogError> {
    let ids = library_ids(root)?;
    if ids.len() > MAX_LIBRARIES {
        return Err(failed(format!(
            "catalog directory holds more than {MAX_LIBRARIES} libraries"
        )));
    }
    let mut libraries = BTreeMap::new();
    for (key, id) in ids {
        if let Some(library) = open_one(root, id)? {
            libraries.insert(key, library);
        }
    }
    Ok(libraries)
}

/// Open the library `id`: its catalog file (created when only its OCFL root exists) and its
/// canonical storage, reconciled with its snapshot. `None` when it is not served (see
/// [`SqliteCatalog::open`]); a library whose snapshot cannot be read is not served either,
/// and the failure is logged as a warning (which the journal layer journals).
/// Filesystem and database I/O.
fn open_one(root: &Path, id: LibraryId) -> Result<Option<OpenLibrary>, CatalogError> {
    let mut conn = schema::open_catalog_db(&schema::file_path(root, &id)?)?;
    let has_root = canonical::library_ocfl_root(root, &id)?.is_dir();
    if names_other_library(&conn, &id)? || (!has_root && !serves(&conn, &id)?) {
        return Ok(None);
    }
    let canonical = canonical::CanonicalRoot::open(root, &id, true)?;
    if let Err(err) = reconcile_library(&mut conn, &canonical, &id) {
        tracing::warn!(
            target: LOG_TARGET,
            operation = reconcile::RECONCILE_OPERATION,
            error_code = error_code(&err),
            "a library could not be reconciled with its snapshot; it is not served"
        );
        return Ok(None);
    }
    let served = serves(&conn, &id)?;
    Ok(served.then_some(OpenLibrary {
        id,
        conn,
        canonical,
    }))
}

/// Reconcile the catalog in `conn` with the snapshot head of `canonical`, if it has one,
/// replaying the journal entries of the versions the catalog missed. Filesystem and database
/// I/O.
fn reconcile_library(
    conn: &mut Connection,
    canonical: &canonical::CanonicalRoot,
    id: &LibraryId,
) -> Result<(), CatalogError> {
    let Some(snapshot) = canonical.read()? else {
        return forget_missing_snapshot(conn, id);
    };
    let seen = reconcile::snapshot_version(conn)?;
    let replay = match seen.as_deref() == Some(snapshot.version.to_string().as_str()) {
        true => Vec::new(),
        false => canonical.read_journal(seen.as_deref())?,
    };
    reconcile::reconcile(conn, id, &snapshot, &replay).map(drop)
}

/// Forget the snapshot version of a catalog whose OCFL root holds no snapshot, and log it as a
/// warning (which the journal layer journals): its next commit publishes every entity again.
/// Database I/O.
fn forget_missing_snapshot(conn: &Connection, id: &LibraryId) -> Result<(), CatalogError> {
    if reconcile::forget_snapshot(conn)? {
        tracing::warn!(
            target: LOG_TARGET,
            operation = reconcile::RECONCILE_OPERATION,
            error_code = "snapshot_missing",
            library = id.as_str(),
            "the library's OCFL root holds no snapshot; the next commit publishes all of it"
        );
    }
    Ok(())
}

/// Whether the catalog file named after `id` holds the row of another library (a renamed or
/// copied file); it is then never served or changed under the wrong identifier, and a
/// warning is logged. Database I/O.
fn names_other_library(conn: &Connection, id: &LibraryId) -> Result<bool, CatalogError> {
    let Some((record, _, _)) = rows::library(conn)? else {
        return Ok(false);
    };
    if record.library.id() == id {
        return Ok(false);
    }
    tracing::warn!(
        target: LOG_TARGET,
        operation = "library.catalog.open",
        error_code = "conflict",
        "a catalog file names another library; it is not served"
    );
    Ok(true)
}

/// Whether the catalog in `conn` holds a library row (the create committed). Database I/O.
fn serves(conn: &Connection, id: &LibraryId) -> Result<bool, CatalogError> {
    Ok(rows::library(conn)?.is_some_and(|(record, _, _)| record.library.id() == id))
}

/// The libraries that have a catalog file or an OCFL root in `root`, by identifier.
/// Filesystem I/O.
fn library_ids(root: &Path) -> Result<BTreeMap<String, LibraryId>, CatalogError> {
    let unavailable =
        |e: std::io::Error| CatalogError::Unavailable(format!("read catalog directory: {e}"));
    let mut ids = BTreeMap::new();
    for entry in std::fs::read_dir(root).map_err(unavailable)? {
        let name = entry.map_err(unavailable)?.file_name();
        let name = name.to_string_lossy();
        let id = schema::library_of(&name).or_else(|| canonical::library_of_root(&name));
        if let Some(id) = id {
            ids.insert(id.as_str().to_string(), id);
        }
    }
    Ok(ids)
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
    crate::owner_only::create_owner_only(dest)
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
        let canonical = self.canonical(&library.canonical);
        let receipt = write::apply_commit(&mut library.conn, &commit, false, &canonical)
            .map_err(|stopped| stopped.error)?;
        self.forward_logged(&library.conn);
        Ok(receipt)
    }
}
