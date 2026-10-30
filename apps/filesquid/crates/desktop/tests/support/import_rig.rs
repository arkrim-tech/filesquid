//! The desktop of one import test or scenario (Tasks DE-1.2, DE-1.7; API-S-036, DE-S-019): the
//! import route served through the `filesquid://` scheme with the per-launch token, a scripted
//! dialog in place of the native one, the native import worker, and one library in a folder
//! of a scratch directory that is removed when the rig is dropped, attached to its catalog
//! index with one destination collection (`ImportRig::collection`), and the navigation and listing routes
//! over that index. The job routes answer over the launch's job service with the import runner
//! installed (Task DE-1.9, DE-S-020), and a [`Gate`] can hold the worker on the file in hand,
//! after its commit and before its registration, until the test releases it.
//!
//! The including test crate MUST declare `mod journal` (`support/journal.rs`) at its root.

use filesquid_api::auth::LaunchToken;
use filesquid_api::imports::{ImportSource, ImportStartError};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{CommandRequest, LibraryCatalog, LibraryCommand, LibraryService};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::operation::OperationId as IntentOperation;
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::{ApiEvent, ApiState};
use filesquid_core::{CollectionId, InstallationId, LibraryId};
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::imports::{ImportStarting, SourceDialog};
use filesquid_desktop_lib::{
    import_build, install_file_scopes, install_import_route, install_import_runner,
    install_launch_token, installation_repository, open_host_monitor, open_library_catalog,
};
use filesquid_native::catalog::{
    LibraryFolder, PendingIntent, RegisterFile, Registration, DEFAULT_LIBRARY_NAME,
};
use filesquid_native::import::index::ImportIndex;
use filesquid_native::import::items::open_import_stores;
use filesquid_native::import::job::IMPORT_FILE_OPERATION;
use filesquid_native::import::worker::{ImportServices, ImportWorker};
use filesquid_native::workers::history_backfill::open_jobs;
use filesquid_native::{
    install_file_service, open_fs_library_root, HostFiles, HostLocations, HostMonitor,
    SqliteCatalog, SqliteJournal, JOURNAL_FILE,
};
use filesquid_ocfl::storage::FsBackend;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The library every rig imports into.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A library no rig opens.
pub const UNOPENED_LIBRARY: &str = "7a2e3d4c-5b6f-4a70-9b8c-0d1e2f3a4b5c";

/// The installation of every rig.
pub const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The folder every import of [`ImportRig::import`] files into.
pub const FOLDER: &str = "trips";

/// Longest wait for the worker to import the expected files.
const IMPORT_WAIT: Duration = Duration::from_secs(30);

/// Rig counter, so concurrent tests get separate scratch directories.
static RIGS: AtomicU32 = AtomicU32::new(0);

/// A fresh, empty scratch directory named after `name` (filesystem I/O).
pub fn fresh_dir(name: &str) -> PathBuf {
    let n = RIGS.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "fs-desktop-imports-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A dialog that answers `picked` and counts how often it was shown.
#[derive(Debug)]
pub struct ScriptedDialog {
    picked: Option<Vec<PathBuf>>,
    shown: Mutex<u32>,
}

impl ScriptedDialog {
    /// How often the dialog was shown.
    pub fn shown(&self) -> u32 {
        *self.shown.lock().expect("shown")
    }
}

/// The scripted dialog shared between the starter and the test.
struct SharedDialog(Arc<ScriptedDialog>);

impl SourceDialog for SharedDialog {
    fn pick(&self, _source: ImportSource) -> Result<Option<Vec<PathBuf>>, ImportStartError> {
        *self.0.shown.lock().expect("shown") += 1;
        Ok(self.0.picked.clone())
    }
}

/// Longest the gate holds the worker, so a test that never releases it cannot hang.
const GATE_HOLD: Duration = Duration::from_secs(30);

/// Holds the import worker on the first file registered after [`Gate::arm`], until
/// [`Gate::release`]: that file is committed and still in hand, before its checkpoint.
#[derive(Debug, Default)]
pub struct Gate {
    /// (armed, holding, released).
    state: Mutex<(bool, bool, bool)>,
    changed: Condvar,
}

impl Gate {
    /// Hold the next registration.
    pub fn arm(&self) {
        *self.state.lock().expect("gate") = (true, false, false);
    }

    /// Wait (at most [`GATE_HOLD`]) until the worker is held; answer whether it is.
    pub fn wait_held(&self) -> bool {
        let guard = self.state.lock().expect("gate");
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, GATE_HOLD, |(_, holding, _)| !*holding)
            .expect("gate");
        guard.1
    }

    /// Let the held worker go on.
    pub fn release(&self) {
        self.state.lock().expect("gate").2 = true;
        self.changed.notify_all();
    }

    /// Hold the calling worker if the gate is armed (once), at most [`GATE_HOLD`].
    fn pass(&self) {
        let mut guard = self.state.lock().expect("gate");
        if !guard.0 {
            return;
        }
        *guard = (false, true, false);
        self.changed.notify_all();
        let _released = self
            .changed
            .wait_timeout_while(guard, GATE_HOLD, |(_, _, released)| !*released)
            .expect("gate");
    }
}

/// The catalog index behind a [`Gate`].
struct GatedIndex {
    inner: Arc<SqliteCatalog>,
    gate: Arc<Gate>,
}

impl ImportIndex for GatedIndex {
    fn has_collection(
        &self,
        library: &LibraryId,
        collection: &CollectionId,
    ) -> Result<bool, String> {
        ImportIndex::has_collection(self.inner.as_ref(), library, collection)
    }

    fn path_taken(&self, collection: &CollectionId, path: &str) -> Result<bool, String> {
        ImportIndex::path_taken(self.inner.as_ref(), collection, path)
    }

    fn begin(&self, intent: &PendingIntent) -> Result<(), String> {
        ImportIndex::begin(self.inner.as_ref(), intent)
    }

    fn register(
        &self,
        library: &LibraryId,
        file: &RegisterFile,
        journal: JournalEntry,
    ) -> Result<Registration, String> {
        self.gate.pass();
        ImportIndex::register(self.inner.as_ref(), library, file, journal)
    }

    fn finish(&self, library: &LibraryId, operation: &IntentOperation) -> Result<(), String> {
        ImportIndex::finish(self.inner.as_ref(), library, operation)
    }
}

/// The desktop of one test.
pub struct ImportRig {
    /// The scratch directory: `data/`, the storage root `root/` and the picked sources.
    pub dir: PathBuf,
    /// The scripted dialog.
    pub dialog: Arc<ScriptedDialog>,
    /// The gate that can hold the worker on the file in hand.
    pub gate: Arc<Gate>,
    /// The library's catalog index, until the rig shuts down.
    catalog: Option<Arc<SqliteCatalog>>,
    /// The destination collection of [`ImportRig::import`].
    pub collection: String,
    state: ApiState,
    token: LaunchToken,
    host: Option<HostMonitor>,
    worker: Arc<ImportWorker>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for ImportRig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportRig").field("dir", &self.dir).finish()
    }
}

impl Drop for ImportRig {
    fn drop(&mut self) {
        self.shut_down();
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("scratch directory not removed: {err}");
        }
    }
}

/// The starter's parts of one launch.
struct Launch {
    catalog: Arc<SqliteCatalog>,
    collection: String,
    state: ApiState,
    token: LaunchToken,
    host: HostMonitor,
    worker: Arc<ImportWorker>,
    stop: Arc<AtomicBool>,
}

impl ImportRig {
    /// Launch the import route over a new, empty library in `dir` with a dialog that picks
    /// `picked` (`None`: the owner cancels). Filesystem and database I/O.
    pub async fn launch(dir: PathBuf, picked: Option<Vec<PathBuf>>) -> Self {
        let dialog = Arc::new(ScriptedDialog {
            picked,
            shown: Mutex::new(0),
        });
        let gate = Arc::new(Gate::default());
        let launch = start(&dir, &dialog, &gate, true).await;
        Self {
            dir,
            dialog,
            gate,
            catalog: Some(launch.catalog),
            collection: launch.collection,
            state: launch.state,
            token: launch.token,
            host: Some(launch.host),
            worker: launch.worker,
            stop: launch.stop,
        }
    }

    /// Quit (the running import job is suspended) and launch again over the same data
    /// directory, as a new process would, resuming every unfinished import job.
    pub async fn relaunch(&mut self) {
        self.shut_down();
        let launch = start(&self.dir, &self.dialog, &self.gate, false).await;
        self.catalog = Some(launch.catalog);
        self.collection = launch.collection;
        self.state = launch.state;
        self.token = launch.token;
        self.host = Some(launch.host);
        self.worker = launch.worker;
        self.stop = launch.stop;
    }

    /// The library's catalog index.
    pub fn catalog(&self) -> &SqliteCatalog {
        self.catalog.as_deref().expect("the rig is running")
    }

    /// Stop the import worker, as a quit does, leaving the route installed.
    pub fn stop_worker(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.wait(Duration::from_secs(10));
    }

    /// Stop the worker and the monitor.
    fn shut_down(&mut self) {
        self.stop_worker();
        if let Some(host) = self.host.take() {
            host.stop();
        }
        // Drop the route's state, and with it every SQLite connection, before the directory
        // goes (Windows cannot remove an open database file).
        self.state = ApiState::new();
        self.catalog = None;
    }

    /// POST an import of the `source` of `library` under `operation` through the scheme as
    /// the owner, asserting that the answer names no scratch path.
    pub async fn import(&self, library: &str, operation: &str, source: &str) -> (u16, Value) {
        let destination = json!({ "collection_id": self.collection, "folder": FOLDER });
        self.import_into(library, operation, source, destination)
            .await
    }

    /// POST an import as [`ImportRig::import`] does, naming `destination` (an object of
    /// `collection_id` and `folder`, either left out to test its refusal).
    pub async fn import_into(
        &self,
        library: &str,
        operation: &str,
        source: &str,
        destination: Value,
    ) -> (u16, Value) {
        let mut body = json!({
            "envelope": {
                "schema_version": 1,
                "operation_id": operation,
                "expected_revision": "r1",
                "scope": format!("library:{library}")
            },
            "library_id": library,
            "source": source
        });
        if let (Some(body), Some(destination)) = (body.as_object_mut(), destination.as_object()) {
            body.extend(destination.clone());
        }
        let bytes = body.to_string().into_bytes();
        self.send("POST", "/api/v1/imports", bytes).await
    }

    /// POST `path` with no body through the scheme as the owner (the job controls).
    pub async fn post(&self, path: &str) -> (u16, Value) {
        self.send("POST", path, Vec::new()).await
    }

    /// A subscriber to this launch's event bus, the bus the desktop event channel forwards.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ApiEvent> {
        self.state.events.subscribe()
    }

    /// GET `path` through the scheme as the owner.
    pub async fn get(&self, path: &str) -> (u16, Value) {
        self.send("GET", path, Vec::new()).await
    }

    /// Send `method` `path` with `body` through the scheme as the owner, asserting that the
    /// answer names no scratch path.
    async fn send(&self, method: &str, path: &str, body: Vec<u8>) -> (u16, Value) {
        let request = http::Request::builder()
            .method(method)
            .uri(format!("filesquid://localhost{path}"))
            .header(http::header::ORIGIN, "tauri://localhost")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.token.secret()),
            )
            .body(body)
            .expect("request");
        let response = handle(self.state.clone(), request).await;
        let text = String::from_utf8_lossy(response.body()).to_string();
        assert!(
            !text.contains(self.dir.to_string_lossy().as_ref()),
            "{text}"
        );
        (
            response.status().as_u16(),
            serde_json::from_str(&text).unwrap_or(Value::Null),
        )
    }

    /// Wait (at most [`IMPORT_WAIT`]) until `count` files are journaled as imported, and
    /// answer how many are.
    pub fn wait_for_files(&self, count: usize) -> usize {
        let deadline = Instant::now() + IMPORT_WAIT;
        let mut done = self.imported().len();
        while done < count && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            done = self.imported().len();
        }
        done
    }

    /// The `import.file` entries of the journal.
    pub fn imported(&self) -> Vec<JournalEntry> {
        self.journal()
            .into_iter()
            .filter(|e| e.operation == IMPORT_FILE_OPERATION)
            .collect()
    }

    /// Every entry of the host's journal store. Database I/O.
    pub fn journal(&self) -> Vec<JournalEntry> {
        let store = SqliteJournal::open(&self.dir.join("data").join(JOURNAL_FILE), "desktop")
            .expect("journal");
        crate::journal::all_entries(&store)
    }

    /// How many file objects the library's storage root holds (filesystem I/O).
    pub fn file_objects(&self) -> usize {
        std::fs::read_dir(self.dir.join("root").join("files"))
            .map(|dir| dir.count())
            .unwrap_or(0)
    }
}

/// A folder `<dir>/Holidays` with two files (filesystem I/O).
pub fn holiday_folder(dir: &Path) -> PathBuf {
    let folder = dir.join("Holidays");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("Beach day.jpg"), b"jpeg").expect("a");
    std::fs::write(folder.join("Notes.txt"), b"notes").expect("b");
    folder
}

/// Create the empty storage root of [`LIBRARY`] at `<dir>/root` (filesystem I/O).
async fn create_root(dir: &Path, host: &HostMonitor) {
    let root = dir.join("root");
    std::fs::create_dir_all(&root).expect("root");
    let backend = FsBackend::new(&root, dir.join("root-parts")).expect("backend");
    let id = LibraryId::new(LIBRARY).expect("library");
    let service = LocalLibraryService::new(
        host.monitor().journal(),
        Arc::new(MemoryRepairLedger::new()),
    );
    service
        .create_root("owner", &id, &backend)
        .await
        .expect("create root");
}

/// Launch the import route over `<dir>/data`: the token, the file service with [`LIBRARY`]
/// open at `<dir>/root` (created first when `create`, with its destination collection),
/// attached to its catalog index, the navigation and listing over it, the jobs, the plans,
/// the worker (which runs every unfinished import job when idle, through the index behind
/// `gate`), the job routes with the import runner, and the route with `dialog`.
async fn start(dir: &Path, dialog: &Arc<ScriptedDialog>, gate: &Arc<Gate>, create: bool) -> Launch {
    let state = ApiState::new();
    let token = install_launch_token(&state).expect("token");
    let files = install_file_service(&state).expect("files");
    let data = dir.join("data");
    let host = open_host_monitor(&state, &data, None).expect("monitor");
    if create {
        create_root(dir, &host).await;
    }
    let installation = InstallationId::parse(INSTALLATION).expect("installation");
    let catalog = Arc::new(open_library_catalog(&data, &host, &installation).expect("catalog"));
    open_library(dir, &files, &catalog, &installation);
    let collection = match create {
        true => create_collection(&catalog, &host),
        false => first_collection(&catalog),
    };
    filesquid_desktop_lib::navigation::install_navigation(&state, catalog.clone(), &installation)
        .expect("navigation");
    install_file_scopes(&files, &catalog).expect("scopes");
    let (plans, items) = open_import_stores(&data.join(JOURNAL_FILE)).expect("stores");
    let jobs = open_jobs(&data, &host)
        .expect("jobs")
        .with_items(items.clone())
        .with_events(state.events.clone());
    assert!(state.install_jobs(Arc::new(jobs.clone())), "the job routes");
    let stop = Arc::new(AtomicBool::new(false));
    let worker = Arc::new(
        ImportWorker::spawn(ImportServices {
            jobs: jobs.clone(),
            files: files.clone(),
            index: Arc::new(GatedIndex {
                inner: catalog.clone(),
                gate: gate.clone(),
            }),
            plans: plans.clone(),
            items,
            journal: host.monitor().journal(),
            build: import_build(),
            actor: "owner".into(),
            staging_dir: data.clone(),
            stop: stop.clone(),
            retry_unit: Duration::from_millis(1),
            recover: true,
        })
        .expect("worker"),
    );
    let starting = ImportStarting {
        files,
        jobs,
        plans,
        worker: worker.clone(),
        dialog: Box::new(SharedDialog(dialog.clone())),
    };
    install_import_route(&state, starting).expect("route");
    install_import_runner(&state, worker.clone()).expect("runner");
    Launch {
        catalog,
        collection,
        state,
        token,
        host,
        worker,
        stop,
    }
}

/// Serve the storage root `<dir>/root` as [`LIBRARY`] through `files` and attach it to its
/// index in `catalog`, through one set of leases, as the desktop's library routes do (creates
/// directories).
fn open_library(
    dir: &Path,
    files: &HostFiles,
    catalog: &SqliteCatalog,
    installation: &InstallationId,
) {
    let id = LibraryId::new(LIBRARY).expect("library");
    let data = dir.join("data");
    let locations = HostLocations::new(&data);
    let location = locations.register(&dir.join("root")).expect("location");
    let root = locations.root_of(&location).expect("root");
    open_fs_library_root(files, &id, &root, &data).expect("opened");
    let folder = LibraryFolder {
        library_id: id,
        repository_id: installation_repository(installation).expect("repository"),
        name: DEFAULT_LIBRARY_NAME.into(),
        root,
        identity: locations.identity_of(&location).expect("identity"),
    };
    catalog.attach(&folder).expect("attached");
}

/// Create the destination collection "Holidays" in [`LIBRARY`] through the library commands;
/// answer its ID.
fn create_collection(catalog: &Arc<SqliteCatalog>, host: &HostMonitor) -> String {
    let library = LibraryId::new(LIBRARY).expect("library");
    let revision = catalog
        .library(&library)
        .expect("read")
        .expect("library")
        .revision;
    let service = LibraryService::new(catalog.clone(), host.monitor().journal());
    let request = CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new("op-collection").expect("op"),
            ExpectedRevision::new(revision).expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::CreateCollection {
            library_id: library,
            title: "Holidays".into(),
            folder_id: None,
            sort: Default::default(),
        },
    };
    let created = service.execute("owner", request).expect("collection");
    created.created_id.expect("collection id")
}

/// The ID of [`LIBRARY`]'s first collection (a relaunch finds the one the launch created).
fn first_collection(catalog: &SqliteCatalog) -> String {
    let library = LibraryId::new(LIBRARY).expect("library");
    let collections = catalog.collections(&library, None, 1).expect("collections");
    collections[0].item.id().as_str().to_string()
}
