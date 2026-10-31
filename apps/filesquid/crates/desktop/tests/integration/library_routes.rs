//! Tasks DE-1.1 and DE-1.7: the desktop's library and repair routes (API-S-025) over folders
//! the owner chose. The host registers each folder under an opaque location ID, injects its
//! storage root and the persistent repair ledger, and serves each library it binds (files,
//! its catalog index in the data directory, validation, PROV-O history). Every library lives
//! in a folder the owner chose; the data directory keeps only its rebuildable index (ADR 041).
//! Driven through the `filesquid://` scheme with the per-launch token, against SQLite files
//! and OCFL storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::auth::LaunchToken;
use filesquid_api::history::backfill::BACKFILL_OPERATION;
use filesquid_api::history::launch::BACKFILL_REQUEST_OPERATION;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{CommandRequest, LibraryCommand, LibraryService};
use filesquid_api::local_library::choice::{
    ChoicePurpose, ChooseError, LocationChooser, CHOOSE_OPERATION,
};
use filesquid_api::local_library::{CONSENT_STATEMENT_ID, REPAIR_OPERATION, UNTITLED_LIBRARY};
use filesquid_api::navigation::{AlwaysTrue, NavigationChange};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::preferences::PreferenceStore;
use filesquid_api::{ApiEvent, ApiState};
use filesquid_core::{InstallationId, LibraryId, RepositoryId};
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::backfill::{BackfillTask, DesktopBackfill};
use filesquid_desktop_lib::folder_choice::{dialog_title, DesktopChooser, FolderDialog};
use filesquid_desktop_lib::libraries::LibraryServing;
use filesquid_desktop_lib::navigation::{install_navigation, owner_event_filter};
use filesquid_desktop_lib::{
    history_backfill, install_backfill_route, install_file_provenance, install_file_scopes,
    install_folder_choice, install_history_source, install_host_preferences, install_job_routes,
    install_launch_token, install_library_routes, open_host_monitor, open_library_catalog,
};
use filesquid_native::catalog::ENTITY_RENAME_OPERATION;
use filesquid_native::workers::history_backfill::{backfill_source, open_jobs};
use filesquid_native::{
    install_file_service, HostFiles, HostLocations, HostMonitor, OpenedRoots, SqliteCatalog,
    SqliteJournal, SqlitePreferences, SqliteRepairLedger, CATALOG_DIR, JOURNAL_FILE,
};
use journal::block_on;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Upper bound on job reads by [`Desktop::finished_job`] (AGENTS.md §22).
const MAX_JOB_POLLS: usize = 500;

/// The installation UUID of these tests.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The installation of these tests.
fn installation() -> InstallationId {
    InstallationId::parse(INSTALLATION).expect("installation")
}

/// A fresh, empty directory for one test (filesystem I/O).
fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fs-desktop-libraries-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A command envelope for `op` at `revision` in `scope`.
fn envelope(op: &str, revision: &str, scope: &str) -> CommandEnvelope {
    CommandEnvelope::new(
        OperationId::new(op).expect("op"),
        ExpectedRevision::new(revision).expect("revision"),
        OperationScope::new(scope).expect("scope"),
    )
}

/// Operation IDs handed out by [`Desktop::command`].
static OPERATIONS: AtomicU64 = AtomicU64::new(0);

/// The name of a [`donor_library`].
const DONOR_NAME: &str = "Donor archive";

/// A library made by another desktop in the folder `<dir>/library` the owner chose there,
/// named [`DONOR_NAME`], with two snapshot versions (its creation and a collection "Holidays", each with its PROV-O
/// record), answering its ID and the folder.
fn donor_library(dir: &Path) -> (String, PathBuf) {
    let donor = Desktop::launch(dir.join("data"));
    let folder = dir.join("library");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = donor.register(&folder);
    let body = json!({ "location_id": location, "name": DONOR_NAME });
    let (status, body) = donor.command("/api/v1/libraries", body);
    assert_eq!(status, 200, "{body}");
    let library = body["library_id"].as_str().expect("library id").to_string();
    donor.create_collection(&library, "Holidays");
    donor.stop();
    (library, folder)
}

/// The desktop of one test: its API state, token, monitor, file service, folder registry and
/// the roots the history reads.
struct Desktop {
    dir: PathBuf,
    state: ApiState,
    token: LaunchToken,
    host: HostMonitor,
    catalog: Arc<SqliteCatalog>,
    files: HostFiles,
    locations: Arc<HostLocations>,
    opened: Arc<OpenedRoots>,
    /// The long-lived runtime the rebuild runner runs jobs on, as the app's runtime.
    runtime: tokio::runtime::Runtime,
    /// The app's exit flag the rebuild runner was given.
    quitting: Arc<AtomicBool>,
}

impl Desktop {
    /// Launch the host's library services in `dir`, as `start_monitor` does.
    fn launch(dir: PathBuf) -> Self {
        let state = ApiState::new();
        let token = install_launch_token(&state).expect("token");
        let files = install_file_service(&state).expect("files");
        let host = open_host_monitor(&state, &dir, None).expect("monitor");
        let catalog =
            Arc::new(open_library_catalog(&dir, &host, &installation()).expect("catalog"));
        let opened = Arc::new(OpenedRoots::new());
        install_history_source(&state, catalog.clone(), opened.clone()).expect("history");
        install_file_provenance(&files, &host, &installation()).expect("provenance");
        let serving = LibraryServing {
            files: files.clone(),
            validation: None,
            opened: opened.clone(),
            data_dir: dir.clone(),
            catalog: catalog.clone(),
            repository: RepositoryId::new(INSTALLATION).expect("repository"),
            rebuilds: Some(state.clone()),
        };
        let locations =
            install_library_routes(&state, &host, &installation(), serving).expect("routes");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_time()
            .build()
            .expect("runtime");
        let jobs = open_jobs(&dir, &host).expect("jobs");
        let quitting = Arc::new(AtomicBool::new(false));
        install_job_routes(&state, &jobs, runtime.handle().clone(), quitting.clone())
            .expect("job routes");
        Self {
            dir,
            state,
            token,
            host,
            catalog,
            files,
            locations,
            opened,
            runtime,
            quitting,
        }
    }

    /// Create a collection titled `title` in the attached `library` through the library
    /// commands; answer its ID.
    fn create_collection(&self, library: &str, title: &str) -> String {
        let library = LibraryId::new(library).expect("library");
        let revision = {
            use filesquid_api::library::LibraryCatalog;
            self.catalog
                .library(&library)
                .expect("read")
                .expect("attached")
                .revision
        };
        let service = LibraryService::new(self.catalog.clone(), self.host.monitor().journal());
        let request = CommandRequest {
            envelope: envelope("op-collection", &revision, "library"),
            command: LibraryCommand::CreateCollection {
                library_id: library,
                title: title.into(),
                folder_id: None,
                sort: Default::default(),
            },
        };
        let created = service.execute("owner", request).expect("collection");
        created.created_id.expect("collection id")
    }

    /// Read job `id` until it has finished (at most [`MAX_JOB_POLLS`] reads, 10 ms apart);
    /// answer its view.
    fn finished_job(&self, id: &str) -> Value {
        let mut view = Value::Null;
        for _ in 0..MAX_JOB_POLLS {
            let (status, body) = self.get(&format!("/api/v1/jobs/{id}"));
            assert_eq!(status, 200, "{body}");
            view = body;
            let state = view["state"].as_str().unwrap_or_default();
            if ["completed", "failed", "cancelled"].contains(&state) {
                return view;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the job did not finish: {view}");
    }

    /// The name of `library`'s entity in its catalog index (its navigation title).
    fn entity_name(&self, library: &str) -> String {
        let library = LibraryId::new(library).expect("library");
        self.catalog.library_name(&library).expect("entity name")
    }

    /// The titles the navigation lists under the installation's repository, in order.
    fn navigation_titles(&self) -> Vec<String> {
        let (status, page) = self.get("/api/v1/navigation");
        assert_eq!(status, 200, "{page}");
        let repository = page["nodes"][0]["node"]
            .as_str()
            .expect("handle")
            .to_string();
        let (status, page) = self.get(&format!("/api/v1/navigation?node={repository}"));
        assert_eq!(status, 200, "{page}");
        let nodes = page["nodes"].as_array().expect("nodes");
        nodes
            .iter()
            .filter(|n| n["kind"] == "library")
            .map(|n| n["name"].as_str().expect("name").to_string())
            .collect()
    }

    /// Register `folder` as the owner's choice.
    fn register(&self, folder: &Path) -> String {
        self.locations.register(folder).expect("registered")
    }

    /// Send `method` `path` with `body` through the scheme, with the token when `owner`.
    fn send(&self, method: &str, path: &str, body: Option<Value>, owner: bool) -> (u16, Value) {
        let mut request = http::Request::builder()
            .method(method)
            .uri(format!("filesquid://localhost{path}"))
            .header(http::header::ORIGIN, "tauri://localhost")
            .header(http::header::CONTENT_TYPE, "application/json");
        if owner {
            let bearer = format!("Bearer {}", self.token.secret());
            request = request.header(http::header::AUTHORIZATION, bearer);
        }
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        let response = block_on(handle(
            self.state.clone(),
            request.body(bytes).expect("req"),
        ));
        let status = response.status().as_u16();
        let text = String::from_utf8_lossy(response.body()).to_string();
        // Every library folder of a test sits beside its data directory, below the test root.
        let root = self.dir.parent().unwrap_or(&self.dir);
        assert!(
            !text.contains(root.to_string_lossy().as_ref()),
            "no host path in an answer: {text}"
        );
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// POST `body` to `path` as the owner.
    fn post(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("POST", path, Some(body), true)
    }

    /// POST the library command `input` to `path` as the owner, under a fresh operation ID
    /// with the scope and expected revision the route takes (API-F2.19): `location:<id>`
    /// and `0` for create and open, `library:<id>` and `1` for copy and rebuild. Answers the
    /// status and, for a 200, the outcome's `result`, else the whole body.
    fn command(&self, path: &str, input: Value) -> (u16, Value) {
        let binds = path == "/api/v1/libraries" || path.ends_with("/openings");
        let (scope, revision) = match binds {
            true => (
                format!("location:{}", input["location_id"].as_str().unwrap_or("")),
                "0",
            ),
            false => (
                format!("library:{}", input["library_id"].as_str().unwrap_or("")),
                "1",
            ),
        };
        let operation = format!(
            "op-{}-{}",
            std::process::id(),
            OPERATIONS.fetch_add(1, Ordering::SeqCst)
        );
        let body = json!({
            "envelope": {
                "schema_version": 1,
                "operation_id": operation,
                "expected_revision": revision,
                "scope": scope,
            },
            "input": input,
        });
        let (status, answer) = self.post(path, body);
        match status {
            200 => (status, answer["result"].clone()),
            _ => (status, answer),
        }
    }

    /// GET `path` as the owner.
    fn get(&self, path: &str) -> (u16, Value) {
        self.send("GET", path, None, true)
    }

    /// Stop the monitor (flushing the journal) and answer the journal's entries.
    fn stop(self) -> (PathBuf, Vec<JournalEntry>) {
        self.runtime.shutdown_timeout(Duration::from_secs(5));
        drop(self.catalog);
        self.host.stop();
        let store = SqliteJournal::open(&self.dir.join(JOURNAL_FILE), "desktop").expect("journal");
        (self.dir.clone(), journal::all_entries(&store))
    }
}

/// A consented repair body for `objects` of `library`.
fn consented(library: &str, objects: &[&str]) -> Value {
    json!({
        "library_id": library,
        "object_ids": objects,
        "consent": {
            "library_id": library,
            "object_ids": objects,
            "statement_id": CONSENT_STATEMENT_ID,
            "confirmed": true
        }
    })
}

/// Remove the version inventory and sidecar of `version` of the snapshot object in `root`:
/// a W010 head.
fn make_w010(root: &Path, version: &str) {
    let head = root.join("snapshot").join(version);
    std::fs::remove_file(head.join("inventory.json")).expect("W010 head");
    std::fs::remove_file(head.join("inventory.json.sha256")).expect("W010 sidecar");
}

#[test]
fn create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files() {
    let dir = fresh_dir("create");
    let desktop = Desktop::launch(dir.join("data"));
    let folder = dir.join("My library");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = desktop.register(&folder);
    let (status, body) = desktop.command("/api/v1/libraries", json!({ "location_id": location }));
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["spec_version"], "1.1");
    let library = LibraryId::new(body["library_id"].as_str().expect("id")).expect("library");
    assert!(desktop.files.is_open(&library), "the file routes serve it");
    let opened = desktop.opened.list().expect("opened");
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].0, library);
    assert!(
        folder.join("0=ocfl_1.1").is_file(),
        "the root is in the chosen folder"
    );
    let (status, body) = desktop.send("POST", "/api/v1/libraries", Some(json!({})), false);
    assert_eq!(status, 401, "{body}");
    let (_, entries) = desktop.stop();
    let denied = entries.iter().filter(|e| e.operation == "api.auth.denied");
    assert_eq!(denied.count(), 1, "the refusal is journaled once");
}

#[test]
fn an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths() {
    let dir = fresh_dir("open");
    let (library, root) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    let (status, body) = desktop.get("/api/v1/history");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["items"].as_array().expect("items").len(),
        0,
        "nothing open yet"
    );
    let location = desktop.register(&root);
    let body = json!({ "location_id": location, "library_id": library });
    let (status, answer) = desktop.command("/api/v1/libraries/openings", body);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["library_id"], library.as_str());
    let (status, body) = desktop.get("/api/v1/history");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["items"].as_array().expect("items").len(),
        2,
        "the chosen root's records: {body}"
    );
    let listing =
        format!("/api/v1/files/listing?library_id={library}&object_id=urn:uuid:{library}");
    let (status, page) = desktop.get(&listing);
    assert_eq!(
        status, 200,
        "the file routes serve the opened folder: {page}"
    );
    assert!(
        !page["entries"].as_array().expect("entries").is_empty(),
        "{page}"
    );
    let listing =
        format!("/api/v1/files/listing?library_id={library}&object_id=urn:uuid:{library}");
    let (status, page) = desktop.get(&listing);
    assert_eq!(
        status, 200,
        "the file routes serve the opened folder: {page}"
    );
    assert!(
        !page["entries"].as_array().expect("entries").is_empty(),
        "{page}"
    );
    let copy = dir.join("copy");
    std::fs::create_dir_all(&copy).expect("copy folder");
    let destination = desktop.register(&copy);
    let request = json!({ "library_id": library, "destination_location_id": destination });
    let (status, report) = desktop.command("/api/v1/libraries/snapshot-copies", request);
    assert_eq!(status, 200, "{report}");
    assert_eq!(report["invalid_objects"], json!([]));
    assert!(copy.join("0=ocfl_1.1").is_file());
    let (status, rebuilt) = desktop.command(
        "/api/v1/libraries/rebuilds",
        json!({ "library_id": library }),
    );
    assert_eq!(status, 200, "{rebuilt}");
    assert_eq!(rebuilt["outcome"], "pending", "{rebuilt}");
    assert_eq!(rebuilt["job"]["kind"], "library.rebuild");
    assert_eq!(rebuilt["scheduled"], true, "the desktop runs rebuild jobs");
    let job = desktop.finished_job(rebuilt["job_id"].as_str().expect("job id"));
    assert_eq!(job["state"], "completed", "{job}");
    assert_eq!(
        job["result"]["objects"].as_array().expect("objects").len(),
        1
    );
    assert_eq!(
        (job["done"].as_u64(), job["total"].as_u64()),
        (Some(1), Some(1))
    );
    let (_, entries) = desktop.stop();
    let completed = entries.iter().filter(|e| e.operation == "job.completed");
    assert_eq!(
        completed.count(),
        1,
        "the rebuild job is journaled in the SQLite journal"
    );
}

#[test]
fn the_owners_consented_repair_is_kept_in_the_persistent_ledger() {
    let dir = fresh_dir("repair");
    let (library, root) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    let location = desktop.register(&root);
    let body = json!({ "location_id": location, "library_id": library });
    assert_eq!(desktop.command("/api/v1/libraries/openings", body).0, 200);
    make_w010(&root, "v2");
    let object = format!("urn:uuid:{library}");
    let uri = format!("/api/v1/libraries/repair-candidates?library_id={library}");
    let (status, candidates) = desktop.get(&uri);
    assert_eq!(status, 200, "{candidates}");
    assert_eq!(candidates["candidates"][0]["object_id"], object.as_str());
    let unconsented = json!({ "library_id": library, "object_ids": [object] });
    let (status, refused) = desktop.post("/api/v1/libraries/repairs", unconsented);
    assert_eq!(status, 409, "{refused}");
    assert!(
        !root.join("snapshot/v2/inventory.json").exists(),
        "no change"
    );
    let (status, answer) =
        desktop.post("/api/v1/libraries/repairs", consented(&library, &[&object]));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["objects"][0]["result"], "repaired", "{answer}");
    assert!(root.join("snapshot/v2/inventory.json").is_file());
    let (data, entries) = desktop.stop();
    let repairs = entries.iter().filter(|e| e.operation == REPAIR_OPERATION);
    assert!(repairs.count() >= 2, "consent and outcome are journaled");
    let ledger = SqliteRepairLedger::open(&data.join(JOURNAL_FILE), "desktop").expect("ledger");
    assert_eq!(ledger.consent_count().expect("consents"), 1);
    let receipts = ledger.receipts().expect("receipts");
    assert_eq!(receipts.len(), 1, "the receipt survives the host");
    assert_eq!(receipts[0].object_id, object);
}

#[test]
fn the_navigation_lists_the_installations_repository_and_its_libraries() {
    let dir = fresh_dir("navigation");
    let desktop = Desktop::launch(dir.join("data"));
    assert!(
        owner_event_filter(&desktop.state).is_none(),
        "nothing to filter yet"
    );
    let (status, _) = desktop.get("/api/v1/navigation");
    assert_eq!(status, 503, "no navigation installed yet");
    install_navigation(&desktop.state, desktop.catalog.clone(), &installation())
        .expect("installed");
    let folder = dir.join("library");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = desktop.register(&folder);
    let (status, body) = desktop.command("/api/v1/libraries", json!({ "location_id": location }));
    assert_eq!(status, 200, "{body}");
    let library = body["library_id"].as_str().expect("library id").to_string();
    let (status, page) = desktop.get("/api/v1/navigation");
    assert_eq!(status, 200, "{page}");
    let nodes = page["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 1, "the installation's one repository: {page}");
    let repository = nodes[0]["node"].as_str().expect("handle").to_string();
    let (status, page) = desktop.get(&format!("/api/v1/navigation?node={repository}"));
    assert_eq!(status, 200, "{page}");
    let libraries: Vec<&serde_json::Value> = page["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|n| n["kind"] == "library")
        .collect();
    assert_eq!(libraries.len(), 1, "the repository's library: {page}");
    assert_eq!(
        page["nodes"].as_array().expect("nodes").last().expect("node")["kind"],
        "recently_deleted",
        "then its one Recently deleted node: {page}"
    );
    assert_eq!(libraries[0]["name"], UNTITLED_LIBRARY, "{page}");
    let repository = nodes[0]["node"].as_str().expect("handle").to_string();
    let (status, page) = desktop.get(&format!("/api/v1/navigation?node={repository}"));
    assert_eq!(status, 200, "{page}");
    let libraries = page["nodes"].as_array().expect("nodes");
    assert_eq!(libraries.len(), 2, "the library, then Recently deleted: {page}");
    assert_eq!(libraries[0]["name"], UNTITLED_LIBRARY, "{page}");
    assert!(page.to_string().contains(&library) || libraries[0]["node"].is_string());
    let filter = owner_event_filter(&desktop.state).expect("filter");
    let change = NavigationChange::All { all: AlwaysTrue }
        .to_event()
        .expect("event");
    assert!(filter(change).is_some(), "the owner reads every change");
    let other = ApiEvent::new("journal.entry", json!({}));
    assert!(filter(other).is_some(), "other events pass unchanged");
    assert!(
        install_navigation(&desktop.state, desktop.catalog.clone(), &installation()).is_err(),
        "installed once"
    );
    desktop.stop();
}

#[test]
fn the_backfill_route_runs_over_the_folders_the_owner_opened() {
    let dir = fresh_dir("backfill");
    let (library, root) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    let (status, _) = desktop.post("/api/v1/admin/history/backfill", json!({}));
    assert_eq!(status, 503, "no launcher installed yet");
    let location = desktop.register(&root);
    let body = json!({ "location_id": location, "library_id": library });
    assert_eq!(desktop.command("/api/v1/libraries/openings", body).0, 200);
    let task = BackfillTask {
        jobs: open_jobs(&desktop.dir, &desktop.host).expect("jobs"),
        backfill: history_backfill(&desktop.host, &installation()),
        catalog: desktop.catalog.clone(),
        source: backfill_source(&installation()),
        opened: desktop.opened.clone(),
    };
    let backfill = Arc::new(DesktopBackfill::new(task, Arc::new(AtomicBool::new(false))));
    install_backfill_route(&desktop.state, &backfill, false).expect("installed");
    assert!(!backfill.is_running(), "nothing runs until requested");
    let (status, answer) = desktop.post("/api/v1/admin/history/backfill", json!({}));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["status"], "started");
    backfill.wait(Duration::from_secs(30));
    assert!(!backfill.is_running());
    let (_, entries) = desktop.stop();
    let requests = entries
        .iter()
        .filter(|e| e.operation == BACKFILL_REQUEST_OPERATION);
    assert_eq!(requests.count(), 2, "the refusal and the start, once each");
    let runs: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == BACKFILL_OPERATION && e.outcome == Outcome::Started)
        .collect();
    assert!(
        !runs.is_empty(),
        "the opened folder's run registered and journaled"
    );
}

#[test]
fn the_owners_preferences_are_kept_in_the_operational_store() {
    let dir = fresh_dir("preferences");
    let desktop = Desktop::launch(dir.join("data"));
    let (status, _) = desktop.get("/api/v1/preferences");
    assert_eq!(status, 503, "no store installed yet");
    install_host_preferences(&desktop.state, &desktop.dir).expect("installed");
    install_file_scopes(&desktop.files, &desktop.catalog).expect("scopes");
    let (status, defaults) = desktop.get("/api/v1/preferences");
    assert_eq!(status, 200, "{defaults}");
    let (status, saved) = desktop.send("PUT", "/api/v1/preferences", Some(defaults.clone()), true);
    assert_eq!(status, 200, "{saved}");
    let data = desktop.dir.clone();
    desktop.stop();
    let store = SqlitePreferences::open(&data.join(JOURNAL_FILE)).expect("store");
    let kept = store.load("owner").expect("load").expect("saved");
    assert_eq!(serde_json::to_value(kept).expect("json"), saved);
    let files = HostFiles::new();
    let catalog = Arc::new(
        SqliteCatalog::open(
            &data.join(CATALOG_DIR),
            Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
        )
        .expect("catalog"),
    );
    assert!(install_file_scopes(&files, &catalog).is_ok());
    assert!(
        install_file_scopes(&files, &catalog).is_err(),
        "installed once"
    );
}

/// A folder dialog in which the owner picks `folder`, or cancels when it is `None`.
struct ScriptedDialog(Option<PathBuf>);

impl FolderDialog for ScriptedDialog {
    fn pick(&self, _purpose: ChoicePurpose) -> Result<Option<PathBuf>, ChooseError> {
        Ok(self.0.clone())
    }
}

#[test]
fn the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id() {
    let dir = fresh_dir("choice");
    let folder = dir.join("Picked library");
    std::fs::create_dir_all(&folder).expect("folder");
    let desktop = Desktop::launch(dir.join("data"));
    let body = json!({ "purpose": "create" });
    let (status, _) = desktop.post("/api/v1/libraries/location-choices", body.clone());
    assert_eq!(status, 503, "no chooser installed yet");
    let dialog = Box::new(ScriptedDialog(Some(folder.clone())));
    install_folder_choice(&desktop.state, &desktop.locations, dialog).expect("installed");
    let (status, choice) = desktop.post("/api/v1/libraries/location-choices", body);
    assert_eq!(status, 200, "{choice}");
    assert_eq!(choice["outcome"], "chosen");
    let location = choice["location_id"]
        .as_str()
        .expect("location")
        .to_string();
    assert!(!choice.to_string().contains("Picked"), "{choice}");
    let (status, again) = desktop.post(
        "/api/v1/libraries/location-choices",
        json!({ "purpose": "open" }),
    );
    assert_eq!(status, 200, "{again}");
    assert_eq!(
        again["location_id"],
        location.as_str(),
        "the same folder keeps its ID"
    );
    let (status, created) =
        desktop.command("/api/v1/libraries", json!({ "location_id": location }));
    assert_eq!(status, 200, "{created}");
    assert!(
        folder.join("0=ocfl_1.1").is_file(),
        "the library is in the picked folder"
    );
    let (_, entries) = desktop.stop();
    let choices: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == CHOOSE_OPERATION)
        .collect();
    assert_eq!(choices.len(), 3, "each request journaled once");
    let journaled = format!("{choices:?}");
    assert!(!journaled.contains("Picked"), "no folder name: {journaled}");
    assert!(
        !journaled.contains(dir.to_string_lossy().as_ref()),
        "no path"
    );
}

#[test]
fn a_cancelled_or_unusable_pick_registers_nothing() {
    let dir = fresh_dir("choice-refused");
    let file = dir.join("notes.txt");
    std::fs::write(&file, b"mine").expect("file");
    let locations = Arc::new(HostLocations::new(&dir.join("data")));
    let cancelled = DesktopChooser::new(locations.clone(), Box::new(ScriptedDialog(None)));
    assert_eq!(cancelled.choose(ChoicePurpose::Open), Ok(None));
    let refused = DesktopChooser::new(locations, Box::new(ScriptedDialog(Some(file))));
    assert_eq!(
        refused.choose(ChoicePurpose::Create),
        Err(ChooseError::Refused)
    );
    assert_eq!(
        dialog_title(ChoicePurpose::CopyDestination),
        "Choose an empty folder for the copy"
    );
}

#[test]
fn a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder() {
    let dir = fresh_dir("facts");
    let desktop = Desktop::launch(dir.join("data"));
    install_navigation(&desktop.state, desktop.catalog.clone(), &installation())
        .expect("navigation");
    let folder = dir.join("Family archive");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = desktop.register(&folder);
    let body = json!({ "location_id": location, "name": "Family archive" });
    let (status, created) = desktop.command("/api/v1/libraries", body);
    assert_eq!(status, 200, "{created}");
    let library = created["library_id"].as_str().expect("id").to_string();
    assert_eq!(desktop.entity_name(&library), "Family archive");
    let (status, facts) = desktop.get(&format!("/api/v1/libraries/{library}/facts"));
    assert_eq!(status, 200, "{facts}");
    assert_eq!(facts["name"], "Family archive");
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(
            facts["free_bytes"].as_u64().is_some_and(|b| b > 0),
            "{facts}"
        );
        assert_eq!(facts["free_bytes_reason"], Value::Null);
    } else {
        assert_eq!(facts["free_bytes"], Value::Null);
        assert_eq!(facts["free_bytes_reason"], "volume_unreadable");
    }
    let path = folder.to_string_lossy().to_string();
    assert!(
        !facts.to_string().contains(&path),
        "no folder path: {facts}"
    );
    let rename = json!({ "library_id": library, "name": "Family archive 1990s" });
    let (status, renamed) = desktop.command("/api/v1/libraries/renames", rename);
    assert_eq!(status, 200, "{renamed}");
    assert_eq!(
        renamed["revision"], "2",
        "a changed name advances the revision"
    );
    assert_eq!(
        desktop.entity_name(&library),
        "Family archive 1990s",
        "the rename reaches the library entity"
    );
    assert_eq!(desktop.navigation_titles(), ["Family archive 1990s"]);
    let (status, recent) = desktop.get("/api/v1/libraries/recent");
    assert_eq!(status, 200, "{recent}");
    let listed = &recent["libraries"][0];
    assert_eq!(listed["library_id"], library.as_str());
    assert_eq!(listed["name"], "Family archive 1990s");
    assert_eq!(listed["open"], true);
    assert!(
        !recent.to_string().contains("loc-"),
        "no location ID: {recent}"
    );
    assert!(
        !recent.to_string().contains(&path),
        "no folder path: {recent}"
    );
    let (_, entries) = desktop.stop();
    let renames: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == "library.rename")
        .collect();
    assert_eq!(renames.len(), 1);
    assert_eq!(renames[0].target_ids, std::slice::from_ref(&library));
    let retitles: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == ENTITY_RENAME_OPERATION)
        .collect();
    assert_eq!(retitles.len(), 1, "one journaled entity rename");
    assert_eq!(retitles[0].target_ids, std::slice::from_ref(&library));
    let journal = serde_json::to_string(&entries).expect("json");
    assert!(
        !journal.contains("Family archive"),
        "no name in the journal"
    );
}

#[test]
fn the_data_directory_keeps_only_each_library_s_index_never_its_files() {
    let dir = fresh_dir("layout");
    let desktop = Desktop::launch(dir.join("data"));
    let folder = dir.join("My library");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = desktop.register(&folder);
    let (status, body) = desktop.command("/api/v1/libraries", json!({ "location_id": location }));
    assert_eq!(status, 200, "{body}");
    let library = body["library_id"].as_str().expect("id").to_string();
    desktop.create_collection(&library, "Holidays");
    let mut names: Vec<String> = std::fs::read_dir(dir.join("data").join(CATALOG_DIR))
        .expect("catalog dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in &names {
        assert!(
            name.starts_with(&library)
                && (name.ends_with(".sqlite")
                    || name.ends_with(".sqlite-wal")
                    || name.ends_with(".sqlite-shm")
                    || name.ends_with(".work")),
            "only the index and its staging: {names:?}"
        );
    }
    assert!(
        folder.join("snapshot").is_dir(),
        "the snapshot is in the chosen folder"
    );
    desktop.stop();
}

#[test]
fn a_library_folder_copied_elsewhere_reopens_with_its_collections() {
    let dir = fresh_dir("moved");
    let (library, folder) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    install_navigation(&desktop.state, desktop.catalog.clone(), &installation())
        .expect("navigation");
    let copy = dir.join("moved library");
    copy_dir(&folder, &copy);
    let location = desktop.register(&copy);
    let (status, answer) = desktop.command(
        "/api/v1/libraries/openings",
        json!({ "location_id": location }),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["library_id"],
        library.as_str(),
        "the copy keeps its library"
    );
    let (status, facts) = desktop.get(&format!("/api/v1/libraries/{library}/facts"));
    assert_eq!(status, 200, "{facts}");
    assert_eq!(facts["name"], DONOR_NAME, "the copy carries its name");
    assert_eq!(desktop.navigation_titles(), [DONOR_NAME]);
    let node =
        filesquid_api::navigation::NodeRef::Library(LibraryId::new(&library).expect("library"))
            .to_handle()
            .expect("handle");
    let (status, page) = desktop.get(&format!("/api/v1/navigation?node={node}"));
    assert_eq!(status, 200, "{page}");
    let titles: Vec<&str> = page["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|n| n["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        titles,
        ["Holidays"],
        "rebuilt from the copied folder: {page}"
    );
    let original = desktop.register(&folder);
    let (status, refused) = desktop.command(
        "/api/v1/libraries/openings",
        json!({ "location_id": original }),
    );
    assert_eq!(status, 409, "{refused}");
    let (_, entries) = desktop.stop();
    assert!(
        entries.iter().any(|e| e.operation == "library.root.open"
            && e.error_code.as_deref() == Some("library_already_open")),
        "a second folder of an open library is refused"
    );
    assert!(
        entries.iter().any(|e| e.operation == "library.reconcile"),
        "the index was rebuilt in a journaled reconcile"
    );
}

/// Copy the directory tree `from` to `to` (bounded walk).
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("dir");
    for entry in std::fs::read_dir(from).expect("read").take(10_000) {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

#[test]
fn a_rebuild_stopped_at_exit_resumes_from_its_checkpoint_when_its_library_opens_again() {
    let dir = fresh_dir("rebuild-exit");
    let (library, root) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    let open = |desktop: &Desktop| {
        let location = desktop.register(&root);
        let body = json!({ "location_id": location, "library_id": library });
        assert_eq!(desktop.command("/api/v1/libraries/openings", body).0, 200);
    };
    open(&desktop);
    desktop.quitting.store(true, Ordering::SeqCst);
    let (status, rebuilt) = desktop.command(
        "/api/v1/libraries/rebuilds",
        json!({ "library_id": library }),
    );
    assert_eq!(status, 200, "{rebuilt}");
    let job = rebuilt["job_id"].as_str().expect("job id").to_string();
    let mut view = Value::Null;
    for _ in 0..MAX_JOB_POLLS {
        view = desktop.get(&format!("/api/v1/jobs/{job}")).1;
        if view["state"] == "pending" && view["attempts"] == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(view["state"], "pending", "suspended at exit: {view}");
    assert_eq!(view["done"], 1, "its checkpoint after the object in hand");
    let (data, entries) = desktop.stop();
    let suspended = entries.iter().filter(|e| e.operation == "job.suspended");
    assert_eq!(suspended.count(), 1, "the exit suspended it once");
    let desktop = Desktop::launch(data);
    let (_, view) = desktop.get(&format!("/api/v1/jobs/{job}"));
    assert_eq!(
        view["state"], "pending",
        "not run before its library opens: {view}"
    );
    open(&desktop);
    let view = desktop.finished_job(&job);
    assert_eq!(view["state"], "completed", "{view}");
    assert_eq!(view["attempts"], 2, "resumed once");
    assert_eq!(
        view["result"]["objects"].as_array().expect("objects").len(),
        1,
        "no checkpointed object read again"
    );
    desktop.stop();
}

#[test]
fn recent_libraries_and_their_facts_survive_a_restart_without_a_path() {
    let dir = fresh_dir("recent-restart");
    let desktop = Desktop::launch(dir.join("data"));
    let folder = dir.join("Family archive");
    std::fs::create_dir_all(&folder).expect("folder");
    let location = desktop.register(&folder);
    let body = json!({ "location_id": location, "name": "Family archive" });
    let (status, created) = desktop.command("/api/v1/libraries", body);
    assert_eq!(status, 200, "{created}");
    let library = created["library_id"].as_str().expect("id").to_string();
    let rename = json!({ "library_id": library, "name": "Family archive 1990s" });
    assert_eq!(desktop.command("/api/v1/libraries/renames", rename).0, 200);
    let (data, _) = desktop.stop();
    let desktop = Desktop::launch(data);
    let (status, recent) = desktop.get("/api/v1/libraries/recent");
    assert_eq!(status, 200, "{recent}");
    let listed = &recent["libraries"][0];
    assert_eq!(listed["library_id"], library.as_str(), "{recent}");
    assert_eq!(
        listed["name"], "Family archive 1990s",
        "the renamed name kept"
    );
    assert_eq!(listed["open"], false, "not opened yet this launch");
    assert_eq!(
        desktop.register(&folder),
        location,
        "the folder keeps its location across launches"
    );
    let body = json!({ "location_id": location, "library_id": library });
    let (status, opened) = desktop.command("/api/v1/libraries/openings", body);
    assert_eq!(status, 200, "{opened}");
    let (status, facts) = desktop.get(&format!("/api/v1/libraries/{library}/facts"));
    assert_eq!(status, 200, "{facts}");
    assert_eq!(facts["name"], "Family archive 1990s");
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(facts["free_bytes"].as_u64().is_some(), "{facts}");
    } else {
        assert_eq!(facts["free_bytes_reason"], "volume_unreadable");
    }
    let text = format!("{recent}{facts}");
    assert!(!text.contains("loc-"), "no location ID: {text}");
    desktop.stop();
}
