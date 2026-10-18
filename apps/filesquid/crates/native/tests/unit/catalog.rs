//! Task DE-0.2: the per-library SQLite catalog (`SqliteCatalog`), the native storage of the
//! `filesquid_api::library::LibraryCatalog` port. Every test runs against SQLite files on
//! disk, through `LibraryService` (the API's meaning) or a raw `CatalogCommit`.

#[path = "../support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{all_entries, catalog_file, temp_root, Fixture, FlakyJournal};
use filesquid_api::journal::{JournalEntry, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, FileListing, FileScope, LibraryCatalog,
    LibraryCommand, LibraryRecord, PageKey, PurgeConfirmation, VisibilityFilter,
    NEW_LIBRARY_REVISION,
};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollectionId, VirtualMembership,
};
use filesquid_native::{
    PendingIntent, SqliteCatalog, CATALOG_DIR, CATALOG_FILE_EXTENSION, CATALOG_SCHEMA_VERSION,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// A canonical UUID v4 for raw commits.
const LIB_A: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";

/// Store `ids` in `library_id` as an import would, with listing order `order`.
fn index(fx: &Fixture, library_id: &LibraryId, home: &CollectionId, ids: &[&str], order: i64) {
    let files: Vec<(File, i64)> = ids
        .iter()
        .map(|id| {
            let file = File::new(
                FileId::new(*id).expect("id"),
                library_id.clone(),
                home.clone(),
            );
            (file, order)
        })
        .collect();
    let entry = JournalEntry::new("library.file.index", Outcome::Succeeded);
    fx.catalog
        .index_files(library_id, &files, entry)
        .expect("index files");
}

/// The IDs of every `scope` file in `visibility`, read in pages of `limit`.
fn list(
    catalog: &SqliteCatalog,
    library_id: &LibraryId,
    scope: FileScope,
    visibility: VisibilityFilter,
    limit: usize,
) -> Vec<String> {
    let listing = FileListing {
        library_id: library_id.clone(),
        scope,
        visibility,
    };
    let mut out = Vec::new();
    let mut after: Option<PageKey> = None;
    for _ in 0..1_000 {
        let page = catalog
            .files(&listing, after.as_ref(), limit)
            .expect("files");
        if page.is_empty() {
            return out;
        }
        after = page.last().map(|k| k.key.clone());
        out.extend(
            page.into_iter()
                .map(|k| k.item.file.id().as_str().to_string()),
        );
    }
    panic!("listing did not end");
}

fn ids(raw: &[&str]) -> Vec<FileId> {
    raw.iter().map(|id| FileId::new(*id).expect("id")).collect()
}

fn names(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

/// A raw commit of `changes` to `library_id` at `revision`.
fn raw(library_id: &str, revision: &str, op: &str, changes: Vec<CatalogChange>) -> CatalogCommit {
    CatalogCommit {
        operation_id: OperationId::new(op).expect("op"),
        library_id: LibraryId::new(library_id).expect("library"),
        expected_revision: ExpectedRevision::new(revision).expect("revision"),
        changes,
        journal: JournalEntry::new("library.test", Outcome::Succeeded).target(library_id),
        created_id: None,
        deletion: None,
    }
}

fn record(library_id: &str) -> LibraryRecord {
    LibraryRecord {
        library: Library::new(
            LibraryId::new(library_id).expect("library"),
            RepositoryId::new("repo-1").expect("repo"),
        ),
        name: "Family archive".into(),
    }
}

/// Catalog files (by extension) directly inside `root`.
fn catalog_files(root: &std::path::Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(root)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(&format!(".{CATALOG_FILE_EXTENSION}")))
        .collect();
    found.sort();
    found
}

#[test]
fn the_catalog_directory_name_is_stable() {
    assert_eq!(CATALOG_DIR, "libraries");
    assert_eq!(CATALOG_FILE_EXTENSION, "sqlite");
    assert_eq!(CATALOG_SCHEMA_VERSION, 1);
}

#[test]
fn each_library_has_its_own_catalog_file_named_by_its_id() {
    let fx = Fixture::new("own-file");
    let a = fx.create_library("Family archive");
    let b = fx.create_library("Work");
    let mut expected = vec![
        format!("{}.sqlite", a.as_str()),
        format!("{}.sqlite", b.as_str()),
    ];
    expected.sort();
    assert_eq!(catalog_files(&fx.root), expected);
    fx.create_collection(&a, "Summer");
    let conn = rusqlite::Connection::open(fx.file_of(&b)).expect("open b");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM collections", [], |r| r.get(0))
        .expect("count");
    assert_eq!(
        count, 0,
        "library b's file holds none of library a's collections"
    );
}

#[test]
fn committed_state_survives_a_reopen() {
    let fx = Fixture::new("reopen");
    let lib = fx.create_library("Family archive");
    let home = fx.create_collection(&lib, "Summer");
    index(&fx, &lib, &home, &["f1", "f2"], 0);
    let vc = fx.create_virtual(&lib, "Best of");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::AddToVirtualCollection {
            library_id: lib.clone(),
            virtual_collection_id: vc.clone(),
            file_ids: ids(&["f2", "f1"]),
        },
    );
    let revision = fx.revision(&lib);
    let root = fx.root.clone();
    drop(fx);
    let again = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    assert_eq!(again.revision(&lib), revision);
    let members = list(
        &again.catalog,
        &lib,
        FileScope::VirtualCollection {
            virtual_collection_id: vc,
        },
        VisibilityFilter::Ordinary,
        10,
    );
    assert_eq!(members, names(&["f2", "f1"]));
    let collection = again
        .catalog
        .collection(&home)
        .expect("read")
        .expect("there");
    assert_eq!(collection.title(), "Summer");
}

#[test]
fn revisions_start_at_one_advance_on_change_and_stale_ones_conflict() {
    let fx = Fixture::new("revision");
    let lib = fx.create_library("Family archive");
    assert_eq!(fx.revision(&lib), "1");
    fx.create_collection(&lib, "Summer");
    assert_eq!(fx.revision(&lib), "2");
    let stale = fx.run_as(
        fx.op(),
        "1",
        LibraryCommand::CreateCollection {
            library_id: lib.clone(),
            title: "Winter".into(),
            folder_id: None,
            sort: Default::default(),
        },
    );
    assert_eq!(stale.expect_err("conflict").error_code(), "conflict");
    assert_eq!(fx.revision(&lib), "2");
}

#[test]
fn a_commit_without_change_keeps_the_revision_but_stores_its_receipt() {
    let fx = Fixture::new("no-change");
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-empty").expect("op");
    let commit = raw(lib.as_str(), "1", "op-empty", Vec::new());
    let receipt = fx.catalog.commit(commit).expect("commit");
    assert_eq!(receipt.revision, "1");
    assert_eq!(fx.catalog.receipt(&op).expect("read"), Some(receipt));
}

#[test]
fn a_repeated_operation_id_answers_its_first_receipt_even_after_reopen() {
    let fx = Fixture::new("replay");
    let lib = fx.create_library("Family archive");
    let op = OperationId::new("op-replay").expect("op");
    let command = || LibraryCommand::CreateCollection {
        library_id: lib.clone(),
        title: "Summer".into(),
        folder_id: None,
        sort: Default::default(),
    };
    let first = fx.run_as(op.clone(), "1", command()).expect("first");
    let root = fx.root.clone();
    drop(fx);
    let again = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    let second = again.run_as(op, "1", command()).expect("replay");
    assert_eq!(second.created_id, first.created_id);
    assert_eq!(again.revision(&lib), "2");
}

#[test]
fn a_receipt_in_one_library_answers_a_commit_naming_another() {
    let fx = Fixture::new("receipt-across");
    let a = fx.create_library("A");
    let b = fx.create_library("B");
    let first = fx
        .catalog
        .commit(raw(a.as_str(), "1", "op-x", Vec::new()))
        .expect("a");
    let second = fx
        .catalog
        .commit(raw(b.as_str(), "1", "op-x", Vec::new()))
        .expect("b");
    assert_eq!(second, first);
}

#[test]
fn deleting_a_collection_keeps_its_files_as_unfiled() {
    let fx = Fixture::new("unfiled");
    let lib = fx.create_library("Family archive");
    let home = fx.create_collection(&lib, "Summer");
    index(&fx, &lib, &home, &["f1", "f2"], 0);
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteCollection {
            library_id: lib.clone(),
            collection_id: home.clone(),
        },
    );
    let unfiled = list(
        &fx.catalog,
        &lib,
        FileScope::Unfiled,
        VisibilityFilter::Ordinary,
        10,
    );
    assert_eq!(unfiled, names(&["f1", "f2"]));
    assert_eq!(fx.catalog.collection(&home).expect("read"), None);
    let kept = fx
        .catalog
        .file(&ids(&["f1"])[0])
        .expect("read")
        .expect("kept");
    assert_eq!(kept.file.home_collection_id(), &home);
}

#[test]
fn deleting_a_virtual_collection_or_purging_a_file_removes_its_memberships() {
    let fx = Fixture::new("memberships");
    let lib = fx.create_library("Family archive");
    let home = fx.create_collection(&lib, "Summer");
    index(&fx, &lib, &home, &["f1", "f2"], 0);
    let keep = fx.create_virtual(&lib, "Keep");
    let gone = fx.create_virtual(&lib, "Gone");
    for vc in [&keep, &gone] {
        fx.run(
            &fx.revision(&lib),
            LibraryCommand::AddToVirtualCollection {
                library_id: lib.clone(),
                virtual_collection_id: vc.clone(),
                file_ids: ids(&["f1", "f2"]),
            },
        );
    }
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteVirtualCollection {
            library_id: lib.clone(),
            virtual_collection_id: gone.clone(),
        },
    );
    assert!(!fx
        .catalog
        .is_virtual_member(&gone, &ids(&["f1"])[0])
        .expect("read"));
    assert_eq!(fx.catalog.last_member_order(&gone).expect("read"), None);
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: ids(&["f1"]),
        },
    );
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::PurgeFiles {
            library_id: lib.clone(),
            file_ids: ids(&["f1"]),
            confirmation: PurgeConfirmation {
                library_id: lib.clone(),
                file_count: 1,
            },
        },
    );
    assert_eq!(fx.catalog.file(&ids(&["f1"])[0]).expect("read"), None);
    let scope = FileScope::VirtualCollection {
        virtual_collection_id: keep,
    };
    assert_eq!(
        list(&fx.catalog, &lib, scope, VisibilityFilter::Ordinary, 10),
        names(&["f2"])
    );
}

#[test]
fn listings_page_by_order_then_id_and_hide_hidden_and_deleted_files() {
    let fx = Fixture::new("paging");
    let lib = fx.create_library("Family archive");
    let home = fx.create_collection(&lib, "Summer");
    index(&fx, &lib, &home, &["e", "c", "a"], 1);
    index(&fx, &lib, &home, &["d", "b"], 0);
    let all = list(
        &fx.catalog,
        &lib,
        FileScope::Library,
        VisibilityFilter::Ordinary,
        2,
    );
    assert_eq!(all, names(&["b", "d", "a", "c", "e"]));
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::HideFiles {
            library_id: lib.clone(),
            file_ids: ids(&["a"]),
        },
    );
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: ids(&["c"]),
        },
    );
    let view = |v| list(&fx.catalog, &lib, FileScope::Library, v, 2);
    assert_eq!(view(VisibilityFilter::Ordinary), names(&["b", "d", "e"]));
    assert_eq!(view(VisibilityFilter::Hidden), names(&["a"]));
    assert_eq!(view(VisibilityFilter::Deleted), names(&["c"]));
    let deleted = fx
        .catalog
        .file(&ids(&["c"])[0])
        .expect("read")
        .expect("there");
    let deletion = deleted.state.deletion.expect("deletion");
    assert!(deletion.purge_after_ms > deletion.deleted_at_ms);
}

#[test]
fn a_failed_commit_leaves_the_catalog_unchanged() {
    let fx = Fixture::new("rollback");
    let lib = fx.create_library("Family archive");
    let journal_before = all_entries(fx.journal.as_ref()).len();
    let collection = Collection::new(CollectionId::new("c-new").expect("id"), lib.clone(), "X")
        .expect("collection");
    let dangling = VirtualMembership::new(
        VirtualCollectionId::new("vc-missing").expect("vc"),
        FileId::new("f-missing").expect("file"),
        0,
    );
    let commit = raw(
        lib.as_str(),
        "1",
        "op-bad",
        vec![
            CatalogChange::PutCollection(collection),
            CatalogChange::AddVirtualMember(dangling),
        ],
    );
    assert!(matches!(
        fx.catalog.commit(commit),
        Err(CatalogError::Failed(_))
    ));
    assert_eq!(fx.revision(&lib), "1");
    assert_eq!(
        fx.catalog
            .collection(&CollectionId::new("c-new").unwrap())
            .unwrap(),
        None
    );
    assert_eq!(
        fx.catalog
            .receipt(&OperationId::new("op-bad").unwrap())
            .unwrap(),
        None
    );
    assert_eq!(all_entries(fx.journal.as_ref()).len(), journal_before);
}

#[test]
fn a_failed_create_leaves_no_catalog_file() {
    let fx = Fixture::new("failed-create");
    let dangling = VirtualMembership::new(
        VirtualCollectionId::new("vc-missing").expect("vc"),
        FileId::new("f-missing").expect("file"),
        0,
    );
    let commit = raw(
        LIB_A,
        NEW_LIBRARY_REVISION,
        "op-create",
        vec![
            CatalogChange::CreateLibrary(record(LIB_A)),
            CatalogChange::AddVirtualMember(dangling),
        ],
    );
    assert!(fx.catalog.commit(commit).is_err());
    assert!(fx
        .catalog
        .library(&LibraryId::new(LIB_A).unwrap())
        .unwrap()
        .is_none());
    let left: Vec<_> = std::fs::read_dir(&fx.root).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn creating_an_existing_library_conflicts() {
    let fx = Fixture::new("create-twice");
    let create = |op: &str| {
        raw(
            LIB_A,
            NEW_LIBRARY_REVISION,
            op,
            vec![CatalogChange::CreateLibrary(record(LIB_A))],
        )
    };
    fx.catalog.commit(create("op-1")).expect("create");
    assert_eq!(
        fx.catalog.commit(create("op-2")),
        Err(CatalogError::Conflict)
    );
}

#[test]
fn a_library_id_that_is_not_a_uuid_is_refused_without_a_file() {
    let fx = Fixture::new("bad-id");
    let bad = "../escape";
    let commit = raw(
        bad,
        NEW_LIBRARY_REVISION,
        "op-1",
        vec![CatalogChange::CreateLibrary(record(bad))],
    );
    assert!(matches!(
        fx.catalog.commit(commit),
        Err(CatalogError::Failed(_))
    ));
    assert!(std::fs::read_dir(&fx.root).unwrap().next().is_none());
    assert!(!fx.root.parent().unwrap().join("escape.sqlite").exists());
}

#[test]
fn every_commit_journal_entry_reaches_the_host_journal_redacted() {
    let fx = Fixture::new("journal");
    let lib = fx.create_library("Family archive");
    fx.create_collection(&lib, "Summer holiday in Lisbon");
    let entries = all_entries(fx.journal.as_ref());
    let ops: Vec<&str> = entries.iter().map(|e| e.operation.as_str()).collect();
    assert_eq!(ops, ["library.create", "library.collection.create"]);
    let text = serde_json::to_string(&entries).expect("json");
    assert!(!text.contains("Summer"), "{text}");
    assert!(!text.contains("Family"), "{text}");
    assert!(!text.contains(fx.root.to_string_lossy().as_ref()), "{text}");
    assert_eq!(fx.catalog.outbox_len(&lib).expect("outbox"), 0);
}

#[test]
fn entries_the_host_journal_refused_are_forwarded_at_the_next_open() {
    let root = temp_root("outbox");
    let flaky = FlakyJournal::down();
    let fx = Fixture::over(root.clone(), flaky.clone());
    let lib = fx.create_library("Family archive");
    assert_eq!(fx.catalog.outbox_len(&lib).expect("outbox"), 1);
    drop(fx);
    flaky.down.store(false, Ordering::SeqCst);
    let again = Fixture::over(root, flaky.clone());
    assert_eq!(again.catalog.outbox_len(&lib).expect("outbox"), 0);
    let ops: Vec<String> = all_entries(&flaky.inner)
        .into_iter()
        .map(|e| e.operation)
        .collect();
    assert!(ops.contains(&"library.create".to_string()), "{ops:?}");
}

#[test]
fn pending_intent_is_recorded_and_cleared_by_its_commit() {
    let fx = Fixture::new("intent");
    let lib = fx.create_library("Family archive");
    let intent = PendingIntent {
        library_id: lib.clone(),
        operation_id: OperationId::new("op-intent").expect("op"),
        operation: "library.collection.create".into(),
        expected_revision: ExpectedRevision::new("1").expect("rev"),
        recorded_at_ms: 1_792_281_600_000,
    };
    fx.catalog.record_intent(&intent).expect("record");
    assert_eq!(
        fx.catalog.pending_intents(&lib, 10).expect("list"),
        vec![intent.clone()]
    );
    let root = fx.root.clone();
    drop(fx);
    let again = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    assert_eq!(
        again.catalog.pending_intents(&lib, 10).expect("list"),
        vec![intent]
    );
    again
        .catalog
        .commit(raw(lib.as_str(), "1", "op-intent", Vec::new()))
        .expect("commit");
    assert!(again
        .catalog
        .pending_intents(&lib, 10)
        .expect("list")
        .is_empty());
}

#[test]
fn a_consistent_backup_holds_the_committed_library_and_is_journaled() {
    let fx = Fixture::new("backup");
    let lib = fx.create_library("Family archive");
    fx.create_collection(&lib, "Summer");
    let dest_dir = temp_root("backup-dest");
    let dest = catalog_file(&dest_dir, &lib);
    fx.catalog.backup(&lib, &dest).expect("backup");
    let copy = Fixture::over(dest_dir.clone(), Arc::new(MemoryJournal::new("desktop")));
    assert_eq!(copy.revision(&lib), "2");
    assert!(
        fx.catalog.backup(&lib, &dest).is_err(),
        "an existing backup is not overwritten"
    );
    let ops: Vec<String> = all_entries(fx.journal.as_ref())
        .into_iter()
        .filter(|e| e.operation == "library.catalog.backup")
        .map(|e| e.outcome.as_str().to_string())
        .collect();
    assert_eq!(ops, ["succeeded", "failed"]);
}

#[test]
fn removing_a_catalog_file_loses_only_the_rebuildable_index() {
    let fx = Fixture::new("rebuild");
    let lib = fx.create_library("Family archive");
    let file = fx.file_of(&lib);
    let root = fx.root.clone();
    drop(fx);
    std::fs::remove_file(&file).expect("remove");
    let _ = std::fs::remove_file(file.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(file.with_extension("sqlite-shm"));
    let again = Fixture::over(root, Arc::new(MemoryJournal::new("desktop")));
    assert!(again.catalog.library(&lib).expect("read").is_none());
}

#[test]
fn folders_are_indexed_and_read_back() {
    let fx = Fixture::new("folders");
    let lib = fx.create_library("Family archive");
    let parent = Folder::new(FolderId::new("p").unwrap(), lib.clone(), "Years").unwrap();
    let child = Folder::new(FolderId::new("c").unwrap(), lib.clone(), "2026")
        .unwrap()
        .nest_under(FolderId::new("p").unwrap());
    let entry = JournalEntry::new("library.folder.index", Outcome::Succeeded);
    fx.catalog
        .index_folders(&lib, &[parent.clone(), child.clone()], entry)
        .expect("index");
    assert_eq!(
        fx.catalog.folder(&FolderId::new("c").unwrap()).unwrap(),
        Some(child)
    );
    assert_eq!(
        fx.catalog.folder(&FolderId::new("p").unwrap()).unwrap(),
        Some(parent)
    );
}

#[test]
fn the_schema_is_versioned_and_a_newer_catalog_is_refused() {
    let fx = Fixture::new("schema");
    let lib = fx.create_library("Family archive");
    let file = fx.file_of(&lib);
    let root = fx.root.clone();
    drop(fx);
    let conn = rusqlite::Connection::open(&file).expect("open");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("version");
    assert_eq!(version, CATALOG_SCHEMA_VERSION);
    conn.pragma_update(None, "user_version", CATALOG_SCHEMA_VERSION + 1)
        .expect("bump");
    drop(conn);
    let err =
        SqliteCatalog::open(&root, Arc::new(MemoryJournal::new("desktop"))).expect_err("refused");
    assert!(format!("{err:?}").contains("schema"), "{err:?}");
    assert!(
        !format!("{err:?}").contains(root.to_string_lossy().as_ref()),
        "{err:?}"
    );
}

#[test]
fn libraries_list_by_repository_in_creation_order() {
    let fx = Fixture::new("list-libraries");
    let a = fx.create_library("A");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let b = fx.create_library("B");
    let repo = RepositoryId::new("repo-1").unwrap();
    let all = fx.catalog.libraries(&repo, None, 10).expect("list");
    let listed: Vec<&str> = all.iter().map(|k| k.item.library.id().as_str()).collect();
    assert_eq!(listed, [a.as_str(), b.as_str()]);
    let after = fx
        .catalog
        .libraries(&repo, Some(&all[0].key), 10)
        .expect("page");
    assert_eq!(after.len(), 1);
    let other = RepositoryId::new("repo-2").unwrap();
    assert!(fx
        .catalog
        .libraries(&other, None, 10)
        .expect("list")
        .is_empty());
}

#[cfg(unix)]
#[test]
fn catalog_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new("modes");
    let lib = fx.create_library("Family archive");
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&fx.file_of(&lib)), 0o600);
    assert_eq!(mode(&fx.root), 0o700);
}

#[test]
fn an_unusable_root_fails_without_naming_its_path() {
    let base = temp_root("not-a-dir");
    let root = base.join("file");
    std::fs::write(&root, b"x").expect("file");
    let err =
        SqliteCatalog::open(&root, Arc::new(MemoryJournal::new("desktop"))).expect_err("refused");
    assert!(
        !format!("{err:?}").contains(base.to_string_lossy().as_ref()),
        "{err:?}"
    );
}

/// Drive `catalog` through a fixed sequence of commits (create, collections, virtual
/// collection, memberships, hide, delete, collection removal) and answer what every listing
/// shows afterwards, with each library's revision.
fn scripted(
    catalog: &dyn LibraryCatalog,
    index: impl Fn(&LibraryId, &CollectionId),
) -> Vec<String> {
    let lib = LibraryId::new(LIB_A).unwrap();
    let commit =
        |rev: &str, op: &str, changes| catalog.commit(raw(LIB_A, rev, op, changes)).unwrap();
    commit(
        NEW_LIBRARY_REVISION,
        "op-1",
        vec![CatalogChange::CreateLibrary(record(LIB_A))],
    );
    let home = CollectionId::new("c-1").unwrap();
    let vc = VirtualCollectionId::new("vc-1").unwrap();
    let put = Collection::new(home.clone(), lib.clone(), "Summer")
        .unwrap()
        .with_order_key(3);
    let virt = filesquid_core::VirtualCollection::new(vc.clone(), lib.clone(), "Best").unwrap();
    commit(
        "1",
        "op-2",
        vec![
            CatalogChange::PutCollection(put),
            CatalogChange::PutVirtualCollection(virt),
        ],
    );
    index(&lib, &home);
    let member = |f: &str, o| {
        CatalogChange::AddVirtualMember(VirtualMembership::new(
            vc.clone(),
            FileId::new(f).unwrap(),
            o,
        ))
    };
    commit("2", "op-3", vec![member("f3", 1), member("f1", 2)]);
    let hide = filesquid_api::library::FileState {
        hidden: true,
        deletion: None,
    };
    commit(
        "3",
        "op-4",
        vec![CatalogChange::SetFileState {
            file_id: FileId::new("f2").unwrap(),
            state: hide,
        }],
    );
    commit(
        "4",
        "op-5",
        vec![CatalogChange::RemoveCollection(home.clone())],
    );
    let mut out = vec![catalog.library(&lib).unwrap().unwrap().revision];
    for scope in [
        FileScope::Library,
        FileScope::Unfiled,
        FileScope::VirtualCollection {
            virtual_collection_id: vc.clone(),
        },
    ] {
        for visibility in [VisibilityFilter::Ordinary, VisibilityFilter::Hidden] {
            let listing = FileListing {
                library_id: lib.clone(),
                scope: scope.clone(),
                visibility,
            };
            let page = catalog.files(&listing, None, 10).unwrap();
            out.push(format!(
                "{scope:?}/{visibility:?}: {:?}",
                page.iter()
                    .map(|k| (k.key.order, k.key.id.clone()))
                    .collect::<Vec<_>>()
            ));
        }
    }
    out.push(format!("{:?}", catalog.last_member_order(&vc).unwrap()));
    out
}

#[test]
fn the_sqlite_catalog_answers_like_the_api_reference_catalog() {
    let files = |lib: &LibraryId, home: &CollectionId| -> Vec<(File, i64)> {
        ["f1", "f2", "f3"]
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (
                    File::new(FileId::new(*f).unwrap(), lib.clone(), home.clone()),
                    10 - i as i64,
                )
            })
            .collect()
    };
    let memory = filesquid_api::library::MemoryCatalog::new();
    let expected = scripted(&memory, |lib, home| {
        files(lib, home)
            .into_iter()
            .for_each(|(f, o)| memory.insert_file(f, o).unwrap())
    });
    let fx = Fixture::new("parity");
    let actual = scripted(fx.catalog.as_ref(), |lib, home| {
        let entry = JournalEntry::new("library.file.index", Outcome::Succeeded);
        fx.catalog
            .index_files(lib, &files(lib, home), entry)
            .unwrap()
    });
    assert_eq!(actual, expected);
}
