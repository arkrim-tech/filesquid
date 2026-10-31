//! Task API-F2.15 (API-S-019, API-S-026; ADR 041 Decisions 8 to 10) on the desktop's SQLite
//! catalog: a deleted file's cause and its deleted home collection's title are kept in the
//! snapshot and survive an index rebuild; a library's own retention is a journaled library
//! command kept in its snapshot; a restore whose home collection is gone creates an
//! "untitled collection" in the same snapshot version.

#[path = "../support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{catalog_file, Fixture};
use filesquid_api::canonical::{read_snapshot, OwnerReadable};
use filesquid_api::library::{DeletionCause, LibraryCatalog, LibraryCommand, UNTITLED_COLLECTION_TITLE};
use filesquid_core::{CollectionId, FileId, LibraryId};
use filesquid_native::catalog::RegisterFile;
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use std::sync::Arc;

/// One day in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// A fresh file ID.
fn new_file() -> FileId {
    FileId::new(filesquid_api::library::mint_uuid_v4().expect("uuid")).expect("file id")
}

/// Register `file` in `collection` of `library` at `path`.
fn register(fx: &Fixture, library: &LibraryId, collection: &CollectionId, file: &FileId, path: &str) {
    let request = RegisterFile {
        file_id: file.clone(),
        collection_id: collection.clone(),
        path: path.to_string(),
    };
    let entry = filesquid_api::journal::JournalEntry::new(
        "library.files.register",
        filesquid_api::journal::Outcome::Succeeded,
    )
    .operation_id(format!("op-register-{}", file.as_str()));
    fx.catalog
        .register_file(library, &request, entry)
        .expect("register");
}

/// The snapshot at the head of `library`'s folder.
fn snapshot_of(fx: &Fixture, library: &LibraryId) -> filesquid_api::canonical::LibrarySnapshot {
    let backend = FsBackend::new(fx.folder_of(library), fx.root.join("read-parts")).expect("fs");
    complete_immediately(read_snapshot(&backend, &OwnerReadable))
        .expect("ready")
        .expect("read")
        .expect("snapshot")
}

/// Drop `library`'s index and reopen the catalog, which rebuilds it from the folder.
fn rebuilt(fx: Fixture, library: &LibraryId) -> Fixture {
    let root = fx.release();
    std::fs::remove_file(catalog_file(&root, library)).expect("lose the index");
    Fixture::over(
        root,
        Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    )
}

#[test]
fn the_cause_and_the_removed_home_title_survive_a_rebuild() {
    let fx = Fixture::new("f215-cause");
    let lib = fx.create_library("Family archive");
    let visits = fx.create_collection(&lib, "Site visits");
    let docs = fx.create_collection(&lib, "Documents");
    let alone = new_file();
    let with = new_file();
    register(&fx, &lib, &docs, &alone, "a.pdf");
    register(&fx, &lib, &visits, &with, "2026/b.jpg");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![alone.clone()],
        },
    );
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteCollection {
            library_id: lib.clone(),
            collection_id: visits.clone(),
        },
    );
    let entity = snapshot_of(&fx, &lib)
        .files
        .into_iter()
        .find(|f| f.file.id() == &with)
        .expect("entity");
    assert_eq!(entity.removed_home_title.as_deref(), Some("Site visits"));
    let fx = rebuilt(fx, &lib);
    let one = fx.catalog.file(&alone).expect("read").expect("file");
    assert_eq!(one.state.deletion.expect("deleted").cause, DeletionCause::OneByOne);
    assert!(one.removed_home_title.is_none());
    let two = fx.catalog.file(&with).expect("read").expect("file");
    assert_eq!(
        two.state.deletion.expect("deleted").cause,
        DeletionCause::WithCollection
    );
    assert_eq!(two.removed_home_title.as_deref(), Some("Site visits"));
}

#[test]
fn a_library_retention_command_is_kept_in_the_snapshot_and_journaled() {
    let fx = Fixture::new("f215-retention");
    let lib = fx.create_library("Family archive");
    let docs = fx.create_collection(&lib, "Documents");
    let file = new_file();
    register(&fx, &lib, &docs, &file, "a.pdf");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::SetLibraryRetention {
            library_id: lib.clone(),
            retention_days: Some(90),
        },
    );
    assert_eq!(snapshot_of(&fx, &lib).library.retention_days, Some(90));
    let fx = rebuilt(fx, &lib);
    let state = fx.catalog.library(&lib).expect("read").expect("library");
    assert_eq!(state.retention_days, Some(90), "survives an index rebuild");
    let deleted = fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![file],
        },
    );
    let deletion = deleted.deletion.expect("deleted");
    assert_eq!(deletion.purge_after_ms - deletion.deleted_at_ms, 90 * DAY_MS);
}

#[test]
fn a_restore_whose_home_is_gone_creates_an_untitled_collection_in_one_version() {
    let fx = Fixture::new("f215-restore");
    let lib = fx.create_library("Family archive");
    let visits = fx.create_collection(&lib, "Site visits");
    let file = new_file();
    register(&fx, &lib, &visits, &file, "2026/site-visit-01.jpg");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteCollection {
            library_id: lib.clone(),
            collection_id: visits.clone(),
        },
    );
    let before = snapshot_of(&fx, &lib).version;
    let restored = fx.run(
        &fx.revision(&lib),
        LibraryCommand::RestoreFiles {
            library_id: lib.clone(),
            file_ids: vec![file.clone()],
        },
    );
    let made = &restored.restored_into[0];
    assert_eq!(made.title, UNTITLED_COLLECTION_TITLE);
    let snapshot = snapshot_of(&fx, &lib);
    let n = |v: String| v.trim_start_matches('v').parse::<u32>().expect("n");
    assert_eq!(n(snapshot.version.to_string()), n(before.to_string()) + 1);
    let entity = snapshot
        .files
        .iter()
        .find(|f| f.file.id() == &file)
        .expect("entity");
    assert_eq!(entity.file.home_collection_id(), &made.collection_id);
    assert_eq!(entity.path.as_deref(), Some("2026/site-visit-01.jpg"));
    assert!(entity.state.deletion.is_none());
    assert!(entity.removed_home_title.is_none());
    assert!(snapshot
        .collections
        .iter()
        .any(|c| c.id() == &made.collection_id && c.title() == UNTITLED_COLLECTION_TITLE));
    assert!(fx
        .catalog
        .folder_exists(&made.collection_id, "2026")
        .expect("read"));
}
