//! The folders a host's owner chose for libraries (Task DE-1.1; API-S-025): the native
//! [`LibraryLocations`] the hosts inject into `filesquid-api`'s library routes.
//!
//! A folder never crosses the API as a path. [`HostLocations::register`] resolves the folder
//! once (every symbolic link on the way resolved, the folder itself never a link), prepares
//! its storage backend and its fenced version leases under `<data dir>/locations/<digest>/`
//! (named by a digest of the canonical folder); registering one folder twice shares its
//! backend and leases, and answers an opaque `loc-<uuid>` ID. The library commands and the file
//! routes of a library opened there write through the same leases
//! ([`HostLocations::root_of`], [`crate::files::open_fs_library_root`]).
//!
//! Registrations live in memory for the host's lifetime; at most [`MAX_LOCATIONS`].

use crate::files::{real_dir, storage_root, HostLeases};
use filesquid_api::history::backfill::{Backfill, BackfillError, BackfillReport};
use filesquid_api::local_library::{LibraryLocations, LibraryRoot};
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The data-directory folder that holds each location's parts and leases.
pub const LOCATIONS_DIR: &str = "locations";

/// Most folders one host registers (AGENTS.md §22).
pub const MAX_LOCATIONS: usize = 256;

/// The prefix of every location ID.
pub const LOCATION_PREFIX: &str = "loc-";

/// The storage root of one location.
pub type HostRoot = LibraryRoot<FsBackend, HostLeases>;

/// One registered folder: its canonical path and its storage root.
struct Location {
    folder: PathBuf,
    root: HostRoot,
}

/// The folders registered on this host, by opaque location ID.
pub struct HostLocations {
    data_dir: PathBuf,
    locations: Mutex<HashMap<String, Location>>,
}

impl std::fmt::Debug for HostLocations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostLocations").finish_non_exhaustive()
    }
}

impl HostLocations {
    /// No locations yet; their leases will live under `<data_dir>/locations/`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            locations: Mutex::new(HashMap::new()),
        }
    }

    /// Register the folder the owner chose and answer its new opaque location ID. Creates
    /// the location's parts and lease folders in the data directory; reads nothing in the
    /// folder itself.
    ///
    /// # Errors
    ///
    /// A message that never names a path: the folder is not a plain directory, the data
    /// directory's folders could not be prepared, [`MAX_LOCATIONS`] are registered, or no
    /// ID could be minted.
    pub fn register(&self, folder: &Path) -> Result<String, String> {
        let folder = storage_root(folder)?;
        let known = self.root_of_folder(&folder)?;
        let root = match known {
            Some(root) => root,
            None => self.prepare_root(&folder)?,
        };
        let id = mint_location_id()?;
        let mut locations = self.lock()?;
        if locations.len() >= MAX_LOCATIONS {
            return Err("too many locations are registered".to_string());
        }
        locations.insert(id.clone(), Location { folder, root });
        Ok(id)
    }

    /// The storage root registered as `location_id`, if any.
    pub fn root_of(&self, location_id: &str) -> Option<HostRoot> {
        let locations = self.locations.lock().ok()?;
        locations.get(location_id).map(|found| found.root.clone())
    }

    /// The storage root already registered for the canonical `folder`, so a folder
    /// registered twice shares one backend and one set of leases.
    fn root_of_folder(&self, folder: &Path) -> Result<Option<HostRoot>, String> {
        let locations = self.lock()?;
        let found = locations.values().find(|known| known.folder == folder);
        Ok(found.map(|known| known.root.clone()))
    }

    /// The backend and leases of `folder` under `<data dir>/locations/<digest>/`. Creates
    /// directories.
    fn prepare_root(&self, folder: &Path) -> Result<HostRoot, String> {
        let base = self.data_dir.join(LOCATIONS_DIR);
        let dir = base.join(folder_digest(folder));
        real_dir(&base, "the locations folder")?;
        real_dir(&dir, "the location's folder")?;
        real_dir(&dir.join("root-parts"), "the location's parts folder")?;
        real_dir(&dir.join("leases"), "the location's lease store")?;
        real_dir(&dir.join("leases-parts"), "the location's lease store")?;
        let backend = FsBackend::new(folder, dir.join("root-parts"))
            .map_err(|_| "the storage root could not be opened".to_string())?;
        let lease_root = FsBackend::new(dir.join("leases"), dir.join("leases-parts"))
            .map_err(|_| "the lease store could not be opened".to_string())?;
        let leases = LeaseStore::new(lease_root, "leases", SystemClock, LeaseConfig::default())
            .map_err(|err| format!("the lease store could not be prepared: {err}"))?;
        Ok(LibraryRoot {
            backend: Arc::new(backend),
            reservations: Arc::new(leases),
        })
    }

    /// The canonical folder of `location_id`, if registered.
    fn folder_of(&self, location_id: &str) -> Option<PathBuf> {
        let locations = self.locations.lock().ok()?;
        locations.get(location_id).map(|found| found.folder.clone())
    }

    /// The locked registrations.
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Location>>, String> {
        self.locations
            .lock()
            .map_err(|_| "the location registry is unavailable".to_string())
    }
}

impl LibraryLocations for HostLocations {
    type Backend = FsBackend;
    type Reservations = HostLeases;

    fn location(&self, location_id: &str) -> Option<HostRoot> {
        self.root_of(location_id)
    }

    fn overlaps(&self, a: &str, b: &str) -> bool {
        match (self.folder_of(a), self.folder_of(b)) {
            (Some(a), Some(b)) => a.starts_with(&b) || b.starts_with(&a),
            _ => true,
        }
    }
}

/// A new opaque location ID, `loc-` and a UUID v4.
fn mint_location_id() -> Result<String, String> {
    filesquid_api::library::mint_uuid_v4()
        .map(|uuid| format!("{LOCATION_PREFIX}{uuid}"))
        .map_err(|_| "no location ID could be minted".to_string())
}

/// The folder name of `folder` under `locations/`: the first 32 hex digits of the SHA-256
/// digest of its canonical path. Pure.
fn folder_digest(folder: &Path) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(folder.as_os_str().as_encoded_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// The storage roots of the libraries opened in owner-chosen folders, by library ID, for the
/// history routes ([`crate::catalog::HostRoots`]) and the history backfill
/// ([`OpenedRoots::backfill`]). At most
/// [`MAX_OPEN_LIBRARIES`](filesquid_api::local_library::MAX_OPEN_LIBRARIES), the bound of
/// the library commands that open them.
#[derive(Default)]
pub struct OpenedRoots {
    roots: Mutex<Vec<(LibraryId, HostRoot)>>,
}

impl std::fmt::Debug for OpenedRoots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenedRoots").finish_non_exhaustive()
    }
}

impl OpenedRoots {
    /// No opened roots.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `library_id` is served from `root`. Answers false, changing nothing,
    /// when it is already recorded or the bound is reached.
    pub fn insert(&self, library_id: &LibraryId, root: HostRoot) -> bool {
        let Ok(mut roots) = self.roots.lock() else {
            return false;
        };
        let known = roots.iter().any(|(id, _)| id == library_id);
        if known || roots.len() >= filesquid_api::local_library::MAX_OPEN_LIBRARIES {
            return false;
        }
        roots.push((library_id.clone(), root));
        true
    }

    /// The opened roots, oldest first.
    ///
    /// # Errors
    ///
    /// A message when the registry is unavailable (its lock was poisoned).
    pub fn list(&self) -> Result<Vec<(LibraryId, HostRoot)>, String> {
        self.roots
            .lock()
            .map(|roots| roots.clone())
            .map_err(|_| "the opened roots are unavailable".to_string())
    }
}

impl OpenedRoots {
    /// Run `backfill` once over each opened root, writing under its own leases, until each
    /// is done, its cap is reached or `cancel` is set (checked between roots and, inside
    /// each run, between objects). Each run registers in the process registry and journals
    /// itself. Filesystem and journal I/O.
    ///
    /// # Errors
    ///
    /// A message when the registry is unavailable; a root whose run could not start is
    /// answered with its error, so the others still run.
    pub fn backfill(
        &self,
        backfill: &Backfill,
        cancel: &AtomicBool,
    ) -> Result<Vec<OpenedBackfill>, String> {
        let roots = self.list()?;
        let done = roots
            .into_iter()
            .take_while(|_| !cancel.load(Ordering::SeqCst))
            .map(|(library_id, root)| OpenedBackfill {
                report: backfill_root(&root, backfill, cancel),
                library_id,
            })
            .collect();
        Ok(done)
    }
}

/// What one backfill run did for one library opened in an owner-chosen folder.
#[derive(Debug)]
pub struct OpenedBackfill {
    /// The library.
    pub library_id: LibraryId,
    /// The run's report, or why it could not start.
    pub report: Result<BackfillReport, BackfillError>,
}

/// One backfill run over `root`; the filesystem backend's futures never wait. Filesystem and
/// journal I/O.
fn backfill_root(
    root: &HostRoot,
    backfill: &Backfill,
    cancel: &AtomicBool,
) -> Result<BackfillReport, BackfillError> {
    let run = backfill.run(root.backend.as_ref(), root.reservations.as_ref(), cancel);
    complete_immediately(run).map_err(|_| BackfillError::Unavailable)?
}
