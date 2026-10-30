//! Tasks DE-0.2 and DE-1.7: the desktop opens its catalog directory at launch, beside the
//! activity journal in its data directory, keeps one index per library there while the
//! library itself lives in a folder of its own, and every catalog commit reaches that journal.
//! Runs against SQLite files and OCFL storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/folder_library.rs"]
#[allow(dead_code)]
mod folder_library;

use filesquid_api::journal::JournalEntry;
use filesquid_api::ApiState;
use filesquid_desktop_lib::{open_host_monitor, open_library_catalog, CATALOG_OPEN_OPERATION};
use filesquid_native::{SqliteJournal, CATALOG_DIR, JOURNAL_FILE};
use folder_library::{attach_new_library, remove_all};
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh, empty data directory for one test (filesystem I/O).
fn data_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fs-desktop-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(folder_library::folders_of(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    dir
}

/// Every entry of the host journal in `dir` (read after the monitor stopped).
fn host_journal(dir: &std::path::Path) -> Vec<JournalEntry> {
    let store = SqliteJournal::open(&dir.join(JOURNAL_FILE), "desktop").expect("journal");
    journal::all_entries(&store)
}

/// The installation UUID of these tests.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The installation of these tests.
fn installation() -> filesquid_core::InstallationId {
    filesquid_core::InstallationId::parse(INSTALLATION).expect("installation")
}

#[test]
fn the_host_keeps_one_catalog_file_per_library_and_journals_its_commits() {
    let dir = data_dir("open");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let catalog = Arc::new(open_library_catalog(&dir, &host, &installation()).expect("catalog"));
    let (library, ocfl) = attach_new_library(&catalog, &dir, "Family archive");
    drop(catalog);
    host.stop();
    let file = dir.join(CATALOG_DIR).join(format!("{library}.sqlite"));
    assert!(file.exists(), "{file:?}");
    assert!(
        !dir.join(CATALOG_DIR)
            .join(format!("{library}.ocfl"))
            .exists(),
        "no library root in the data directory"
    );
    assert!(
        ocfl.join("snapshot").join("0=ocfl_object_1.1").exists(),
        "{ocfl:?}"
    );
    let entries = host_journal(&dir);
    let created: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == "library.create")
        .collect();
    assert_eq!(created.len(), 1, "{entries:?}");
    let text = serde_json::to_string(&entries).expect("json");
    assert!(!text.contains("Family"), "{text}");
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
    remove_all(&dir);
}

#[test]
fn a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path() {
    let dir = data_dir("blocked");
    std::fs::write(dir.join(CATALOG_DIR), b"not a directory").expect("block");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let err = open_library_catalog(&dir, &host, &installation()).expect_err("refused");
    host.stop();
    assert!(!err.contains(dir.to_string_lossy().as_ref()), "{err}");
    let failures: Vec<JournalEntry> = host_journal(&dir)
        .into_iter()
        .filter(|e| e.operation == CATALOG_OPEN_OPERATION)
        .collect();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].outcome.as_str(), "failed");
    assert_eq!(failures[0].error_code.as_deref(), Some("unavailable"));
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// DE-0.10: catalog outbox entries reach the host journal through its
/// announcing store, so each raises a `journal.entry` event like every
/// other entry (ADR 040 Decision 6).
#[test]
fn catalog_commits_raise_journal_entry_events() {
    let dir = data_dir("events");
    let state = ApiState::new();
    let mut rx = state.events.subscribe();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let catalog = Arc::new(open_library_catalog(&dir, &host, &installation()).expect("catalog"));
    attach_new_library(&catalog, &dir, "Family archive");
    let mut operations = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if event.topic == "journal.entry" {
            operations.push(event.data["operation"].as_str().unwrap_or("").to_string());
        }
    }
    assert!(
        operations.iter().any(|op| op == "library.create"),
        "{operations:?}"
    );
    drop(catalog);
    host.stop();
    remove_all(&dir);
}

/// DE-0.10: the installation UUID is created once, journaled as
/// `host.installation.created` once, and read back unchanged at the next
/// launch (DE-S-012).
#[test]
fn the_installation_uuid_is_created_once_and_journaled() {
    use filesquid_desktop_lib::load_installation;
    use filesquid_native::installation::INSTALLATION_CREATED;
    let dir = data_dir("installation");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let first = load_installation(&dir, &host).expect("first launch");
    let again = load_installation(&dir, &host).expect("next launch");
    assert_eq!(first, again);
    host.stop();
    let entries = host_journal(&dir);
    let created: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == INSTALLATION_CREATED)
        .collect();
    assert_eq!(created.len(), 1, "{entries:?}");
    let text = serde_json::to_string(&entries).expect("json");
    assert!(
        !text.contains(first.as_str()),
        "the UUID is not journaled: {text}"
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// DE-0.10: a catalog commit publishes one snapshot version whose OCFL `user` is the
/// installation, whose message names the operation and its kind, and which carries the
/// operation's PROV-O record and journal entries; the host journal stores those same entries
/// (same IDs), linked by operation ID and object version (ADR 040 Decisions 3, 4 and 6).
#[test]
fn a_catalog_commit_carries_its_history_in_the_same_version() {
    let dir = data_dir("history");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let catalog = Arc::new(open_library_catalog(&dir, &host, &installation()).expect("catalog"));
    let (_, folder) = attach_new_library(&catalog, &dir, "Family archive");
    drop(catalog);
    host.stop();
    let object = folder.join("snapshot");
    let inventory: serde_json::Value =
        serde_json::from_slice(&std::fs::read(object.join("inventory.json")).expect("inventory"))
            .expect("inventory json");
    let v1 = &inventory["versions"]["v1"];
    assert_eq!(
        v1["user"]["address"],
        format!("urn:uuid:{INSTALLATION}"),
        "{v1}"
    );
    let message = v1["message"].as_str().expect("message");
    assert!(
        message.starts_with("filesquid.operation.v1 ")
            && message.contains("\"kind\":\"catalog.organisation\""),
        "{message}"
    );
    let paths: Vec<&str> = v1["state"]
        .as_object()
        .expect("state")
        .values()
        .flat_map(|paths| paths.as_array().expect("paths").iter())
        .filter_map(|path| path.as_str())
        .collect();
    assert!(paths.contains(&"provenance/v1.jsonld"), "{paths:?}");
    assert!(paths.contains(&"provenance/v1.journal.jsonl"), "{paths:?}");
    let lines = std::fs::read_to_string(object.join("v1/content/provenance/v1.journal.jsonl"))
        .expect("journal lines");
    let carried: Vec<JournalEntry> = lines
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect();
    let operations: Vec<&str> = carried.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(operations, ["ocfl.commit", "library.create"]);
    let object_ref = format!("{}@v1", inventory["id"].as_str().expect("id"));
    let operation = carried[0].operation_id.clone();
    assert!(operation.is_some(), "the create's operation ID");
    for entry in &carried {
        assert_eq!(entry.operation_id, operation, "one operation");
        assert_eq!(entry.object_ref.as_deref(), Some(object_ref.as_str()));
    }
    let stored = host_journal(&dir);
    for entry in &carried {
        let id = entry.id.expect("pre-allocated id");
        let same = stored
            .iter()
            .find(|e| e.id == Some(id))
            .expect("stored once");
        assert_eq!(same.operation, entry.operation);
        assert_eq!(same.object_ref, entry.object_ref);
    }
    let record =
        std::fs::read_to_string(object.join("v1/content/provenance/v1.jsonld")).expect("record");
    assert!(
        record.contains(&format!("urn:uuid:{INSTALLATION}")),
        "{record}"
    );
    assert!(
        !record.contains("Family"),
        "no title in the record: {record}"
    );
    remove_all(&dir);
}
