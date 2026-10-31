//! Task DE-1.7 (DE-S-009, DE-S-010, DE-S-018, API-S-019, API-S-026; ADR 041): every library
//! lives in a folder the owner chose and the catalog directory keeps only its rebuildable
//! index; each file is registered in its home collection at its collection logical path with
//! its own object; a taken path is numbered; deleting a collection deletes its files in place;
//! and the index is rebuilt from the folder when it is missing, of another copy or behind.

#[path = "../support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{all_entries, catalog_file, folders_dir, temp_root, Fixture};
use filesquid_api::canonical::{
    commit_snapshot, operation_message, read_snapshot, snapshot_object_id, LibraryHead,
    OperationRecord, OwnerReadable, SnapshotDelta, SnapshotEntity, SNAPSHOT_ROOT,
};
use filesquid_api::files::{ScopeAfter, ScopeEntry};
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, FileListing, FileScope, LibraryCatalog,
    LibraryCommand, VisibilityFilter, COLLECTION_HOLDS_FILES,
};
use filesquid_api::operation::ExpectedRevision;
use filesquid_core::{CollectionId, File, FileId, LibraryId};
use filesquid_native::catalog::{RegisterFile, Registration, ENTITY_RENAME_OPERATION};
use filesquid_native::{CATALOG_SCHEMA_VERSION, RECONCILE_OPERATION};
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use std::path::Path;

/// One day in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("filesystem futures complete at once")
}

/// A fresh file ID.
fn new_file() -> FileId {
    FileId::new(filesquid_api::library::mint_uuid_v4().expect("uuid")).expect("file id")
}

/// The journal entry an import gives a registration under `operation_id`.
fn registration_entry(operation_id: &str) -> JournalEntry {
    JournalEntry::new("library.files.register", Outcome::Succeeded).operation_id(operation_id)
}

/// Register `file` in `collection` of `library` at `path` under `operation_id`.
fn register(
    fx: &Fixture,
    library: &LibraryId,
    collection: &CollectionId,
    file: &FileId,
    path: &str,
    operation_id: &str,
) -> Registration {
    let request = RegisterFile {
        file_id: file.clone(),
        collection_id: collection.clone(),
        path: path.to_string(),
    };
    fx.catalog
        .register_file(library, &request, registration_entry(operation_id))
        .expect("register")
}

/// The path a registration answered.
fn path_of(registration: Registration) -> String {
    match registration {
        Registration::Registered { path } | Registration::AlreadyRegistered { path } => path,
        Registration::Refused(code) => panic!("refused: {code}"),
    }
}

/// The names in the catalog directory, sorted.
fn catalog_names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .expect("catalog dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

/// The snapshot at the head of `library`'s folder.
fn snapshot_of(fx: &Fixture, library: &LibraryId) -> filesquid_api::canonical::LibrarySnapshot {
    let backend = FsBackend::new(fx.folder_of(library), fx.root.join("read-parts")).expect("fs");
    ready(read_snapshot(&backend, &OwnerReadable))
        .expect("read")
        .expect("snapshot")
}

#[test]
fn no_library_root_or_original_is_ever_created_in_the_catalog_directory() {
    let fx = Fixture::new("de17-no-root");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "a.jpg", "op-1"));
    for name in catalog_names(&fx.root) {
        let ok = name == format!("{}.sqlite", lib.as_str())
            || name == format!("{}.sqlite-wal", lib.as_str())
            || name == format!("{}.sqlite-shm", lib.as_str())
            || name == format!("{}.work", lib.as_str());
        assert!(ok, "unexpected {name} in the catalog directory");
    }
    let folder = fx.folder_of(&lib);
    assert!(
        folder.join("0=ocfl_1.1").is_file(),
        "the folder is the storage root"
    );
    assert!(
        folder.join(SNAPSHOT_ROOT).is_dir(),
        "its snapshot lives in the folder"
    );
}

#[test]
fn a_create_library_command_is_refused_and_creates_nothing() {
    let fx = Fixture::new("de17-no-create");
    let refused = fx.run_as(
        fx.op(),
        filesquid_api::library::NEW_LIBRARY_REVISION,
        LibraryCommand::CreateLibrary {
            repository_id: fx.repository.clone(),
            name: "Elsewhere".into(),
        },
    );
    assert!(
        refused.is_err(),
        "libraries are created in a folder the owner chose"
    );
    assert!(catalog_names(&fx.root).is_empty(), "nothing was created");
}

#[test]
fn a_retired_in_app_root_is_neither_opened_nor_served() {
    let fx = Fixture::new("de17-retired");
    let lib = fx.create_library("Family archive");
    let root = fx.release();
    let stray = LibraryId::new("4a3b2c1d-0e9f-4a8b-9c7d-6e5f4a3b2c1d").expect("id");
    std::fs::create_dir_all(root.join(format!("{}.ocfl", stray.as_str()))).expect("old root");
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    assert!(fx.catalog.library(&stray).expect("read").is_none());
    assert!(fx.catalog.library(&lib).expect("read").is_some());
}

#[test]
fn a_version_2_catalog_is_migrated_to_the_current_version_and_rebuilt_from_its_folder() {
    let fx = Fixture::new("de17-v2");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "2024/a.jpg", "op-1"));
    let root = fx.release();
    {
        let conn = rusqlite::Connection::open(catalog_file(&root, &lib)).expect("open");
        conn.execute_batch(
            "DROP INDEX files_place; DROP INDEX files_dir;
             ALTER TABLE files DROP COLUMN path; ALTER TABLE files DROP COLUMN object_id;
             ALTER TABLE files DROP COLUMN dir_key; ALTER TABLE files DROP COLUMN name;
             ALTER TABLE library DROP COLUMN retention_days;
             ALTER TABLE library DROP COLUMN folder_identity;
             ALTER TABLE files DROP COLUMN deletion_cause;
             ALTER TABLE files DROP COLUMN removed_home_title;
             ALTER TABLE receipts DROP COLUMN deletion_cause; DROP TABLE pending_purges;
             PRAGMA user_version = 2;",
        )
        .expect("downgrade to v2");
    }
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    let conn = rusqlite::Connection::open(catalog_file(&fx.root, &lib)).expect("open");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("version");
    assert_eq!(version, CATALOG_SCHEMA_VERSION);
    assert_eq!(CATALOG_SCHEMA_VERSION, 5);
    let (path, object): (String, String) = conn
        .query_row(
            "SELECT path, object_id FROM files WHERE id = ?1",
            [file.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the rebuilt row");
    assert_eq!(path, "2024/a.jpg");
    assert_eq!(object, format!("urn:uuid:{}", file.as_str()));
}

#[test]
fn a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files() {
    let fx = Fixture::new("de17-copy");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "2024/a.jpg", "op-1"));
    let before = snapshot_of(&fx, &lib);
    let root = fx.release();
    let moved = root.with_extension("moved");
    let _ = std::fs::remove_dir_all(&moved);
    std::fs::rename(folders_dir(&root).join(lib.as_str()), &moved).expect("move the folder");
    std::fs::remove_file(catalog_file(&root, &lib)).expect("lose the index");
    let journal = std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop"));
    let fx = Fixture::over(root, journal.clone());
    let report = fx.attach(&lib, &moved).expect("attach the moved folder");
    assert!(report.rebuilt && !report.created);
    assert_eq!(report.unregistered, 0);
    let collection = fx
        .catalog
        .collection(&trips)
        .expect("read")
        .expect("collection");
    assert_eq!(collection.title(), "Trips");
    let entry = fx.catalog.file(&file).expect("read").expect("file");
    assert_eq!(entry.file.home_collection_id(), &trips);
    assert_eq!(
        fx.catalog
            .subfolders(&trips, "", None, 10)
            .expect("folders"),
        ["2024"]
    );
    let reconciles: Vec<_> = all_entries(journal.as_ref())
        .into_iter()
        .filter(|e| e.operation == RECONCILE_OPERATION)
        .collect();
    assert_eq!(reconciles.len(), 1, "one journaled reconcile");
    assert_eq!(before.files, snapshot_of_folder(&fx, &moved).files);
    let _ = std::fs::remove_dir_all(&moved);
}

#[test]
fn renaming_a_library_retitles_its_entity_in_one_journaled_version_and_a_same_name_rename_publishes_nothing(
) {
    let journal = std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop"));
    let fx = Fixture::over(temp_root("de17-rename"), journal.clone());
    let lib = fx.create_library("Family archive");
    let before = snapshot_of(&fx, &lib);
    fx.catalog
        .rename_library(&lib, "Family archive 1990s", "owner")
        .expect("rename");
    assert_eq!(
        fx.catalog.library_name(&lib).expect("name"),
        "Family archive 1990s"
    );
    let after = snapshot_of(&fx, &lib);
    assert_eq!(after.library.record.name, "Family archive 1990s");
    assert_eq!(
        after.version.to_string(),
        next_version(&before.version.to_string())
    );
    fx.catalog
        .rename_library(&lib, "Family archive 1990s", "owner")
        .expect("same name");
    assert_eq!(
        snapshot_of(&fx, &lib).version,
        after.version,
        "a same-name rename publishes no version"
    );
    let entries = all_entries(journal.as_ref());
    let renames: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == ENTITY_RENAME_OPERATION)
        .collect();
    assert_eq!(renames.len(), 1, "one journaled entity rename");
    assert_eq!(renames[0].target_ids, [lib.as_str()]);
    assert_eq!(renames[0].actor, "owner");
    let text = serde_json::to_string(&entries).expect("json");
    assert!(!text.contains("Family archive"), "no name in the journal");
}

/// The snapshot at the head of `folder`.
fn snapshot_of_folder(fx: &Fixture, folder: &Path) -> filesquid_api::canonical::LibrarySnapshot {
    let backend = FsBackend::new(folder, fx.root.join("read-parts")).expect("fs");
    ready(read_snapshot(&backend, &OwnerReadable))
        .expect("read")
        .expect("snapshot")
}

#[test]
fn an_index_built_from_another_copy_of_the_folder_is_rebuilt() {
    let fx = Fixture::new("de17-other-copy");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let root = fx.release();
    let copy = root.with_extension("copy");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(&folders_dir(&root).join(lib.as_str()), &copy);
    std::fs::remove_dir_all(folders_dir(&root).join(lib.as_str())).expect("leave only the copy");
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    let report = fx.attach(&lib, &copy).expect("attach the copy");
    assert!(report.rebuilt, "the index named another folder");
    assert!(fx.catalog.collection(&trips).expect("read").is_some());
    let _ = std::fs::remove_dir_all(&copy);
}

#[test]
fn a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered() {
    let fx = Fixture::new("de17-unregistered");
    let lib = fx.create_library("Family archive");
    let stray = new_file();
    commit_stray_object(&fx, &lib, &stray);
    let root = fx.release();
    std::fs::remove_file(catalog_file(&root, &lib)).expect("lose the index");
    let journal = std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop"));
    let fx = Fixture::over(root, journal.clone());
    assert!(
        fx.catalog.file(&stray).expect("read").is_none(),
        "never placed anywhere"
    );
    let reconcile = all_entries(journal.as_ref())
        .into_iter()
        .find(|e| e.operation == RECONCILE_OPERATION)
        .expect("a reconcile entry");
    assert!(
        reconcile.target_ids.iter().any(|t| t == "unregistered:1"),
        "{:?}",
        reconcile.target_ids
    );
}

/// Commit a file object at `files/<file>` in `lib`'s folder, as an earlier build did.
fn commit_stray_object(fx: &Fixture, lib: &LibraryId, file: &FileId) {
    let location = fx.locations.register(&fx.folder_of(lib)).expect("register");
    let root = fx.locations.root_of(&location).expect("root");
    let staging = FsBackend::new(fx.root.join("stray-staging"), fx.root.join("stray-parts"))
        .expect("staging");
    let area = StagingArea::new(&staging, "stray").expect("area");
    let model = ChangeModel::new(
        root.backend.as_ref(),
        root.reservations.as_ref(),
        "fixture",
        &format!("files/{}", file.as_str()),
        format!("urn:uuid:{}", file.as_str()),
        area,
    );
    ready(model.stage_bytes("original/x.bin", b"bytes")).expect("stage");
    ready(model.commit(VersionMeta::new("2026-10-23T00:00:00Z"))).expect("commit");
}

#[test]
fn a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index() {
    let journal = std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop"));
    let fx = Fixture::over(catalog_fixture::temp_root("de17-register"), journal.clone());
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    let first = register(&fx, &lib, &trips, &file, "2024/Caf\u{e9}.jpg", "op-import");
    assert_eq!(
        first,
        Registration::Registered {
            path: "2024/Caf\u{e9}.jpg".into()
        }
    );
    let snapshot = snapshot_of(&fx, &lib);
    let entity = snapshot
        .files
        .iter()
        .find(|f| f.file.id() == &file)
        .expect("entity");
    assert_eq!(entity.path.as_deref(), Some("2024/Caf\u{e9}.jpg"));
    assert_eq!(
        entity.object_id,
        Some(format!("urn:uuid:{}", file.as_str()))
    );
    assert_eq!(entity.file.home_collection_id(), &trips);
    let operation = snapshot.operations.last().expect("operation");
    assert_eq!(
        operation.operation_id, "op-import",
        "under the import's operation ID"
    );
    let head = snapshot.version.clone();
    let again = register(&fx, &lib, &trips, &file, "2024/Caf\u{e9}.jpg", "op-import");
    assert_eq!(
        again,
        Registration::AlreadyRegistered {
            path: "2024/Caf\u{e9}.jpg".into()
        }
    );
    assert_eq!(snapshot_of(&fx, &lib).version, head, "no second version");
    let entries: Vec<_> = all_entries(journal.as_ref())
        .into_iter()
        .filter(|e| e.operation == "library.files.register")
        .collect();
    assert_eq!(entries.len(), 1);
    let text = serde_json::to_string(&entries).expect("json");
    assert!(
        !text.contains("Caf") && !text.contains("2024/"),
        "no name or path: {text}"
    );
}

#[test]
fn the_catalog_is_the_home_collections_of_uploads_and_copies() {
    use filesquid_api::canonical::collection_path::ItemKind;
    use filesquid_api::home::{register_entry, HomeCollections, HomeRequest, Placed, Reserved};
    let fx = Fixture::new("api-f214-home");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let home: &dyn HomeCollections = fx.catalog.as_ref();
    assert_eq!(
        home.collection_library(&trips).expect("lookup"),
        Some(lib.clone())
    );
    let minted = new_file();
    let reserved = home
        .reserve(&lib, "op-up-1", "fp-1", &minted)
        .expect("reserve");
    assert_eq!(reserved, Reserved::File(minted.clone()));
    let again = home
        .reserve(&lib, "op-up-1", "fp-1", &new_file())
        .expect("reserve again");
    assert_eq!(
        again,
        Reserved::File(minted.clone()),
        "a retry gets the same file ID"
    );
    let other = home
        .reserve(&lib, "op-up-1", "fp-2", &new_file())
        .expect("reserve another input");
    assert_eq!(
        other,
        Reserved::Reused,
        "another command under the same operation ID"
    );
    path_of(register(
        &fx,
        &lib,
        &trips,
        &new_file(),
        "Summer/beach.jpg",
        "op-other",
    ));
    let free = home.free_path(&lib, &trips, "Summer/beach.jpg", ItemKind::File);
    assert_eq!(free.expect("read"), Ok("Summer/beach-1.jpg".to_string()));
    let request = HomeRequest {
        file_id: minted.clone(),
        collection_id: trips.clone(),
        path: "Summer/beach.jpg".into(),
    };
    let placed = home
        .register(&lib, "op-up-1", &request, register_entry("op-up-1"))
        .expect("register");
    assert_eq!(
        placed,
        Placed::Registered {
            path: "Summer/beach-1.jpg".into()
        }
    );
    let fresh = new_file();
    let after = home
        .reserve(&lib, "op-up-1", "fp-3", &fresh)
        .expect("reserve after");
    assert_eq!(
        after,
        Reserved::File(fresh),
        "the registration released the reservation"
    );
    home.release(&lib, "op-up-1").expect("release");
    let replay = home
        .register(&lib, "op-up-1", &request, register_entry("op-up-1"))
        .expect("register again");
    assert_eq!(
        replay,
        Placed::AlreadyRegistered {
            collection_id: trips.clone(),
            path: "Summer/beach-1.jpg".into()
        }
    );
}

#[test]
fn a_registered_file_is_relocated_in_one_version_and_its_own_path_never_takes_it() {
    use filesquid_api::home::{place_entry, HomeCollections, HomeRequest, Placed};
    let fx = Fixture::new("api-f214-relocate");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let home: &dyn HomeCollections = fx.catalog.as_ref();
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "a/photo.jpg", "op-1"));
    path_of(register(
        &fx,
        &lib,
        &trips,
        &new_file(),
        "a/harbour.jpg",
        "op-2",
    ));
    let to = |path: &str| HomeRequest {
        file_id: file.clone(),
        collection_id: trips.clone(),
        path: path.into(),
    };
    let same = home
        .relocate(&lib, &to("a/photo.jpg"), place_entry("op-3"))
        .expect("same");
    assert_eq!(
        same,
        Placed::AlreadyRegistered {
            collection_id: trips.clone(),
            path: "a/photo.jpg".into()
        }
    );
    let before = snapshot_of(&fx, &lib).version;
    let moved = home
        .relocate(&lib, &to("a/harbour.jpg"), place_entry("op-4"))
        .expect("move");
    assert_eq!(
        moved,
        Placed::Registered {
            path: "a/harbour-1.jpg".into()
        }
    );
    let after = snapshot_of(&fx, &lib);
    assert_eq!(
        after.version.to_string(),
        next_version(&before.to_string()),
        "exactly one snapshot version"
    );
    let operation = after.operations.last().expect("operation");
    assert!(
        format!("{operation:?}").contains("op-4"),
        "under the operation ID"
    );
    assert_eq!(
        home.placement(&lib, &file).expect("read"),
        Some((trips.clone(), "a/harbour-1.jpg".into()))
    );
    let stranger = HomeRequest {
        file_id: new_file(),
        ..to("b.jpg")
    };
    let refused = home
        .relocate(&lib, &stranger, place_entry("op-5"))
        .expect("refused");
    assert_eq!(refused, Placed::Refused("not_found"));
}

#[test]
fn a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts() {
    let fx = Fixture::new("de17-numbering");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let first = new_file();
    path_of(register(&fx, &lib, &trips, &first, "photo.jpg", "op-1"));
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![first],
        },
    );
    assert!(fx.catalog.path_taken(&trips, "photo.jpg").expect("taken"));
    let second = new_file();
    let placed = path_of(register(&fx, &lib, &trips, &second, "photo.jpg", "op-2"));
    assert_eq!(placed, "photo-1.jpg");
    let third = new_file();
    let placed = path_of(register(&fx, &lib, &trips, &third, "photo.jpg", "op-3"));
    assert_eq!(placed, "photo-2.jpg");
    assert!(fx.catalog.path_taken(&trips, "photo-2.jpg").expect("taken"));
    let nested = new_file();
    path_of(register(
        &fx,
        &lib,
        &trips,
        &nested,
        "directoryY/a.txt",
        "op-4",
    ));
    assert!(fx
        .catalog
        .path_taken(&trips, "directoryY")
        .expect("a folder is taken"));
    assert!(!fx
        .catalog
        .path_taken(&trips, "directory")
        .expect("not a prefix match"));
}

#[test]
fn a_registration_whose_numbers_are_all_taken_is_refused_and_changes_nothing() {
    let fx = Fixture::new("de17-exhausted");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let long = format!("{}.jpg", "n".repeat(251));
    path_of(register(&fx, &lib, &trips, &new_file(), &long, "op-1"));
    let head = snapshot_of(&fx, &lib).version;
    let refused = register(&fx, &lib, &trips, &new_file(), &long, "op-2");
    let Registration::Refused(code) = refused else {
        panic!("a numbered name over 255 bytes is refused")
    };
    assert_eq!(code, "name_too_long");
    assert_eq!(snapshot_of(&fx, &lib).version, head, "nothing changed");
}

#[test]
fn deleting_a_collection_deletes_its_files_in_place_in_one_version() {
    let fx = Fixture::new("de17-delete-collection");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let earlier = new_file();
    let fresh = new_file();
    path_of(register(&fx, &lib, &trips, &earlier, "a.jpg", "op-1"));
    path_of(register(&fx, &lib, &trips, &fresh, "b/c.jpg", "op-2"));
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![earlier.clone()],
        },
    );
    let kept = fx
        .catalog
        .file(&earlier)
        .expect("read")
        .expect("file")
        .state
        .deletion;
    let before = snapshot_of(&fx, &lib).version;
    let deleted = fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteCollection {
            library_id: lib.clone(),
            collection_id: trips.clone(),
        },
    );
    let deletion = deleted.deletion.expect("a newly deleted file");
    let snapshot = snapshot_of(&fx, &lib);
    assert_eq!(
        snapshot.version.to_string(),
        next_version(&before.to_string()),
        "one snapshot version"
    );
    assert!(snapshot.collections.iter().all(|c| c.id() != &trips));
    let entity = |id: &FileId| {
        snapshot
            .files
            .iter()
            .find(|f| f.file.id() == id)
            .expect("entity")
            .clone()
    };
    assert_eq!(entity(&fresh).state.deletion, Some(deletion));
    assert_eq!(
        entity(&fresh).file.home_collection_id(),
        &trips,
        "nothing is moved"
    );
    assert_eq!(entity(&fresh).path.as_deref(), Some("b/c.jpg"));
    assert_eq!(entity(&earlier).state.deletion, kept, "earlier times kept");
    let root = fx.release();
    std::fs::remove_file(catalog_file(&root, &lib)).expect("lose the index");
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    let rebuilt = fx.catalog.file(&fresh).expect("read").expect("file");
    assert_eq!(rebuilt.state.deletion, Some(deletion));
    assert_eq!(rebuilt.file.home_collection_id(), &trips);
    let rebuilt = fx.catalog.file(&earlier).expect("read").expect("file");
    assert_eq!(rebuilt.state.deletion, kept);
}

/// The version after `version` (`v3` after `v2`).
fn next_version(version: &str) -> String {
    let n: u32 = version.trim_start_matches('v').parse().expect("number");
    format!("v{}", n + 1)
}

#[test]
fn the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after() {
    let fx = Fixture::new("de17-retention");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "a.jpg", "op-1"));
    let root_dir = fx.root.clone();
    set_retention_externally(&fx, &lib, 7);
    let root = fx.release();
    assert_eq!(root, root_dir);
    std::fs::remove_file(catalog_file(&root, &lib)).expect("lose the index");
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    let state = fx.catalog.library(&lib).expect("read").expect("library");
    assert_eq!(state.retention_days, Some(7));
    let deleted = fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![file],
        },
    );
    let deletion = deleted.deletion.expect("deleted");
    assert_eq!(deletion.purge_after_ms - deletion.deleted_at_ms, 7 * DAY_MS);
    assert_eq!(snapshot_of(&fx, &lib).library.retention_days, Some(7));
}

/// Publish a snapshot version setting `lib`'s own retention, as the settings command of
/// API-F2.15 will, through the folder's own leases.
fn set_retention_externally(fx: &Fixture, lib: &LibraryId, days: u32) {
    let snapshot = snapshot_of(fx, lib);
    let location = fx.locations.register(&fx.folder_of(lib)).expect("register");
    let root = fx.locations.root_of(&location).expect("root");
    let staging =
        FsBackend::new(fx.root.join("ext-staging"), fx.root.join("ext-parts")).expect("staging");
    let area = StagingArea::new(&staging, "external").expect("area");
    let model = ChangeModel::new(
        root.backend.as_ref(),
        root.reservations.as_ref(),
        "fixture",
        SNAPSHOT_ROOT,
        snapshot_object_id(lib),
        area,
    );
    let head = LibraryHead {
        retention_days: Some(days),
        revision: snapshot.library.revision + 1,
        ..snapshot.library
    };
    let mut delta = SnapshotDelta::default();
    delta
        .put(&SnapshotEntity::Library(head.clone()))
        .expect("put");
    let record = OperationRecord {
        operation_id: "op-retention".into(),
        kind: None,
        revision: head.revision.to_string(),
        created_id: None,
        deletion: None,
    };
    let meta = VersionMeta::new("2026-10-23T00:00:00Z").with_message(operation_message(&record));
    ready(commit_snapshot(&model, &OwnerReadable, &delta, meta))
        .expect("commit")
        .expect("a version");
}

#[test]
fn folders_and_scope_entries_come_from_the_index_of_ordinary_files() {
    let fx = Fixture::new("de17-folders");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let ids: Vec<FileId> = (0..6).map(|_| new_file()).collect();
    for (file, path) in ids.iter().zip([
        "trips/2024/a.jpg",
        "trips/b.jpg",
        "c.jpg",
        "secret/x.jpg",
        "gone/y.jpg",
        "trips-old/z.jpg",
    ]) {
        path_of(register(&fx, &lib, &trips, file, path, "op"));
    }
    let rev = fx.revision(&lib);
    fx.run(
        &rev,
        LibraryCommand::HideFiles {
            library_id: lib.clone(),
            file_ids: vec![ids[3].clone()],
        },
    );
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![ids[4].clone()],
        },
    );
    assert_eq!(
        fx.catalog
            .subfolders(&trips, "", None, 10)
            .expect("folders"),
        ["trips", "trips-old"],
        "code-point order; hidden and deleted files imply no folder"
    );
    assert_eq!(
        fx.catalog
            .subfolders(&trips, "", Some("trips"), 10)
            .expect("folders"),
        ["trips-old"]
    );
    assert_eq!(
        fx.catalog
            .subfolders(&trips, "trips", None, 10)
            .expect("folders"),
        ["2024"]
    );
    assert!(fx
        .catalog
        .folder_exists(&trips, "trips/2024")
        .expect("exists"));
    assert!(!fx
        .catalog
        .folder_exists(&trips, "secret")
        .expect("hidden only"));
    assert!(!fx
        .catalog
        .folder_exists(&trips, "gone")
        .expect("deleted only"));
    let page = fx
        .catalog
        .scope_entries(&trips, "trips", None, 10)
        .expect("entries")
        .expect("indexed");
    assert_eq!(page.library_id, lib.as_str());
    assert_eq!(
        page.entries,
        [
            ScopeEntry::Folder {
                name: "2024".into()
            },
            ScopeEntry::File {
                name: "b.jpg".into(),
                file_id: ids[1].as_str().to_string()
            }
        ]
    );
    let after = ScopeAfter::Folder {
        name: "2024".into(),
    };
    let rest = fx
        .catalog
        .scope_entries(&trips, "trips", Some(&after), 10)
        .expect("entries")
        .expect("indexed");
    assert_eq!(rest.entries.len(), 1);
    let unknown = CollectionId::new("no-such-collection").expect("id");
    assert!(fx
        .catalog
        .scope_entries(&unknown, "", None, 10)
        .expect("read")
        .is_none());
}

#[test]
fn an_index_write_for_a_library_that_is_not_attached_is_refused() {
    let fx = Fixture::new("de17-not-attached");
    let lib = LibraryId::new("7b6a5c4d-3e2f-4a1b-8c9d-0e1f2a3b4c5d").expect("id");
    let trips = CollectionId::new("c-1").expect("id");
    let request = RegisterFile {
        file_id: new_file(),
        collection_id: trips,
        path: "a.jpg".into(),
    };
    let refused = fx
        .catalog
        .register_file(&lib, &request, registration_entry("op"))
        .unwrap_err();
    assert!(matches!(refused, CatalogError::Failed(_)));
}

#[test]
fn removing_a_collection_that_still_holds_files_is_refused_before_anything_is_published() {
    let fx = Fixture::new("de17-unpaired-removal");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    path_of(register(&fx, &lib, &trips, &new_file(), "a.jpg", "op-1"));
    let head = snapshot_of(&fx, &lib).version;
    let removal = |revision: String| CatalogCommit {
        operation_id: fx.op(),
        library_id: lib.clone(),
        expected_revision: ExpectedRevision::new(revision).expect("revision"),
        changes: vec![CatalogChange::RemoveCollection(trips.clone())],
        journal: JournalEntry::new("library.collection.delete", Outcome::Succeeded),
        created_id: None,
        deletion: None,
        restored_into: Vec::new(),
    };
    let stale = fx.catalog.commit(removal("1".into())).expect_err("stale");
    assert_eq!(
        stale,
        CatalogError::Conflict,
        "the revision rule answers first"
    );
    let refused = fx
        .catalog
        .commit(removal(fx.revision(&lib)))
        .expect_err("refused");
    assert_eq!(
        refused,
        CatalogError::Failed(COLLECTION_HOLDS_FILES.to_string())
    );
    assert_eq!(
        snapshot_of(&fx, &lib).version,
        head,
        "nothing was published"
    );
    assert!(fx.catalog.collection(&trips).expect("read").is_some());
}

#[test]
fn a_file_s_path_is_taken_below_it_and_no_file_sits_below_a_file() {
    let fx = Fixture::new("de17-file-prefix");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    path_of(register(&fx, &lib, &trips, &new_file(), "notes", "op-1"));
    path_of(register(
        &fx,
        &lib,
        &trips,
        &new_file(),
        "beach/a.jpg",
        "op-2",
    ));
    let taken = |path: &str| fx.catalog.path_taken(&trips, path).expect("read");
    assert!(taken("notes"));
    assert!(taken("notes/inside.txt"), "a file is not a folder");
    assert!(taken("notes/deeper/inside.txt"));
    assert!(taken("beach"), "a folder of a file");
    assert!(!taken("notes-2"));
    assert!(!taken("beach-1"));
}

#[test]
fn a_file_of_the_earlier_form_is_placed_when_it_is_registered() {
    let fx = Fixture::new("de17-earlier-form");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    let unplaced = File::new(file.clone(), lib.clone(), trips.clone());
    fx.catalog
        .index_files(&lib, &[(unplaced, 1)], registration_entry("op-index"))
        .expect("index without a path");
    let placed = register(&fx, &lib, &trips, &file, "a.jpg", "op-1");
    assert_eq!(
        placed,
        Registration::Registered {
            path: "a.jpg".into()
        }
    );
    let again = register(&fx, &lib, &trips, &file, "a.jpg", "op-2");
    assert_eq!(
        again,
        Registration::AlreadyRegistered {
            path: "a.jpg".into()
        }
    );
}

#[test]
fn a_refused_registration_is_journaled_as_failed_with_its_code() {
    let fx = Fixture::new("de17-refused-journal");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let long = format!("{}.jpg", "n".repeat(251));
    path_of(register(&fx, &lib, &trips, &new_file(), &long, "op-1"));
    let refused = register(&fx, &lib, &trips, &new_file(), &long, "op-2");
    assert_eq!(refused, Registration::Refused("name_too_long"));
    let failed: Vec<JournalEntry> = all_entries(fx.journal.as_ref())
        .into_iter()
        .filter(|e| e.operation == "library.files.register" && e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(failed[0].error_code.as_deref(), Some("name_too_long"));
    assert_eq!(failed[0].operation_id.as_deref(), Some("op-2"));
    assert!(!format!("{failed:?}").contains("nnnn"), "no name journaled");
}

#[test]
fn an_empty_folder_attached_with_an_index_of_another_folder_starts_afresh() {
    let fx = Fixture::new("de17-stale-index");
    let lib = fx.create_library("Family archive");
    fx.create_collection(&lib, "Trips");
    let root = fx.release();
    std::fs::remove_dir_all(folders_dir(&root).join(lib.as_str())).expect("lose the folder");
    let fx = Fixture::over(
        root,
        std::sync::Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
    );
    let empty = fx.root.with_extension("empty");
    let _ = std::fs::remove_dir_all(&empty);
    std::fs::create_dir_all(&empty).expect("empty folder");
    let report = fx.attach(&lib, &empty).expect("attach");
    assert!(report.created, "the folder's own library entity");
    let collections = fx.catalog.collections(&lib, None, 10).expect("read");
    assert!(
        collections.is_empty(),
        "nothing of the other folder: {collections:?}"
    );
    let _ = std::fs::remove_dir_all(&empty);
}

#[test]
fn a_folder_that_cannot_be_a_library_leaves_nothing_in_the_catalog_directory() {
    let fx = Fixture::new("de17-unusable-folder");
    let folder = fx.root.with_extension("unusable");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("holiday.jpg"), b"not a library").expect("a file");
    let lib = LibraryId::new(filesquid_api::library::mint_uuid_v4().expect("uuid")).expect("id");
    assert!(
        fx.attach(&lib, &folder).is_err(),
        "not an OCFL storage root"
    );
    let left: Vec<String> = catalog_names(&fx.root)
        .into_iter()
        .filter(|name| name.starts_with(lib.as_str()))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn no_index_is_created_beyond_the_catalog_s_limit_of_files() {
    let fx = Fixture::new("de17-index-limit");
    for _ in 0..filesquid_native::MAX_LIBRARIES {
        let id = filesquid_api::library::mint_uuid_v4().expect("uuid");
        std::fs::write(fx.root.join(format!("{id}.sqlite")), b"").expect("an index file");
    }
    let folder = fx.root.with_extension("one-more");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    let lib = LibraryId::new(filesquid_api::library::mint_uuid_v4().expect("uuid")).expect("id");
    let refused = fx.attach(&lib, &folder).expect_err("refused");
    assert!(matches!(refused, CatalogError::Failed(_)), "{refused:?}");
    assert!(!catalog_file(&fx.root, &lib).exists(), "no new index");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn files_deleted_with_their_collection_are_deleted_not_unfiled() {
    let fx = Fixture::new("de17-not-unfiled");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "a.jpg", "op-1"));
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteCollection {
            library_id: lib.clone(),
            collection_id: trips,
        },
    );
    let listed = |scope: FileScope, visibility: VisibilityFilter| -> Vec<FileId> {
        let listing = FileListing {
            library_id: lib.clone(),
            scope,
            visibility,
        };
        fx.catalog
            .files(&listing, None, 10)
            .expect("read")
            .into_iter()
            .map(|keyed| keyed.item.file.id().clone())
            .collect()
    };
    assert!(
        listed(FileScope::Library, VisibilityFilter::Ordinary).is_empty(),
        "no file is ever unfiled"
    );
    assert_eq!(
        listed(FileScope::Library, VisibilityFilter::Deleted),
        [file]
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
fn a_persistent_registry_restores_names_locations_and_free_bytes_after_a_restart() {
    use filesquid_api::local_library::{FreeSpace, LibraryRegistry};
    use filesquid_native::registry_sqlite::SqliteLibraryRegistry;
    use filesquid_native::HostLocations;
    let dir = std::env::temp_dir().join(format!("fs-native-registry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (data, folder) = (dir.join("data"), dir.join("Family"));
    std::fs::create_dir_all(&data).expect("data");
    std::fs::create_dir_all(&folder).expect("folder");
    let store = || SqliteLibraryRegistry::open(&data.join("filesquid.sqlite")).expect("store");
    let library = LibraryId::new("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b").expect("library");
    let first = HostLocations::persistent(&data, store()).expect("registry");
    let location = first.register(&folder).expect("location");
    first.remember(&library, &location, Some("Family archive"), 1_000);
    assert_eq!(first.rename(&library, "Family photos"), Some(true));
    drop(first);
    let second = HostLocations::persistent(&data, store()).expect("restored");
    let entry = second.entry(&library).expect("remembered across launches");
    assert_eq!(entry.name, "Family photos");
    assert_eq!(entry.location_id, location, "the same opaque location");
    assert_eq!(entry.last_opened_ms, 1_000);
    assert!(
        second.root_of(&location).is_some(),
        "its folder registered again"
    );
    let free = second.free_space(&location);
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(matches!(free, FreeSpace::Bytes(_)), "{free:?}");
    } else {
        assert_eq!(free, FreeSpace::VolumeUnreadable);
    }
    std::fs::remove_dir_all(&folder).expect("folder gone");
    drop(second);
    let third = HostLocations::persistent(&data, store()).expect("restored");
    assert!(
        third.entry(&library).is_some(),
        "kept although its folder is gone"
    );
    assert_eq!(third.free_space(&location), FreeSpace::VolumeUnreadable);
    drop(third);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_relocated_file_s_own_path_never_takes_its_target_at_it_below_it_or_above_it() {
    use filesquid_api::home::{place_entry, HomeCollections, HomeRequest, Placed};
    let fx = Fixture::new("api-f214-own-path");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let home: &dyn HomeCollections = fx.catalog.as_ref();
    let numbered = new_file();
    path_of(register(
        &fx,
        &lib,
        &trips,
        &new_file(),
        "a/photo.jpg",
        "op-1",
    ));
    path_of(register(
        &fx,
        &lib,
        &trips,
        &numbered,
        "a/photo-1.jpg",
        "op-2",
    ));
    let to = |file: &FileId, path: &str| HomeRequest {
        file_id: file.clone(),
        collection_id: trips.clone(),
        path: path.into(),
    };
    let back = home
        .relocate(&lib, &to(&numbered, "b/photo.jpg"), place_entry("op-3"))
        .expect("away");
    assert_eq!(
        back,
        Placed::Registered {
            path: "b/photo.jpg".into()
        }
    );
    let notes = new_file();
    path_of(register(&fx, &lib, &trips, &notes, "notes", "op-4"));
    let below = home
        .relocate(&lib, &to(&notes, "notes/notes"), place_entry("op-5"))
        .expect("below its own path");
    assert_eq!(
        below,
        Placed::Registered {
            path: "notes/notes".into()
        },
        "the file's own path is no ancestor of its target"
    );
    let above = home
        .relocate(&lib, &to(&notes, "notes"), place_entry("op-6"))
        .expect("above its own path");
    assert_eq!(
        above,
        Placed::Registered {
            path: "notes".into()
        },
        "the file's own path is no descendant of its target"
    );
}

#[test]
fn a_relocation_into_a_removed_collection_is_collection_not_found_even_at_its_own_path() {
    use filesquid_api::home::{place_entry, HomeCollections, HomeRequest, Placed};
    let fx = Fixture::new("api-f214-removed-home");
    let lib = fx.create_library("Family archive");
    let trips = fx.create_collection(&lib, "Trips");
    let file = new_file();
    path_of(register(&fx, &lib, &trips, &file, "a.jpg", "op-1"));
    let deletion = filesquid_api::library::Deletion {
        deleted_at_ms: 1_000,
        purge_after_ms: 2_000,
        cause: Default::default(),
    };
    let removal = CatalogCommit {
        operation_id: fx.op(),
        library_id: lib.clone(),
        expected_revision: ExpectedRevision::new(fx.revision(&lib)).expect("revision"),
        changes: vec![
            CatalogChange::DeleteCollectionFiles {
                collection_id: trips.clone(),
                deletion,
                title: "Trips".into(),
            },
            CatalogChange::RemoveCollection(trips.clone()),
        ],
        journal: JournalEntry::new("library.collection.delete", Outcome::Succeeded),
        created_id: None,
        deletion: Some(deletion),
        restored_into: Vec::new(),
    };
    fx.catalog.commit(removal).expect("removed");
    let home: &dyn HomeCollections = fx.catalog.as_ref();
    let request = HomeRequest {
        file_id: file,
        collection_id: trips,
        path: "a.jpg".into(),
    };
    let refused = home
        .relocate(&lib, &request, place_entry("op-2"))
        .expect("refused");
    assert_eq!(refused, Placed::Refused("collection_not_found"));
}
