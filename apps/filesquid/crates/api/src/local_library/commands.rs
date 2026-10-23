//! [`LibraryCommands`]: the library routes' one entry point into the CL-1.1
//! [`LocalLibraryService`] (API-S-025). It resolves the opaque location and library IDs a
//! request names to the storage roots the host injected, keeps which location each open
//! library lives in, and calls the service, which keeps the one meaning of every operation
//! and journals its outcomes and refusals. A refusal made here, before the service runs (an
//! unknown location or library, a location in use, the open-library bound), is journaled
//! here once, under the operation's own name.

use super::error::LocalLibraryError;
use super::facts::{
    check_name, LibraryFacts, LibraryRegistry, RecentLibraries, RecentLibrary,
    RenameLibraryRequest, MAX_RECENT_LIBRARIES,
};
use super::locations::{
    LibraryLocations, LibraryRequest, LibraryRoot, LibraryView, LocationRequest,
    SnapshotCopyRequest, MAX_LOCATION_ID_BYTES,
};
use super::rebuild::{RebuildEnd, RebuildProgress};
use super::repair::{RepairAnswer, RepairCandidates, RepairRequest, REPAIR_OPERATION};
use super::service::{
    LocalLibraryService, CANDIDATES_OPERATION, COPY_OPERATION, CREATE_OPERATION, FACTS_OPERATION,
    OPEN_OPERATION, REBUILD_OPERATION, RECENT_OPERATION, RENAME_OPERATION,
};
use super::snapshot::CopyReport;
use crate::auth::Principal;
use crate::library::mint_uuid_v4;
use filesquid_core::LibraryId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Most libraries one host keeps open at once (AGENTS.md §22).
pub const MAX_OPEN_LIBRARIES: usize = 64;

/// Longest library ID a request may name, in bytes.
const MAX_LIBRARY_ID_BYTES: usize = 128;

/// The storage root a host's locations resolve to.
type Root<L> = LibraryRoot<<L as LibraryLocations>::Backend, <L as LibraryLocations>::Reservations>;

/// Whether an operation makes a new storage root or opens an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binding {
    Create,
    Open,
}

impl Binding {
    /// The journal operation of this binding.
    fn operation(self) -> &'static str {
        match self {
            Binding::Create => CREATE_OPERATION,
            Binding::Open => OPEN_OPERATION,
        }
    }
}

/// The library operations of the routes, over the locations a host injects.
pub struct LibraryCommands<L: LibraryLocations> {
    service: LocalLibraryService,
    locations: Arc<L>,
    /// The location of each open library, by library ID.
    open: Mutex<HashMap<LibraryId, String>>,
    /// What the host remembers of the libraries it bound (API-S-038); without one, the
    /// facts, recent and rename routes are unavailable.
    registry: Option<Arc<dyn LibraryRegistry>>,
    /// Serialises create, open and copy, so two of them cannot bind one location or library
    /// twice between the check and the binding, and no library is opened in a copy's
    /// destination while the copy runs.
    binding: tokio::sync::Mutex<()>,
}

impl<L: LibraryLocations> std::fmt::Debug for LibraryCommands<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryCommands").finish_non_exhaustive()
    }
}

impl<L: LibraryLocations> LibraryCommands<L> {
    /// Commands that run `service` over the storage roots `locations` resolves.
    pub fn new(service: LocalLibraryService, locations: Arc<L>) -> Self {
        Self {
            service,
            locations,
            open: Mutex::new(HashMap::new()),
            registry: None,
            binding: tokio::sync::Mutex::new(()),
        }
    }

    /// These commands, remembering each library they bind in `registry` and answering the
    /// library facts from it (API-S-038).
    pub fn with_registry(mut self, registry: Arc<dyn LibraryRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Rename a library this host opened, for `actor`, and answer its facts. A rename that
    /// changes nothing answers the facts and is not journaled.
    ///
    /// # Errors
    ///
    /// `invalid_name`, `library_not_found` (a library this host never opened), or
    /// `unavailable` without a registry; each journaled once.
    pub async fn rename(
        &self,
        actor: &str,
        request: RenameLibraryRequest,
    ) -> Result<LibraryFacts, LocalLibraryError> {
        let library = &request.library_id;
        let checked = self.registry().and_then(|registry| {
            name_or_refusal(Some(&request.name))?;
            let renamed = registry.rename(library, &request.name);
            renamed
                .map(|changed| (registry, changed))
                .ok_or_else(library_not_found)
        });
        let (registry, changed) = self.refused(actor, RENAME_OPERATION, Some(library), checked)?;
        if changed {
            self.service
                .journal_success(actor, RENAME_OPERATION, library);
        }
        let entry = registry.entry(library).ok_or(LocalLibraryError::Internal)?;
        Ok(LibraryFacts::of(
            &entry,
            &registry.free_space(&entry.location_id),
        ))
    }

    /// The facts of a library this host opened, for `actor`: its name and the free space of
    /// its volume.
    ///
    /// # Errors
    ///
    /// `library_not_found`, or `unavailable` without a registry; each journaled once.
    pub async fn facts(
        &self,
        actor: &str,
        library: &LibraryId,
    ) -> Result<LibraryFacts, LocalLibraryError> {
        let checked = self.registry().and_then(|registry| {
            let entry = registry.entry(library).ok_or_else(library_not_found)?;
            Ok((registry, entry))
        });
        let (registry, entry) = self.refused(actor, FACTS_OPERATION, Some(library), checked)?;
        Ok(LibraryFacts::of(
            &entry,
            &registry.free_space(&entry.location_id),
        ))
    }

    /// The libraries this host opened that `readable` keeps, then at most
    /// [`MAX_RECENT_LIBRARIES`] of them, newest first, with the free space of each one's
    /// volume, for `actor`.
    ///
    /// # Errors
    ///
    /// `unavailable` without a registry, journaled once.
    pub async fn recent(
        &self,
        actor: &str,
        readable: &(dyn Fn(&LibraryId) -> bool + Send + Sync),
    ) -> Result<RecentLibraries, LocalLibraryError> {
        let registry = self.refused(actor, RECENT_OPERATION, None, self.registry())?;
        let open = self.lock().clone();
        let libraries = registry
            .entries()
            .into_iter()
            .filter(|entry| readable(&entry.library_id))
            .take(MAX_RECENT_LIBRARIES)
            .map(|entry| {
                let space = registry.free_space(&entry.location_id);
                RecentLibrary::of(&entry, open.contains_key(&entry.library_id), &space)
            })
            .collect();
        Ok(RecentLibraries { libraries })
    }

    /// The installed registry, or `unavailable`.
    fn registry(&self) -> Result<Arc<dyn LibraryRegistry>, LocalLibraryError> {
        self.registry.clone().ok_or(LocalLibraryError::Unavailable)
    }

    /// The location ID the open library `library_id` lives in, so a host can serve that
    /// library's files from the same storage root.
    pub fn location_of(&self, library_id: &LibraryId) -> Option<String> {
        self.lock().get(library_id).cloned()
    }

    /// Create a library's storage root in the empty location the request names, for
    /// `actor`, and keep it open.
    ///
    /// # Errors
    ///
    /// `invalid_location`, `invalid_library`, `location_not_found`, `location_in_use` (the
    /// location is, holds or lies inside an open library's folder), `library_already_open`,
    /// `too_many_libraries`, or the create errors of [`LocalLibraryService::create_root`].
    pub async fn create(
        &self,
        actor: &str,
        request: LocationRequest,
    ) -> Result<LibraryView, LocalLibraryError> {
        self.bind(actor, request, Binding::Create).await
    }

    /// Open the storage root in the location the request names, for `actor`, and keep it
    /// open. Opening a location that is already open answers its library again.
    ///
    /// # Errors
    ///
    /// As [`LibraryCommands::create`], with the open errors of
    /// [`LocalLibraryService::open_root`].
    pub async fn open(
        &self,
        actor: &str,
        request: LocationRequest,
    ) -> Result<LibraryView, LocalLibraryError> {
        self.bind(actor, request, Binding::Open).await
    }

    /// Copy the completed snapshot of an open library into an empty location, for `actor`.
    ///
    /// # Errors
    ///
    /// `library_not_open`, `location_not_found`, `destination_overlaps_library`, or the
    /// copy errors of [`LocalLibraryService::copy_snapshot`].
    pub async fn copy_snapshot(
        &self,
        actor: &str,
        request: SnapshotCopyRequest,
    ) -> Result<CopyReport, LocalLibraryError> {
        let _serialised = self.binding.lock().await;
        let library = &request.library_id;
        let dest_id = &request.destination_location_id;
        let checked = self.copy_roots(library, dest_id);
        let (source, dest) = self.refused(actor, COPY_OPERATION, Some(library), checked)?;
        self.service
            .copy_snapshot(
                actor,
                library,
                source.backend.as_ref(),
                dest.backend.as_ref(),
            )
            .await
    }

    /// Refuse a rebuild of a library that is not open, for `actor`, before its job is
    /// submitted; the refusal is journaled once as `library.rebuild`.
    ///
    /// # Errors
    ///
    /// `library_not_open`.
    pub fn rebuild_ready(
        &self,
        actor: &str,
        request: &LibraryRequest,
    ) -> Result<(), LocalLibraryError> {
        let library = &request.library_id;
        let root = self.root(library);
        self.refused(actor, REBUILD_OPERATION, Some(library), root)
            .map(drop)
    }

    /// Rebuild an open library from its files after the cursor `after`, for `actor`, telling
    /// `progress` about each object: the step of a rebuild job.
    ///
    /// # Errors
    ///
    /// `library_not_open`, or the errors of [`LocalLibraryService::rebuild_from`].
    pub async fn rebuild_from(
        &self,
        actor: &str,
        request: LibraryRequest,
        after: Option<String>,
        progress: &mut (dyn RebuildProgress + Send),
    ) -> Result<RebuildEnd, LocalLibraryError> {
        let library = &request.library_id;
        let root = self.refused(actor, REBUILD_OPERATION, Some(library), self.root(library))?;
        self.service
            .rebuild_from(actor, library, root.backend.as_ref(), after, progress)
            .await
    }

    /// The repair candidates of an open library, for `actor`.
    ///
    /// # Errors
    ///
    /// `library_not_open`, or the errors of [`LocalLibraryService::repair_candidates`].
    pub async fn repair_candidates(
        &self,
        actor: &str,
        request: LibraryRequest,
    ) -> Result<RepairCandidates, LocalLibraryError> {
        let library = &request.library_id;
        let root = self.root(library);
        let root = self.refused(actor, CANDIDATES_OPERATION, Some(library), root)?;
        self.service
            .repair_candidates(actor, library, root.backend.as_ref())
            .await
    }

    /// Run the consented version-inventory repair the request names, for `caller`.
    ///
    /// # Errors
    ///
    /// `library_not_open`, or the errors of
    /// [`LocalLibraryService::repair_version_inventory`].
    pub async fn repair(
        &self,
        caller: &Principal,
        request: RepairRequest,
    ) -> Result<RepairAnswer, LocalLibraryError> {
        let library = request.library_id.clone();
        let root = self.root(&library);
        let root = self.refused(caller.actor(), REPAIR_OPERATION, Some(&library), root)?;
        self.service
            .repair_version_inventory(
                caller,
                request,
                root.backend.as_ref(),
                root.reservations.as_ref(),
            )
            .await
    }

    /// Create or open, then keep the library bound to its location.
    async fn bind(
        &self,
        actor: &str,
        request: LocationRequest,
        binding: Binding,
    ) -> Result<LibraryView, LocalLibraryError> {
        let _serialised = self.binding.lock().await;
        let operation = binding.operation();
        let target = request.library_id.clone();
        let planned = name_or_refusal(request.name.as_deref())
            .and_then(|()| self.plan_binding(&request, binding));
        let (library_id, root) = self.refused(actor, operation, target.as_ref(), planned)?;
        let backend = root.backend.as_ref();
        let summary = match binding {
            Binding::Create => self.service.create_root(actor, &library_id, backend).await,
            Binding::Open => self.service.open_root(actor, &library_id, backend).await,
        }?;
        if let Some(registry) = &self.registry {
            let now = self.service.now_ms();
            let name = request.name.as_deref();
            registry.remember(&library_id, &request.location_id, name, now);
        }
        self.lock().insert(library_id.clone(), request.location_id);
        Ok(LibraryView {
            library_id,
            spec_version: summary.spec_version,
            objects: summary.objects,
        })
    }

    /// The library ID and root a create or open would bind, checking the IDs, the location,
    /// what is already open (a second location ID for an open library's folder, or one
    /// inside or around it, is refused, so one OCFL root is never bound twice) and the
    /// bound. Nothing is written.
    fn plan_binding(
        &self,
        request: &LocationRequest,
        binding: Binding,
    ) -> Result<(LibraryId, Root<L>), LocalLibraryError> {
        check_location(&request.location_id)?;
        if let Some(id) = &request.library_id {
            check_library(id)?;
        }
        let root = self.location(&request.location_id)?;
        let open = self.lock();
        let bound = open
            .iter()
            .find(|(_, location)| **location == request.location_id)
            .map(|(library, _)| library.clone());
        if let Some(bound) = bound {
            let same = request.library_id.as_ref().is_none_or(|id| *id == bound);
            if binding == Binding::Open && same {
                return Ok((bound, root));
            }
            return Err(location_in_use());
        }
        let aliased = open
            .values()
            .any(|location| self.locations.overlaps(location, &request.location_id));
        if aliased {
            return Err(location_in_use());
        }
        let library_id = match &request.library_id {
            Some(id) if open.contains_key(id) => return Err(library_already_open()),
            Some(id) => id.clone(),
            None => mint_library_id()?,
        };
        if open.len() >= MAX_OPEN_LIBRARIES {
            return Err(too_many_libraries());
        }
        Ok((library_id, root))
    }

    /// The roots of an open library and a copy destination that overlaps no open library,
    /// so a copy never lands inside an OCFL root (OCFL-S-019).
    fn copy_roots(
        &self,
        library: &LibraryId,
        dest_id: &str,
    ) -> Result<(Root<L>, Root<L>), LocalLibraryError> {
        check_library(library)?;
        check_location(dest_id)?;
        let source_id = self.location_of(library).ok_or_else(library_not_open)?;
        let source = self.location(&source_id)?;
        let dest = self.location(dest_id)?;
        let open: Vec<String> = self.lock().values().cloned().collect();
        if open
            .iter()
            .any(|location| self.locations.overlaps(location, dest_id))
        {
            return Err(LocalLibraryError::refused(
                "destination_overlaps_library",
                "Choose a folder outside every open library for the copy.",
            ));
        }
        Ok((source, dest))
    }

    /// The root of the open library `library`.
    fn root(&self, library: &LibraryId) -> Result<Root<L>, LocalLibraryError> {
        check_library(library)?;
        let location = self.location_of(library).ok_or_else(library_not_open)?;
        self.location(&location)
    }

    /// The root the host registered as `location_id`.
    fn location(&self, location_id: &str) -> Result<Root<L>, LocalLibraryError> {
        self.locations.location(location_id).ok_or_else(|| {
            LocalLibraryError::not_found(
                "location_not_found",
                "That folder is no longer available. Choose it again.",
            )
        })
    }

    /// Pass `checked` through, journaling it once as a refusal of `operation` when it failed.
    fn refused<T>(
        &self,
        actor: &str,
        operation: &'static str,
        library: Option<&LibraryId>,
        checked: Result<T, LocalLibraryError>,
    ) -> Result<T, LocalLibraryError> {
        checked.inspect_err(|err| {
            let target = library.map(LibraryId::as_str);
            self.service.journal_refusal(actor, operation, target, err);
        })
    }

    /// The open-library map. A poisoned lock still holds a consistent map: every change is
    /// one insert.
    fn lock(&self) -> MutexGuard<'_, HashMap<LibraryId, String>> {
        self.open
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Refuse a blank or oversized location ID.
fn check_location(location_id: &str) -> Result<(), LocalLibraryError> {
    if location_id.trim().is_empty() || location_id.len() > MAX_LOCATION_ID_BYTES {
        return Err(LocalLibraryError::invalid(
            "invalid_location",
            "Name the folder by the ID FileSquid gave it.",
        ));
    }
    Ok(())
}

/// Refuse an oversized library ID.
fn check_library(library_id: &LibraryId) -> Result<(), LocalLibraryError> {
    if library_id.as_str().len() > MAX_LIBRARY_ID_BYTES {
        return Err(LocalLibraryError::invalid(
            "invalid_library",
            "A library ID is at most 128 bytes.",
        ));
    }
    Ok(())
}

/// A fresh library ID.
fn mint_library_id() -> Result<LibraryId, LocalLibraryError> {
    let raw = mint_uuid_v4().map_err(|_| LocalLibraryError::Unavailable)?;
    LibraryId::new(raw).map_err(|_| LocalLibraryError::Internal)
}

/// `Ok` for no name or a name that keeps the rule of [`check_name`], else `invalid_name`.
fn name_or_refusal(name: Option<&str>) -> Result<(), LocalLibraryError> {
    match name {
        Some(name) if !check_name(name) => Err(LocalLibraryError::invalid(
            "invalid_name",
            "A library name is 1 to 1,024 bytes, not blank, with no control characters.",
        )),
        _ => Ok(()),
    }
}

/// The refusal for a library this host never opened.
fn library_not_found() -> LocalLibraryError {
    LocalLibraryError::not_found("library_not_found", "That library is not known here.")
}

/// The refusal for a library that is not open on this host.
fn library_not_open() -> LocalLibraryError {
    LocalLibraryError::not_found(
        "library_not_open",
        "That library is not open. Open it first.",
    )
}

/// The refusal for a location that already holds an open library.
fn location_in_use() -> LocalLibraryError {
    LocalLibraryError::refused(
        "location_in_use",
        "That folder already holds an open library.",
    )
}

/// The refusal for a library ID that is already open elsewhere.
fn library_already_open() -> LocalLibraryError {
    LocalLibraryError::refused(
        "library_already_open",
        "That library is already open from another folder.",
    )
}

/// The refusal once [`MAX_OPEN_LIBRARIES`] are open.
fn too_many_libraries() -> LocalLibraryError {
    LocalLibraryError::refused(
        "too_many_libraries",
        "Too many libraries are open. Restart FileSquid to open another.",
    )
}
