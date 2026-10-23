//! A library's canonical OCFL storage root: the folder the owner chose for it (Tasks DE-0.3,
//! DE-1.7; DE-S-010, DE-S-018,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)).
//! The folder holds the library's collection snapshot and one object per file; the catalog
//! commits its snapshot versions there through the same backend and fenced version leases as
//! the file routes ([`crate::locations::HostRoot`]), so one folder never has two sets of
//! leases. `<catalog dir>/<library-id>.work/` holds only the change model's staging area,
//! outside the storage root as the engine requires (OCFL-S-016). The data directory never
//! holds a library's storage root, originals or metadata records: the in-app roots
//! `<library-id>.ocfl/` of earlier builds are retired, with no migration (owner statement
//! 2026-10-22). The layout and encoding of what is stored are `filesquid-api`'s
//! ([`filesquid_api::canonical`]); this module only drives the API's commits and reads.

use super::{failed, schema};
use crate::locations::HostRoot;
use filesquid_api::canonical::{
    commit_snapshot_with, create_storage_root, read_commit_journal, read_snapshot,
    snapshot_object_id, unregistered_file_objects, CanonicalError, LibrarySnapshot, OwnerReadable,
    SnapshotDelta, SNAPSHOT_ROOT,
};
use filesquid_api::history::backfill::{Backfill, BackfillError, BackfillReport};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::CatalogError;
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use filesquid_ocfl::write::{CommitReceipt, VersionBoundFiles};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Suffix of a library's working directory (staging) in the catalog directory.
pub const WORK_DIR_SUFFIX: &str = ".work";

/// The reservation holder this host writes under.
const HOLDER: &str = "filesquid-desktop";

/// `<root>/<library-id><suffix>` for a canonical UUID v4 identifier. Pure.
fn sibling(root: &Path, library_id: &LibraryId, suffix: &str) -> Result<PathBuf, CatalogError> {
    schema::file_name(library_id)
        .map(|_| root.join(format!("{}{suffix}", library_id.as_str())))
        .ok_or_else(|| failed("library identifier is not a UUID v4"))
}

/// A storage-layer failure as a catalog error. The detail never names a path.
fn unavailable(what: &str) -> impl Fn(filesquid_ocfl::storage::StorageError) -> CatalogError + '_ {
    move |_| CatalogError::Unavailable(format!("{what} unavailable"))
}

/// The backends of one library's canonical storage.
pub(crate) struct CanonicalRoot {
    ocfl: Arc<FsBackend>,
    staging: FsBackend,
    leases: Arc<crate::HostLeases>,
    object_id: String,
}

impl CanonicalRoot {
    /// The canonical storage of `library_id` in the owner's folder `folder`, with its staging
    /// area in `<root>/<library-id>.work/` (created owner-only). The folder's storage-root
    /// declaration is written when the folder is empty (refused when it holds anything else).
    /// Filesystem I/O.
    ///
    /// # Errors
    /// [`CatalogError::Unavailable`] when a directory or backend cannot be opened;
    /// [`CatalogError::Failed`] when the storage root cannot be declared.
    pub(crate) fn open(
        root: &Path,
        library_id: &LibraryId,
        folder: &HostRoot,
    ) -> Result<Self, CatalogError> {
        let work = sibling(root, library_id, WORK_DIR_SUFFIX)?;
        create_owner_dir(root, &work)?;
        let staging = FsBackend::new(work.join("staging"), work.join("staging-scratch"))
            .map_err(unavailable("staging"))?;
        let canonical = Self {
            ocfl: folder.backend.clone(),
            staging,
            leases: folder.reservations.clone(),
            object_id: snapshot_object_id(library_id),
        };
        canonical.declare()?;
        Ok(canonical)
    }

    /// Write the storage-root declaration unless the root already has one. Filesystem I/O.
    fn declare(&self) -> Result<(), CatalogError> {
        if self.is_declared()? {
            return Ok(());
        }
        drive(create_storage_root(self.ocfl.as_ref()))?.map_err(CatalogError::from)
    }

    /// Whether the storage root's declaration exists. Filesystem I/O.
    fn is_declared(&self) -> Result<bool, CatalogError> {
        use filesquid_ocfl::storage::{StorageBackend, StorageKey};
        let name = filesquid_ocfl::Declaration::storage_root(filesquid_ocfl::SpecVersion::V1_1)
            .file_name();
        let key = StorageKey::new(&name).map_err(unavailable("OCFL root"))?;
        drive(self.ocfl.exists(&key))?.map_err(unavailable("OCFL root"))
    }

    /// The storage root's backend, shared with the file routes, for the history routes.
    pub(crate) fn backend(&self) -> Arc<FsBackend> {
        self.ocfl.clone()
    }

    /// Commit `delta` as the snapshot version described by `meta` (its operation message,
    /// installation `user` and creation time), with `bound` writing the commit's history files
    /// into the same version; `None` when the delta changes nothing, so no version is
    /// written. Filesystem I/O.
    ///
    /// # Errors
    /// As [`commit_snapshot_with`], mapped to the catalog port.
    pub(crate) fn commit(
        &self,
        delta: &SnapshotDelta,
        meta: VersionMeta,
        bound: Option<&dyn VersionBoundFiles>,
    ) -> Result<Option<CommitReceipt>, CatalogError> {
        let area = StagingArea::new(&self.staging, "snapshot")
            .map_err(|e| CatalogError::from(CanonicalError::from(e)))?;
        let model = ChangeModel::new(
            self.ocfl.as_ref(),
            self.leases.as_ref(),
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
    /// pre-allocated IDs: what a catalog that missed those commits replays. Filesystem I/O.
    ///
    /// # Errors
    /// As [`read_commit_journal`], mapped to the catalog port; [`CatalogError::Failed`] for a
    /// line that is not a journal entry, so the catalog never advances past history it did
    /// not replay.
    pub(crate) fn read_journal(
        &self,
        after: Option<&str>,
    ) -> Result<Vec<JournalEntry>, CatalogError> {
        let after = after.and_then(|name| filesquid_ocfl::VersionNum::parse(name).ok());
        let lines = drive(read_commit_journal(
            self.ocfl.as_ref(),
            &OwnerReadable,
            after.as_ref(),
        ))?
        .map_err(CatalogError::from)?;
        lines.iter().map(|line| journal_line(line)).collect()
    }

    /// Run `backfill` once over this library's storage root, under the same version
    /// reservations as the catalog's own commits, until it is done, capped or `cancel` is set
    /// (Task FS-2.8 host wiring). Filesystem I/O; the backends' futures never wait.
    ///
    /// # Errors
    /// [`BackfillError::Unavailable`] when the run cannot start, or its future waited.
    pub(crate) fn backfill(
        &self,
        backfill: &Backfill,
        cancel: &AtomicBool,
    ) -> Result<BackfillReport, BackfillError> {
        complete_immediately(backfill.run(self.ocfl.as_ref(), self.leases.as_ref(), cancel))
            .map_err(|_| BackfillError::Unavailable)?
    }

    /// The snapshot at the head, or `None` when nothing was published yet. Filesystem I/O.
    ///
    /// # Errors
    /// As [`read_snapshot`], mapped to the catalog port.
    pub(crate) fn read(&self) -> Result<Option<LibrarySnapshot>, CatalogError> {
        drive(read_snapshot(self.ocfl.as_ref(), &OwnerReadable))?.map_err(CatalogError::from)
    }

    /// How many file objects of the folder `snapshot` does not register (all of them when
    /// there is none): counted by a rebuild, never placed in a collection (ADR 041 Decision
    /// 6). Filesystem I/O.
    ///
    /// # Errors
    /// As [`unregistered_file_objects`], mapped to the catalog port.
    pub(crate) fn unregistered(
        &self,
        snapshot: Option<&LibrarySnapshot>,
    ) -> Result<u64, CatalogError> {
        drive(unregistered_file_objects(self.ocfl.as_ref(), snapshot))?.map_err(CatalogError::from)
    }
}

/// The journal entry one line holds, redacted. Pure apart from the warning it logs (stdout)
/// for an unreadable line, so operators see that code rather than a generic failure.
///
/// # Errors
/// [`CatalogError::Failed`] (`journal_line_unreadable`) when the line holds none.
fn journal_line(line: &str) -> Result<JournalEntry, CatalogError> {
    serde_json::from_str::<JournalEntry>(line)
        .map(JournalEntry::redacted)
        .map_err(|_| {
            tracing::warn!(
                target: super::LOG_TARGET,
                operation = super::reconcile::RECONCILE_OPERATION,
                error_code = "journal_line_unreadable",
                "a commit's journal line is unreadable; the catalog is not reconciled"
            );
            failed("journal_line_unreadable: a commit's journal line is unreadable")
        })
}

/// Run a backend future that never waits (the filesystem backend's futures complete at once).
fn drive<F: std::future::Future>(future: F) -> Result<F::Output, CatalogError> {
    complete_immediately(future).map_err(|_| failed("a storage future did not complete"))
}

/// Create `dir` if missing and make it owner-only (0700) on Unix. A symbolic link at `dir`,
/// or a `dir` that does not resolve to a direct child of the catalog directory `root`, is
/// refused before any backend opens it, so no staging is read or written outside the catalog
/// directory. Filesystem I/O; the error names no path.
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
