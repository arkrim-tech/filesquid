//! Task DE-0.3: the FileSquid OCFL application profile (`filesquid_api::canonical`): its
//! layout, entity encoding and operation message, and snapshot commits and reads through
//! the change model on the in-memory storage backend.

use filesquid_api::canonical::{
    commit_file_original, commit_snapshot, create_storage_root, file_object_id, file_object_root,
    operation_message, parse_operation_message, read_file_original, read_snapshot,
    snapshot_object_id, touched, CanonicalError, FileOriginal, LibraryHead, OperationRecord,
    OwnerReadable, SnapshotDelta, SnapshotEntity, SnapshotFile, SnapshotKey, VirtualEntry,
    SNAPSHOT_ROOT,
};
use filesquid_api::library::{CatalogChange, CatalogError, FileState, LibraryRecord};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId, VirtualMembership,
};
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

/// Commit `bytes` as the original of `file`, named `original_name` by the owner.
fn commit_original(
    rig: &Rig,
    file: &FileId,
    original_name: &str,
    op: &str,
) -> Result<filesquid_ocfl::write::CommitReceipt, CanonicalError> {
    let root = file_object_root(file).unwrap();
    let model = rig.model(&root, &file_object_id(file));
    ready(commit_file_original(
        &model,
        &OwnerReadable,
        file,
        original_name,
        b"jpeg bytes",
        meta(&record(op, "1")),
    ))
}

/// The logical paths at the head of `file`'s object.
fn head_paths(rig: &Rig, file: &FileId) -> Vec<String> {
    let object = ready(Object::open(&rig.root, &file_object_root(file).unwrap())).unwrap();
    let state = object.state(object.head()).unwrap();
    state.paths().map(str::to_string).collect()
}

#[test]
fn a_file_original_is_its_own_object_with_its_companion() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let receipt = commit_original(&rig, &file, "IMG_0001.JPG", "op-9").unwrap();
    assert_eq!(receipt.object_id, format!("urn:uuid:{FILE}"));
    let stored = format!("original/IMG_0001_{FILE}.jpg");
    assert_eq!(
        head_paths(&rig, &file),
        vec![stored.clone(), format!("{stored}.metadata.md")]
    );
    assert_eq!(
        ready(read_file_original(&rig.root, &OwnerReadable, &file)).unwrap(),
        Some(FileOriginal {
            path: stored,
            original_name: "IMG_0001.JPG".into(),
        })
    );
}

#[test]
fn a_name_that_needs_sanitizing_round_trips_through_its_companion() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let original = "Café au lait — 写真 (1).JPEG";
    commit_original(&rig, &file, original, "op-9").unwrap();
    let stored = format!("original/Caf__au_lait_______1__{FILE}.jpeg");
    assert!(head_paths(&rig, &file).contains(&stored));
    let read = ready(read_file_original(&rig.root, &OwnerReadable, &file))
        .unwrap()
        .expect("original");
    assert_eq!(read.path, stored);
    assert_eq!(read.original_name, original);
}

#[test]
fn the_companion_is_readable_front_matter() {
    let bytes = filesquid_api::canonical::original::encode_companion("a \"b\".txt").unwrap();
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        "---\nschema: filesquid.original.v1\noriginal_name: \"a \\\"b\\\".txt\"\n---\n"
    );
    let path = "original/x.metadata.md";
    let decoded = filesquid_api::canonical::original::decode_companion(path, &bytes).unwrap();
    assert_eq!(decoded, "a \"b\".txt");
    let other = b"---\nschema: other\noriginal_name: \"a\"\n---\n";
    let refused = filesquid_api::canonical::original::decode_companion(path, other);
    assert!(matches!(refused, Err(CanonicalError::Decode { .. })));
}

#[test]
fn a_name_without_a_safe_stem_or_extension_is_the_file_identifier() {
    let file = FileId::new(FILE).unwrap();
    let name = |n| filesquid_api::canonical::original::stored_name(&file, n).unwrap();
    assert_eq!(name("写真"), FILE);
    assert_eq!(name("写真.PNG"), format!("{FILE}.png"));
    assert_eq!(name(".bashrc"), format!("bashrc_{FILE}"));
    assert_eq!(name("..."), FILE);
    assert_eq!(name("notes.tar.gz"), format!("notes.tar_{FILE}.gz"));
    assert_eq!(name("report.v2 draft"), format!("report.v2_draft_{FILE}"));
}

#[test]
fn an_original_name_that_is_not_a_file_name_is_refused() {
    let rig = Rig::new();
    let file = FileId::new(FILE).unwrap();
    let long = "a".repeat(1025);
    for bad in ["", "a/b", "a\0b", long.as_str()] {
        let refused = commit_original(&rig, &file, bad, "op-10");
        assert!(
            matches!(refused, Err(CanonicalError::InvalidIdentifier(_))),
            "{bad:?}"
        );
    }
    assert_eq!(
        ready(read_file_original(&rig.root, &OwnerReadable, &file)).unwrap(),
        None
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
