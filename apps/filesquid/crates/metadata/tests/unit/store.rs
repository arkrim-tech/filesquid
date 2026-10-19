//! The `ocfl` store commits each metadata edit as a new OCFL version through
//! the `filesquid-ocfl` change model (MD-0.2; MD-S-003, MD-S-008; owner
//! decision 2026-10-18, ADR 027 Decision 2). Earlier versions are never
//! modified, and a head without its version inventory (W010) stays refused
//! (OCFL-S-020).

use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::json::sha256_hex;
use filesquid_metadata::record::Record;
use filesquid_metadata::store::ocfl::OcflStore;
use filesquid_metadata::store::{MetadataStore, StoreError, METADATA_MD};
use filesquid_metadata::MetadataNamespace;
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::write::{CommitError, SystemClock};

#[path = "../support/ocfl_rig.rs"]
mod rig;

use rig::{object_root, ready, v, Rig, OBJECT_ID};

/// The described file of the object.
fn photo() -> FileRef {
    FileRef::new(OBJECT_ID, "photo.jpg").expect("file ref")
}

/// A record holding `dcterms:title`.
fn titled(title: &str) -> Record {
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", title)
        .expect("title");
    record
}

/// Save `record` and return the version it published.
fn commit<B: StorageBackend>(rig: &Rig<B>, file: &FileRef, record: &Record) -> Option<String> {
    ready(rig.store().commit_record(file, record))
        .expect("commit")
        .map(|version| version.to_string())
}

/// The `dcterms:title` of the record at `version` (`None`: the head).
fn title_at<B: StorageBackend>(rig: &Rig<B>, version: Option<&str>) -> Option<String> {
    let mut file = photo();
    if let Some(version) = version {
        file = file.with_version(version);
    }
    let record = ready(rig.store().load(&file)).expect("load");
    record.get("dcterms:title").map(|s| s.value.clone())
}

#[test]
fn each_edit_commits_exactly_one_new_version() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    assert_eq!(
        commit(&rig, &photo(), &titled("Harbour")),
        Some("v2".into())
    );
    assert_eq!(commit(&rig, &photo(), &titled("Dawn")), Some("v3".into()));
    assert_eq!(rig.versions(OBJECT_ID), ["v1", "v2", "v3"]);
    assert_eq!(title_at(&rig, None).as_deref(), Some("Dawn"));
    assert_eq!(title_at(&rig, Some("v2")).as_deref(), Some("Harbour"));
    assert_eq!(title_at(&rig, Some("v1")), None);
}

#[test]
fn an_edit_never_modifies_an_earlier_version() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    commit(&rig, &photo(), &titled("Harbour"));
    let root = object_root(OBJECT_ID);
    let v1 = rig.snapshot(&format!("{root}/v1/"));
    let v2 = rig.snapshot(&format!("{root}/v2/"));
    assert!(!v2.is_empty());
    commit(&rig, &photo(), &titled("Dawn"));
    assert_eq!(rig.snapshot(&format!("{root}/v1/")), v1);
    assert_eq!(rig.snapshot(&format!("{root}/v2/")), v2);
}

#[test]
fn metadata_md_is_a_logical_file_inside_a_version() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    commit(&rig, &photo(), &titled("Harbour"));
    let root = object_root(OBJECT_ID);
    let keys = rig.snapshot(&format!("{root}/"));
    assert!(keys.contains_key(&format!("{root}/v2/content/{METADATA_MD}")));
    assert!(!keys.contains_key(&format!("{root}/{METADATA_MD}")));
    let report = ready(validate_object(&rig.storage, &root)).expect("validate");
    assert!(report.is_valid(), "{:?}", report.errors());
}

#[test]
fn an_unchanged_record_publishes_nothing() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    commit(&rig, &photo(), &titled("Harbour"));
    assert_eq!(commit(&rig, &photo(), &titled("Harbour")), None);
    assert_eq!(rig.head(OBJECT_ID), v("v2"));
}

#[test]
fn the_first_edit_of_a_new_object_creates_v1() {
    let rig = Rig::memory();
    let file = FileRef::new("urn:example:new", "photo.jpg").unwrap();
    assert_eq!(commit(&rig, &file, &titled("Harbour")), Some("v1".into()));
    let loaded = ready(rig.store().load(&file)).expect("load");
    assert_eq!(loaded.get("dcterms:title").unwrap().value, "Harbour");
}

#[test]
fn a_missing_object_or_metadata_md_is_an_empty_record() {
    let rig = Rig::memory();
    let absent = FileRef::new("urn:example:absent", "photo.jpg").unwrap();
    assert!(ready(rig.store().load(&absent))
        .unwrap()
        .statements
        .is_empty());
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    assert!(ready(rig.store().load(&photo()))
        .unwrap()
        .statements
        .is_empty());
}

#[test]
fn a_pinned_version_that_is_not_the_head_is_a_conflict() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    commit(&rig, &photo(), &titled("Harbour"));
    let stale = photo().with_version("v1");
    let result = ready(rig.store().commit_record(&stale, &titled("Dawn")));
    assert!(
        matches!(result, Err(StoreError::Conflict { .. })),
        "{result:?}"
    );
    assert_eq!(rig.head(OBJECT_ID), v("v2"));
    let current = photo().with_version("v2");
    assert_eq!(commit(&rig, &current, &titled("Dawn")), Some("v3".into()));
}

#[test]
fn a_head_without_its_version_inventory_stays_refused() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    rig.delete(OBJECT_ID, "v1/inventory.json");
    let result = ready(rig.store().save(&photo(), &titled("Harbour")));
    let Err(StoreError::Backend(source)) = result else {
        panic!("expected a refusal, got {result:?}");
    };
    let change = source.downcast_ref::<ChangeError>().expect("change error");
    assert!(
        matches!(
            change,
            ChangeError::Commit(CommitError::MissingVersionInventory { .. })
        ),
        "{change:?}"
    );
    assert_eq!(rig.versions(OBJECT_ID), ["v1"]);
    let root = object_root(OBJECT_ID);
    assert!(!rig
        .snapshot(&format!("{root}/v1/"))
        .contains_key(&format!("{root}/v1/inventory.json")));
}

#[test]
fn records_larger_than_one_put_round_trip() {
    let rig = Rig::memory();
    let long = "x".repeat(9 * 1024 * 1024);
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "description", &long)
        .unwrap();
    commit(&rig, &photo(), &record);
    let loaded = ready(rig.store().load(&photo())).expect("load");
    assert_eq!(loaded.get("dcterms:description").unwrap().value, long);
}

#[test]
fn a_csv_line_break_survives_commit_and_load() {
    let rig = Rig::memory();
    let mut record = Record::default();
    filesquid_metadata::csv::apply_csv(
        "dcterms:description\n\"line one\nline two\"\n",
        &mut record,
    )
    .unwrap();
    commit(&rig, &photo(), &record);
    let loaded = ready(rig.store().load(&photo())).expect("load");
    assert_eq!(
        loaded.get("dcterms:description").unwrap().value,
        "line one\nline two"
    );
}

#[test]
fn a_committed_record_that_is_not_utf8_is_an_error() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[(METADATA_MD, &[0xff, 0xfe])]);
    assert!(ready(rig.store().load(&photo())).is_err());
}

#[test]
fn an_object_root_that_could_escape_is_an_invalid_path() {
    let rig = Rig::memory();
    let store = OcflStore {
        storage: &rig.storage,
        reservations: &rig.leases,
        staging: &rig.staging,
        staging_prefix: "metadata-changes".to_string(),
        holder: "metadata-test".to_string(),
        clock: SystemClock,
        object_root: |_: &str| "/objects/a".to_string(),
    };
    let result = ready(store.load(&photo()));
    assert!(matches!(result, Err(StoreError::InvalidPath)), "{result:?}");
    let result = ready(store.save(&photo(), &titled("Harbour")));
    assert!(matches!(result, Err(StoreError::InvalidPath)), "{result:?}");
}

#[test]
fn reading_a_root_that_holds_another_object_is_refused() {
    let rig = Rig::memory();
    rig.seed("urn:example:other", &[("photo.jpg", b"pixels")]);
    let store = OcflStore {
        storage: &rig.storage,
        reservations: &rig.leases,
        staging: &rig.staging,
        staging_prefix: "metadata-changes".to_string(),
        holder: "metadata-test".to_string(),
        clock: SystemClock,
        object_root: |_: &str| object_root("urn:example:other"),
    };
    for result in [
        ready(store.file_sha256(&photo())).map(|_| ()),
        ready(store.read_file(&photo(), 1024)).map(|_| ()),
        ready(store.load(&photo())).map(|_| ()),
    ] {
        assert!(result.is_err(), "another object's bytes are never returned");
    }
    let result = ready(store.file_sha256(&photo()));
    assert!(
        matches!(result, Err(StoreError::WrongObject { .. })),
        "{result:?}"
    );
}

#[test]
fn file_sha256_hashes_the_logical_file_through_filesquid_ocfl() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    let digest = ready(rig.store().file_sha256(&photo())).expect("digest");
    assert_eq!(digest, sha256_hex(b"pixels"));
    let missing = FileRef::new(OBJECT_ID, "absent.jpg").unwrap();
    assert!(ready(rig.store().file_sha256(&missing)).is_err());
}

#[test]
fn staging_is_cleared_after_each_edit() {
    let rig = Rig::memory();
    commit(&rig, &photo(), &titled("Harbour"));
    let page = ready(rig.staging.list("metadata-changes/", None, 100)).unwrap();
    assert!(page.keys.is_empty(), "{:?}", page.keys);
}

#[test]
fn a_stale_change_set_of_this_writer_is_discarded_before_staging() {
    let rig = Rig::memory();
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    let prefix = rig.store().staging_prefix_for(OBJECT_ID);
    let staging = StagingArea::new(&rig.staging, &prefix).unwrap();
    let root = object_root(OBJECT_ID);
    let model = ChangeModel::new(
        &rig.storage,
        &rig.leases,
        "metadata-test",
        &root,
        OBJECT_ID,
        staging,
    );
    ready(model.stage_remove("photo.jpg")).expect("stale change");
    commit(&rig, &photo(), &titled("Harbour"));
    let object = ready(filesquid_ocfl::read::Object::open(&rig.storage, &root)).unwrap();
    let state = object.state(&v("v2")).unwrap();
    assert!(state.get("photo.jpg").is_some(), "photo.jpg kept");
}

/// The store runs over any `filesquid-ocfl` backend: a local directory too.
#[test]
fn per_edit_versions_on_a_filesystem_backend() {
    let dir = std::env::temp_dir().join(format!("md-0.2-store-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = filesquid_ocfl::storage::FsBackend::new(dir.join("root"), dir.join("multipart"))
        .expect("fs backend");
    let rig = Rig::over(storage);
    rig.seed(OBJECT_ID, &[("photo.jpg", b"pixels")]);
    assert_eq!(
        commit(&rig, &photo(), &titled("Harbour")),
        Some("v2".into())
    );
    assert_eq!(commit(&rig, &photo(), &titled("Dawn")), Some("v3".into()));
    let report = ready(validate_object(&rig.storage, &object_root(OBJECT_ID))).unwrap();
    assert!(report.is_valid(), "{:?}", report.errors());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The native helper the CLI uses: an object directory on the local disk.
mod local_object {
    use super::*;
    use filesquid_metadata::store::ocfl::LocalObject;

    /// A fresh scratch directory for one test.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("md-0.2-local-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_new_object_directory_needs_an_identifier() {
        let dir = scratch("needs-id");
        let result = LocalObject::open(&dir.join("obj"), None);
        assert!(
            matches!(result, Err(StoreError::MissingObjectId)),
            "{:?}",
            result.err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_save_is_a_new_version_of_a_valid_object() {
        let dir = scratch("versions");
        let object_dir = dir.join("obj");
        let object = LocalObject::open(&object_dir, Some("urn:x:1")).expect("open");
        assert_eq!(object.save(&titled("Harbour")).unwrap(), Some(v("v1")));
        let object = LocalObject::open(&object_dir, None).expect("reopen");
        assert_eq!(object.object_id(), "urn:x:1");
        assert_eq!(object.save(&titled("Dawn")).unwrap(), Some(v("v2")));
        assert_eq!(
            object.load().unwrap().get("dcterms:title").unwrap().value,
            "Dawn"
        );
        assert!(object_dir.join("v1/content").join(METADATA_MD).is_file());
        assert!(!object_dir.join(METADATA_MD).exists());
        let fs = filesquid_ocfl::storage::FsBackend::with_default_staging(&object_dir).unwrap();
        let report = ready(validate_object(&fs, "")).unwrap();
        assert!(report.is_valid(), "{:?}", report.errors());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_edit_from_a_head_another_writer_moved_is_a_conflict() {
        let dir = scratch("stale-base");
        let object_dir = dir.join("obj");
        let object = LocalObject::open(&object_dir, Some("urn:x:1")).unwrap();
        object.save(&titled("Harbour")).unwrap();
        let (base, _) = object.load_head().unwrap();
        assert_eq!(base, Some(v("v1")));
        object.save(&titled("Dawn")).unwrap();
        let result = object.save_over(base.as_ref(), &titled("Dusk"));
        assert!(
            matches!(result, Err(StoreError::Conflict { .. })),
            "{:?}",
            result.err()
        );
        assert_eq!(
            object.load().unwrap().get("dcterms:title").unwrap().value,
            "Dawn"
        );
        let (head, _) = object.load_head().unwrap();
        assert_eq!(head, Some(v("v2")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn another_identifier_is_refused() {
        let dir = scratch("other-id");
        let object_dir = dir.join("obj");
        let object = LocalObject::open(&object_dir, Some("urn:x:1")).unwrap();
        object.save(&titled("Harbour")).unwrap();
        let result = LocalObject::open(&object_dir, Some("urn:x:2"));
        assert!(
            matches!(result, Err(StoreError::WrongObject { .. })),
            "{:?}",
            result.err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_git_working_tree_is_refused() {
        let dir = scratch("git");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let result = LocalObject::open(&dir, Some("urn:x:1"));
        assert!(matches!(result, Err(StoreError::Git)), "{:?}", result.err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_digest_of_a_missing_logical_file_is_an_error() {
        let dir = scratch("digest");
        let object = LocalObject::open(&dir.join("obj"), Some("urn:x:1")).unwrap();
        object.save(&titled("Harbour")).unwrap();
        assert!(object.file_sha256("photo.jpg", None).is_err());
        let first = object.file_sha256(METADATA_MD, None).unwrap();
        assert_eq!(first.len(), 64);
        object.save(&titled("Dawn")).unwrap();
        assert_eq!(
            object.file_sha256(METADATA_MD, Some(&v("v1"))).unwrap(),
            first
        );
        assert_ne!(object.file_sha256(METADATA_MD, None).unwrap(), first);
        assert!(object.read_file(METADATA_MD, Some(&v("v1"))).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Two handles on one directory in one process never share a staging area,
/// so one handle's reset cannot discard or publish the other's change set.
#[test]
fn two_local_objects_in_one_process_stage_apart() {
    use filesquid_metadata::store::ocfl::LocalObject;
    let dir = std::env::temp_dir().join(format!("md-0.2-two-handles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = LocalObject::open(&dir, Some("urn:x:1")).unwrap();
    first.save(&titled("Harbour")).unwrap();
    let second = LocalObject::open(&dir, None).unwrap();
    let (base, _) = first.load_head().unwrap();
    assert_eq!(
        second.save_over(base.as_ref(), &titled("Dawn")).unwrap(),
        Some(v("v2"))
    );
    let result = first.save_over(base.as_ref(), &titled("Dusk"));
    assert!(
        matches!(result, Err(StoreError::Conflict { .. })),
        "{:?}",
        result.err()
    );
    assert_eq!(
        first.load().unwrap().get("dcterms:title").unwrap().value,
        "Dawn"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
