//! The desktop's library routes (Task DE-1.1; API-S-025): `filesquid-api`'s library commands
//! over the folders the owner chose, installed with
//! [`ApiState::install_libraries`](filesquid_api::ApiState::install_libraries).
//!
//! The commands keep the one meaning of create, open, snapshot copy, rebuild (as the
//! `library.rebuild` job's runs, Task API-F2.17) and repair
//! ([`LibraryCommands`] over [`LocalLibraryService`]); the desktop only injects where the
//! storage roots come from ([`HostLocations`]) and the persistent repair ledger
//! ([`SqliteRepairLedger`](filesquid_native::SqliteRepairLedger) in the operational store). [`DesktopLibraries`] adds what a host
//! does once a library is bound to a folder: it serves the library's files from the same
//! root and leases ([`open_fs_library_root`]), attaches the folder to the library's catalog
//! index in the data directory, which is created or rebuilt from the folder when missing,
//! of another copy or behind (Task DE-1.7, ADR 041), schedules its OCFL validation and lists
//! its root for the PROV-O history ([`OpenedRoots`]). The library's name (API-S-038) lives in
//! two places the host keeps in step: the location registry, which the facts and recent
//! routes answer from, and the library entity of the folder's snapshot, which the navigation
//! shows as the title and which a moved or copied folder carries with it. A new library's
//! entity takes the bound name; a reopened folder's snapshot names the registry; a name
//! given on open and a rename retitle the entity in a journaled snapshot version
//! (`library.entity.rename`, the library ID only). No path ever reaches an answer.
//!
//! The repair is the desktop owner's alone: the per-launch token's holder is the only
//! principal this host authenticates, and the API refuses the repair for anyone else.

use filesquid_api::auth::Principal;
use filesquid_api::library::CatalogError;
use filesquid_api::local_library::{
    CopyReport, LibraryCommands, LibraryFacts, LibraryFuture, LibraryOperations, LibraryRegistry,
    LibraryRequest, LibraryView, LocalLibraryService, LocationRequest, Readable, RebuildEnd,
    RebuildProgress, RecentLibraries, RenameLibraryRequest, RepairAnswer, RepairCandidates,
    RepairRequest, SnapshotCopyRequest, UNTITLED_LIBRARY,
};
use filesquid_api::ocfl_validation::ValidationService;
use filesquid_core::{LibraryId, RepositoryId};
use filesquid_native::catalog::LibraryFolder;
use filesquid_native::{
    open_fs_library_root, HostFiles, HostLocations, OpenedRoots, SqliteCatalog,
};
use filesquid_ocfl::storage::FsBackend;
use std::path::PathBuf;
use std::sync::Arc;

/// Journal operation of a bound library whose files, validation or history could not be
/// served.
pub const SERVE_OPERATION: &str = "library.serve";

/// What the desktop serves for each library bound to a folder: its files, its validation and
/// its history.
#[derive(Clone)]
pub struct LibraryServing {
    /// The file service of the file and content routes.
    pub files: HostFiles,
    /// The OCFL validation, once the host started it.
    pub validation: Option<ValidationService<FsBackend>>,
    /// The roots the history routes and the backfill read.
    pub opened: Arc<OpenedRoots>,
    /// The data directory (leases and staging).
    pub data_dir: PathBuf,
    /// The library catalog indexes, keyed by library UUID (DE-S-009).
    pub catalog: Arc<SqliteCatalog>,
    /// This installation as a repository, which a new library entity names.
    pub repository: RepositoryId,
}

/// The desktop's library commands: [`LibraryCommands`] over [`HostLocations`], serving each
/// library it binds.
pub struct DesktopLibraries {
    commands: LibraryCommands<HostLocations>,
    locations: Arc<HostLocations>,
    serving: LibraryServing,
}

impl std::fmt::Debug for DesktopLibraries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopLibraries").finish_non_exhaustive()
    }
}

impl DesktopLibraries {
    /// The commands of `service` over `locations`, serving each bound library with
    /// `serving`; `locations` also keeps the name and last-opened time of each library they
    /// bind and answers its volume's free bytes (API-S-038). Pure.
    pub fn new(
        service: LocalLibraryService,
        locations: Arc<HostLocations>,
        serving: LibraryServing,
    ) -> Self {
        Self {
            commands: LibraryCommands::new(service, locations.clone())
                .with_registry(locations.clone()),
            locations,
            serving,
        }
    }

    /// The folders the owner registered.
    pub fn locations(&self) -> &Arc<HostLocations> {
        &self.locations
    }

    /// Serve the bound library of `view` for `actor`: its files from its folder's root and
    /// leases, its catalog index (named as [`Self::align_name`] says, with `requested` the
    /// name the bind request gave), its validation and its history. A part that cannot be
    /// served is logged as a warning with its error code only (the journal layer journals
    /// it); the library stays open in the commands. Creates directories in the data
    /// directory; the catalog attach (which may rebuild a whole index) and the name
    /// alignment run on blocking threads.
    async fn serve(&self, actor: &str, view: &LibraryView, requested: Option<&str>) {
        let location = self.commands.location_of(&view.library_id);
        let root = location
            .as_deref()
            .and_then(|location| self.locations.root_of(location));
        let Some(root) = root else {
            warn_unserved(&view.library_id, "location_not_found");
            return;
        };
        let identity = location.and_then(|location| self.locations.identity_of(&location));
        if self.attach(&view.library_id, &root, identity).await {
            self.align_name(actor, &view.library_id, requested).await;
        }
        if open_fs_library_root(
            &self.serving.files,
            &view.library_id,
            &root,
            &self.serving.data_dir,
        )
        .is_err()
        {
            warn_unserved(&view.library_id, "files_unavailable");
        }
        if let Some(validation) = &self.serving.validation {
            validation.open_library(view.library_id.clone(), root.backend.clone());
        }
        self.serving.opened.insert(&view.library_id, root);
    }

    /// Attach `library_id`'s folder `root` (identified as `identity`) to its catalog index on
    /// a blocking thread, logging a folder that cannot be attached with its error code only
    /// (`conflict`, `unavailable` or `internal`, as the catalog classified it); a folder
    /// without a snapshot gets its library entity under the registry's name of the library
    /// (the bound name, else [`UNTITLED_LIBRARY`]). Answers whether the folder is attached.
    /// Filesystem and database I/O.
    async fn attach(
        &self,
        library_id: &LibraryId,
        root: &filesquid_native::HostRoot,
        identity: Option<String>,
    ) -> bool {
        let Some(identity) = identity else {
            warn_unserved(library_id, "location_not_found");
            return false;
        };
        let folder = LibraryFolder {
            library_id: library_id.clone(),
            repository_id: self.serving.repository.clone(),
            name: self.registered_name(library_id),
            root: root.clone(),
            identity,
        };
        let catalog = self.serving.catalog.clone();
        let attached = tokio::task::spawn_blocking(move || catalog.attach(&folder)).await;
        match attached {
            Ok(Ok(_)) => true,
            Ok(Err(err)) => {
                warn_unserved(library_id, attach_error_code(&err));
                false
            }
            Err(_) => {
                warn_unserved(library_id, "internal");
                false
            }
        }
    }

    /// The name the location registry has for `library_id`, else [`UNTITLED_LIBRARY`].
    fn registered_name(&self, library_id: &LibraryId) -> String {
        self.locations
            .entry(library_id)
            .map(|entry| entry.name)
            .unwrap_or_else(|| UNTITLED_LIBRARY.to_string())
    }

    /// Keep the registry's name of the attached `library_id` and the name of its library
    /// entity in step: a name the bind request gave (`requested`) retitles the entity when
    /// it differs, journaled as `library.entity.rename` by `actor`; without one, the
    /// registry adopts the entity's name, so a reopened, moved or copied folder answers the
    /// name it carries. A name that cannot be read or written is logged with its error code
    /// only. Database and filesystem I/O on a blocking thread.
    async fn align_name(&self, actor: &str, library_id: &LibraryId, requested: Option<&str>) {
        let Some(entity) = self.entity_name(library_id).await else {
            return;
        };
        match requested {
            Some(name) if name != entity => self.retitle(actor, library_id, name).await,
            Some(_) => {}
            None => {
                let _adopted = self.locations.rename(library_id, &entity);
            }
        }
    }

    /// The name of `library_id`'s library entity, read from its catalog index on a blocking
    /// thread; `None`, logged with the error code, when it cannot be read.
    async fn entity_name(&self, library_id: &LibraryId) -> Option<String> {
        let catalog = self.serving.catalog.clone();
        let id = library_id.clone();
        let read = tokio::task::spawn_blocking(move || catalog.library_name(&id)).await;
        match read {
            Ok(Ok(name)) => Some(name),
            Ok(Err(err)) => {
                warn_unserved(library_id, attach_error_code(&err));
                None
            }
            Err(_) => {
                warn_unserved(library_id, "internal");
                None
            }
        }
    }

    /// Retitle `library_id`'s library entity to `name` in its index and snapshot, journaled
    /// as `library.entity.rename` by `actor` (the catalog journals a failed write as
    /// `failed`), on a blocking thread; a failure is also logged with its error code only.
    /// Database, filesystem and journal I/O.
    async fn retitle(&self, actor: &str, library_id: &LibraryId, name: &str) {
        let catalog = self.serving.catalog.clone();
        let id = library_id.clone();
        let (name, actor) = (name.to_string(), actor.to_string());
        let written =
            tokio::task::spawn_blocking(move || catalog.rename_library(&id, &name, &actor)).await;
        match written {
            Ok(Ok(())) => {}
            Ok(Err(err)) => warn_unserved(library_id, attach_error_code(&err)),
            Err(_) => warn_unserved(library_id, "internal"),
        }
    }

    /// Bind with `bind` for `actor`, then serve the bound library, `requested` being the
    /// name the bind request gave.
    async fn bind_and_serve(
        &self,
        actor: &str,
        requested: Option<String>,
        bind: impl std::future::Future<
            Output = Result<LibraryView, filesquid_api::local_library::LocalLibraryError>,
        >,
    ) -> Result<LibraryView, filesquid_api::local_library::LocalLibraryError> {
        let view = bind.await?;
        self.serve(actor, &view, requested.as_deref()).await;
        Ok(view)
    }
}

/// The error code of a catalog attach failure: whether a retry may help. Pure.
fn attach_error_code(err: &CatalogError) -> &'static str {
    match err {
        CatalogError::Conflict => "conflict",
        CatalogError::Unavailable(_) => "unavailable",
        CatalogError::Failed(_) => "internal",
    }
}

/// Log that `library_id` is open but a part of it could not be served, with `code` only.
fn warn_unserved(library_id: &LibraryId, code: &str) {
    tracing::warn!(
        operation = SERVE_OPERATION,
        library_id = library_id.as_str(),
        error_code = code,
        "an opened library could not be served in full"
    );
}

impl LibraryOperations for DesktopLibraries {
    fn create(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move {
            let requested = request.name.clone();
            self.bind_and_serve(&actor, requested, self.commands.create(&actor, request))
                .await
        })
    }

    fn open(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move {
            let requested = request.name.clone();
            self.bind_and_serve(&actor, requested, self.commands.open(&actor, request))
                .await
        })
    }

    fn copy_snapshot(
        &self,
        actor: String,
        request: SnapshotCopyRequest,
    ) -> LibraryFuture<'_, CopyReport> {
        Box::pin(async move { self.commands.copy_snapshot(&actor, request).await })
    }

    fn rebuild_ready(&self, actor: String, request: LibraryRequest) -> LibraryFuture<'_, ()> {
        Box::pin(async move { self.commands.rebuild_ready(&actor, &request) })
    }

    fn rebuild_from<'a>(
        &'a self,
        actor: String,
        request: LibraryRequest,
        after: Option<String>,
        progress: &'a mut (dyn RebuildProgress + Send),
    ) -> LibraryFuture<'a, RebuildEnd> {
        Box::pin(async move {
            self.commands
                .rebuild_from(&actor, request, after, progress)
                .await
        })
    }

    fn repair_candidates(
        &self,
        actor: String,
        request: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates> {
        Box::pin(async move { self.commands.repair_candidates(&actor, request).await })
    }

    fn repair(&self, caller: Principal, request: RepairRequest) -> LibraryFuture<'_, RepairAnswer> {
        Box::pin(async move { self.commands.repair(&caller, request).await })
    }

    /// The commands' rename (the registry, journaled `library.rename`), then the retitle of
    /// the library entity, so the navigation title follows (API-S-038); a retitle the
    /// catalog refuses is journaled by it and logged, and the facts still answer the
    /// registry's name.
    fn rename(
        &self,
        actor: String,
        request: RenameLibraryRequest,
    ) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move {
            let (library_id, name) = (request.library_id.clone(), request.name.clone());
            let facts = self.commands.rename(&actor, request).await?;
            self.retitle(&actor, &library_id, &name).await;
            Ok(facts)
        })
    }

    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { self.commands.facts(&actor, &library_id).await })
    }

    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries> {
        Box::pin(async move { self.commands.recent(&actor, readable.as_ref()).await })
    }
}
