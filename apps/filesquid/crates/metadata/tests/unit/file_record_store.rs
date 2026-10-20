//! The `ocfl` store keeps each file's record at its own path,
//! `<file-uuid>.metadata.md` beside the stored original (MD-0.8; MD-S-008,
//! MD-S-013, ADR 038 Decisions 2 and 5). The import section is append-only:
//! a save that edits or removes an entry or the original name is refused and
//! publishes nothing.

use filesquid_metadata::fact::Fact;
use filesquid_metadata::file_record::{FileRecord, RecordError};
use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::import::NameForm;
use filesquid_metadata::raw::Raw;
use filesquid_metadata::record::Record;
use filesquid_metadata::store::ocfl::{CommitOptions, OcflStore};
use filesquid_metadata::store::{MetadataStore, StoreError, METADATA_MD};
use filesquid_metadata::MetadataNamespace;
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::validate::validate_object;

#[path = "../support/import_rig.rs"]
mod import;
#[path = "../support/ocfl_rig.rs"]
mod rig;

use import::{entry, entry_named, OBJECT, RECORD, STORED};
use rig::{object_root, ready, Rig};

/// The reference to the stored original and its per-file record.
fn file() -> FileRef {
    FileRef::new(OBJECT, STORED)
        .unwrap()
        .with_record_path(RECORD)
        .unwrap()
}

/// The standard file record.
fn record() -> FileRecord {
    FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        Some("image/jpeg".to_string()),
        entry(),
    )
    .unwrap()
}

/// Stage the imported bytes, then commit the record over them: one version.
fn import(rig: &Rig<MemoryBackend>, record: &FileRecord) -> String {
    let store = rig.store();
    let model = store.change_model(OBJECT).unwrap();
    ready(model.reset()).unwrap();
    ready(model.stage_bytes(STORED, b"pixels")).unwrap();
    let receipt = ready(store.commit_import_with(&file(), record, &CommitOptions::default()))
        .expect("import")
        .expect("a version");
    receipt.version.to_string()
}

/// The head record, or the store's refusal.
fn read(rig: &Rig<MemoryBackend>) -> FileRecord {
    ready(rig.store().read_file_record(&file()))
        .expect("read")
        .expect("a record")
}

/// A different import of the same file.
fn second_entry() -> filesquid_metadata::import::ImportEntry {
    let mut again = entry();
    again.acquisition.job_id = Fact::value("job-2".to_string());
    again
}

#[test]
fn the_record_lives_beside_the_original_in_the_same_version_as_its_content() {
    let rig = Rig::memory();
    assert_eq!(import(&rig, &record()), "v1");
    let root = object_root(OBJECT);
    let keys = rig.snapshot(&format!("{root}/"));
    assert!(keys.contains_key(&format!("{root}/v1/content/{STORED}")));
    assert!(keys.contains_key(&format!("{root}/v1/content/{RECORD}")));
    assert!(!keys.contains_key(&format!("{root}/v1/content/{METADATA_MD}")));
    let report = ready(validate_object(&rig.storage, &root)).expect("validate");
    assert!(report.is_valid(), "{:?}", report.errors());
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}

#[test]
fn the_record_reads_back_exactly() {
    let rig = Rig::memory();
    import(&rig, &record());
    assert_eq!(read(&rig), record());
}

#[test]
fn a_re_import_appends_an_entry_in_a_new_version_and_leaves_v1_untouched() {
    let rig = Rig::memory();
    import(&rig, &record());
    let root = object_root(OBJECT);
    let v1 = rig.snapshot(&format!("{root}/v1/"));
    let receipt = ready(rig.store().append_import_with(
        &file(),
        second_entry(),
        &CommitOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(receipt.version.to_string(), "v2");
    assert_eq!(rig.snapshot(&format!("{root}/v1/")), v1);
    let now = read(&rig);
    assert_eq!(now.imports().len(), 2);
    assert_eq!(now.imports()[0], record().imports()[0]);
    let pinned = ready(rig.store().read_file_record(&file().with_version("v1")))
        .unwrap()
        .unwrap();
    assert_eq!(pinned.imports().len(), 1);
}

#[test]
fn a_re_import_uses_the_callers_commit_options() {
    let rig = Rig::memory();
    import(&rig, &record());
    let options = CommitOptions {
        message: Some("filesquid.operation.v1 {}".to_string()),
        ..CommitOptions::default()
    };
    ready(
        rig.store()
            .append_import_with(&file(), second_entry(), &options),
    )
    .unwrap()
    .unwrap();
    let root = object_root(OBJECT);
    let object = ready(filesquid_ocfl::read::Object::open(&rig.storage, &root)).unwrap();
    let last = object.history().into_iter().last().unwrap();
    assert_eq!(last.message.as_deref(), Some("filesquid.operation.v1 {}"));
}

#[test]
fn a_save_that_changes_an_import_entry_is_refused_and_publishes_nothing() {
    let rig = Rig::memory();
    import(&rig, &record());
    let text = record()
        .to_markdown()
        .replace("studio.local", "other.local");
    let tampered = FileRecord::from_markdown(&text).unwrap();
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &tampered,
        &CommitOptions::default(),
    ));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::ImportEntryChanged {
                index: 0
            }))
        ),
        "{result:?}"
    );
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}

#[test]
fn a_save_that_removes_an_import_entry_is_refused() {
    let rig = Rig::memory();
    import(&rig, &record());
    ready(
        rig.store()
            .append_import_with(&file(), second_entry(), &CommitOptions::default()),
    )
    .unwrap();
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &record(),
        &CommitOptions::default(),
    ));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::ImportEntryRemoved))
        ),
        "{result:?}"
    );
    assert_eq!(rig.versions(OBJECT), ["v1", "v2"]);
}

#[test]
fn a_save_that_changes_the_original_name_is_refused() {
    let rig = Rig::memory();
    import(&rig, &record());
    let text = record().to_markdown().replacen(
        r#"original_name: "Holiday 2023.JPG""#,
        r#"original_name: "Other.JPG""#,
        1,
    );
    let changed = FileRecord::from_markdown(&text).unwrap();
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &changed,
        &CommitOptions::default(),
    ));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::OriginalNameChanged))
        ),
        "{result:?}"
    );
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}

#[test]
fn a_rename_is_a_new_version_that_changes_only_the_display_name() {
    let rig = Rig::memory();
    import(&rig, &record());
    let mut renamed = read(&rig);
    renamed.set_display_name("Harbour.jpg").unwrap();
    let receipt = ready(rig.store().commit_file_record_with(
        &file(),
        &renamed,
        &CommitOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(receipt.version.to_string(), "v2");
    let now = read(&rig);
    assert_eq!(now.display_name(), "Harbour.jpg");
    assert_eq!(now.original_name().as_bytes(), b"Holiday 2023.JPG");
    assert_eq!(now.imports(), record().imports());
}

#[test]
fn an_unchanged_record_publishes_nothing() {
    let rig = Rig::memory();
    import(&rig, &record());
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &record(),
        &CommitOptions::default(),
    ))
    .unwrap();
    assert!(result.is_none());
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}

#[test]
fn a_statements_only_save_keeps_the_header_and_every_import_entry() {
    let rig = Rig::memory();
    import(&rig, &record());
    let mut statements = Record::default();
    statements
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .unwrap();
    ready(rig.store().save(&file(), &statements)).unwrap();
    let now = read(&rig);
    assert_eq!(
        now.statements().get("dcterms:title").unwrap().value,
        "Harbour"
    );
    assert_eq!(now.imports(), record().imports());
    assert_eq!(now.original_name().as_bytes(), b"Holiday 2023.JPG");
    let loaded = ready(rig.store().load(&file())).unwrap();
    assert_eq!(loaded.get("dcterms:title").unwrap().value, "Harbour");
    assert_eq!(rig.versions(OBJECT), ["v1", "v2"]);
}

#[test]
fn a_statements_only_save_without_a_file_record_is_refused() {
    let rig = Rig::memory();
    rig.seed(OBJECT, &[(STORED, b"pixels")]);
    let mut statements = Record::default();
    statements
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .unwrap();
    let result = ready(rig.store().save(&file(), &statements));
    assert!(
        matches!(result, Err(StoreError::Record(RecordError::NoFileRecord))),
        "{result:?}"
    );
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}

#[test]
fn appending_to_a_missing_record_is_refused() {
    let rig = Rig::memory();
    rig.seed(OBJECT, &[(STORED, b"pixels")]);
    let result = ready(rig.store().append_import_with(
        &file(),
        second_entry(),
        &CommitOptions::default(),
    ));
    assert!(
        matches!(result, Err(StoreError::Record(RecordError::NoFileRecord))),
        "{result:?}"
    );
}

#[test]
fn an_entry_must_cite_the_object_and_the_path_of_the_file_it_is_committed_for() {
    let rig = Rig::memory();
    let mut other_object = entry();
    other_object.acquisition.object_id = "urn:uuid:other".to_string();
    let record = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        other_object,
    )
    .unwrap();
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &record,
        &CommitOptions::default(),
    ));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::BoundToOtherObject))
        ),
        "{result:?}"
    );
    let mut other_path = entry();
    other_path.acquisition.logical_path = Some("original/other.JPG".to_string());
    let record = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        other_path,
    )
    .unwrap();
    let result = ready(rig.store().commit_file_record_with(
        &file(),
        &record,
        &CommitOptions::default(),
    ));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::BoundToOtherPath))
        ),
        "{result:?}"
    );
    assert!(!rig.storage_has_object(OBJECT));
}

#[test]
fn a_non_utf8_name_survives_the_store() {
    let rig = Rig::memory();
    let name = [b'a', 0xFF, b'.', b'j', b'p', b'g'];
    let record = FileRecord::new(
        Raw::from_bytes(&name),
        NameForm::Unnormalised,
        Some("jpg".to_string()),
        None,
        None,
        entry_named(&name),
    )
    .unwrap();
    import(&rig, &record);
    let back = read(&rig);
    assert_eq!(back.original_name().as_bytes(), &name);
    assert_eq!(back.imports()[0].source.original_name.as_bytes(), &name);
}

#[test]
fn a_plain_metadata_md_is_unchanged_by_the_per_file_path() {
    let rig = Rig::memory();
    rig.seed(OBJECT, &[("photo.jpg", b"pixels")]);
    let plain = FileRef::new(OBJECT, "photo.jpg").unwrap();
    assert_eq!(plain.record_path(), None);
    let mut statements = Record::default();
    statements
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .unwrap();
    ready(rig.store().save(&plain, &statements)).unwrap();
    let root = object_root(OBJECT);
    let keys = rig.snapshot(&format!("{root}/"));
    assert!(keys.contains_key(&format!("{root}/v2/content/{METADATA_MD}")));
}

#[test]
fn a_record_path_must_be_a_valid_logical_path() {
    let file = FileRef::new(OBJECT, STORED).unwrap();
    for bad in ["", "/abs.metadata.md", "../x.metadata.md", "a\\b", "a//b"] {
        assert!(file.clone().with_record_path(bad).is_err(), "{bad}");
    }
}

#[test]
fn the_store_type_is_usable_with_the_default_commit_helper() {
    let rig = Rig::memory();
    let store: OcflStore<'_, _, _, _, _, _> = rig.store();
    let model = store.change_model(OBJECT).unwrap();
    ready(model.stage_bytes(STORED, b"pixels")).unwrap();
    let version = ready(store.commit_import(&file(), &record()))
        .unwrap()
        .unwrap();
    assert_eq!(version.to_string(), "v1");
}

/// Stage `bytes` at the stored original of the scene.
fn stage(rig: &Rig<MemoryBackend>, bytes: &[u8]) {
    let model = rig.store().change_model(OBJECT).unwrap();
    ready(model.reset()).unwrap();
    ready(model.stage_bytes(STORED, bytes)).unwrap();
}

#[test]
fn staged_content_that_is_not_the_verified_content_is_refused() {
    let rig = Rig::memory();
    stage(&rig, b"other bytes");
    let result = ready(rig.store().commit_import(&file(), &record()));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::ContentDigestMismatch))
        ),
        "{result:?}"
    );
    assert!(!rig.storage_has_object(OBJECT));
}

#[test]
fn an_import_without_its_staged_content_is_refused() {
    let rig = Rig::memory();
    let model = rig.store().change_model(OBJECT).unwrap();
    ready(model.reset()).unwrap();
    ready(model.stage_bytes("original/elsewhere.JPG", b"pixels")).unwrap();
    let result = ready(rig.store().commit_import(&file(), &record()));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::ContentNotStaged))
        ),
        "{result:?}"
    );
    assert!(!rig.storage_has_object(OBJECT));
}

#[test]
fn a_failed_import_leaves_its_change_set_so_a_retry_can_publish() {
    let rig = Rig::memory();
    stage(&rig, b"pixels");
    let mut wrong = entry();
    wrong.acquisition.sha256 = "0".repeat(64);
    let bad = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        wrong,
    )
    .unwrap();
    assert!(ready(rig.store().commit_import(&file(), &bad)).is_err());
    let version = ready(rig.store().commit_import(&file(), &record()))
        .unwrap()
        .unwrap();
    assert_eq!(version.to_string(), "v1");
}

#[test]
fn a_record_saved_at_the_content_path_is_refused() {
    let rig = Rig::memory();
    stage(&rig, b"pixels");
    let at_content = FileRef::new(OBJECT, STORED)
        .unwrap()
        .with_record_path(STORED)
        .unwrap();
    let result = ready(rig.store().commit_import(&at_content, &record()));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::RecordAtContentPath))
        ),
        "{result:?}"
    );
    let default_path = FileRef::new(OBJECT, "metadata.md").unwrap();
    let mut entry = entry();
    entry.acquisition.logical_path = Some("metadata.md".to_string());
    let colliding = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        entry,
    )
    .unwrap();
    let model = rig.store().change_model(OBJECT).unwrap();
    ready(model.reset()).unwrap();
    ready(model.stage_bytes("metadata.md", b"pixels")).unwrap();
    let result = ready(rig.store().commit_import(&default_path, &colliding));
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::RecordAtContentPath))
        ),
        "{result:?}"
    );
}

#[test]
fn a_referenced_import_commits_its_record_without_content() {
    let rig = Rig::memory();
    let mut referenced = entry();
    referenced.acquisition.storage = filesquid_metadata::import::ByteStorage::Referenced;
    referenced.acquisition.logical_path = None;
    let record = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        referenced,
    )
    .unwrap();
    let file = FileRef::new(OBJECT, RECORD)
        .unwrap()
        .with_record_path(RECORD)
        .unwrap();
    let version = ready(rig.store().commit_import(&file, &record))
        .unwrap()
        .unwrap();
    assert_eq!(version.to_string(), "v1");
    let root = object_root(OBJECT);
    let keys = rig.snapshot(&format!("{root}/"));
    assert!(keys.contains_key(&format!("{root}/v1/content/{RECORD}")));
    assert!(!keys.contains_key(&format!("{root}/v1/content/{STORED}")));
    let back = ready(rig.store().read_file_record(&file)).unwrap().unwrap();
    assert_eq!(
        back.imports()[0].acquisition.storage,
        filesquid_metadata::import::ByteStorage::Referenced
    );
}

#[test]
fn a_re_import_must_cite_content_the_object_holds() {
    let rig = Rig::memory();
    import(&rig, &record());
    let mut other = second_entry();
    other.acquisition.sha256 = "1".repeat(64);
    let result = ready(
        rig.store()
            .append_import_with(&file(), other, &CommitOptions::default()),
    );
    assert!(
        matches!(
            result,
            Err(StoreError::Record(RecordError::ContentDigestMismatch))
        ),
        "{result:?}"
    );
    assert_eq!(rig.versions(OBJECT), ["v1"]);
}
