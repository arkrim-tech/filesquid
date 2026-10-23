//! The desktop's library routes (Task DE-1.1; API-S-025): `filesquid-api`'s library commands
//! over the folders the owner chose, installed with
//! [`ApiState::install_libraries`](filesquid_api::ApiState::install_libraries).
//!
//! The commands keep the one meaning of create, open, snapshot copy, rebuild and repair
//! ([`LibraryCommands`] over [`LocalLibraryService`]); the desktop only injects where the
//! storage roots come from ([`HostLocations`]) and the persistent repair ledger
//! ([`SqliteRepairLedger`](filesquid_native::SqliteRepairLedger) in the operational store). [`DesktopLibraries`] adds what a host
//! does once a library is bound to a folder: it serves the library's files from the same
//! root and leases ([`open_fs_library_root`]), attaches the folder to the library's catalog
//! index in the data directory, which is created or rebuilt from the folder when missing,
//! of another copy or behind (Task DE-1.7, ADR 041), schedules its OCFL validation and lists
//! its root for the PROV-O history ([`OpenedRoots`]). No path ever reaches an answer.
//!
//! The repair is the desktop owner's alone: the per-launch token's holder is the only
//! principal this host authenticates, and the API refuses the repair for anyone else.

use filesquid_api::auth::Principal;
use filesquid_api::library::CatalogError;
use filesquid_api::local_library::{
    CopyReport, LibraryCommands, LibraryFuture, LibraryOperations, LibraryRequest, LibraryView,
    LocalLibraryService, LocationRequest, RebuildReport, RepairAnswer, RepairCandidates,
    RepairRequest, SnapshotCopyRequest,
};
use filesquid_api::ocfl_validation::ValidationService;
use filesquid_core::{LibraryId, RepositoryId};
use filesquid_native::catalog::{LibraryFolder, DEFAULT_LIBRARY_NAME};
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
    /// `serving`. Pure.
    pub fn new(
        service: LocalLibraryService,
        locations: Arc<HostLocations>,
        serving: LibraryServing,
    ) -> Self {
        Self {
            commands: LibraryCommands::new(service, locations.clone()),
            locations,
            serving,
        }
    }

    /// The folders the owner registered.
    pub fn locations(&self) -> &Arc<HostLocations> {
        &self.locations
    }

    /// Serve the bound library of `view`: its files from its folder's root and leases, its
    /// catalog index, its validation and its history. A part that cannot be served is logged
    /// as a warning with its error code only (the journal layer journals it); the library
    /// stays open in the commands. Creates directories in the data directory; the catalog
    /// attach (which may rebuild a whole index) runs on a blocking thread.
    async fn serve(&self, view: &LibraryView) {
        let location = self.commands.location_of(&view.library_id);
        let root = location
            .as_deref()
            .and_then(|location| self.locations.root_of(location));
        let Some(root) = root else {
            warn_unserved(&view.library_id, "location_not_found");
            return;
        };
        let identity = location.and_then(|location| self.locations.identity_of(&location));
        self.attach(&view.library_id, &root, identity).await;
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
    /// (`conflict`, `unavailable` or `internal`, as the catalog classified it). Filesystem and
    /// database I/O.
    async fn attach(
        &self,
        library_id: &LibraryId,
        root: &filesquid_native::HostRoot,
        identity: Option<String>,
    ) {
        let Some(identity) = identity else {
            warn_unserved(library_id, "location_not_found");
            return;
        };
        let folder = LibraryFolder {
            library_id: library_id.clone(),
            repository_id: self.serving.repository.clone(),
            name: DEFAULT_LIBRARY_NAME.to_string(),
            root: root.clone(),
            identity,
        };
        let catalog = self.serving.catalog.clone();
        let attached = tokio::task::spawn_blocking(move || catalog.attach(&folder)).await;
        match attached {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => warn_unserved(library_id, attach_error_code(&err)),
            Err(_) => warn_unserved(library_id, "internal"),
        }
    }

    /// Bind with `bind`, then serve the bound library.
    async fn bind_and_serve(
        &self,
        bind: impl std::future::Future<
            Output = Result<LibraryView, filesquid_api::local_library::LocalLibraryError>,
        >,
    ) -> Result<LibraryView, filesquid_api::local_library::LocalLibraryError> {
        let view = bind.await?;
        self.serve(&view).await;
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
            self.bind_and_serve(self.commands.create(&actor, request))
                .await
        })
    }

    fn open(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move {
            self.bind_and_serve(self.commands.open(&actor, request))
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

    fn rebuild(&self, actor: String, request: LibraryRequest) -> LibraryFuture<'_, RebuildReport> {
        Box::pin(async move { self.commands.rebuild(&actor, request).await })
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
}
