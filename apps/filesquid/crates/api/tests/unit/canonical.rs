//! Task DE-0.3: the FileSquid OCFL application profile (`filesquid_api::canonical`): its
//! layout, entity encoding and operation message, and snapshot commits and reads through
//! the change model on the in-memory storage backend.

use filesquid_api::canonical::original::original_path_for;
use filesquid_api::canonical::{
    commit_file_original, commit_snapshot, create_storage_root, file_object_id, file_object_root,
    new_upload_record, operation_message, parse_operation_message, read_file_original,
    read_snapshot, reupload_record, snapshot_object_id, touched, CanonicalError, FileOriginal,
    LibraryHead, OperationRecord, OwnerReadable, SnapshotDelta, SnapshotEntity, SnapshotFile,
    SnapshotKey, UploadInput, VirtualEntry, SNAPSHOT_ROOT,
};
use filesquid_api::library::{CatalogChange, CatalogError, FileState, LibraryRecord};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId, VirtualMembership,
};
use filesquid_metadata::file_record::{extension_of, FileRecord};
use filesquid_metadata::import::NameForm;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{complete_immediately, MemoryBackend};
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};

const LIB: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";
const CREATED: &str = "2026-10-19T00:00:00.000Z";
const FILE: &str = "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80";

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("memory futures complete at once")
}

/// A library storage root, a staging backend outside it and a lease store.
struct Rig {
    root: MemoryBackend,
    staging: MemoryBackend,
    leases: LeaseStore<MemoryBackend, SystemClock>,
}

impl Rig {
    fn new() -> Self {
        let root = MemoryBackend::new();
        ready(create_storage_root(&root)).expect("root");
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            SystemClock,
            LeaseConfig::default(),
        )
        .expect("leases");
        Self {
            root,
            staging: MemoryBackend::new(),
            leases,
        }
    }

    /// A change model on the object at `object_root`.
    fn model(
        &self,
        object_root: &str,
        object_id: &str,
    ) -> ChangeModel<'_, MemoryBackend, LeaseStore<MemoryBackend, SystemClock>, MemoryBackend> {
        let staging =
            StagingArea::new(&self.staging, &format!("staging/{object_root}")).expect("staging");
        ChangeModel::new(
            &self.root,
            &self.leases,
            "desktop",
            object_root,
            object_id,
            staging,
        )
    }
}

fn library_id() -> LibraryId {
    LibraryId::new(LIB).unwrap()
}

fn head(revision: u64) -> SnapshotEntity {
    SnapshotEntity::Library(LibraryHead {
        record: LibraryRecord {
            library: Library::new(library_id(), RepositoryId::new("repo-1").unwrap()),
            name: "Family archive".into(),
        },
        revision,
        created_order: 1,
    })
}

fn collection(id: &str) -> Collection {
    Collection::new(CollectionId::new(id).unwrap(), library_id(), "Summer")
        .unwrap()
        .with_order_key(2)
}

fn snapshot_file(id: &str, home: &str) -> SnapshotFile {
    SnapshotFile {
        file: File::new(
            FileId::new(id).unwrap(),
            library_id(),
            CollectionId::new(home).unwrap(),
        ),
        order: 5,
        state: FileState {
            hidden: true,
            deletion: None,
        },
    }
}

fn virtual_entry(id: &str, members: &[&str]) -> VirtualEntry {
    let vc = VirtualCollectionId::new(id).unwrap();
    VirtualEntry {
        collection: VirtualCollection::new(vc.clone(), library_id(), "Best").unwrap(),
        members: members
            .iter()
            .enumerate()
            .map(|(i, f)| VirtualMembership::new(vc.clone(), FileId::new(*f).unwrap(), i as i64))
            .collect(),
    }
}

fn meta(record: &OperationRecord) -> VersionMeta {
    VersionMeta::new(CREATED).with_message(operation_message(record))
}

fn record(op: &str, revision: &str) -> OperationRecord {
    OperationRecord {
        operation_id: op.into(),
        kind: None,
        revision: revision.into(),
        created_id: None,
        deletion: None,
    }
}

#[test]
fn every_entity_has_one_logical_path_and_back() {
    let keys = [
        SnapshotKey::Library,
        SnapshotKey::Collection(CollectionId::new("c-1").unwrap()),
        SnapshotKey::VirtualCollection(VirtualCollectionId::new("vc-1").unwrap()),
        SnapshotKey::Folder(FolderId::new("d-1").unwrap()),
        SnapshotKey::File(FileId::new("f-1").unwrap()),
    ];
    let paths: Vec<String> = keys.iter().map(|k| k.path().unwrap()).collect();
    assert_eq!(
        paths,
        [
            "library.json",
            "collections/c-1.json",
            "virtual-collections/vc-1.json",
            "folders/d-1.json",
            "files/f-1.json"
        ]
    );
    for (key, path) in keys.iter().zip(&paths) {
        assert_eq!(SnapshotKey::from_path(path).as_ref(), Some(key));
    }
    assert_eq!(SnapshotKey::from_path("collections/../x.json"), None);
    assert_eq!(SnapshotKey::from_path("other/x.json"), None);
}

#[test]
fn identifiers_that_cannot_name_a_path_segment_are_refused() {
    for bad in ["a/b", "..", ".", "a b", "é"] {
        let key = SnapshotKey::File(FileId::new(bad).unwrap());
        assert!(
            matches!(key.path(), Err(CanonicalError::InvalidIdentifier(_))),
            "{bad}"
        );
    }
    assert_eq!(
        file_object_root(&FileId::new("f-1").unwrap()).unwrap(),
        "files/f-1"
    );
    assert_eq!(file_object_id(&FileId::new("f-1").unwrap()), "urn:uuid:f-1");
    assert_eq!(snapshot_object_id(&library_id()), format!("urn:uuid:{LIB}"));
}

#[test]
fn each_change_touches_the_library_and_its_own_entity() {
    let vc = VirtualCollectionId::new("vc-1").unwrap();
    let change = CatalogChange::AddVirtualMember(VirtualMembership::new(
        vc.clone(),
        FileId::new("f").unwrap(),
        0,
    ));
    assert_eq!(
        touched(&change),
        vec![SnapshotKey::Library, SnapshotKey::VirtualCollection(vc)]
    );
    let create = CatalogChange::CreateLibrary(LibraryRecord {
        library: Library::new(library_id(), RepositoryId::new("repo-1").unwrap()),
        name: "Family archive".into(),
    });
    assert_eq!(touched(&create), vec![SnapshotKey::Library]);
    let purge = CatalogChange::PurgeFile(FileId::new("f").unwrap());
    assert_eq!(
        touched(&purge),
        vec![
            SnapshotKey::Library,
            SnapshotKey::File(FileId::new("f").unwrap())
        ]
    );
}

#[test]
fn the_operation_message_carries_its_receipt() {
    let first = OperationRecord {
        created_id: Some("c-1".into()),
        ..record("op-1", "2")
    };
    assert_eq!(
        parse_operation_message(&operation_message(&first)),
        Ok(Some(first))
    );
    assert_eq!(parse_operation_message("imported by hand"), Ok(None));
    assert!(parse_operation_message("filesquid.operation.v1 {broken").is_err());
}

#[test]
fn a_committed_snapshot_reads_back_entity_for_entity() {
    let rig = Rig::new();
    assert_eq!(
        ready(read_snapshot(&rig.root, &OwnerReadable)).unwrap(),
        None
    );
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut delta = SnapshotDelta::default();
    for entity in [
        head(2),
        SnapshotEntity::Collection(collection("c-1")),
        SnapshotEntity::VirtualCollection(virtual_entry("vc-1", &["f-2", "f-1"])),
        SnapshotEntity::Folder(
            Folder::new(FolderId::new("d-1").unwrap(), library_id(), "Years").unwrap(),
        ),
        SnapshotEntity::File(snapshot_file("f-1", "c-1")),
    ] {
        delta.put(&entity).unwrap();
    }
    let receipt = ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &delta,
        meta(&record("op-1", "2")),
    ))
    .unwrap()
    .expect("a version");
    assert_eq!(receipt.object_id, snapshot_object_id(&library_id()));
    let snapshot = ready(read_snapshot(&rig.root, &OwnerReadable))
        .unwrap()
        .expect("snapshot");
    assert_eq!(snapshot.library.revision, 2);
    assert_eq!(snapshot.collections, vec![collection("c-1")]);
    assert_eq!(
        snapshot.virtual_collections,
        vec![virtual_entry("vc-1", &["f-2", "f-1"])]
    );
    assert_eq!(snapshot.files, vec![snapshot_file("f-1", "c-1")]);
    assert_eq!(snapshot.folders.len(), 1);
    assert_eq!(snapshot.operations, vec![record("op-1", "2")]);
}

#[test]
fn a_later_version_changes_only_the_entities_it_touches() {
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut first = SnapshotDelta::default();
    first.put(&head(1)).unwrap();
    first
        .put(&SnapshotEntity::Collection(collection("c-1")))
        .unwrap();
    first
        .put(&SnapshotEntity::Collection(collection("c-2")))
        .unwrap();
    ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &first,
        meta(&record("op-1", "1")),
    ))
    .unwrap();
    let unchanged = ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &first,
        meta(&record("op-same", "1")),
    ))
    .unwrap();
    assert_eq!(
        unchanged, None,
        "a delta that changes nothing writes no version"
    );
    let mut second = SnapshotDelta::default();
    second.put(&head(2)).unwrap();
    second
        .remove(&SnapshotKey::Collection(CollectionId::new("c-1").unwrap()))
        .unwrap();
    second
        .remove(&SnapshotKey::Folder(FolderId::new("never").unwrap()))
        .unwrap();
    ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &second,
        meta(&record("op-2", "2")),
    ))
    .unwrap();
    let object = ready(Object::open(&rig.root, SNAPSHOT_ROOT)).unwrap();
    let v1 = filesquid_ocfl::VersionNum::parse("v1").unwrap();
    let changes = object.diff(&v1, object.head()).unwrap();
    assert_eq!(changes.len(), 2, "{changes:?}");
    let snapshot = ready(read_snapshot(&rig.root, &OwnerReadable))
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.collections, vec![collection("c-2")]);
    assert_eq!(snapshot.operations.len(), 2);
}

#[test]
fn a_stale_draft_left_in_staging_is_never_published() {
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    ready(model.stage_bytes("collections/leftover.json", b"crash leftovers")).unwrap();
    let mut delta = SnapshotDelta::default();
    delta.put(&head(1)).unwrap();
    ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &delta,
        meta(&record("op-1", "1")),
    ))
    .unwrap();
    let snapshot = ready(read_snapshot(&rig.root, &OwnerReadable))
        .unwrap()
        .unwrap();
    assert!(snapshot.collections.is_empty());
}

#[test]
fn an_entity_that_does_not_match_its_path_is_refused_on_read() {
    let bytes =
        filesquid_api::canonical::entity::encode(&SnapshotEntity::Collection(collection("c-1")))
            .unwrap();
    let refused = filesquid_api::canonical::entity::decode("collections/c-2.json", &bytes);
    assert!(matches!(refused, Err(CanonicalError::Decode { .. })));
    let other_schema = br#"{"schema":2,"entity":{"kind":"folder"}}"#;
    assert!(filesquid_api::canonical::entity::decode("folders/x.json", other_schema).is_err());
}

/// The upload facts of `name` with `media_type`, declaring `size` bytes.
fn upload<'a>(name: &'a str, media_type: &'a str, size: u64, op: &'a str) -> UploadInput<'a> {
    UploadInput {
        name,
        name_normalization: NameForm::Nfc,
        declared_size: size,
        media_type,
        last_modified: Some("2026-10-18T09:30:00Z"),
        operation_id: Some(op),
        started: "2026-10-19T00:00:00Z",
        finished: "2026-10-19T00:00:01Z",
        object_version: "v1",
    }
}

/// The record of the first upload of `bytes` as `original_name`.
fn first_record(
    file: &FileId,
    original_name: &str,
    media_type: &str,
    bytes: &[u8],
) -> Result<FileRecord, CanonicalError> {
    let input = upload(original_name, media_type, bytes.len() as u64, "op-9");
    new_upload_record(file, &input, bytes)
}

/// Commit `record` and `bytes` as the original of `file` over `previous`.
fn commit_record(
    rig: &Rig,
    file: &FileId,
    record: &FileRecord,
    previous: Option<&FileRecord>,
    bytes: &[u8],
    op: &str,
) -> Result<Option<filesquid_ocfl::write::CommitReceipt>, CanonicalError> {
    let root = file_object_root(file).unwrap();
    let model = rig.model(&root, &file_object_id(file));
    ready(commit_file_original(
        &model,
        &OwnerReadable,
        file,
        record,
        previous,
        bytes,
        meta(&record_of(op)),
    ))
}

/// Commit the first upload of `bytes` as the original of `file`, named `original_name`.
fn commit_original(
    rig: &Rig,
    file: &FileId,
    original_name: &str,
    media_type: &str,
    op: &str,
) -> Result<Option<filesquid_ocfl::write::CommitReceipt>, CanonicalError> {
    let record = first_record(file, original_name, media_type, JPEG)?;
    commit_record(rig, file, &record, None, JPEG, op)
}

/// The bytes the original tests store.
const JPEG: &[u8] = b"jpeg bytes";

/// The operation record of `op`.
fn record_of(op: &str) -> OperationRecord {
    record(op, "1")
}

/// The logical paths at the head of `file`'s object.
fn head_paths(rig: &Rig, file: &FileId) -> Vec<String> {
    let object = ready(Object::open(&rig.root, &file_object_root(file).unwrap())).unwrap();
    let state = object.state(object.head()).unwrap();
    state.paths().map(str::to_string).collect()
}

/// The original of `file` at its head.
fn read_original(rig: &Rig, file: &FileId) -> FileOriginal {
    ready(read_file_original(&rig.root, &OwnerReadable, file))
        .unwrap()
        .expect("original")
}

#[test]
fn a_file_original_is_its_own_object_with_its_metadata_record() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let receipt = commit_original(&rig, &file, "IMG_0001.JPG", "image/jpeg", "op-9")
        .unwrap()
        .expect("a new version");
    assert_eq!(receipt.object_id, format!("urn:uuid:{FILE}"));
    assert_eq!(
        head_paths(&rig, &file),
        vec![
            format!("original/{FILE}.JPG"),
            format!("original/{FILE}.metadata.md")
        ]
    );
    let read = read_original(&rig, &file);
    assert_eq!(read.path, format!("original/{FILE}.JPG"));
    assert_eq!(read.record.original_name().as_text(), Some("IMG_0001.JPG"));
    assert_eq!(read.record.display_name(), "IMG_0001.JPG");
    assert_eq!(read.record.extension(), Some("JPG"));
    assert_eq!(read.record.media_type(), Some("image/jpeg"));
    assert_eq!(read.record.imports().len(), 1);
    let again = commit_record(&rig, &file, &read.record, Some(&read.record), JPEG, "op-10");
    assert!(
        again.unwrap().is_none(),
        "the same original and record write no version"
    );
}

#[test]
fn an_owner_name_round_trips_through_its_metadata_record() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let original = "Café au lait — 写真 (1).JPEG";
    commit_original(&rig, &file, original, "image/jpeg", "op-9").unwrap();
    let read = read_original(&rig, &file);
    assert_eq!(read.path, format!("original/{FILE}.JPEG"));
    assert_eq!(read.record.original_name().as_text(), Some(original));
    assert!(head_paths(&rig, &file).iter().all(|p| !p.contains("Caf")));
}

#[test]
fn the_metadata_record_is_the_metadata_modules_file_record() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    commit_original(&rig, &file, "a.jpg", "image/jpeg", "op-9").unwrap();
    let read = read_original(&rig, &file);
    let object = ready(Object::open(&rig.root, &file_object_root(&file).unwrap())).unwrap();
    let record_path = format!("original/{FILE}.metadata.md");
    let stored = ready(filesquid_ocfl::view::read_range(
        &object,
        object.head(),
        &record_path,
        0,
        1 << 20,
    ))
    .unwrap();
    let text = String::from_utf8(stored).unwrap();
    assert_eq!(text, read.record.to_markdown(), "one codec writes it");
    assert_eq!(FileRecord::from_markdown(&text).unwrap(), read.record);
    let entry = &read.record.imports()[0].acquisition;
    assert_eq!(entry.object_id, format!("urn:uuid:{FILE}"));
    assert_eq!(entry.logical_path.as_deref(), Some(read.path.as_str()));
    assert_eq!(entry.size, JPEG.len() as u64);
}

#[test]
fn the_old_four_line_record_is_refused_on_read() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let old = "---\nschema: filesquid.original.v1\noriginal_name: \"a.jpg\"\nextension: \"jpg\"\nmedia_type: \"image/jpeg\"\n---\n";
    let model = rig.model(&file_object_root(&file).unwrap(), &file_object_id(&file));
    ready(model.stage_bytes(&format!("original/{FILE}.jpg"), JPEG)).unwrap();
    ready(model.stage_bytes(&format!("original/{FILE}.metadata.md"), old.as_bytes())).unwrap();
    ready(model.commit(meta(&record_of("op-12")))).unwrap();
    let refused = ready(read_file_original(&rig.root, &OwnerReadable, &file));
    assert!(matches!(refused, Err(CanonicalError::Decode { .. })));
}

#[test]
fn the_stored_name_is_the_file_identifier_and_its_extension_only() {
    let file = FileId::new(FILE).unwrap();
    let path = |n: &str| original_path_for(&file, extension_of(n)).unwrap();
    assert_eq!(path("写真.PNG"), format!("original/{FILE}.PNG"));
    assert_eq!(path("notes.tar.gz"), format!("original/{FILE}.gz"));
    assert_eq!(path("Holiday 2023.jpg"), format!("original/{FILE}.jpg"));
    let bare = format!("original/{FILE}");
    for no_extension in [
        "写真",
        ".bashrc",
        "...",
        "report.v2 draft",
        "photo.jp g",
        "a.abcdefghijklmnopq",
    ] {
        assert_eq!(path(no_extension), bare, "{no_extension:?}");
    }
    let stored = |ext| filesquid_api::canonical::original::stored_name(&file, Some(ext));
    for bad in ["metadata.md", "", "j pg", "abcdefghijklmnopq"] {
        assert!(
            matches!(stored(bad), Err(CanonicalError::InvalidIdentifier(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn a_long_extension_stays_in_the_record_but_not_in_the_stored_name() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    commit_original(&rig, &file, "scan.abcdefghijklmnopq", "image/tiff", "op-9").unwrap();
    let read = read_original(&rig, &file);
    assert_eq!(read.path, format!("original/{FILE}"));
    assert_eq!(read.record.extension(), Some("abcdefghijklmnopq"));
}

#[test]
fn an_original_name_or_media_type_that_is_not_valid_is_refused() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let long = "a".repeat(1025);
    for bad in ["", "a/b", "a\0b", long.as_str()] {
        let refused = commit_original(&rig, &file, bad, "image/jpeg", "op-10");
        assert!(matches!(refused, Err(CanonicalError::Encode(_))), "{bad:?}");
    }
    for bad in [
        "",
        "image",
        "image/",
        "/jpeg",
        "image/jp eg",
        "+image/jpeg",
        "image/.jpeg",
        "image/jpeg; q=1",
    ] {
        let refused = commit_original(&rig, &file, "a.jpg", bad, "op-11");
        assert!(matches!(refused, Err(CanonicalError::Encode(_))), "{bad:?}");
    }
    assert_eq!(
        ready(read_file_original(&rig.root, &OwnerReadable, &file)).unwrap(),
        None
    );
}

#[test]
fn an_upload_whose_declared_size_differs_builds_no_record() {
    let file = FileId::new(FILE).unwrap();
    let input = upload("a.jpg", "image/jpeg", JPEG.len() as u64 + 1, "op-9");
    let refused = new_upload_record(&file, &input, JPEG);
    assert!(matches!(refused, Err(CanonicalError::Encode(_))));
}

#[test]
fn a_record_that_does_not_cite_the_bytes_commits_nothing() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let record = first_record(&file, "a.jpg", "image/jpeg", JPEG).unwrap();
    let refused = commit_record(&rig, &file, &record, None, b"other bytes", "op-9");
    assert!(matches!(refused, Err(CanonicalError::Encode(_))));
    let other = FileId::new("8e2d3c0f-5a4b-4f72-9b1c-3d0e6f7a8b91").unwrap();
    let refused = commit_record(&rig, &other, &record, None, JPEG, "op-9");
    assert!(matches!(refused, Err(CanonicalError::Encode(_))));
    assert_eq!(
        ready(read_file_original(&rig.root, &OwnerReadable, &file)).unwrap(),
        None
    );
}

#[test]
fn the_head_record_committed_again_must_cite_the_bytes() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    commit_original(&rig, &file, "a.jpg", "image/jpeg", "op-9").unwrap();
    let head = read_original(&rig, &file).record;
    let refused = commit_record(&rig, &file, &head, Some(&head), b"other bytes", "op-10");
    assert!(matches!(refused, Err(CanonicalError::Encode(_))));
    assert_eq!(head_paths(&rig, &file).len(), 2);
    assert_eq!(read_original(&rig, &file).record, head);
}

#[test]
fn a_referenced_import_entry_cannot_describe_a_stored_original() {
    use filesquid_metadata::import::{ByteStorage, ImportEntry};
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let record = first_record(&file, "a.jpg", "image/jpeg", JPEG).unwrap();
    let mut entry: ImportEntry = record.imports()[0].clone();
    entry.acquisition.storage = ByteStorage::Referenced;
    entry.acquisition.logical_path = None;
    let referenced = FileRecord::new(
        record.original_name().clone(),
        NameForm::Nfc,
        record.extension().map(str::to_string),
        None,
        record.media_type().map(str::to_string),
        entry,
    )
    .unwrap();
    let refused = commit_record(&rig, &file, &referenced, None, JPEG, "op-9");
    assert!(matches!(refused, Err(CanonicalError::Encode(_))));
}

#[test]
fn a_new_upload_of_the_same_file_appends_one_import_entry() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    commit_original(&rig, &file, "a.jpg", "image/jpeg", "op-9").unwrap();
    let first = read_original(&rig, &file).record;
    let bytes = b"new jpeg bytes";
    let input = UploadInput {
        object_version: "v2",
        ..upload("a (2).jpg", "image/jpeg", bytes.len() as u64, "op-10")
    };
    let next = reupload_record(&file, &first, &input, bytes).unwrap();
    let receipt = commit_record(&rig, &file, &next, Some(&first), bytes, "op-10")
        .unwrap()
        .expect("a new version");
    assert_eq!(receipt.version.to_string(), "v2");
    let read = read_original(&rig, &file);
    assert_eq!(read.path, format!("original/{FILE}.jpg"));
    assert_eq!(read.record.imports().len(), 2);
    assert_eq!(read.record.display_name(), "a.jpg", "the name stays");
    let replaced = commit_record(&rig, &file, &first, Some(&next), JPEG, "op-11");
    assert!(
        matches!(replaced, Err(CanonicalError::Encode(_))),
        "an import entry is never removed"
    );
}

#[test]
fn a_storage_root_is_created_only_in_an_empty_folder() {
    let rig = Rig::new();
    let again = ready(create_storage_root(&rig.root));
    assert!(matches!(again, Err(CanonicalError::Failed(code)) if code == "root_not_empty"));
}

#[test]
fn canonical_failures_keep_their_meaning_at_the_catalog_port() {
    assert_eq!(
        CatalogError::from(CanonicalError::Conflict),
        CatalogError::Conflict
    );
    assert_eq!(
        CatalogError::from(CanonicalError::Unavailable("storage unavailable".into())),
        CatalogError::Unavailable("storage unavailable".into())
    );
    let refused = CatalogError::from(CanonicalError::HeadWithoutInventory);
    assert!(
        matches!(refused, CatalogError::Failed(detail) if detail.contains("head_without_inventory"))
    );
}

#[test]
fn a_head_without_its_version_inventory_is_refused_and_nothing_is_written() {
    use filesquid_ocfl::storage::{StorageBackend, StorageKey};
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut delta = SnapshotDelta::default();
    delta.put(&head(1)).unwrap();
    ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &delta,
        meta(&record("op-1", "1")),
    ))
    .unwrap();
    for name in [
        "inventory.json",
        "inventory.json.sha512",
        "inventory.json.sha256",
    ] {
        let key = StorageKey::new(&format!("{SNAPSHOT_ROOT}/v1/{name}")).unwrap();
        ready(rig.root.delete(&key)).unwrap();
    }
    let mut next = SnapshotDelta::default();
    next.put(&head(2)).unwrap();
    let refused = ready(commit_snapshot(
        &model,
        &OwnerReadable,
        &next,
        meta(&record("op-2", "2")),
    ));
    assert_eq!(refused, Err(CanonicalError::HeadWithoutInventory));
    let v2 = StorageKey::new(&format!("{SNAPSHOT_ROOT}/v2/inventory.json")).unwrap();
    assert!(
        !ready(rig.root.exists(&v2)).unwrap(),
        "nothing of v2 is written"
    );
    let v1 = StorageKey::new(&format!("{SNAPSHOT_ROOT}/v1/inventory.json")).unwrap();
    assert!(
        !ready(rig.root.exists(&v1)).unwrap(),
        "the head is never repaired here"
    );
}

#[test]
fn an_unreadable_operation_message_is_refused_on_read() {
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut delta = SnapshotDelta::default();
    delta.put(&head(1)).unwrap();
    let broken = VersionMeta::new(CREATED).with_message("filesquid.operation.v1 {broken");
    ready(commit_snapshot(&model, &OwnerReadable, &delta, broken)).unwrap();
    let refused = ready(read_snapshot(&rig.root, &OwnerReadable));
    assert!(
        matches!(refused, Err(CanonicalError::Decode { .. })),
        "{refused:?}"
    );
}

#[test]
fn an_entity_file_larger_than_the_bound_is_refused_before_it_is_read() {
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    ready(model.stage_bytes("library.json", &vec![b' '; (1 << 20) + 1])).unwrap();
    ready(model.commit(meta(&record("op-1", "1")))).unwrap();
    let refused = ready(read_snapshot(&rig.root, &OwnerReadable));
    assert!(
        matches!(refused, Err(CanonicalError::Decode { .. })),
        "{refused:?}"
    );
}

#[test]
fn an_entity_whose_identifier_cannot_name_a_path_is_a_decode_error() {
    let bad =
        br#"{"schema":1,"entity":{"kind":"folder","id":"a/b","library_id":"l","name":"Years"}}"#;
    let refused = filesquid_api::canonical::entity::decode("folders/a.json", bad);
    assert!(
        matches!(&refused, Err(CanonicalError::Decode { path, .. }) if path == "folders/a.json"),
        "{refused:?}"
    );
}

#[test]
fn a_recorded_kind_is_written_and_read_back_by_both_readers() {
    let kinded = OperationRecord {
        kind: Some("delete".into()),
        ..record("op-k", "3")
    };
    let text = operation_message(&kinded);
    assert_eq!(parse_operation_message(&text), Ok(Some(kinded)));
    let core = filesquid_core::OperationMessage::parse(&text)
        .expect("core reads the catalog message")
        .expect("a FileSquid message");
    assert_eq!(core.kind(), filesquid_core::OperationKind::Delete);
    assert_eq!(core.revision(), Some(3));
}

#[test]
fn catalog_operations_map_to_history_kinds() {
    use filesquid_api::canonical::operation_kind;
    use filesquid_core::OperationKind;
    let cases = [
        ("library.file.hide", OperationKind::Hide),
        ("library.file.unhide", OperationKind::Unhide),
        ("library.file.delete", OperationKind::Delete),
        ("library.file.restore", OperationKind::Restore),
        ("library.file.purge", OperationKind::Purge),
        ("library.snapshot.copy", OperationKind::Copy),
        ("library.create", OperationKind::CatalogOrganisation),
        (
            "library.virtual_collection.add",
            OperationKind::CatalogOrganisation,
        ),
        ("monitor.report", OperationKind::Unknown),
    ];
    for (operation, kind) in cases {
        assert_eq!(operation_kind(operation), kind, "{operation}");
    }
}

/// A version-bound function writing a record and one journal line naming the version.
fn history_files(
    context: &filesquid_ocfl::write::BoundContext<'_>,
) -> Result<std::collections::BTreeMap<String, Vec<u8>>, String> {
    let version = context.version.to_string();
    let line = format!("{{\"op\":\"{version}\"}}\n");
    Ok(std::collections::BTreeMap::from([
        (format!("provenance/{version}.jsonld"), b"{}".to_vec()),
        (
            format!("provenance/{version}.journal.jsonl"),
            line.into_bytes(),
        ),
    ]))
}

#[test]
fn history_files_ride_the_snapshot_version_and_are_not_entities() {
    use filesquid_api::canonical::{commit_snapshot_with, read_commit_journal};
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut first = SnapshotDelta::default();
    first.put(&head(1)).unwrap();
    let bound = &history_files as &dyn filesquid_ocfl::write::VersionBoundFiles;
    let meta1 = meta(&record("op-1", "1"));
    ready(commit_snapshot_with(
        &model,
        &OwnerReadable,
        &first,
        meta1,
        Some(bound),
    ))
    .unwrap()
    .expect("v1");
    let mut second = SnapshotDelta::default();
    second.put(&head(2)).unwrap();
    let meta2 = meta(&record("op-2", "2"));
    ready(commit_snapshot_with(
        &model,
        &OwnerReadable,
        &second,
        meta2,
        Some(bound),
    ))
    .unwrap()
    .expect("v2");
    let same = ready(commit_snapshot_with(
        &model,
        &OwnerReadable,
        &second,
        meta(&record("op-3", "2")),
        Some(bound),
    ))
    .unwrap();
    assert_eq!(same, None, "no change writes no version and no history");
    let snapshot = ready(read_snapshot(&rig.root, &OwnerReadable))
        .unwrap()
        .expect("snapshot");
    assert_eq!(snapshot.library.revision, 2, "history files are skipped");
    let all = ready(read_commit_journal(&rig.root, &OwnerReadable, None)).unwrap();
    assert_eq!(all, ["{\"op\":\"v1\"}", "{\"op\":\"v2\"}"]);
    let v1 = filesquid_ocfl::VersionNum::parse("v1").unwrap();
    let missed = ready(read_commit_journal(&rig.root, &OwnerReadable, Some(&v1))).unwrap();
    assert_eq!(missed, ["{\"op\":\"v2\"}"]);
}

#[test]
fn a_root_without_a_snapshot_has_no_commit_journal() {
    use filesquid_api::canonical::read_commit_journal;
    let rig = Rig::new();
    let lines = ready(read_commit_journal(&rig.root, &OwnerReadable, None)).unwrap();
    assert!(lines.is_empty());
}

#[test]
fn a_provenance_only_message_names_no_receipt() {
    let text = "filesquid.operation.v1 {\"operation_id\":\"run-1\",\"kind\":\"backfill\"}";
    assert_eq!(parse_operation_message(text), Ok(None));
    assert!(parse_operation_message("filesquid.operation.v1 {\"kind\":\"backfill\"}").is_err());
}

#[test]
fn a_commit_journal_a_replay_could_not_read_is_refused_before_publishing() {
    use filesquid_api::canonical::{commit_snapshot_with, MAX_JOURNAL_LINES};
    let rig = Rig::new();
    let model = rig.model(SNAPSHOT_ROOT, &snapshot_object_id(&library_id()));
    let mut delta = SnapshotDelta::default();
    delta.put(&head(1)).unwrap();
    let lines = "{}\n".repeat(MAX_JOURNAL_LINES + 1);
    let oversized = move |context: &filesquid_ocfl::write::BoundContext<'_>| {
        let path = format!("provenance/{}.journal.jsonl", context.version);
        Ok(std::collections::BTreeMap::from([(
            path,
            lines.clone().into_bytes(),
        )]))
    };
    let bound = &oversized as &dyn filesquid_ocfl::write::VersionBoundFiles;
    let meta1 = meta(&record("op-1", "1"));
    let result = ready(commit_snapshot_with(
        &model,
        &OwnerReadable,
        &delta,
        meta1,
        Some(bound),
    ));
    assert!(result.is_err(), "{result:?}");
    assert_eq!(
        ready(read_snapshot(&rig.root, &OwnerReadable)).unwrap(),
        None
    );
}
