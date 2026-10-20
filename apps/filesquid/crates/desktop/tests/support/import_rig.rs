//! The desktop of one import test or scenario (Task DE-1.2; API-S-036, DE-S-019): the import
//! route served through the `filesquid://` scheme with the per-launch token, a scripted
//! dialog in place of the native one, the native import worker, and one folder-bound library
//! whose OCFL storage root lies in a scratch directory that is removed when the rig is dropped.
//!
//! The including test crate MUST declare `mod journal` (`support/journal.rs`) at its root.

use filesquid_api::auth::LaunchToken;
use filesquid_api::imports::{ImportSource, ImportStartError};
use filesquid_api::journal::JournalEntry;
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::ApiState;
use filesquid_core::LibraryId;
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::imports::{ImportStarting, SourceDialog};
use filesquid_desktop_lib::{
    import_build, install_import_route, install_launch_token, open_host_monitor,
};
use filesquid_native::import::job::IMPORT_FILE_OPERATION;
use filesquid_native::import::plans::SqliteImportPlans;
use filesquid_native::import::worker::{ImportServices, ImportWorker};
use filesquid_native::workers::history_backfill::open_jobs;
use filesquid_native::{
    install_file_service, open_fs_library, HostFiles, HostMonitor, SqliteJournal, JOURNAL_FILE,
};
use filesquid_ocfl::storage::FsBackend;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The library every rig imports into.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A library no rig opens.
pub const UNOPENED_LIBRARY: &str = "7a2e3d4c-5b6f-4a70-9b8c-0d1e2f3a4b5c";

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

/// The desktop of one test.
pub struct ImportRig {
    /// The scratch directory: `data/`, the storage root `root/` and the picked sources.
    pub dir: PathBuf,
    /// The scripted dialog.
    pub dialog: Arc<ScriptedDialog>,
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
        let launch = start(&dir, &dialog, false, true).await;
        Self {
            dir,
            dialog,
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
        let launch = start(&self.dir, &self.dialog, true, false).await;
        self.state = launch.state;
        self.token = launch.token;
        self.host = Some(launch.host);
        self.worker = launch.worker;
        self.stop = launch.stop;
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
    }

    /// POST an import of the `source` of `library` under `operation` through the scheme as
    /// the owner, asserting that the answer names no scratch path.
    pub async fn import(&self, library: &str, operation: &str, source: &str) -> (u16, Value) {
        let body = json!({
            "envelope": {
                "schema_version": 1,
                "operation_id": operation,
                "expected_revision": "r1",
                "scope": format!("library:{library}")
            },
            "library_id": library,
            "source": source
        });
        let request = http::Request::post("filesquid://localhost/api/v1/imports")
            .header(http::header::ORIGIN, "tauri://localhost")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.token.secret()),
            )
            .body(body.to_string().into_bytes())
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
/// open at `<dir>/root` (created first when `create`), the jobs, the plans, the worker (queuing
/// every unfinished import job when `resume`) and the route with `dialog`.
async fn start(dir: &Path, dialog: &Arc<ScriptedDialog>, resume: bool, create: bool) -> Launch {
    let state = ApiState::new();
    let token = install_launch_token(&state).expect("token");
    let files = install_file_service(&state).expect("files");
    let data = dir.join("data");
    let host = open_host_monitor(&state, &data, None).expect("monitor");
    if create {
        create_root(dir, &host).await;
    }
    open_library(dir, &files);
    let jobs = open_jobs(&data, &host).expect("jobs");
    let plans = Arc::new(SqliteImportPlans::open(&data.join(JOURNAL_FILE)).expect("plans"));
    let stop = Arc::new(AtomicBool::new(false));
    let worker = Arc::new(
        ImportWorker::spawn(ImportServices {
            jobs: jobs.clone(),
            files: files.clone(),
            plans: plans.clone(),
            journal: host.monitor().journal(),
            build: import_build(),
            actor: "owner".into(),
            staging_dir: data.clone(),
            stop: stop.clone(),
            retry_unit: Duration::from_millis(1),
        })
        .expect("worker"),
    );
    if resume {
        worker.resume_unfinished(&jobs);
    }
    let starting = ImportStarting {
        files,
        jobs,
        plans,
        worker: worker.clone(),
        dialog: Box::new(SharedDialog(dialog.clone())),
    };
    install_import_route(&state, starting).expect("route");
    Launch {
        state,
        token,
        host,
        worker,
        stop,
    }
}

/// Serve the storage root `<dir>/root` as [`LIBRARY`] through `files` (creates directories).
fn open_library(dir: &Path, files: &HostFiles) {
    let id = LibraryId::new(LIBRARY).expect("library");
    open_fs_library(files, &id, &dir.join("root"), &dir.join("data")).expect("opened");
}
