//! A library's canonical OCFL storage root on the owner's disk, beside its catalog file
//! (Task DE-0.3): `<catalog dir>/<library-id>.ocfl/` holds the library's collection snapshot
//! (and later its file objects); `<catalog dir>/<library-id>.work/` holds the change model's
//! staging area, the version reservations (leases) and the backends' scratch space, all
//! outside the storage root as the engine requires (OCFL-S-016, OCFL-S-018). The layout and
//! encoding of what is stored are `filesquid-api`'s ([`filesquid_api::canonical`]); this
//! module only opens the backends and drives the API's commits and reads.

use super::{failed, schema};
use filesquid_api::canonical::{
    commit_snapshot_with, create_storage_root, read_commit_journal, read_snapshot,
    snapshot_object_id, CanonicalError, LibrarySnapshot, OwnerReadable, SnapshotDelta,
    SNAPSHOT_ROOT,
};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::CatalogError;
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use filesquid_ocfl::write::{
    CommitReceipt, LeaseConfig, LeaseStore, SystemClock, VersionBoundFiles,
};
use std::path::{Path, PathBuf};

/// Suffix of a library's OCFL storage root in the catalog directory.
pub const OCFL_ROOT_SUFFIX: &str = ".ocfl";

/// Suffix of a library's working directory (staging, leases) in the catalog directory.
pub const WORK_DIR_SUFFIX: &str = ".work";

/// The reservation holder this host writes under.
const HOLDER: &str = "filesquid-desktop";

/// The OCFL storage root of `library_id` in the catalog directory `root`. Pure.
///
/// # Errors
/// [`CatalogError::Failed`] when the identifier is not a canonical UUID v4.
pub fn library_ocfl_root(root: &Path, library_id: &LibraryId) -> Result<PathBuf, CatalogError> {
    sibling(root, library_id, OCFL_ROOT_SUFFIX)
}

/// `<root>/<library-id><suffix>` for a canonical UUID v4 identifier. Pure.
fn sibling(root: &Path, library_id: &LibraryId, suffix: &str) -> Result<PathBuf, CatalogError> {
    schema::file_name(library_id)
        .map(|_| root.join(format!("{}{suffix}", library_id.as_str())))
        .ok_or_else(|| failed("library identifier is not a UUID v4"))
}

/// The library an OCFL root directory name belongs to, or `None` for any other name. Pure.
pub(crate) fn library_of_root(name: &str) -> Option<LibraryId> {
    let stem = name.strip_suffix(OCFL_ROOT_SUFFIX)?;
    schema::library_of(&format!("{stem}.{}", super::CATALOG_FILE_EXTENSION))
}

/// A storage-layer failure as a catalog error. The detail never names a path.
fn unavailable(what: &str) -> impl Fn(filesquid_ocfl::storage::StorageError) -> CatalogError + '_ {
    move |_| CatalogError::Unavailable(format!("{what} unavailable"))
}

/// The backends of one library's canonical storage.
pub(crate) struct CanonicalRoot {
    ocfl: FsBackend,
    staging: FsBackend,
    leases: LeaseStore<FsBackend, SystemClock>,
    object_id: String,
}

impl CanonicalRoot {
    /// Open (creating its directories owner-only, and its storage-root declaration when
    /// `create`) the canonical storage of `library_id` in the catalog directory `root`.
    /// Filesystem I/O.
    ///
    /// # Errors
    /// [`CatalogError::Unavailable`] when a directory or backend cannot be opened;
    /// [`CatalogError::Failed`] when the storage root cannot be declared.
    pub(crate) fn open(
        root: &Path,
        library_id: &LibraryId,
        create: bool,
    ) -> Result<Self, CatalogError> {
        let ocfl_dir = library_ocfl_root(root, library_id)?;
        let work = sibling(root, library_id, WORK_DIR_SUFFIX)?;
        for dir in [&ocfl_dir, &work] {
            create_owner_dir(root, dir)?;
        }
        let ocfl = FsBackend::new(&ocfl_dir, work.join("root-scratch"))
            .map_err(unavailable("OCFL root"))?;
        let staging = FsBackend::new(work.join("staging"), work.join("staging-scratch"))
            .map_err(unavailable("staging"))?;
        let lease_backend = FsBackend::new(work.join("leases"), work.join("leases-scratch"))
            .map_err(unavailable("leases"))?;
        let leases = LeaseStore::new(lease_backend, "leases", SystemClock, LeaseConfig::default())
            .map_err(|_| CatalogError::Unavailable("leases unavailable".into()))?;
        let canonical = Self {
            ocfl,
            staging,
            leases,
            object_id: snapshot_object_id(library_id),
        };
        if create {
            canonical.declare()?;
        }
        Ok(canonical)
    }

    /// Write the storage-root declaration unless the root already has one. Filesystem I/O.
    fn declare(&self) -> Result<(), CatalogError> {
        if self.is_declared()? {
            return Ok(());
        }
        drive(create_storage_root(&self.ocfl))?.map_err(CatalogError::from)
    }

    /// Whether the storage root's declaration exists. Filesystem I/O.
    fn is_declared(&self) -> Result<bool, CatalogError> {
        use filesquid_ocfl::storage::{StorageBackend, StorageKey};
        let name = filesquid_ocfl::Declaration::storage_root(filesquid_ocfl::SpecVersion::V1_1)
            .file_name();
        let key = StorageKey::new(&name).map_err(unavailable("OCFL root"))?;
        drive(self.ocfl.exists(&key))?.map_err(unavailable("OCFL root"))
    }

    /// Commit `delta` as the snapshot version described by `meta` (its operation message,
    /// installation `user` and creation time), with `bound` writing the commit's history files
    /// into the same version; `None` when the delta changes nothing, so no version is
    /// written. Filesystem I/O.
    ///
    /// # Errors
    /// As [`commit_snapshot`], mapped to the catalog port.
    pub(crate) fn commit(
        &self,
        delta: &SnapshotDelta,
        meta: VersionMeta,
        bound: Option<&dyn VersionBoundFiles>,
    ) -> Result<Option<CommitReceipt>, CatalogError> {
        let area = StagingArea::new(&self.staging, "snapshot")
            .map_err(|e| CatalogError::from(CanonicalError::from(e)))?;
        let model = ChangeModel::new(
            &self.ocfl,
            &self.leases,
            HOLDER,
            SNAPSHOT_ROOT,
            self.object_id.clone(),
            area,
        );
        drive(commit_snapshot_with(
            &model,
            &OwnerReadable,
            delta,
            meta,
            bound,
        ))?
        .map_err(CatalogError::from)
    }

    /// The journal entries the snapshot versions after `after` (every version when `None` or
    /// not a version name) wrote beside their PROV-O records, oldest first, keeping their
    /// pre-allocated IDs: what a catalog that missed those commits replays. A line that is not
    /// a journal entry is skipped and logged as a warning. Filesystem I/O.
    ///
    /// # Errors
    /// As [`read_commit_journal`], mapped to the catalog port.
    pub(crate) fn read_journal(
        &self,
        after: Option<&str>,
    ) -> Result<Vec<JournalEntry>, CatalogError> {
        let after = after.and_then(|name| filesquid_ocfl::VersionNum::parse(name).ok());
        let lines = drive(read_commit_journal(
            &self.ocfl,
            &OwnerReadable,
            after.as_ref(),
        ))?
        .map_err(CatalogError::from)?;
        Ok(lines.iter().filter_map(|line| journal_line(line)).collect())
    }

    /// The snapshot at the head, or `None` when nothing was published yet. Filesystem I/O.
    ///
    /// # Errors
    /// As [`read_snapshot`], mapped to the catalog port.
    pub(crate) fn read(&self) -> Result<Option<LibrarySnapshot>, CatalogError> {
        drive(read_snapshot(&self.ocfl, &OwnerReadable))?.map_err(CatalogError::from)
    }
}

/// The journal entry one line holds, or `None` (logged as a warning) when it holds none.
/// Pure apart from the log line.
fn journal_line(line: &str) -> Option<JournalEntry> {
    match serde_json::from_str::<JournalEntry>(line) {
        Ok(entry) => Some(entry.redacted()),
        Err(_) => {
            tracing::warn!(
                target: super::LOG_TARGET,
                operation = super::reconcile::RECONCILE_OPERATION,
                error_code = "journal_line_unreadable",
                "a commit's journal line is not a journal entry; it is not replayed"
            );
            None
        }
    }
}

/// Run a backend future that never waits (the filesystem backend's futures complete at once).
fn drive<F: std::future::Future>(future: F) -> Result<F::Output, CatalogError> {
    complete_immediately(future).map_err(|_| failed("a storage future did not complete"))
}

/// Create `dir` if missing and make it owner-only (0700) on Unix. A symbolic link at `dir`,
/// or a `dir` that does not resolve to a direct child of the catalog directory `root`, is
/// refused before any backend opens it, so no library data is read or written outside the
/// catalog directory. Filesystem I/O; the error names no path.
fn create_owner_dir(root: &Path, dir: &Path) -> Result<(), CatalogError> {
    let unavailable =
        |e: std::io::Error| CatalogError::Unavailable(format!("library directory: {e}"));
    if dir
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(failed("a library directory is a symbolic link"));
    }
    std::fs::create_dir_all(dir).map_err(unavailable)?;
    let resolved = std::fs::canonicalize(dir).map_err(unavailable)?;
    let parent = std::fs::canonicalize(root).map_err(unavailable)?;
    if resolved.parent() != Some(parent.as_path()) {
        return Err(failed(
            "a library directory resolves outside the catalog directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&resolved, std::fs::Permissions::from_mode(0o700))
            .map_err(unavailable)?;
    }
    Ok(())
}

/// Remove the OCFL root and working directory of `library_id` that a failed create made.
/// A failure is logged to stdout. Filesystem I/O.
pub(crate) fn remove_canonical_dirs(root: &Path, library_id: &LibraryId) {
    for suffix in [OCFL_ROOT_SUFFIX, WORK_DIR_SUFFIX] {
        let Ok(dir) = sibling(root, library_id, suffix) else {
            continue;
        };
        if let Err(err) = std::fs::remove_dir_all(&dir) {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    target: super::LOG_TARGET,
                    operation = "library.catalog.remove",
                    error_kind = ?err.kind(),
                    "an uncommitted library directory could not be removed"
                );
            }
        }
    }
}
