//! Attaching a library's folder to its catalog index (Task DE-1.7; DE-S-009, DE-S-010,
//! DE-S-018, [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 1, 2 and 6).
//!
//! When the owner creates or opens a library in a folder, the host attaches it: the index
//! `<catalog dir>/<library-id>.sqlite` is opened (created when missing, migrated forward) and
//! brought to the folder's collection snapshot in one journaled `library.reconcile`. An index
//! that is missing, was built from another copy of the folder (a moved or copied library has
//! another folder identity) or is behind the snapshot head is rebuilt from it; the file objects
//! of the folder that the snapshot does not register are counted in that entry and stay
//! unregistered. A folder without a snapshot (a library the owner just created, or one an
//! earlier build created without a collection snapshot) gets its library entity, published as
//! the first snapshot version and journaled as `library.create`; an index built from another
//! folder is emptied first, so nothing of that folder is served from this one.

use super::intent::{self, PendingIntent};
use super::{
    canonical, error_code, failed, library_ids, names_other_library, reconcile, reconcile_library,
    rows, schema, write, OpenLibrary, SqliteCatalog, LOG_TARGET, MAX_LIBRARIES,
};
use crate::locations::HostRoot;
use filesquid_api::canonical::SnapshotKey;
use filesquid_api::journal::{now_ms, JournalEntry, Outcome};
use filesquid_api::library::{mint_uuid_v4, CatalogError, LibraryRecord, NEW_LIBRARY_REVISION};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{Library, LibraryId, RepositoryId};
use rusqlite::{params, Connection};

/// The name a library gets when its folder holds no snapshot yet; the owner can rename it.
pub const DEFAULT_LIBRARY_NAME: &str = "Library";

/// Journal operation of creating a library's entity in its folder.
pub const CREATE_OPERATION: &str = "library.create";

/// A library folder the host attaches: where it lives and who it belongs to.
#[derive(Clone)]
pub struct LibraryFolder {
    /// The library: the one its snapshot names, or the ID its create minted.
    pub library_id: LibraryId,
    /// The repository (this installation) a new library entity names.
    pub repository_id: RepositoryId,
    /// The name a new library entity gets.
    pub name: String,
    /// The folder's storage backend and the fenced version leases every writer of it shares.
    pub root: HostRoot,
    /// An opaque identity of the canonical folder (no path), so an index built from another
    /// copy of the library is rebuilt.
    pub identity: String,
}

impl std::fmt::Debug for LibraryFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryFolder")
            .field("library_id", &self.library_id)
            .finish_non_exhaustive()
    }
}

/// What attaching a folder did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttachReport {
    /// The folder held no snapshot and no index named the library: its entity was created.
    pub created: bool,
    /// The index was rebuilt from the folder's snapshot.
    pub rebuilt: bool,
    /// File objects of the folder the snapshot does not register (counted when the index was
    /// created or rebuilt, else 0).
    pub unregistered: u64,
}

impl SqliteCatalog {
    /// Serve the library of `folder`: open, create or rebuild its index as the module
    /// documentation says, and forward its journal outbox. Attaching the same folder again
    /// answers an empty report. Filesystem, database and journal I/O.
    ///
    /// # Errors
    /// [`CatalogError::Conflict`] when the library is already attached from another folder;
    /// [`CatalogError::Failed`] for more than [`MAX_LIBRARIES`] libraries, an index of another
    /// library, a snapshot of another library or a schema newer than this host;
    /// [`CatalogError::Unavailable`] for storage trouble. Nothing is served then.
    pub fn attach(&self, folder: &LibraryFolder) -> Result<AttachReport, CatalogError> {
        let mut libraries = self.lock()?;
        if let Some(open) = libraries.get(folder.library_id.as_str()) {
            return match open.identity == folder.identity {
                true => Ok(AttachReport::default()),
                false => Err(CatalogError::Conflict),
            };
        }
        if libraries.len() >= MAX_LIBRARIES || self.directory_full(&folder.library_id)? {
            return Err(failed(format!(
                "a catalog serves at most {MAX_LIBRARIES} libraries"
            )));
        }
        let (library, report) = self.open_folder(folder).inspect_err(warn_unattached)?;
        self.forward_logged(&library.conn);
        libraries.insert(folder.library_id.as_str().to_string(), library);
        Ok(report)
    }

    /// Whether a new index for `library_id` would put more than [`MAX_LIBRARIES`] catalog
    /// files in the catalog directory, which the next launch would then refuse to open: its
    /// file is not there yet and the directory already holds that many. Reads the directory.
    fn directory_full(&self, library_id: &LibraryId) -> Result<bool, CatalogError> {
        let ids = library_ids(&self.root)?;
        Ok(!ids.contains_key(library_id.as_str()) && ids.len() >= MAX_LIBRARIES)
    }

    /// Open `folder`'s index and canonical storage and bring the index to its snapshot.
    /// Filesystem and database I/O.
    fn open_folder(
        &self,
        folder: &LibraryFolder,
    ) -> Result<(OpenLibrary, AttachReport), CatalogError> {
        let id = &folder.library_id;
        let canonical = canonical::CanonicalRoot::open(&self.root, id, &folder.root)?;
        let mut conn = schema::open_catalog_db(&schema::file_path(&self.root, id)?)?;
        if names_other_library(&conn, id)? {
            return Err(failed("a catalog file names another library"));
        }
        let snapshot = canonical.read()?;
        if rows::folder_identity(&conn)?.as_deref() != Some(folder.identity.as_str()) {
            match snapshot.is_some() {
                true => reconcile::forget_snapshot(&conn).map(drop)?,
                false => reconcile::clear_index(&mut conn)?,
            }
        }
        let report = match rows::library(&conn)?.is_some() || snapshot.is_some() {
            true => rebuilt(reconcile_library(&mut conn, &canonical, id)?),
            false => self.create_entity(&mut conn, &canonical, folder)?,
        };
        write::set_folder_identity(&conn, &folder.identity)?;
        let library = OpenLibrary {
            id: id.clone(),
            conn,
            canonical,
            identity: folder.identity.clone(),
        };
        Ok((library, report))
    }

    /// Create the library entity of a folder without a snapshot, in the commit sequence of
    /// DE-S-010: its intent first, then its row at revision 1 and its first snapshot version,
    /// journaled as [`CREATE_OPERATION`], with the intent's removal; file objects an earlier
    /// build left there are counted in a `library.reconcile` entry and stay unregistered. A
    /// create that fails discards its intent. Filesystem, database and journal-store I/O.
    fn create_entity(
        &self,
        conn: &mut Connection,
        canonical: &canonical::CanonicalRoot,
        folder: &LibraryFolder,
    ) -> Result<AttachReport, CatalogError> {
        let unregistered = canonical.unregistered(None)?;
        let id = &folder.library_id;
        let intent = create_intent(id)?;
        intent::record(conn, &intent)?;
        let op = intent.operation_id.as_str().to_string();
        let record = LibraryRecord {
            library: Library::new(id.clone(), folder.repository_id.clone()),
            name: folder.name.clone(),
        };
        let journal = JournalEntry::new(CREATE_OPERATION, Outcome::Succeeded)
            .at_ms(intent.recorded_at_ms)
            .operation_id(op.as_str())
            .target(id.as_str());
        let keys = vec![SnapshotKey::Library];
        let created = self.write_indexed(conn, canonical, id, &journal, keys, |tx| {
            write::insert_library(tx, &record, intent.recorded_at_ms)?;
            tx.execute("UPDATE library SET revision = 1", params![])
                .map_err(super::rows::sql)?;
            intent::clear(tx, &op)
        });
        if created.is_err() {
            discard_logged(conn, &op);
        }
        created?;
        if unregistered > 0 {
            let version = reconcile::snapshot_version(conn)?.unwrap_or_default();
            let entry = reconcile::reconcile_entry(id, &version, unregistered);
            super::journal_alone(conn, &entry)?;
        }
        Ok(AttachReport {
            created: true,
            rebuilt: false,
            unregistered,
        })
    }
}

/// The intent of creating `library_id`'s entity: a fresh operation ID at the new-library
/// revision, recorded now. Reads the clock and the random source.
fn create_intent(library_id: &LibraryId) -> Result<PendingIntent, CatalogError> {
    let raw = mint_uuid_v4()
        .map_err(|_| CatalogError::Unavailable("random source unavailable".into()))?;
    Ok(PendingIntent {
        library_id: library_id.clone(),
        operation_id: OperationId::new(raw).map_err(|_| failed("empty operation ID"))?,
        operation: CREATE_OPERATION.to_string(),
        expected_revision: ExpectedRevision::new(NEW_LIBRARY_REVISION)
            .map_err(|_| failed("empty revision"))?,
        recorded_at_ms: now_ms(),
    })
}

/// Discard the intent of a create that failed; a failure to do so is logged with its code
/// and leaves the intent pending. Database I/O.
fn discard_logged(conn: &Connection, operation_id: &str) {
    if let Err(err) = intent::discard(conn, operation_id) {
        tracing::warn!(
            target: LOG_TARGET,
            operation = CREATE_OPERATION,
            error_code = error_code(&err),
            "the intent of a failed library create was kept"
        );
    }
}

/// The report of a reconcile that rebuilt the index (`Some`, with the unregistered count) or
/// found it current (`None`). Pure.
fn rebuilt(unregistered: Option<u64>) -> AttachReport {
    AttachReport {
        created: false,
        rebuilt: unregistered.is_some(),
        unregistered: unregistered.unwrap_or(0),
    }
}

/// Log that a folder could not be attached, with its error code only (the journal layer
/// journals the warning).
fn warn_unattached(err: &CatalogError) {
    tracing::warn!(
        target: LOG_TARGET,
        operation = reconcile::RECONCILE_OPERATION,
        error_code = error_code(err),
        "a library folder could not be attached; it is not served"
    );
}
