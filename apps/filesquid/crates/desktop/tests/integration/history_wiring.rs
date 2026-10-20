//! Task FS-2.8 host wiring: the desktop installs the PROV-O history source over its library
//! storage roots, builds the repair service with its provenance follow-up, and runs the history
//! backfill as a registered, journaled job (API-S-028, API-S-029, DE-S-011, DE-S-012; ADR 040
//! Decisions 6 and 7). Runs against SQLite files and OCFL storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::auth::{LaunchToken, Principal};
use filesquid_api::history::backfill::BACKFILL_OPERATION;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{
    CommandRequest, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION,
};
use filesquid_api::local_library::{
    MemoryRepairLedger, RepairConsent, RepairRequest, RepairResult, CONSENT_STATEMENT_ID,
};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::registry::ProcessState;
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId, RepositoryId};
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::{
    history_backfill, install_history_source, install_launch_token, local_library_service,
    open_host_monitor, open_library_catalog,
};
use filesquid_native::{HostMonitor, SqliteCatalog, SqliteJournal, CATALOG_DIR, JOURNAL_FILE};
use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use journal::block_on;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// The installation UUID of these tests.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The installation of these tests.
fn installation() -> InstallationId {
    InstallationId::parse(INSTALLATION).expect("installation")
}

/// A fresh, empty data directory for one test (filesystem I/O).
fn data_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fs-desktop-history-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    dir
}

/// Every entry of the host journal in `dir` (read after the monitor stopped).
fn host_journal(dir: &Path) -> Vec<JournalEntry> {
    let store = SqliteJournal::open(&dir.join(JOURNAL_FILE), "desktop").expect("journal");
    journal::all_entries(&store)
}

/// A command envelope for `op` at `revision` in `scope`.
fn envelope(op: &str, revision: &str, scope: &str) -> CommandEnvelope {
    CommandEnvelope::new(
        OperationId::new(op).expect("op"),
        ExpectedRevision::new(revision).expect("revision"),
        OperationScope::new(scope).expect("scope"),
    )
}

/// Create a library through the API's library service and answer its ID and revision.
fn create_library(service: &LibraryService) -> (String, String) {
    create_library_as(service, "op-create")
}

/// Create a library as operation `op` and answer its ID and revision.
fn create_library_as(service: &LibraryService, op: &str) -> (String, String) {
    let request = CommandRequest {
        envelope: envelope(op, NEW_LIBRARY_REVISION, "library"),
        command: LibraryCommand::CreateLibrary {
            repository_id: RepositoryId::new("repo-1").expect("repo"),
            name: "Family archive".into(),
        },
    };
    let result = service.execute("owner", request).expect("create");
    let library = result.created_id.expect("library id");
    (library, result.revision.expect("revision"))
}

/// Add a collection to `library` at `revision`: a second snapshot version.
fn add_collection(service: &LibraryService, library: &str, revision: &str, op: &str) {
    let request = CommandRequest {
        envelope: envelope(op, revision, library),
        command: LibraryCommand::CreateCollection {
            library_id: LibraryId::new(library).expect("library"),
            title: "Holidays".into(),
            folder_id: None,
            sort: Default::default(),
        },
    };
    service.execute("owner", request).expect("collection");
}

/// The host of one test: its API state, launch token, monitor and catalog.
struct Desktop {
    dir: PathBuf,
    state: ApiState,
    token: LaunchToken,
    host: HostMonitor,
    catalog: Arc<SqliteCatalog>,
}

impl Desktop {
    /// Launch the host's journaling and catalogs in a fresh data directory, as `start_monitor`
    /// does.
    fn launch(name: &str) -> Self {
        let dir = data_dir(name);
        Self::launch_in(dir)
    }

    /// Launch in an existing data directory.
    fn launch_in(dir: PathBuf) -> Self {
        let state = ApiState::new();
        let token = install_launch_token(&state).expect("token");
        let host = open_host_monitor(&state, &dir, None).expect("monitor");
        let catalog =
            Arc::new(open_library_catalog(&dir, &host, &installation()).expect("catalog"));
        Self {
            dir,
            state,
            token,
            host,
            catalog,
        }
    }

    /// The API's library service over this host's catalog.
    fn library_service(&self) -> LibraryService {
        LibraryService::new(self.catalog.clone(), self.host.monitor().journal())
    }

    /// GET `path` through the custom scheme as the desktop owner; status and JSON body.
    fn get(&self, path: &str) -> (u16, Vec<u8>) {
        let request = http::Request::get(format!("filesquid://localhost{path}"))
            .header(http::header::ORIGIN, "tauri://localhost")
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", self.token.secret()),
            )
            .body(Vec::new())
            .expect("request");
        let response = block_on(handle(self.state.clone(), request));
        (response.status().as_u16(), response.body().clone())
    }

    /// Stop the monitor (flushing the journal) and answer the journal's entries.
    fn stop(self) -> (PathBuf, Vec<JournalEntry>) {
        drop(self.catalog);
        self.host.stop();
        let entries = host_journal(&self.dir);
        (self.dir, entries)
    }
}

/// The library's OCFL storage root in the data directory `dir`.
fn library_root(dir: &Path, library: &str) -> PathBuf {
    dir.join(CATALOG_DIR).join(format!("{library}.ocfl"))
}

/// The JSON of `body`.
fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("json")
}

#[test]
fn the_history_routes_answer_from_a_real_desktop_library() {
    let desktop = Desktop::launch("routes");
    let (library, _) = create_library(&desktop.library_service());
    let (status, _) = desktop.get("/api/v1/history");
    assert_eq!(status, 503, "no history source installed yet");
    install_history_source(&desktop.state, desktop.catalog.clone()).expect("installed");
    let (status, body) = desktop.get("/api/v1/history");
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let page = json(&body);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{page}");
    assert_eq!(page["truncated"], false);
    let object = format!("urn:uuid:{library}");
    assert_eq!(items[0]["version"]["object_id"], object.as_str(), "{page}");
    assert_eq!(items[0]["reconstructed"], false);
    let (status, body) = desktop.get(&format!("/api/v1/files/{library}/history"));
    assert_eq!(status, 200);
    assert_eq!(json(&body)["items"].as_array().expect("items").len(), 1);
    let (status, body) = desktop.get(&format!("/api/v1/files/{library}/versions/v1/prov"));
    assert_eq!(status, 200);
    let record = json(&body);
    assert!(!record["@context"].is_null(), "JSON-LD: {record}");
    let text = String::from_utf8_lossy(&body).to_string();
    assert!(text.contains(INSTALLATION), "the installation agent");
    assert!(
        !text.contains(desktop.dir.to_string_lossy().as_ref()),
        "no host path"
    );
    assert!(
        install_history_source(&desktop.state, desktop.catalog.clone()).is_err(),
        "installed once"
    );
    let (dir, _) = desktop.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// The filesystem backends of the library's storage root and of a lease store beside it.
fn repair_target(dir: &Path, library: &str) -> (FsBackend, LeaseStore<FsBackend, SystemClock>) {
    let work = dir.join("repair-work");
    let root = FsBackend::new(library_root(dir, library), work.join("root")).expect("root");
    let lease_backend =
        FsBackend::new(work.join("leases"), work.join("leases-parts")).expect("leases");
    let leases = LeaseStore::new(lease_backend, "leases", SystemClock, LeaseConfig::default())
        .expect("lease store");
    (root, leases)
}

/// A consented repair request for the snapshot object of `library`.
fn consented(library: &str) -> RepairRequest {
    let library_id = LibraryId::new(library).expect("library");
    let objects = vec![format!("urn:uuid:{library}")];
    RepairRequest {
        library_id: library_id.clone(),
        object_ids: objects.clone(),
        consent: Some(RepairConsent {
            library_id,
            object_ids: objects,
            statement_id: CONSENT_STATEMENT_ID.to_string(),
            confirmed: true,
        }),
    }
}

#[test]
fn a_repair_commits_its_provenance_follow_up_under_the_repair_run_id() {
    let desktop = Desktop::launch("repair");
    let service = desktop.library_service();
    let (library, revision) = create_library(&service);
    add_collection(&service, &library, &revision, "op-collection");
    let head = library_root(&desktop.dir, &library)
        .join("snapshot")
        .join("v2");
    std::fs::remove_file(head.join("inventory.json")).expect("W010 head");
    std::fs::remove_file(head.join("inventory.json.sha256")).expect("W010 sidecar");
    let repairs = local_library_service(
        &desktop.host,
        &installation(),
        Arc::new(MemoryRepairLedger::new()),
    );
    let (root, leases) = repair_target(&desktop.dir, &library);
    let answer = block_on(repairs.repair_version_inventory(
        &Principal::Owner,
        consented(&library),
        &root,
        &leases,
    ))
    .expect("answered");
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Repaired, "{answer:?}");
    assert_eq!(one.provenance_version.as_deref(), Some("v3"), "{answer:?}");
    install_history_source(&desktop.state, desktop.catalog.clone()).expect("installed");
    let (status, body) = desktop.get(&format!("/api/v1/files/{library}/versions/v3/prov"));
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let text = String::from_utf8_lossy(&body).to_string();
    assert!(text.contains(&answer.run_id), "the record names the run");
    let (status, body) = desktop.get(&format!("/api/v1/history?operation={}", answer.run_id));
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let page = json(&body);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{page}");
    assert_eq!(items[0]["kind"], "repair");
    assert_eq!(items[0]["version"]["version"], "v3");
    drop(service);
    let (dir, entries) = desktop.stop();
    let commits: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == "ocfl.commit")
        .filter(|e| e.operation_id.as_deref() == Some(answer.run_id.as_str()))
        .collect();
    assert_eq!(commits.len(), 1, "{entries:?}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Commit a library without any PROV-O record, as a catalog before DE-0.10 did, in `dir`.
fn library_without_history(dir: &Path) -> String {
    let state = ApiState::new();
    let host = open_host_monitor(&state, dir, None).expect("monitor");
    let plain = SqliteCatalog::open(&dir.join(CATALOG_DIR), host.monitor().announcing_store())
        .expect("catalog");
    let plain = Arc::new(plain);
    let service = LibraryService::new(plain.clone(), host.monitor().journal());
    let (library, _) = create_library(&service);
    drop(service);
    drop(plain);
    host.stop();
    library
}

#[test]
fn the_backfill_fills_the_history_of_versions_committed_without_records() {
    let dir = data_dir("backfill");
    let library = library_without_history(&dir);
    let desktop = Desktop::launch_in(dir);
    install_history_source(&desktop.state, desktop.catalog.clone()).expect("installed");
    let (_, body) = desktop.get("/api/v1/history");
    assert_eq!(json(&body)["items"].as_array().expect("items").len(), 0);
    let backfill = history_backfill(&desktop.host, &installation());
    let cancel = AtomicBool::new(false);
    let runs = desktop
        .catalog
        .backfill_history(&backfill, &cancel)
        .expect("libraries listed");
    assert_eq!(runs.len(), 1, "one library");
    assert!(runs[0].error.is_none(), "{runs:?}");
    assert_eq!(runs[0].reports.len(), 1, "under the cap: one run");
    let report = &runs[0].reports[0];
    assert_eq!(runs[0].library_id.as_str(), library);
    assert_eq!(report.objects.len(), 1, "{report:?}");
    assert_eq!(report.objects[0].reconstructed, ["v1"]);
    assert_eq!(report.objects[0].version, "v2");
    let (_, body) = desktop.get(&format!("/api/v1/files/{library}/history"));
    let page = json(&body);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 2, "{page}");
    assert!(items.iter().any(|item| item["reconstructed"] == true));
    assert!(items.iter().any(|item| item["kind"] == "backfill"));
    let again = desktop
        .catalog
        .backfill_history(&backfill, &cancel)
        .expect("libraries listed");
    let second = &again[0].reports[0];
    assert!(second.objects.is_empty(), "resumable: nothing left to fill");
    let processes = desktop.host.monitor().registry().snapshot();
    let ran: Vec<_> = processes
        .iter()
        .filter(|p| p.kind == BACKFILL_OPERATION)
        .collect();
    assert_eq!(ran.len(), 2, "each run registered: {ran:?}");
    assert!(
        ran.iter().all(|p| p.state == ProcessState::Completed),
        "{ran:?}"
    );
    add_collection(&desktop.library_service(), &library, "1", "op-after");
    let (dir, entries) = desktop.stop();
    let runs: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == BACKFILL_OPERATION)
        .collect();
    assert_eq!(
        runs.iter()
            .filter(|e| e.outcome == Outcome::Started)
            .count(),
        2,
        "{runs:?}"
    );
    assert_eq!(
        runs.iter()
            .filter(|e| e.outcome == Outcome::Succeeded)
            .count(),
        2
    );
    let filled: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == "ocfl.commit")
        .filter(|e| e.operation_id.as_deref() == Some(report.run_id.as_str()))
        .collect();
    assert_eq!(filled.len(), 1, "{entries:?}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// App exit sets the stop flag the backfill was given: a cancelled call starts no run, and
/// the versions stay without records for the next launch.
#[test]
fn a_cancelled_backfill_commits_nothing_and_leaves_the_rest_for_the_next_launch() {
    let dir = data_dir("cancelled");
    let library = library_without_history(&dir);
    let desktop = Desktop::launch_in(dir);
    let backfill = history_backfill(&desktop.host, &installation());
    let cancel = AtomicBool::new(true);
    let runs = desktop
        .catalog
        .backfill_history(&backfill, &cancel)
        .expect("libraries listed");
    assert!(runs.is_empty(), "{runs:?}");
    install_history_source(&desktop.state, desktop.catalog.clone()).expect("installed");
    let (_, body) = desktop.get(&format!("/api/v1/files/{library}/history"));
    assert_eq!(json(&body)["items"].as_array().expect("items").len(), 0);
    let next = desktop
        .catalog
        .backfill_history(&backfill, &AtomicBool::new(false))
        .expect("libraries listed");
    assert_eq!(
        next[0].reports[0].objects.len(),
        1,
        "the next launch fills it"
    );
    let (dir, _) = desktop.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Cubic round 1: a library whose storage root cannot be opened is left out and flags the
/// answers; the other libraries' history is still served.
#[test]
fn one_unreadable_library_leaves_the_others_history_served() {
    let desktop = Desktop::launch("unreadable");
    let service = desktop.library_service();
    let (readable, _) = create_library_as(&service, "op-readable");
    let (broken, _) = create_library_as(&service, "op-broken");
    let scratch = desktop
        .dir
        .join(CATALOG_DIR)
        .join(format!("{broken}.work"))
        .join("root-scratch");
    std::fs::remove_dir_all(&scratch).expect("scratch");
    std::fs::write(&scratch, b"not a directory").expect("block");
    install_history_source(&desktop.state, desktop.catalog.clone()).expect("installed");
    let (status, body) = desktop.get("/api/v1/history");
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let page = json(&body);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{page}");
    assert_eq!(
        items[0]["version"]["object_id"],
        format!("urn:uuid:{readable}").as_str()
    );
    assert_eq!(page["truncated"], true, "a library was left out");
    let (status, _) = desktop.get(&format!("/api/v1/files/{readable}/versions/v1/prov"));
    assert_eq!(status, 200);
    let text = String::from_utf8_lossy(&body).to_string();
    assert!(
        !text.contains(desktop.dir.to_string_lossy().as_ref()),
        "no host path"
    );
    drop(service);
    let (dir, _) = desktop.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}
