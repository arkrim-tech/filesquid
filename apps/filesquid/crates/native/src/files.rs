//! The file service every host installs (Task API-F1.3): `filesquid-api`'s
//! [`FileService`] over OCFL storage roots on the local filesystem.
//!
//! The API owns what each file operation means and does no storage I/O
//! ([API-S-006](../../api/resources/srs.md)); this module gives it the filesystem backends: the
//! library's storage root, fenced `vN` leases and a staging area, the last two in the host's
//! data directory under `files/<library>/`, never inside the root. A host installs the
//! service once at startup ([`install_file_service`]) and opens each library it serves with
//! [`open_fs_library`] (DE-1.1 on the desktop, the server's library tasks on
//! `filesquid-server`).

use filesquid_api::files::{FileLibrary, FileService};
use filesquid_api::ApiState;
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::path::Path;
use std::sync::Arc;

/// The file service of a host: local OCFL roots, leases and staging on the filesystem.
pub type HostFiles = FileService<FsBackend, HostLeases, FsBackend>;

/// The fenced `vN` leases of one library, in the host's data directory.
pub type HostLeases = LeaseStore<FsBackend, SystemClock>;

/// The data-directory folder that holds each library's leases and staging area.
pub const FILES_DIR: &str = "files";

/// The key prefix of the API's staging areas on a library's staging backend.
pub const STAGING_PREFIX: &str = "api";

/// Install an empty [`HostFiles`] in `state` and return it, so the host can open libraries
/// later. Until a library is opened, every file route answers `not_found`.
///
/// # Errors
///
/// A message when `state` already has a file service.
pub fn install_file_service(state: &ApiState) -> Result<HostFiles, String> {
    let files = HostFiles::new();
    if !state.install_files(Arc::new(files.clone())) {
        return Err("a file service is already installed".to_string());
    }
    Ok(files)
}

/// Serve the OCFL storage root at `root` as `library_id` through `files`, with its leases and
/// staging area under `<data_dir>/files/<library_id>/`. Returns false, changing nothing, when
/// the library is already open. Creates directories; reads and writes nothing else.
///
/// # Errors
///
/// A message naming what could not be prepared (a directory, or a backend whose staging would
/// lie inside the root); it never names a path.
pub fn open_fs_library(
    files: &HostFiles,
    library_id: &LibraryId,
    root: &Path,
    data_dir: &Path,
) -> Result<bool, String> {
    let dir = data_dir.join(FILES_DIR).join(library_id.as_str());
    let backend = backend(root, &dir.join("root-parts"), "the storage root")?;
    let lease_root = backend_at(&dir, "leases", "the lease store")?;
    let leases = LeaseStore::new(lease_root, "leases", SystemClock, LeaseConfig::default())
        .map_err(|err| format!("the lease store could not be prepared: {err}"))?;
    let staging = backend_at(&dir, "staging", "the staging area")?;
    let library = FileLibrary {
        backend: Arc::new(backend),
        reservations: Arc::new(leases),
        staging: Arc::new(staging),
        staging_prefix: STAGING_PREFIX.to_string(),
    };
    Ok(files.open_library(library_id, library))
}

/// A filesystem backend at `<dir>/<name>`, with its multipart parts beside it.
fn backend_at(dir: &Path, name: &str, what: &str) -> Result<FsBackend, String> {
    let root = dir.join(name);
    std::fs::create_dir_all(&root).map_err(|_| format!("{what} could not be created"))?;
    backend(&root, &dir.join(format!("{name}-parts")), what)
}

/// A filesystem backend over `root` with its parts under `parts`.
fn backend(root: &Path, parts: &Path, what: &str) -> Result<FsBackend, String> {
    FsBackend::new(root, parts).map_err(|_| format!("{what} could not be opened"))
}
