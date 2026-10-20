//! Task DE-1.2: the desktop's import route (API-S-036, DE-S-019) through the `filesquid://`
//! scheme with the per-launch token: the owner picks a folder in the host's dialog (scripted
//! here), one durable import job runs on the native worker and commits every file as its own
//! object with its record; the same operation ID answers the same job; a cancelled dialog makes
//! no job; a library that is not open is refused; and no answer or journal entry names a path.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::auth::LaunchToken;
use filesquid_api::imports::{ImportSource, ImportStartError, IMPORT_START_OPERATION};
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
    install_file_service, open_fs_library, HostMonitor, SqliteJournal, JOURNAL_FILE,
};
use filesquid_ocfl::storage::FsBackend;
use journal::block_on;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The library every test imports into.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A fresh, empty directory for one test (filesystem I/O).
fn fresh_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fs-desktop-imports-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A dialog that answers `picked` and counts how often it was shown.
struct ScriptedDialog {
    picked: Option<Vec<PathBuf>>,
    shown: Mutex<u32>,
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
struct Desktop {
    dir: PathBuf,
    state: ApiState,
    token: LaunchToken,
    host: HostMonitor,
    worker: Arc<ImportWorker>,
    stop: Arc<AtomicBool>,
    dialog: Arc<ScriptedDialog>,
}

impl Desktop {
    /// Launch the import route over an empty library with a dialog that picks `picked`.
    fn launch(dir: PathBuf, picked: Option<Vec<PathBuf>>) -> Self {
        let state = ApiState::new();
        let token = install_launch_token(&state).expect("token");
        let files = install_file_service(&state).expect("files");
        let data = dir.join("data");
        let host = open_host_monitor(&state, &data, None).expect("monitor");
        open_library(&dir, &files, &host);
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
        let dialog = Arc::new(ScriptedDialog {
            picked,
            shown: Mutex::new(0),
        });
        let starting = ImportStarting {
            files,
            jobs,
            plans,
            worker: worker.clone(),
            dialog: Box::new(SharedDialog(dialog.clone())),
        };
        install_import_route(&state, starting).expect("route");
        Self {
            dir,
            state,
            token,
            host,
            worker,
            stop,
            dialog,
        }
    }

    /// POST an import of `library` under `operation` through the scheme as the owner.
    fn import(&self, library: &str, operation: &str) -> (u16, Value) {
        let body = json!({
            "envelope": {
                "schema_version": 1,
                "operation_id": operation,
                "expected_revision": "r1",
                "scope": format!("library:{library}")
            },
            "library_id": library,
            "source": "folder"
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
        let response = block_on(handle(self.state.clone(), request));
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

    /// Wait (bounded) until `count` files are journaled as imported.
    fn wait_for_files(&self, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let journal = SqliteJournal::open(&self.dir.join("data").join(JOURNAL_FILE), "d");
            let done = journal::all_entries(&journal.expect("journal"))
                .iter()
                .filter(|e| e.operation == IMPORT_FILE_OPERATION)
                .count();
            if done >= count {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Stop the worker and the monitor, answering the journal's entries.
    fn stop(self) -> Vec<JournalEntry> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.worker.wait(Duration::from_secs(10));
        self.host.stop();
        let store = SqliteJournal::open(&self.dir.join("data").join(JOURNAL_FILE), "desktop")
            .expect("journal");
        journal::all_entries(&store)
    }
}

/// An empty storage root for [`LIBRARY`] at `<dir>/root`, served by `files`.
fn open_library(dir: &Path, files: &filesquid_native::HostFiles, host: &HostMonitor) {
    let root = dir.join("root");
    std::fs::create_dir_all(&root).expect("root");
    let backend = FsBackend::new(&root, dir.join("root-parts")).expect("backend");
    let id = LibraryId::new(LIBRARY).expect("library");
    let service = LocalLibraryService::new(
        host.monitor().journal(),
        Arc::new(MemoryRepairLedger::new()),
    );
    block_on(service.create_root("owner", &id, &backend)).expect("create root");
    open_fs_library(files, &id, &root, &dir.join("data")).expect("opened");
}

/// A folder `<dir>/Holidays` with two files.
fn holiday_folder(dir: &Path) -> PathBuf {
    let folder = dir.join("Holidays");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("Beach day.jpg"), b"jpeg").expect("a");
    std::fs::write(folder.join("Notes.txt"), b"notes").expect("b");
    folder
}

#[test]
fn a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job() {
    let dir = fresh_dir("folder");
    let folder = holiday_folder(&dir);
    let desktop = Desktop::launch(dir.clone(), Some(vec![folder]));
    let (status, answer) = desktop.import(LIBRARY, "op-import-1");
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["outcome"], "started");
    let job = answer["job_id"].as_str().expect("job").to_string();
    let (status, again) = desktop.import(LIBRARY, "op-import-1");
    assert_eq!(status, 200, "{again}");
    assert_eq!(
        again["job_id"],
        job.as_str(),
        "the same operation answers the same job"
    );
    assert_eq!(
        *desktop.dialog.shown.lock().expect("shown"),
        1,
        "no second dialog"
    );
    desktop.wait_for_files(2);
    let entries = desktop.stop();
    let files: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == IMPORT_FILE_OPERATION)
        .collect();
    assert_eq!(files.len(), 2, "each file imported once");
    let text = format!("{entries:?}");
    assert!(
        !text.contains("Beach day") && !text.contains("Holidays"),
        "no names"
    );
    let objects = std::fs::read_dir(dir.join("root").join("files"))
        .expect("file objects")
        .count();
    assert_eq!(objects, 2, "one object per file");
}

#[test]
fn a_cancelled_dialog_or_an_unopened_library_makes_no_job() {
    let dir = fresh_dir("cancel");
    let desktop = Desktop::launch(dir.clone(), None);
    let (status, answer) = desktop.import(LIBRARY, "op-cancel");
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer,
        json!({ "operation_id": "op-cancel", "outcome": "cancelled" })
    );
    let other = "7a2e3d4c-5b6f-4a70-9b8c-0d1e2f3a4b5c";
    let (status, _) = desktop.import(other, "op-other");
    assert_eq!(status, 404, "that library is not open");
    let entries = desktop.stop();
    let starts: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == IMPORT_START_OPERATION)
        .collect();
    assert_eq!(starts.len(), 2, "each request journaled once");
    assert!(!dir.join("root").join("files").exists(), "nothing imported");
}
