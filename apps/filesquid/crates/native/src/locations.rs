//! The folders a host's owner chose for libraries (Task DE-1.1; API-S-025): the native
//! [`LibraryLocations`] the hosts inject into `filesquid-api`'s library routes.
//!
//! A folder never crosses the API as a path. [`HostLocations::register`] resolves the folder
//! once (every symbolic link on the way resolved, the folder itself never a link), prepares
//! its storage backend and its fenced version leases under `<data dir>/locations/<digest>/`
//! (named by a digest of the canonical folder); registering one folder again answers its
//! existing ID, and a second host-chosen ID for it shares its backend and leases, and answers an opaque `loc-<uuid>` ID. The library commands and the file
//! routes of a library opened there write through the same leases
//! ([`HostLocations::root_of`], [`crate::files::open_fs_library_root`]).
//!
//! Registrations live in memory for the host's lifetime; at most [`MAX_LOCATIONS`].
//!
//! The registry is also the desktop's [`LibraryRegistry`] (Task API-F2.18; API-S-038): it keeps
//! each library's name, the location it was opened from and when, in a [`RegistryBook`], and,
//! when opened with [`HostLocations::persistent`], writes them through to the operational
//! store and restores them at the next launch with each folder registered again under its
//! location ID (Task DE-1.8, [`crate::registry_sqlite`]), and answers the free bytes of a
//! registered folder's volume on Linux and macOS with `fs4`. Any other build answers
//! `volume_unreadable` (owner decision 5 of 2026-10-23: the Win32 read is DE-F3.3's).

use crate::files::{real_dir, storage_root, HostLeases};
use crate::registry_sqlite::{RegistryRow, SqliteLibraryRegistry};
use filesquid_api::history::backfill::{Backfill, BackfillError, BackfillReport};
use filesquid_api::local_library::{
    FreeSpace, LibraryLocations, LibraryRegistry, LibraryRoot, RegistryBook, RegistryEntry,
};
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
    /// What this host remembers of each library it bound (API-S-038).
    libraries: RegistryBook,
    /// Where that is kept across launches, when it is.
    store: Option<SqliteLibraryRegistry>,
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
            libraries: RegistryBook::new(),
            store: None,
        }
    }

    /// The registry of a host that keeps what it remembers of its libraries across launches
    /// in `store`: every kept library is remembered again and its folder registered again
    /// under its location ID; a folder that cannot be registered now (moved, unplugged) keeps
    /// its entry, whose free bytes then read as unreadable. Reads the store and each folder's
    /// metadata; creates each location's folders in the data directory.
    ///
    /// # Errors
    /// The store cannot be read; nothing is restored then.
    pub fn persistent(data_dir: &Path, store: SqliteLibraryRegistry) -> Result<Self, String> {
        let rows = store.rows()?;
        let mut locations = Self::new(data_dir);
        for row in rows.iter().rev() {
            locations.restore(row);
        }
        locations.store = Some(store);
        Ok(locations)
    }

    /// Remember `row` again and register its folder under its location ID, logging a folder
    /// that cannot be registered with no path.
    fn restore(&self, row: &RegistryRow) {
        let known = self.root_of(&row.location_id).is_some();
        if !known {
            if let Err(reason) = self.register_as(&row.location_id, &row.folder) {
                tracing::info!(
                    target: "filesquid::native",
                    library_id = row.library_id.as_str(),
                    reason = reason.as_str(),
                    "a remembered library's folder could not be registered again"
                );
            }
        }
        self.libraries.remember(
            &row.library_id,
            &row.location_id,
            Some(&row.name),
            row.last_opened_ms,
        );
    }

    /// Keep what this host remembers of `library_id` in its store, when it has one; a failure
    /// is logged with no path (the library stays remembered for this launch).
    fn persist(&self, library_id: &LibraryId) {
        let (Some(store), Some(entry)) = (&self.store, self.libraries.entry(library_id)) else {
            return;
        };
        let Some(folder) = self.folder_of(&entry.location_id) else {
            return;
        };
        let row = RegistryRow {
            library_id: entry.library_id,
            name: entry.name,
            location_id: entry.location_id,
            folder,
            last_opened_ms: entry.last_opened_ms,
        };
        let retained: Vec<LibraryId> = self
            .libraries
            .entries()
            .into_iter()
            .map(|entry| entry.library_id)
            .collect();
        if let Err(reason) = store.save(&row, &retained) {
            tracing::warn!(
                target: "filesquid::native",
                error_code = "unavailable",
                reason = reason.as_str(),
                "a library could not be remembered across launches"
            );
        }
    }

    /// Register the folder the owner chose and answer its opaque location ID: a new one, or
    /// the existing one when the folder is already registered (API-S-034), so repeated
    /// choices never accumulate registrations for one folder. Creates the location's parts
    /// and lease folders in the data directory; reads nothing in the folder itself.
    ///
    /// # Errors
    ///
    /// A message that never names a path: the folder is not a plain directory, it overlaps
    /// the app's data directory, the data directory's folders could not be prepared,
    /// [`MAX_LOCATIONS`] are registered, or no ID could be minted.
    pub fn register(&self, folder: &Path) -> Result<String, String> {
        let folder = self.checked_folder(folder)?;
        let mut locations = self.lock()?;
        if let Some(known) = id_of_folder(&locations, &folder) {
            return Ok(known);
        }
        let id = mint_location_id()?;
        self.insert(&mut locations, &id, folder)?;
        Ok(id)
    }

    /// Register `folder` under the opaque `location_id` the host chose (`loc-` and at most
    /// [`MAX_LOCATION_ID_BYTES`](filesquid_api::local_library::MAX_LOCATION_ID_BYTES) bytes
    /// of ASCII letters, digits and `-`). A folder registered under another ID shares that
    /// registration's backend and leases.
    ///
    /// # Errors
    ///
    /// As [`HostLocations::register`], or the ID is malformed or already registered.
    pub fn register_as(&self, location_id: &str, folder: &Path) -> Result<(), String> {
        if !valid_location_id(location_id) {
            return Err("the location ID is malformed".to_string());
        }
        let folder = self.checked_folder(folder)?;
        let mut locations = self.lock()?;
        self.insert(&mut locations, location_id, folder)
    }

    /// `folder` resolved to its canonical path and checked against the data directory.
    /// Reads filesystem metadata.
    fn checked_folder(&self, folder: &Path) -> Result<PathBuf, String> {
        let folder = storage_root(folder)?;
        self.refuse_data_dir_overlap(&folder)?;
        Ok(folder)
    }

    /// Insert `folder` as `location_id` into the locked registry: the checks, the
    /// preparation of a new folder's storage and the insertion happen under one lock, so
    /// concurrent registrations of one folder share its root and a refused one prepares
    /// nothing. Creates directories for a folder registered for the first time.
    fn insert(
        &self,
        locations: &mut HashMap<String, Location>,
        location_id: &str,
        folder: PathBuf,
    ) -> Result<(), String> {
        if locations.len() >= MAX_LOCATIONS {
            return Err("too many locations are registered".to_string());
        }
        if locations.contains_key(location_id) {
            return Err("the location ID is already registered".to_string());
        }
        let known = locations.values().find(|known| known.folder == folder);
        let root = match known {
            Some(known) => known.root.clone(),
            None => self.prepare_root(&folder)?,
        };
        locations.insert(location_id.to_string(), Location { folder, root });
        Ok(())
    }

    /// Refuse a folder that is, holds or lies inside the data directory, so no lease, parts
    /// or staging folder of the host can become part of a library's storage root. Reads
    /// filesystem metadata.
    fn refuse_data_dir_overlap(&self, folder: &Path) -> Result<(), String> {
        let data = self
            .data_dir
            .canonicalize()
            .map_err(|_| "the data directory could not be resolved".to_string())?;
        if folder.starts_with(&data) || data.starts_with(folder) {
            return Err("the folder overlaps the app's data directory".to_string());
        }
        Ok(())
    }

    /// The storage root registered as `location_id`, if any.
    pub fn root_of(&self, location_id: &str) -> Option<HostRoot> {
        let locations = self.locations.lock().ok()?;
        locations.get(location_id).map(|found| found.root.clone())
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

    /// An opaque identity of the folder registered as `location_id`: the digest of its
    /// canonical path, never the path. Two registrations of one folder share it; a moved or
    /// copied library folder has another, so the catalog rebuilds its index (Task DE-1.7).
    pub fn identity_of(&self, location_id: &str) -> Option<String> {
        self.folder_of(location_id)
            .map(|folder| folder_digest(&folder))
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

impl LibraryRegistry for HostLocations {
    fn remember(
        &self,
        library_id: &LibraryId,
        location_id: &str,
        name: Option<&str>,
        opened_ms: i64,
    ) -> String {
        let kept = self
            .libraries
            .remember(library_id, location_id, name, opened_ms);
        self.persist(library_id);
        kept
    }

    fn rename(&self, library_id: &LibraryId, name: &str) -> Option<bool> {
        let renamed = self.libraries.rename(library_id, name);
        if renamed == Some(true) {
            self.persist(library_id);
        }
        renamed
    }

    fn entry(&self, library_id: &LibraryId) -> Option<RegistryEntry> {
        self.libraries.entry(library_id)
    }

    fn entries(&self) -> Vec<RegistryEntry> {
        self.libraries.entries()
    }

    /// The free bytes of the registered folder's volume; `volume_unreadable` for an unknown
    /// location, a volume that cannot be read, or a build that does not read it. Queries the
    /// volume.
    fn free_space(&self, location_id: &str) -> FreeSpace {
        match self.folder_of(location_id) {
            Some(folder) => volume_free_space(&folder),
            None => FreeSpace::VolumeUnreadable,
        }
    }
}

/// The bytes free to this process on the volume that holds `folder` (Linux and macOS).
/// Queries the volume.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn volume_free_space(folder: &Path) -> FreeSpace {
    match fs4::available_space(folder) {
        Ok(bytes) => FreeSpace::Bytes(bytes),
        Err(err) => {
            tracing::info!(
                target: "filesquid::native",
                error_kind = ?err.kind(),
                "the free space of a library's volume could not be read"
            );
            FreeSpace::VolumeUnreadable
        }
    }
}

/// Not read by this build (owner decision 5 of 2026-10-23; DE-F3.3 adds the Win32 read).
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn volume_free_space(_folder: &Path) -> FreeSpace {
    FreeSpace::VolumeUnreadable
}

/// The ID a canonical `folder` is registered under in `locations` (the smallest, when it
/// has several), if any. Pure.
fn id_of_folder(locations: &HashMap<String, Location>, folder: &Path) -> Option<String> {
    locations
        .iter()
        .filter(|(_, known)| known.folder == folder)
        .map(|(id, _)| id.clone())
        .min()
}

/// Whether `id` is a location ID: `loc-`, then ASCII letters, digits and `-`, at most
/// [`MAX_LOCATION_ID_BYTES`](filesquid_api::local_library::MAX_LOCATION_ID_BYTES) bytes.
/// Pure.
fn valid_location_id(id: &str) -> bool {
    let rest = id.strip_prefix(LOCATION_PREFIX).unwrap_or_default();
    let allowed = |c: char| c.is_ascii_alphanumeric() || c == '-';
    !rest.is_empty()
        && id.len() <= filesquid_api::local_library::MAX_LOCATION_ID_BYTES
        && rest.chars().all(allowed)
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
    /// Run `backfill` over each opened root, writing under its own leases, until each is
    /// done, its run cap is reached or `cancel` is set (checked between roots and, inside
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
    /// The reports of its runs (more than one when a run reached its cap), or why a run
    /// could not start.
    pub report: Result<Vec<BackfillReport>, BackfillError>,
}

/// Backfill `root` until it is done, [`MAX_BACKFILL_RUNS`](crate::MAX_BACKFILL_RUNS) runs
/// were made, or `cancel` is set: a run that reached its cap is followed by another, as the
/// catalog's backfill does. Answers every run's report, or the first run that could not start.
/// Filesystem and journal I/O.
fn backfill_root(
    root: &HostRoot,
    backfill: &Backfill,
    cancel: &AtomicBool,
) -> Result<Vec<BackfillReport>, BackfillError> {
    let mut reports = Vec::new();
    for _ in 0..crate::MAX_BACKFILL_RUNS {
        let report = backfill_once(root, backfill, cancel)?;
        let again = report.reached_cap && !report.cancelled;
        reports.push(report);
        if !again {
            break;
        }
    }
    Ok(reports)
}

/// One backfill run over `root`; the filesystem backend's futures never wait. Filesystem and
/// journal I/O.
fn backfill_once(
    root: &HostRoot,
    backfill: &Backfill,
    cancel: &AtomicBool,
) -> Result<BackfillReport, BackfillError> {
    let run = backfill.run(root.backend.as_ref(), root.reservations.as_ref(), cancel);
    complete_immediately(run).map_err(|_| BackfillError::Unavailable)?
}
