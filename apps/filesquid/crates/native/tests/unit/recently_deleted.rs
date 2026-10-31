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

// ---- Expiry and the tombstone (ADR 041 Decision 11) ----

use filesquid_api::canonical::tombstone::{
    is_tombstone, TombstoneRecord, FORMER_PROVENANCE, TOMBSTONE_RECORD,
};
use filesquid_api::library::{
    ExpiryMode, InstallationSettings, MemorySettings, ObjectPurger, RetentionSettings,
    EXPIRY_OPERATION,
};
use filesquid_native::catalog::PurgeStep;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::StorageRoot;

/// Drive a future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("ready")
}

/// Write the file object of `file` in `library`'s folder: two versions of an original, a
/// metadata record and a PROV-O record, as an import would leave it.
fn write_file_object(fx: &Fixture, library: &LibraryId, file: &FileId) {
    let location = fx.locations.register(&fx.folder_of(library)).expect("register");
    let root = fx.locations.root_of(&location).expect("root");
    let staging = FsBackend::new(fx.root.join("obj-staging"), fx.root.join("obj-parts"))
        .expect("staging");
    let area = StagingArea::new(&staging, "object").expect("area");
    let object_root = format!("files/{}", file.as_str());
    let model = ChangeModel::new(
        root.backend.as_ref(),
        root.reservations.as_ref(),
        "fixture",
        object_root.as_str(),
        format!("urn:uuid:{}", file.as_str()),
        area,
    );
    let original = format!("original/{}.txt", file.as_str());
    for (n, text) in [(1, "first draft"), (2, "second draft")] {
        ready(model.stage_bytes(&original, text.as_bytes())).expect("stage");
        ready(model.stage_bytes(
            &format!("original/{}.metadata.md", file.as_str()),
            format!("record {n}").as_bytes(),
        ))
        .expect("stage");
        ready(model.stage_bytes(&format!("provenance/v{n}.jsonld"), b"{\"@context\":{}}"))
            .expect("stage");
        let meta = VersionMeta::new(format!("2026-10-0{n}T00:00:00Z")).with_message("import");
        ready(model.commit(meta)).expect("commit");
    }
}

/// A library with one registered file that has an object, deleted at the fixture's time.
fn deleted_file(name: &str) -> (Fixture, LibraryId, FileId) {
    let fx = Fixture::new(name);
    let lib = fx.create_library("Family archive");
    let docs = fx.create_collection(&lib, "Documents");
    let file = new_file();
    write_file_object(&fx, &lib, &file);
    register(&fx, &lib, &docs, &file, "notes.txt");
    fx.run(
        &fx.revision(&lib),
        LibraryCommand::DeleteFiles {
            library_id: lib.clone(),
            file_ids: vec![file.clone()],
        },
    );
    (fx, lib, file)
}

/// The service of `fx` 31 days from now under `expiry`.
fn later_service(fx: &Fixture, expiry: ExpiryMode) -> filesquid_api::library::LibraryService {
    let settings = Arc::new(MemorySettings::new());
    let stored = InstallationSettings {
        default_retention_days: 30,
        expiry,
    };
    settings
        .store(
            &stored,
            filesquid_api::journal::JournalEntry::new("x", filesquid_api::journal::Outcome::Succeeded),
        )
        .expect("store");
    let later = filesquid_api::journal::now_ms() + 31 * DAY_MS;
    fx.service
        .clone()
        .with_clock(Arc::new(move || later))
        .with_settings(settings)
}

/// The folder's backend.
fn folder_backend(fx: &Fixture, lib: &LibraryId) -> FsBackend {
    FsBackend::new(fx.folder_of(lib), fx.root.join("check-parts")).expect("fs")
}

/// The logical paths of the tombstone object of `file` at its head.
fn tombstone_paths(fx: &Fixture, lib: &LibraryId, file: &FileId) -> Vec<String> {
    let backend = folder_backend(fx, lib);
    let root = ready(StorageRoot::open(&backend)).expect("root");
    let object = ready(root.open_object(&format!("files/{}", file.as_str()))).expect("object");
    assert_eq!(object.head().to_string(), "v1", "its own first version is the tombstone");
    let state = object.state(object.head()).expect("state");
    state.paths().map(str::to_string).collect()
}

/// Assert `file`'s object is a complete tombstone keeping both former versions.
fn assert_tombstone(fx: &Fixture, lib: &LibraryId, file: &FileId) {
    let backend = folder_backend(fx, lib);
    let object_root = format!("files/{}", file.as_str());
    assert!(complete_immediately(is_tombstone(&backend, &object_root)).expect("ready"));
    let paths = tombstone_paths(fx, lib, file);
    assert!(paths.iter().all(|p| !p.starts_with("original/")), "no bytes or record: {paths:?}");
    assert!(paths.contains(&TOMBSTONE_RECORD.to_string()));
    assert!(paths.contains(&format!("{FORMER_PROVENANCE}/v2.jsonld")));
    let record_path = fx
        .folder_of(lib)
        .join(&object_root)
        .join("v1/content")
        .join(TOMBSTONE_RECORD);
    let record: TombstoneRecord =
        serde_json::from_slice(&std::fs::read(record_path).expect("record")).expect("json");
    assert_eq!(record.versions.len(), 2, "every former version's facts");
    let original = record.versions[1]
        .files
        .iter()
        .find(|f| f.path.ends_with(".txt"))
        .expect("the original's facts");
    assert_eq!(original.size, Some(12), "its size, without its bytes");
    let kinds: Vec<&str> = record.events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["purge", "tombstone"]);
    assert!(record.events.iter().all(|e| e.operation_id == record.purge.operation_id));
    assert!(record.events[0].activity.starts_with("urn:filesquid:activity:"));
    assert!(!record.events[0].activity.ends_with(":unknown"), "the purge's snapshot version");
    let bytes = std::fs::read_dir(fx.folder_of(lib).join(&object_root).join("v1/content/tombstone"))
        .expect("content")
        .count();
    assert!(bytes > 0);
}

#[test]
fn keep_version_history_rewrites_the_object_as_a_tombstone_and_journals_the_purge() {
    let (fx, lib, file) = deleted_file("f215-tombstone");
    let report = later_service(&fx, ExpiryMode::KeepVersionHistory)
        .expire_library("system", &lib, fx.catalog.as_ref())
        .expect("expire");
    assert_eq!((report.expired, report.objects_done, report.objects_failed), (1, 1, 0));
    assert!(fx.catalog.file(&file).expect("read").is_none(), "gone from the index");
    assert!(snapshot_of(&fx, &lib).files.iter().all(|f| f.file.id() != &file));
    assert_tombstone(&fx, &lib, &file);
    assert!(fx.catalog.pending_purges(&lib, 10).expect("read").is_empty());
    let entries = catalog_fixture::all_entries(fx.journal.as_ref());
    let purge = entries
        .iter()
        .find(|e| e.operation == EXPIRY_OPERATION)
        .expect("the purge is journaled");
    let tombstone = entries
        .iter()
        .find(|e| e.operation == "file.tombstone")
        .expect("the tombstone is journaled");
    assert_eq!(purge.operation_id, tombstone.operation_id, "one purge operation ID");
    for entry in &entries {
        let text = serde_json::to_string(entry).expect("json");
        assert!(!text.contains("notes.txt") && !text.contains("Documents"), "{text}");
    }
    let fx = rebuilt(fx, &lib);
    assert!(fx.catalog.file(&file).expect("read").is_none(), "recovered as history, not a file");
}

#[test]
fn delete_all_version_history_deletes_the_object() {
    let (fx, lib, file) = deleted_file("f215-delete-all");
    later_service(&fx, ExpiryMode::DeleteAllVersionHistory)
        .expire_library("system", &lib, fx.catalog.as_ref())
        .expect("expire");
    let object = fx.folder_of(&lib).join("files").join(file.as_str());
    assert!(!object.exists(), "the object is deleted for good");
    assert!(fx.catalog.file(&file).expect("read").is_none());
}

#[test]
fn a_crash_at_each_step_of_the_tombstone_write_is_completed_by_the_next_run() {
    for (n, step) in [
        PurgeStep::PayloadSaved,
        PurgeStep::OriginalDeleted,
        PurgeStep::TombstoneCreated,
    ]
    .into_iter()
    .enumerate()
    {
        let (fx, lib, file) = deleted_file(&format!("f215-crash-{n}"));
        let service = later_service(&fx, ExpiryMode::KeepVersionHistory);
        fx.catalog.fail_purges_at(Some(step));
        let first = service
            .expire_library("system", &lib, fx.catalog.as_ref())
            .expect("expire");
        assert_eq!(first.objects_failed, 1, "{step:?}");
        assert_eq!(fx.catalog.pending_purges(&lib, 10).expect("read").len(), 1, "kept: {step:?}");
        fx.catalog.fail_purges_at(None);
        let second = service
            .expire_library("system", &lib, fx.catalog.as_ref())
            .expect("expire");
        assert_eq!(second.objects_done, 1, "{step:?}");
        assert_tombstone(&fx, &lib, &file);
        assert!(fx.catalog.pending_purges(&lib, 10).expect("read").is_empty());
    }
}

#[test]
fn a_deletion_stopped_part_way_is_finished_by_the_next_run() {
    let (fx, lib, file) = deleted_file("f215-partial");
    let service = later_service(&fx, ExpiryMode::KeepVersionHistory);
    fx.catalog.fail_purges_at(Some(PurgeStep::PayloadSaved));
    service
        .expire_library("system", &lib, fx.catalog.as_ref())
        .expect("expire");
    let v2 = fx.folder_of(&lib).join("files").join(file.as_str()).join("v2");
    std::fs::remove_dir_all(&v2).expect("half-delete the original");
    fx.catalog.fail_purges_at(None);
    let done = service
        .expire_library("system", &lib, fx.catalog.as_ref())
        .expect("expire");
    assert_eq!(done.objects_done, 1);
    assert_tombstone(&fx, &lib, &file);
}

#[test]
fn the_expiry_runs_as_a_durable_job_over_every_open_library() {
    use filesquid_api::jobs::{JobState, Jobs, MemoryJobStore};
    use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
    use filesquid_native::workers::expiry::run_expiry_job;
    let (fx, lib, file) = deleted_file("f215-job");
    let monitor = Monitor::start(
        Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        filesquid_api::EventBus::new(),
    );
    let jobs = Jobs::new(
        Arc::new(MemoryJobStore::new()),
        monitor.journal(),
        monitor.registry().clone(),
        "test-launch",
    );
    let service = later_service(&fx, ExpiryMode::KeepVersionHistory);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let (state, report) =
        run_expiry_job(&jobs, &service, &fx.catalog, "installation:test", &stop).expect("run");
    assert_eq!(state, JobState::Completed);
    assert_eq!((report.libraries, report.expired, report.objects_done), (1, 1, 1));
    assert_tombstone(&fx, &lib, &file);
    let (_, again) =
        run_expiry_job(&jobs, &service, &fx.catalog, "installation:test", &stop).expect("rerun");
    assert_eq!(again.expired, 0, "a second run finds nothing due");
    drop(monitor);
}
