//! Task DE-0.2: the desktop opens its per-library SQLite catalogs at launch, beside the
//! activity journal in its data directory, and every catalog commit reaches that journal.
//! Runs against SQLite files on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{
    CommandRequest, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION,
};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::ApiState;
use filesquid_core::RepositoryId;
use filesquid_desktop_lib::{open_host_monitor, open_library_catalog, CATALOG_OPEN_OPERATION};
use filesquid_native::{SqliteJournal, CATALOG_DIR, JOURNAL_FILE};
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh, empty data directory for one test (filesystem I/O).
fn data_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fs-desktop-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    dir
}

/// Every entry of the host journal in `dir` (read after the monitor stopped).
fn host_journal(dir: &std::path::Path) -> Vec<JournalEntry> {
    let store = SqliteJournal::open(&dir.join(JOURNAL_FILE), "desktop").expect("journal");
    journal::all_entries(&store)
}

/// A create-library request on a fixed repository.
fn create_library() -> CommandRequest {
    CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new("op-create").expect("op"),
            ExpectedRevision::new(NEW_LIBRARY_REVISION).expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::CreateLibrary {
            repository_id: RepositoryId::new("repo-1").expect("repo"),
            name: "Family archive".into(),
        },
    }
}

#[test]
fn the_host_keeps_one_catalog_file_per_library_and_journals_its_commits() {
    let dir = data_dir("open");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let catalog = Arc::new(open_library_catalog(&dir, &host).expect("catalog"));
    let service = LibraryService::new(catalog.clone(), host.monitor().journal());
    let result = service.execute("owner", create_library()).expect("create");
    let library = result.created_id.expect("library id");
    drop(service);
    drop(catalog);
    host.stop();
    let file = dir.join(CATALOG_DIR).join(format!("{library}.sqlite"));
    assert!(file.exists(), "{file:?}");
    let entries = host_journal(&dir);
    let created: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == "library.create")
        .collect();
    assert_eq!(created.len(), 1, "{entries:?}");
    let text = serde_json::to_string(&entries).expect("json");
    assert!(!text.contains("Family"), "{text}");
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path() {
    let dir = data_dir("blocked");
    std::fs::write(dir.join(CATALOG_DIR), b"not a directory").expect("block");
    let state = ApiState::new();
    let host = open_host_monitor(&state, &dir, None).expect("monitor");
    let err = open_library_catalog(&dir, &host).expect_err("refused");
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
