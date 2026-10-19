//! Task DE-0.3: the desktop's commit sequence (ADR 005): intent, then one OCFL snapshot
//! version through the change model, then the catalog from the published receipt; and
//! reconcile from the snapshot at open. Crash-injection tests stop a commit just before or
//! just after the version is published (`CommitProbe`), then reopen the catalog files on
//! disk as a restarted process would.

#[path = "../support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{all_entries, catalog_file, temp_root, Fixture};
use filesquid_api::canonical::{read_snapshot, OwnerReadable, SNAPSHOT_ROOT};
use filesquid_api::journal::{JournalEntry, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogError, FileListing, FileScope, LibraryCatalog, LibraryCommand, VisibilityFilter,
};
use filesquid_api::operation::OperationId;
use filesquid_core::{CollectionId, File, FileId, LibraryId};
use filesquid_native::{library_ocfl_root, CommitProbe};
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Stops the commit of one operation where a crash would.
struct CrashAt {
    operation: &'static str,
    before_snapshot: bool,
}

impl CommitProbe for CrashAt {
    fn before_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        self.stop(operation_id, true)
    }

    fn after_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        self.stop(operation_id, false)
    }
}

impl CrashAt {
    fn stop(&self, operation_id: &str, before: bool) -> Result<(), CatalogError> {
        if operation_id == self.operation && before == self.before_snapshot {
            return Err(CatalogError::Unavailable("simulated crash".into()));
        }
        Ok(())
    }
}

/// The library's snapshot read straight from its OCFL root on disk.
fn snapshot(root: &Path, library: &LibraryId) -> Option<filesquid_api::canonical::LibrarySnapshot> {
    let ocfl = library_ocfl_root(root, library).expect("root");
    let backend = FsBackend::with_default_staging(ocfl).expect("backend");
    complete_immediately(read_snapshot(&backend, &OwnerReadable))
        .expect("drive")
        .expect("read")
}

fn summer(library: &LibraryId) -> LibraryCommand {
    LibraryCommand::CreateCollection {
        library_id: library.clone(),
        title: "Summer".into(),
        folder_id: None,
        sort: Default::default(),
    }
}

/// A fixture whose commit of `operation` stops before or after the snapshot.
fn crashing(name: &str, operation: &'static str, before_snapshot: bool) -> Fixture {
    let probe = Arc::new(CrashAt {
        operation,
        before_snapshot,
    });
    Fixture::crashing(
        temp_root(name),
        Arc::new(MemoryJournal::new("desktop")),
        probe,
    )
}

/// Close `fx` and open its directory again with no probe, as a restarted host.
fn restart(fx: Fixture) -> Fixture {
    let journal = fx.journal.clone();
    Fixture::over(fx.release(), journal)
}

fn collections(fx: &Fixture, library: &LibraryId) -> Vec<String> {
    fx.catalog
        .collections(library, None, 100)
        .expect("collections")
        .into_iter()
        .map(|k| k.item.title().to_string())
        .collect()
}

#[test]
fn a_command_publishes_one_snapshot_version_before_the_catalog_answers() {
    let fx = Fixture::new("publish");
    let lib = fx.create_library("Family archive");
    fx.create_collection(&lib, "Summer");
    let snap = snapshot(&fx.root, &lib).expect("snapshot");
    assert_eq!(snap.version.to_string(), "v2");
    assert_eq!(snap.library.revision, 2);
    assert_eq!(snap.collections.len(), 1);
    let ops: Vec<&str> = snap
        .operations
        .iter()
        .map(|o| o.revision.as_str())
        .collect();
    assert_eq!(ops, ["1", "2"]);
    assert!(fx
        .catalog
        .pending_intents(&lib, 10)
        .expect("intents")
        .is_empty());
}

#[test]
fn a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once() {
    let fx = crashing("crash-before", "op-summer", true);
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-summer").unwrap();
    assert!(fx.run_as(op.clone(), "1", summer(&lib)).is_err());
    assert_eq!(fx.revision(&lib), "1");
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v1");
    let fx = restart(fx);
    let pending = fx.catalog.pending_intents(&lib, 10).expect("intents");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, op);
    let first = fx.run_as(op.clone(), "1", summer(&lib)).expect("retry");
    let again = fx.run_as(op, "1", summer(&lib)).expect("replay");
    assert_eq!(again.created_id, first.created_id);
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v2");
    assert_eq!(collections(&fx, &lib), ["Summer"]);
    assert!(fx
        .catalog
        .pending_intents(&lib, 10)
        .expect("intents")
        .is_empty());
}

#[test]
fn a_crash_after_the_snapshot_loses_no_acknowledged_state() {
    let fx = crashing("crash-after", "op-summer", false);
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-summer").unwrap();
    assert!(fx.run_as(op.clone(), "1", summer(&lib)).is_err());
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v2");
    let fx = restart(fx);
    assert_eq!(fx.revision(&lib), "2");
    assert_eq!(collections(&fx, &lib), ["Summer"]);
    assert!(fx
        .catalog
        .pending_intents(&lib, 10)
        .expect("intents")
        .is_empty());
    let replay = fx.run_as(op, "1", summer(&lib)).expect("replay");
    assert_eq!(replay.revision.as_deref(), Some("2"));
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v2");
    let reconciled: Vec<JournalEntry> = all_entries(fx.journal.as_ref())
        .into_iter()
        .filter(|e| e.operation == "library.reconcile")
        .collect();
    assert_eq!(reconciled.len(), 1, "{reconciled:?}");
    assert_eq!(reconciled[0].outcome, Outcome::Succeeded);
}

#[test]
fn a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts() {
    let fx = Fixture::new("rebuild-ocfl");
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-summer").unwrap();
    let first = fx
        .run_as(op.clone(), "1", summer(&lib))
        .expect("collection");
    let home = CollectionId::new(first.created_id.clone().unwrap()).unwrap();
    let file = File::new(FileId::new("f-1").unwrap(), lib.clone(), home);
    let entry = JournalEntry::new("library.file.index", Outcome::Succeeded);
    fx.catalog
        .index_files(&lib, &[(file, 3)], entry)
        .expect("index");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::HideFiles {
            library_id: lib.clone(),
            file_ids: vec![FileId::new("f-1").unwrap()],
        },
    );
    let revision = fx.revision(&lib);
    let file_path = fx.file_of(&lib);
    let root = fx.release();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = file_path.clone().into_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(name));
    }
    let fx = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    assert_eq!(fx.revision(&lib), revision);
    assert_eq!(collections(&fx, &lib), ["Summer"]);
    let hidden = FileListing {
        library_id: lib.clone(),
        scope: FileScope::Library,
        visibility: VisibilityFilter::Hidden,
    };
    assert_eq!(fx.catalog.files(&hidden, None, 10).expect("files").len(), 1);
    let replay = fx.run_as(op, "1", summer(&lib)).expect("replay");
    assert_eq!(replay.created_id, first.created_id);
}

#[test]
fn a_head_without_its_version_inventory_is_refused_and_not_repaired() {
    let fx = Fixture::new("w010");
    let lib = fx.create_library("Family archive");
    let ocfl = library_ocfl_root(&fx.root, &lib).unwrap();
    let inventory = ocfl.join(SNAPSHOT_ROOT).join("v1").join("inventory.json");
    std::fs::remove_file(&inventory).expect("remove");
    std::fs::remove_file(inventory.with_extension("json.sha512")).ok();
    std::fs::remove_file(inventory.with_extension("json.sha256")).ok();
    let refused = fx.run_as(OperationId::new("op-x").unwrap(), "1", summer(&lib));
    assert_eq!(refused.expect_err("refused").error_code(), "internal");
    assert!(!inventory.exists(), "the head is never repaired implicitly");
    assert_eq!(fx.revision(&lib), "1");
    assert!(collections(&fx, &lib).is_empty());
}

#[test]
fn an_unpublished_version_left_by_a_crash_is_ignored_and_replaced() {
    let fx = Fixture::new("unpublished");
    let lib = fx.create_library("Family archive");
    let ocfl = library_ocfl_root(&fx.root, &lib).unwrap();
    let stray = ocfl
        .join(SNAPSHOT_ROOT)
        .join("v2")
        .join("content")
        .join("collections");
    std::fs::create_dir_all(&stray).expect("dir");
    std::fs::write(stray.join("ghost.json"), b"half written").expect("write");
    let fx = restart(fx);
    assert!(collections(&fx, &lib).is_empty());
    fx.create_collection(&lib, "Summer");
    let snap = snapshot(&fx.root, &lib).unwrap();
    assert_eq!(snap.version.to_string(), "v2");
    assert_eq!(snap.collections.len(), 1);
}

#[test]
fn every_library_keeps_its_ocfl_root_beside_its_catalog_file() {
    let fx = Fixture::new("ocfl-layout");
    let lib = fx.create_library("Family archive");
    let ocfl = library_ocfl_root(&fx.root, &lib).unwrap();
    assert!(ocfl.join("0=ocfl_1.1").exists());
    assert!(ocfl.join(SNAPSHOT_ROOT).join("0=ocfl_object_1.1").exists());
    assert!(catalog_file(&fx.root, &lib).exists());
    let text = serde_json::to_string(&all_entries(fx.journal.as_ref())).unwrap();
    assert!(!text.contains(fx.root.to_string_lossy().as_ref()), "{text}");
}
