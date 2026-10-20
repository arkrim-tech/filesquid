//! Steps of `desktop_libraries_F-001.feature` (Task DE-1.1; SRS-F-001, API-S-025, DE-S-006):
//! the desktop's library routes over folders the owner chose, driven through the
//! `filesquid://` scheme with the per-launch token, against SQLite files and OCFL storage roots
//! in a scratch directory that is removed when the scenario ends.

use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::auth::LaunchToken;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{
    CommandRequest, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION,
};
use filesquid_api::local_library::{CONSENT_STATEMENT_ID, CREATE_OPERATION};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId, RepositoryId};
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::libraries::LibraryServing;
use filesquid_desktop_lib::{
    install_history_source, install_launch_token, install_library_routes, open_host_monitor,
    open_library_catalog,
};
use filesquid_native::{
    install_file_service, HostFiles, HostLocations, HostMonitor, OpenedRoots, SqliteJournal,
    SqliteRepairLedger, CATALOG_DIR, JOURNAL_FILE,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// The installation of these scenarios.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// Scenario counter, so concurrent scenarios get separate scratch directories.
static SCENARIOS: AtomicU32 = AtomicU32::new(0);

/// The desktop of one scenario and what its steps chose and saw.
pub struct LibraryRig {
    dir: PathBuf,
    state: ApiState,
    token: LaunchToken,
    host: Option<HostMonitor>,
    files: HostFiles,
    locations: Arc<HostLocations>,
    location: Option<String>,
    folder: Option<PathBuf>,
    library: Option<String>,
    root: Option<PathBuf>,
    answer: Option<(u16, String)>,
}

impl std::fmt::Debug for LibraryRig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryRig")
            .field("dir", &self.dir)
            .finish()
    }
}

impl Drop for LibraryRig {
    fn drop(&mut self) {
        if let Some(host) = self.host.take() {
            host.stop();
        }
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("scenario directory not removed: {err}");
        }
    }
}

/// The installation of these scenarios.
fn installation() -> InstallationId {
    InstallationId::parse(INSTALLATION).expect("installation")
}

impl LibraryRig {
    /// Start the desktop's library routes in a fresh scratch directory.
    fn start() -> Self {
        let n = SCENARIOS.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "fs-desktop-library-feature-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let data = dir.join("data");
        std::fs::create_dir_all(&data).expect("data dir");
        let state = ApiState::new();
        let token = install_launch_token(&state).expect("token");
        let files = install_file_service(&state).expect("files");
        let host = open_host_monitor(&state, &data, None).expect("monitor");
        let catalog = Arc::new(open_library_catalog(&data, &host, &installation()).expect("cat"));
        let opened = Arc::new(OpenedRoots::new());
        install_history_source(&state, catalog, opened.clone()).expect("history");
        let serving = LibraryServing {
            files: files.clone(),
            validation: None,
            opened,
            data_dir: data,
        };
        let locations =
            install_library_routes(&state, &host, &installation(), serving).expect("routes");
        Self {
            dir,
            state,
            token,
            host: Some(host),
            files,
            locations,
            location: None,
            folder: None,
            library: None,
            root: None,
            answer: None,
        }
    }

    /// A new folder `name` in the scratch directory.
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.dir.join(name);
        std::fs::create_dir_all(&folder).expect("folder");
        folder
    }

    /// Register `folder` as the owner's choice and remember it.
    fn choose(&mut self, folder: PathBuf) {
        self.location = Some(self.locations.register(&folder).expect("registered"));
        self.folder = Some(folder);
    }

    /// POST `body` to `path` as the owner and keep the answer.
    async fn post(&mut self, path: &str, body: Value) {
        let request = http::Request::post(format!("filesquid://localhost{path}"))
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
        self.answer = Some((response.status().as_u16(), text));
    }

    /// The last answer's status and JSON.
    fn answer(&self) -> (u16, Value) {
        let (status, text) = self.answer.as_ref().expect("an answer");
        (*status, serde_json::from_str(text).unwrap_or(Value::Null))
    }

    /// Stop the host, flushing the journal, and answer its entries.
    fn stop(&mut self) -> Vec<JournalEntry> {
        if let Some(host) = self.host.take() {
            host.stop();
        }
        let path = self.dir.join("data").join(JOURNAL_FILE);
        let store = SqliteJournal::open(&path, "desktop").expect("journal");
        super::journal::all_entries(&store)
    }
}

/// A command envelope for `op` at `revision` in `scope`.
fn envelope(op: &str, revision: &str, scope: &str) -> CommandEnvelope {
    CommandEnvelope::new(
        OperationId::new(op).expect("op"),
        ExpectedRevision::new(revision).expect("revision"),
        OperationScope::new(scope).expect("scope"),
    )
}

/// A library made by another desktop in `dir` with two snapshot versions, answering its ID
/// and its OCFL storage root folder.
fn donor_library(dir: &Path) -> (String, PathBuf) {
    std::fs::create_dir_all(dir).expect("donor dir");
    let state = ApiState::new();
    let host = open_host_monitor(&state, dir, None).expect("donor monitor");
    let catalog = Arc::new(open_library_catalog(dir, &host, &installation()).expect("catalog"));
    let service = LibraryService::new(catalog.clone(), host.monitor().journal());
    let create = CommandRequest {
        envelope: envelope("op-create", NEW_LIBRARY_REVISION, "library"),
        command: LibraryCommand::CreateLibrary {
            repository_id: RepositoryId::new("repo-1").expect("repo"),
            name: "Family archive".into(),
        },
    };
    let created = service.execute("owner", create).expect("create");
    let library = created.created_id.expect("library id");
    let collection = CommandRequest {
        envelope: envelope("op-collection", &created.revision.expect("rev"), &library),
        command: LibraryCommand::CreateCollection {
            library_id: LibraryId::new(&library).expect("library"),
            title: "Holidays".into(),
            folder_id: None,
            sort: Default::default(),
        },
    };
    service.execute("owner", collection).expect("collection");
    drop(catalog);
    host.stop();
    (
        library.clone(),
        dir.join(CATALOG_DIR).join(format!("{library}.ocfl")),
    )
}

/// The scenario's rig.
fn rig(world: &mut DesktopWorld) -> &mut LibraryRig {
    world.library.as_mut().expect("library routes started")
}

/// The head version directory of the snapshot object in `root`.
fn head(root: &Path) -> PathBuf {
    root.join("snapshot").join("v2")
}

#[given("the desktop host has started its library routes")]
fn started(world: &mut DesktopWorld) {
    world.library = Some(LibraryRig::start());
}

#[given("the owner chose an empty folder")]
fn chose_empty(world: &mut DesktopWorld) {
    let rig = rig(world);
    let folder = rig.folder("chosen");
    rig.choose(folder);
}

#[given("the owner chose a folder that holds a file")]
fn chose_with_file(world: &mut DesktopWorld) {
    let rig = rig(world);
    let folder = rig.folder("busy");
    std::fs::write(folder.join("notes.txt"), b"mine").expect("file");
    rig.choose(folder);
}

#[given("the owner opened a folder that holds a library")]
async fn opened_library(world: &mut DesktopWorld) {
    open_donor(rig(world), false).await;
}

#[given("the owner opened a folder that holds a library whose head lacks its version inventory")]
async fn opened_w010(world: &mut DesktopWorld) {
    open_donor(rig(world), true).await;
}

/// Open a donor library's folder through the open route, making its head lack its version
/// inventory first when `w010`.
async fn open_donor(rig: &mut LibraryRig, w010: bool) {
    let (library, root) = donor_library(&rig.dir.join("donor"));
    if w010 {
        std::fs::remove_file(head(&root).join("inventory.json")).expect("W010 head");
        std::fs::remove_file(head(&root).join("inventory.json.sha256")).expect("W010 sidecar");
    }
    let location = rig.locations.register(&root).expect("registered");
    let body = json!({ "location_id": location, "library_id": library });
    rig.post("/api/v1/libraries/openings", body).await;
    assert_eq!(rig.answer().0, 200, "{:?}", rig.answer);
    rig.library = Some(library);
    rig.root = Some(root);
}

#[given("the owner chose a folder inside that library")]
fn chose_inside(world: &mut DesktopWorld) {
    let rig = rig(world);
    let inside = rig.root.clone().expect("root").join("extensions");
    std::fs::create_dir_all(&inside).expect("inside");
    rig.choose(inside);
}

#[when("the app creates a library in that folder")]
async fn create_there(world: &mut DesktopWorld) {
    let rig = rig(world);
    let location = rig.location.clone().expect("a chosen folder");
    rig.post("/api/v1/libraries", json!({ "location_id": location }))
        .await;
}

#[when("the app creates a library in a folder the host never registered")]
async fn create_unknown(world: &mut DesktopWorld) {
    let rig = rig(world);
    rig.post("/api/v1/libraries", json!({ "location_id": "loc-unknown" }))
        .await;
}

#[when("the app copies the library's snapshot into that folder")]
async fn copy_there(world: &mut DesktopWorld) {
    let rig = rig(world);
    let body = json!({
        "library_id": rig.library.clone().expect("library"),
        "destination_location_id": rig.location.clone().expect("a chosen folder"),
    });
    rig.post("/api/v1/libraries/snapshot-copies", body).await;
}

#[when("the app asks to repair it without the owner's consent")]
async fn repair_unconsented(world: &mut DesktopWorld) {
    let rig = rig(world);
    let library = rig.library.clone().expect("library");
    let body = json!({ "library_id": library, "object_ids": [format!("urn:uuid:{library}")] });
    rig.post("/api/v1/libraries/repairs", body).await;
}

#[when("the owner confirms the repair of that library")]
async fn repair_consented(world: &mut DesktopWorld) {
    let rig = rig(world);
    let library = rig.library.clone().expect("library");
    let objects = [format!("urn:uuid:{library}")];
    let body = json!({
        "library_id": library,
        "object_ids": objects,
        "consent": {
            "library_id": library,
            "object_ids": objects,
            "statement_id": CONSENT_STATEMENT_ID,
            "confirmed": true
        }
    });
    rig.post("/api/v1/libraries/repairs", body).await;
}

#[then("the answer should name a new library without any folder path")]
fn new_library(world: &mut DesktopWorld) {
    let rig = rig(world);
    let (status, body) = rig.answer();
    assert_eq!(status, 200, "{body}");
    let text = &rig.answer.as_ref().expect("answer").1;
    assert!(!text.contains(rig.dir.to_string_lossy().as_ref()), "{text}");
    assert!(!text.contains("chosen"), "{text}");
    rig.library = body["library_id"].as_str().map(str::to_string);
    assert!(rig.library.is_some(), "{body}");
}

#[then("the desktop should serve that library's files")]
fn serves_files(world: &mut DesktopWorld) {
    let rig = rig(world);
    let library = LibraryId::new(rig.library.clone().expect("library")).expect("id");
    assert!(rig.files.is_open(&library));
}

#[then("the copy should succeed with no invalid object")]
fn copied(world: &mut DesktopWorld) {
    let (status, body) = rig(world).answer();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["invalid_objects"], json!([]));
}

#[then("the chosen folder should hold an OCFL storage root")]
fn holds_root(world: &mut DesktopWorld) {
    let folder = rig(world).folder.clone().expect("folder");
    assert!(folder.join("0=ocfl_1.1").is_file());
}

#[then("the request should be refused as a conflict")]
fn refused_conflict(world: &mut DesktopWorld) {
    let (status, body) = rig(world).answer();
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["code"], "conflict");
}

#[then("the request should be refused as not found")]
fn refused_not_found(world: &mut DesktopWorld) {
    let (status, body) = rig(world).answer();
    assert_eq!(status, 404, "{body}");
}

#[then("the folder should still hold only that file")]
fn only_that_file(world: &mut DesktopWorld) {
    let folder = rig(world).folder.clone().expect("folder");
    let names: Vec<_> = std::fs::read_dir(folder)
        .expect("folder")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, ["notes.txt"]);
}

#[then("the head should still lack its version inventory")]
fn still_w010(world: &mut DesktopWorld) {
    let root = rig(world).root.clone().expect("root");
    assert!(!head(&root).join("inventory.json").exists());
}

#[then("the journal should record the refusal once without any folder path")]
fn refusal_journaled(world: &mut DesktopWorld) {
    let rig = rig(world);
    let dir = rig.dir.to_string_lossy().to_string();
    let entries = rig.stop();
    let refusals: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == CREATE_OPERATION && e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    let text = format!("{refusals:?}");
    assert!(!text.contains(&dir), "{text}");
}

#[then("the repair should succeed")]
fn repaired(world: &mut DesktopWorld) {
    let rig = rig(world);
    let (status, body) = rig.answer();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["objects"][0]["result"], "repaired", "{body}");
    let root = rig.root.clone().expect("root");
    assert!(head(&root).join("inventory.json").is_file());
}

#[then("after the app restarts the repair ledger should still hold its consent and receipt")]
fn ledger_kept(world: &mut DesktopWorld) {
    let rig = rig(world);
    rig.stop();
    let path = rig.dir.join("data").join(JOURNAL_FILE);
    let ledger = SqliteRepairLedger::open(&path, "desktop").expect("ledger");
    assert_eq!(ledger.consent_count().expect("consents"), 1);
    let receipts = ledger.receipts().expect("receipts");
    assert_eq!(receipts.len(), 1);
    let library = rig.library.clone().expect("library");
    assert_eq!(receipts[0].object_id, format!("urn:uuid:{library}"));
}
