//! Task DE-0.3: the FileSquid OCFL application profile (`filesquid_api::canonical`): its
//! layout, entity encoding and operation message, and snapshot commits and reads through
//! the change model on the in-memory storage backend.

use filesquid_api::canonical::{
    commit_file_original, commit_snapshot, create_storage_root, file_object_id, file_object_root,
    operation_message, parse_operation_message, read_snapshot, snapshot_object_id, touched,
    CanonicalError, LibraryHead, OperationRecord, OwnerReadable, SnapshotDelta, SnapshotEntity,
    SnapshotFile, SnapshotKey, VirtualEntry, SNAPSHOT_ROOT,
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
        Some(first)
    );
    assert_eq!(parse_operation_message("imported by hand"), None);
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
    .unwrap();
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

#[test]
fn a_file_original_is_its_own_object() {
    let rig = Rig::new();
    let file = FileId::new("f-1").unwrap();
    let root = file_object_root(&file).unwrap();
    let model = rig.model(&root, &file_object_id(&file));
    let receipt = ready(commit_file_original(
        &model,
        &OwnerReadable,
        "IMG_0001.JPG",
        b"jpeg bytes",
        meta(&record("op-9", "1")),
    ))
    .unwrap();
    assert_eq!(receipt.object_id, "urn:uuid:f-1");
    let object = ready(Object::open(&rig.root, &root)).unwrap();
    let state = object.state(object.head()).unwrap();
    assert_eq!(
        state.paths().collect::<Vec<_>>(),
        vec!["original/IMG_0001.JPG"]
    );
    let refused = ready(commit_file_original(
        &model,
        &OwnerReadable,
        "a/b",
        b"x",
        meta(&record("op-10", "1")),
    ));
    assert!(matches!(refused, Err(CanonicalError::InvalidIdentifier(_))));
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
