//! Task DE-1.1: the desktop's library and repair routes (API-S-025) over folders the owner
//! chose. The host registers each folder under an opaque location ID, injects its storage root
//! and the persistent repair ledger, and serves each library it binds (files, validation,
//! PROV-O history). Driven through the `filesquid://` scheme with the per-launch token, against
//! SQLite files and OCFL storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::auth::LaunchToken;
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{
    CommandRequest, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION,
};
use filesquid_api::local_library::{CONSENT_STATEMENT_ID, REPAIR_OPERATION};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId, RepositoryId};
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::libraries::LibraryServing;
use filesquid_desktop_lib::{
    install_file_provenance, install_history_source, install_launch_token, install_library_routes,
    open_host_monitor, open_library_catalog,
};
use filesquid_native::{
    install_file_service, HostFiles, HostLocations, HostMonitor, OpenedRoots, SqliteCatalog,
    SqliteJournal, SqliteRepairLedger, CATALOG_DIR, JOURNAL_FILE,
};
use journal::block_on;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// A library made by another desktop in `dir` with two snapshot versions (each with its
/// PROV-O record), answering its ID and its OCFL storage root folder.
fn donor_library(dir: &Path) -> (String, PathBuf) {
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
    let root = dir.join(CATALOG_DIR).join(format!("{library}.ocfl"));
    (library, root)
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
        };
        let locations =
            install_library_routes(&state, &host, &installation(), serving).expect("routes");
        Self {
            dir,
            state,
            token,
            host,
            catalog,
            files,
            locations,
            opened,
        }
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
        assert!(
            !text.contains(self.dir.to_string_lossy().as_ref()),
            "no host path in an answer: {text}"
        );
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// POST `body` to `path` as the owner.
    fn post(&self, path: &str, body: Value) -> (u16, Value) {
        self.send("POST", path, Some(body), true)
    }

    /// GET `path` as the owner.
    fn get(&self, path: &str) -> (u16, Value) {
        self.send("GET", path, None, true)
    }

    /// Stop the monitor (flushing the journal) and answer the journal's entries.
    fn stop(self) -> (PathBuf, Vec<JournalEntry>) {
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
    let (status, body) = desktop.post("/api/v1/libraries", json!({ "location_id": location }));
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
    let (status, answer) = desktop.post("/api/v1/libraries/openings", body);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["library_id"], library.as_str());
    let (status, body) = desktop.get("/api/v1/history");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["items"].as_array().expect("items").len(),
        2,
        "the chosen root's records: {body}"
    );
    let copy = dir.join("copy");
    std::fs::create_dir_all(&copy).expect("copy folder");
    let destination = desktop.register(&copy);
    let request = json!({ "library_id": library, "destination_location_id": destination });
    let (status, report) = desktop.post("/api/v1/libraries/snapshot-copies", request);
    assert_eq!(status, 200, "{report}");
    assert_eq!(report["invalid_objects"], json!([]));
    assert!(copy.join("0=ocfl_1.1").is_file());
    let (status, rebuilt) = desktop.post(
        "/api/v1/libraries/rebuilds",
        json!({ "library_id": library }),
    );
    assert_eq!(status, 200, "{rebuilt}");
    assert_eq!(rebuilt["objects"].as_array().expect("objects").len(), 1);
    desktop.stop();
}

#[test]
fn the_owners_consented_repair_is_kept_in_the_persistent_ledger() {
    let dir = fresh_dir("repair");
    let (library, root) = donor_library(&dir.join("donor"));
    let desktop = Desktop::launch(dir.join("data"));
    let location = desktop.register(&root);
    let body = json!({ "location_id": location, "library_id": library });
    assert_eq!(desktop.post("/api/v1/libraries/openings", body).0, 200);
    make_w010(&root, "v2");
    let object = format!("urn:uuid:{library}");
    let uri = format!("/api/v1/libraries/repair-candidates?library_id={library}");
    let (status, candidates) = desktop.get(&uri);
    assert_eq!(status, 200, "{candidates}");
    assert_eq!(candidates["candidates"][0]["object_id"], object.as_str());
    let unconsented = json!({ "library_id": library, "object_ids": [object] });
    let (status, refused) = desktop.post("/api/v1/libraries/repairs", unconsented);
    assert!((400..500).contains(&status), "{status} {refused}");
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
