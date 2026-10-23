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
//! root and leases ([`open_fs_library_root`]), schedules its OCFL validation and lists its
//! root for the PROV-O history ([`OpenedRoots`]). No path ever reaches an answer.
//!
//! The repair is the desktop owner's alone: the per-launch token's holder is the only
//! principal this host authenticates, and the API refuses the repair for anyone else.

use filesquid_api::auth::Principal;
use filesquid_api::local_library::{
    CopyReport, LibraryCommands, LibraryFacts, LibraryFuture, LibraryOperations, LibraryRequest,
    LibraryView, LocalLibraryService, LocationRequest, Readable, RebuildEnd, RebuildProgress,
    RecentLibraries, RenameLibraryRequest, RepairAnswer, RepairCandidates, RepairRequest,
    SnapshotCopyRequest,
};
use filesquid_api::ocfl_validation::ValidationService;
use filesquid_core::LibraryId;
use filesquid_native::{open_fs_library_root, HostFiles, HostLocations, OpenedRoots};
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

    /// Serve the bound library of `view`: its files from its folder's root and leases, its
    /// validation and its history. A part that cannot be served is logged as a warning with
    /// its error code only (the journal layer journals it); the library stays open in the
    /// commands. Creates directories in the data directory.
    fn serve(&self, view: &LibraryView) {
        let root = self
            .commands
            .location_of(&view.library_id)
            .and_then(|location| self.locations.root_of(&location));
        let Some(root) = root else {
            warn_unserved(&view.library_id, "location_not_found");
            return;
        };
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

    /// Bind with `bind`, then serve the bound library.
    async fn bind_and_serve(
        &self,
        bind: impl std::future::Future<
            Output = Result<LibraryView, filesquid_api::local_library::LocalLibraryError>,
        >,
    ) -> Result<LibraryView, filesquid_api::local_library::LocalLibraryError> {
        let view = bind.await?;
        self.serve(&view);
        Ok(view)
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

    fn rename(
        &self,
        actor: String,
        request: RenameLibraryRequest,
    ) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { self.commands.rename(&actor, request).await })
    }

    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { self.commands.facts(&actor, &library_id).await })
    }

    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries> {
        Box::pin(async move { self.commands.recent(&actor, readable.as_ref()).await })
    }
}
