//! Task DE-0.3: the desktop's commit sequence (ADR 005): intent, then one OCFL snapshot
//! version through the change model, then the catalog from the published receipt; and
//! reconcile from the snapshot at open. Crash-injection tests stop a commit just before or
//! just after the version is published (`CommitProbe`), then reopen the catalog files on
//! disk as a restarted process would.

#[path = "../support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{all_entries, catalog_file, temp_root, Fixture};
use filesquid_api::canonical::{read_snapshot, OwnerReadable, SNAPSHOT_ROOT};
use filesquid_api::journal::{JournalEntry, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::library::{
    CatalogError, FileListing, FileScope, LibraryCatalog, LibraryCommand, VisibilityFilter,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::operation::OperationId;
use filesquid_api::EventBus;
use filesquid_core::{CollectionId, File, FileId, Folder, FolderId, LibraryId};
use filesquid_native::{library_ocfl_root, CommitProbe, SqliteCatalog};
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Remove `path` and SQLite's `-wal` and `-shm` files beside it. The main file must exist;
/// only a missing sidecar is tolerated, so a failed removal fails the test.
fn remove_catalog_files(path: &std::path::Path) {
    std::fs::remove_file(path).expect("remove the catalog file");
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        match std::fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("remove a catalog sidecar: {err}"),
        }
    }
}

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
    remove_catalog_files(&file_path);
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
    assert!(
        fx.catalog
            .pending_intents(&lib, 10)
            .expect("intents")
            .is_empty(),
        "a refusal before publication discards its intent"
    );
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
    let escaped = serde_json::to_string(&fx.root.to_string_lossy()).unwrap();
    assert!(!text.contains(escaped.trim_matches('"')), "{text}");
}

#[cfg(unix)]
#[test]
fn a_library_directory_that_is_a_symbolic_link_is_refused() {
    let fx = Fixture::new("symlinked-work");
    let outside = temp_root("symlinked-work-target");
    let lib = LibraryId::new("0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a07").unwrap();
    let work = fx.root.join(format!(
        "{}{}",
        lib.as_str(),
        filesquid_native::WORK_DIR_SUFFIX
    ));
    std::os::unix::fs::symlink(&outside, &work).expect("link");
    let create = filesquid_api::library::CatalogCommit {
        operation_id: OperationId::new("op-create").unwrap(),
        library_id: lib.clone(),
        expected_revision: filesquid_api::operation::ExpectedRevision::new(
            filesquid_api::library::NEW_LIBRARY_REVISION,
        )
        .unwrap(),
        changes: vec![filesquid_api::library::CatalogChange::CreateLibrary(
            filesquid_api::library::LibraryRecord {
                library: filesquid_core::Library::new(
                    lib.clone(),
                    filesquid_core::RepositoryId::new("repo-1").unwrap(),
                ),
                name: "Family archive".into(),
            },
        )],
        journal: JournalEntry::new("library.create", Outcome::Succeeded),
        created_id: None,
        deletion: None,
    };
    assert!(fx.catalog.commit(create).is_err());
    assert!(
        std::fs::read_dir(&outside).unwrap().next().is_none(),
        "nothing written outside"
    );
    std::fs::remove_dir_all(outside).expect("cleanup");
}

#[test]
fn a_missing_snapshot_is_published_again_in_full_by_the_next_commit() {
    let fx = Fixture::new("snapshot-missing");
    let lib = fx.create_library("Family archive");
    fx.create_collection(&lib, "Summer");
    let ocfl = library_ocfl_root(&fx.root, &lib).unwrap();
    std::fs::remove_dir_all(ocfl.join(SNAPSHOT_ROOT)).expect("remove snapshot");
    let fx = restart(fx);
    fx.create_collection(&lib, "Winter");
    let snap = snapshot(&fx.root, &lib).expect("snapshot");
    assert_eq!(snap.version.to_string(), "v1");
    assert_eq!(
        snap.collections.len(),
        2,
        "every entity, not only the new one"
    );
}

#[test]
fn a_snapshot_of_another_library_is_never_loaded() {
    let fx = Fixture::new("foreign-snapshot");
    let lib = fx.create_library("Family archive");
    let other = fx.create_library("Work");
    fx.create_collection(&other, "Work only");
    let root = fx.release();
    let theirs = library_ocfl_root(&root, &other)
        .unwrap()
        .join(SNAPSHOT_ROOT);
    let ours = library_ocfl_root(&root, &lib).unwrap().join(SNAPSHOT_ROOT);
    std::fs::remove_dir_all(&ours).expect("remove ours");
    std::fs::rename(&theirs, &ours).expect("move theirs");
    let file = catalog_file(&root, &lib);
    remove_catalog_files(&file);
    let again = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    assert!(again.catalog.library(&lib).expect("read").is_none());
}

/// DE-0.10: a commit's journal entries ride its snapshot version, so a crash after the
/// version was published (the catalog transaction, and its outbox, lost) replays them at the
/// next open with their pre-allocated IDs: each reaches the host journal exactly once,
/// linked to the operation and the object version.
#[test]
fn a_crash_after_the_snapshot_replays_the_commits_journal_entries_once() {
    let journal: Arc<dyn filesquid_api::journal::JournalStore> =
        Arc::new(MemoryJournal::new("desktop"));
    let probe = Arc::new(CrashAt {
        operation: "op-summer",
        before_snapshot: false,
    });
    let fx = Fixture::recording(temp_root("replay-history"), journal.clone(), probe);
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-summer").unwrap();
    assert!(fx.run_as(op, "1", summer(&lib)).is_err());
    let lost: Vec<JournalEntry> = all_entries(journal.as_ref())
        .into_iter()
        .filter(|e| e.operation_id.as_deref() == Some("op-summer"))
        .collect();
    assert!(lost.is_empty(), "the crash lost the outbox: {lost:?}");
    let fx = Fixture::recording(fx.release(), journal.clone(), Arc::new(NoCrash));
    let replayed: Vec<JournalEntry> = all_entries(journal.as_ref())
        .into_iter()
        .filter(|e| e.operation_id.as_deref() == Some("op-summer"))
        .collect();
    let operations: Vec<&str> = replayed.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(operations, ["ocfl.commit", "library.collection.create"]);
    for entry in &replayed {
        assert!(entry.id.is_some());
        assert!(
            entry
                .object_ref
                .as_deref()
                .is_some_and(|r| r.ends_with("@v2")),
            "{entry:?}"
        );
    }
    drop(fx);
}

/// DE-0.10: without a provenance context every published commit still journals its
/// `ocfl.commit` entry and its own entry, linked by operation ID and object version.
#[test]
fn an_unrecorded_catalog_still_links_its_commits() {
    let fx = Fixture::new("unrecorded");
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-summer").unwrap();
    fx.run_as(op, "1", summer(&lib)).expect("collection");
    let linked: Vec<JournalEntry> = all_entries(fx.journal.as_ref())
        .into_iter()
        .filter(|e| e.operation_id.as_deref() == Some("op-summer"))
        .collect();
    let operations: Vec<&str> = linked.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(operations, ["ocfl.commit", "library.collection.create"]);
    assert!(linked
        .iter()
        .all(|e| e.object_ref.as_deref().is_some_and(|r| r.ends_with("@v2"))));
    let snap = snapshot(&fx.root, &lib).expect("snapshot");
    assert_eq!(
        snap.collections.len(),
        1,
        "no history file is read as an entity"
    );
}

/// A probe that never stops a commit.
struct NoCrash;

impl CommitProbe for NoCrash {
    fn before_snapshot(&self, _: &str) -> Result<(), CatalogError> {
        Ok(())
    }

    fn after_snapshot(&self, _: &str) -> Result<(), CatalogError> {
        Ok(())
    }
}

/// DE-0.10: when the host journal cannot allocate the commit's journal IDs, the version is
/// still published (naming its operation and the installation) and the commit journals a
/// `prov.record` warning that carries only its error code: no path, title or installation.
#[test]
fn a_commit_without_journal_ids_publishes_and_warns_without_identifying_data() {
    let flaky = catalog_fixture::FlakyJournal::down();
    flaky.down.store(false, std::sync::atomic::Ordering::SeqCst);
    let journal: Arc<dyn filesquid_api::journal::JournalStore> = flaky.clone();
    let fx = Fixture::recording(temp_root("no-ids"), journal, Arc::new(NoCrash));
    let lib = fx.create_library("Family archive");
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v1");
    let warnings: Vec<JournalEntry> = all_entries(&flaky.inner)
        .into_iter()
        .filter(|e| e.operation == "prov.record")
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(
        warnings[0].error_code.as_deref(),
        Some("journal_ids_unavailable")
    );
    let text = serde_json::to_string(&warnings).expect("json");
    assert!(!text.contains("Family"), "{text}");
    assert!(!text.contains(catalog_fixture::INSTALLATION), "{text}");
    assert!(!text.contains(fx.root.to_string_lossy().as_ref()), "{text}");
}

/// The entries in `journal` linked to operation `op`.
fn linked_to(journal: &dyn filesquid_api::journal::JournalStore, op: &str) -> Vec<JournalEntry> {
    all_entries(journal)
        .into_iter()
        .filter(|e| e.operation_id.as_deref() == Some(op))
        .collect()
}

/// Owner decision 2026-10-19 with DE-0.10 (ADR 040 Decision 1): in a catalog that records
/// PROV-O history, a command that changes nothing commits no version and therefore no PROV-O
/// record, and stores no receipt and no journal entry (no `ocfl.commit` either); the next
/// command that does change something still commits a version with its `ocfl.commit` and own
/// entry, linked to that version.
#[test]
fn a_recorded_no_op_commits_no_version_and_journals_nothing() {
    let journal: Arc<dyn filesquid_api::journal::JournalStore> =
        Arc::new(MemoryJournal::new("desktop"));
    let fx = Fixture::recording(
        temp_root("recorded-no-op"),
        journal.clone(),
        Arc::new(NoCrash),
    );
    let lib = fx.create_library("Family archive");
    let created = fx.run("1", summer(&lib));
    let collection = CollectionId::new(created.created_id.expect("id")).expect("collection");
    let before = all_entries(journal.as_ref()).len();
    let rename = |title: &str| LibraryCommand::RenameCollection {
        library_id: lib.clone(),
        collection_id: collection.clone(),
        title: title.into(),
    };
    let same = OperationId::new("op-same-title").unwrap();
    let result = fx
        .run_as(same.clone(), "2", rename("Summer"))
        .expect("no-op");
    assert_eq!(result.revision.as_deref(), Some("2"));
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v2");
    assert_eq!(fx.catalog.receipt(&same).expect("read"), None);
    assert_eq!(all_entries(journal.as_ref()).len(), before);
    assert!(linked_to(journal.as_ref(), "op-same-title").is_empty());
    let changed = OperationId::new("op-new-title").unwrap();
    fx.run_as(changed, "2", rename("Summer 2026"))
        .expect("rename");
    assert_eq!(snapshot(&fx.root, &lib).unwrap().version.to_string(), "v3");
    let linked = linked_to(journal.as_ref(), "op-new-title");
    let operations: Vec<&str> = linked.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(operations, ["ocfl.commit", "library.collection.rename"]);
    assert!(linked
        .iter()
        .all(|e| e.object_ref.as_deref().is_some_and(|r| r.ends_with("@v3"))));
}

/// The snapshot version name of `library` (`v1`, `v2`, ...).
fn head(fx: &Fixture, library: &LibraryId) -> String {
    snapshot(&fx.root, library)
        .expect("snapshot")
        .version
        .to_string()
}

/// Index `files` (each with its listing order) in `library`, journaled as
/// `library.file.index`.
fn index_files(fx: &Fixture, library: &LibraryId, files: &[(File, i64)]) {
    let entry = JournalEntry::new("library.file.index", Outcome::Succeeded);
    fx.catalog
        .index_files(library, files, entry)
        .expect("index files");
}

/// Index `folders` in `library`, journaled as `library.folder.index`.
fn index_folders(fx: &Fixture, library: &LibraryId, folders: &[Folder]) {
    let entry = JournalEntry::new("library.folder.index", Outcome::Succeeded);
    fx.catalog
        .index_folders(library, folders, entry)
        .expect("index folders");
}

/// The entries `journal` stored after its first `before` entries.
fn journaled_since(
    journal: &dyn filesquid_api::journal::JournalStore,
    before: usize,
) -> Vec<JournalEntry> {
    all_entries(journal).into_iter().skip(before).collect()
}

/// A library created in `fx` with its collection "Summer" (at v2), answered with that
/// collection.
fn library_with_home(fx: &Fixture) -> (LibraryId, CollectionId) {
    let lib = fx.create_library("Family archive");
    let created = fx.run("1", summer(&lib));
    let home = CollectionId::new(created.created_id.expect("id")).expect("collection");
    (lib, home)
}

/// The root folder `years` of `library`, named `name`.
fn years(library: &LibraryId, name: &str) -> Folder {
    Folder::new(FolderId::new("years").unwrap(), library.clone(), name).unwrap()
}

/// Owner decision 2026-10-19: a catalog index refresh that changes nothing is treated like a
/// no-op command. Re-indexing the same files or folders, or indexing none, commits no
/// version and journals nothing (no outbox entry either); a refresh that does change
/// something still commits a version and journals its `ocfl.commit` and own entry, linked to
/// that version, exactly as before.
#[test]
fn an_index_refresh_that_changes_nothing_journals_nothing() {
    let fx = Fixture::new("index-no-op");
    let (lib, home) = library_with_home(&fx);
    let file = |order| {
        let file = File::new(FileId::new("f-1").unwrap(), lib.clone(), home.clone());
        (file, order)
    };
    index_files(&fx, &lib, &[file(3)]);
    index_folders(&fx, &lib, &[years(&lib, "Years")]);
    assert_eq!(head(&fx, &lib), "v4");
    let before = all_entries(fx.journal.as_ref()).len();
    index_files(&fx, &lib, &[file(3)]);
    index_files(&fx, &lib, &[]);
    index_folders(&fx, &lib, &[years(&lib, "Years")]);
    index_folders(&fx, &lib, &[]);
    assert_eq!(head(&fx, &lib), "v4");
    assert!(journaled_since(fx.journal.as_ref(), before).is_empty());
    assert_eq!(fx.catalog.outbox_len(&lib).expect("outbox"), 0);
    index_files(&fx, &lib, &[file(4)]);
    index_folders(&fx, &lib, &[years(&lib, "Decades")]);
    assert_eq!(head(&fx, &lib), "v6");
    let changed = journaled_since(fx.journal.as_ref(), before);
    let operations: Vec<&str> = changed.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(
        operations,
        [
            "ocfl.commit",
            "library.file.index",
            "ocfl.commit",
            "library.folder.index"
        ]
    );
    let linked: Vec<bool> = changed
        .iter()
        .zip(["@v5", "@v5", "@v6", "@v6"])
        .map(|(e, v)| e.object_ref.as_deref().is_some_and(|r| r.ends_with(v)))
        .collect();
    assert_eq!(linked, [true; 4], "{changed:?}");
}

/// A refresh that fails (a file or folder of another library) is not a no-op: it publishes
/// no version and is journaled once as `failed` with its error code, and nothing else.
#[test]
fn a_failed_index_refresh_is_journaled_as_failed() {
    let fx = Fixture::new("index-failed");
    let (lib, home) = library_with_home(&fx);
    let other = LibraryId::new("0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01").unwrap();
    let stray = File::new(FileId::new("f-x").unwrap(), other.clone(), home);
    let before = all_entries(fx.journal.as_ref()).len();
    let files = JournalEntry::new("library.file.index", Outcome::Succeeded);
    assert!(fx.catalog.index_files(&lib, &[(stray, 1)], files).is_err());
    let folders = JournalEntry::new("library.folder.index", Outcome::Succeeded);
    assert!(fx
        .catalog
        .index_folders(&lib, &[years(&other, "Years")], folders)
        .is_err());
    assert_eq!(head(&fx, &lib), "v2");
    assert_eq!(fx.catalog.outbox_len(&lib).expect("outbox"), 0);
    let journaled = journaled_since(fx.journal.as_ref(), before);
    let seen: Vec<(&str, Outcome, Option<&str>)> = journaled
        .iter()
        .map(|e| (e.operation.as_str(), e.outcome, e.error_code.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [
            ("library.file.index", Outcome::Failed, Some("internal")),
            ("library.folder.index", Outcome::Failed, Some("internal"))
        ]
    );
    let text = format!("{journaled:?}");
    assert!(
        journaled.iter().all(|e| e.target_ids == [lib.as_str()]),
        "{text}"
    );
    for private in [
        "Years",
        "f-x",
        other.as_str(),
        fx.root.to_string_lossy().as_ref(),
    ] {
        assert!(!text.contains(private), "redacted: {text}");
    }
}

/// A refresh of a library the catalog does not hold has no outbox to journal into: it is
/// refused, and its `failed` entry, targeting that library, goes through the host's bounded
/// failure journal queue (never dropped, producers wait), not straight to the store.
#[test]
fn a_refresh_of_an_unknown_library_is_journaled_as_failed() {
    let root = temp_root("index-unknown");
    let journal: Arc<dyn JournalStore> = Arc::new(MemoryJournal::new("desktop"));
    let failures: Arc<dyn JournalStore> = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        failures.clone(),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let catalog = SqliteCatalog::open(&root, journal.clone())
        .expect("open")
        .with_failure_journal(writer.sender());
    let unknown = LibraryId::new("0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a02").unwrap();
    let entry = JournalEntry::new("library.folder.index", Outcome::Succeeded);
    let refused = catalog.index_folders(&unknown, &[years(&unknown, "Years")], entry);
    assert!(
        matches!(refused, Err(CatalogError::Failed(_))),
        "{refused:?}"
    );
    writer.sender().flush();
    assert!(all_entries(journal.as_ref()).is_empty());
    let journaled = all_entries(failures.as_ref());
    let seen: Vec<(&str, Outcome, Option<&str>, &[String])> = journaled
        .iter()
        .map(|e| {
            let targets = e.target_ids.as_slice();
            (
                e.operation.as_str(),
                e.outcome,
                e.error_code.as_deref(),
                targets,
            )
        })
        .collect();
    let target = [unknown.as_str().to_string()];
    assert_eq!(
        seen,
        [(
            "library.folder.index",
            Outcome::Failed,
            Some("internal"),
            &target[..]
        )]
    );
    assert!(!format!("{journaled:?}").contains("Years"), "{journaled:?}");
    writer.shutdown();
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// The same rule in a catalog that records PROV-O history (DE-0.10): an index refresh that
/// changes nothing writes no version, so no PROV-O record, and journals nothing; one that
/// changes something journals exactly the entries its version carries.
#[test]
fn a_recorded_index_refresh_that_changes_nothing_journals_nothing() {
    let journal: Arc<dyn filesquid_api::journal::JournalStore> =
        Arc::new(MemoryJournal::new("desktop"));
    let fx = Fixture::recording(
        temp_root("recorded-index-no-op"),
        journal.clone(),
        Arc::new(NoCrash),
    );
    let (lib, home) = library_with_home(&fx);
    let file = |order| {
        let file = File::new(FileId::new("f-1").unwrap(), lib.clone(), home.clone());
        (file, order)
    };
    index_files(&fx, &lib, &[file(3)]);
    assert_eq!(head(&fx, &lib), "v3");
    let before = all_entries(journal.as_ref()).len();
    index_files(&fx, &lib, &[file(3)]);
    index_folders(&fx, &lib, &[]);
    assert_eq!(head(&fx, &lib), "v3");
    assert!(journaled_since(journal.as_ref(), before).is_empty());
    assert_eq!(fx.catalog.outbox_len(&lib).expect("outbox"), 0);
    index_files(&fx, &lib, &[file(4)]);
    assert_eq!(head(&fx, &lib), "v4");
    let changed = journaled_since(journal.as_ref(), before);
    let operations: Vec<&str> = changed.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(operations, ["ocfl.commit", "library.file.index"]);
    assert!(changed
        .iter()
        .all(|e| e.object_ref.as_deref().is_some_and(|r| r.ends_with("@v4"))));
}
