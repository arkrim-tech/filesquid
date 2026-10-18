//! Persist metadata.md through filesquid-ocfl helpers.

use filesquid_metadata::record::Record;
use filesquid_metadata::store::ocfl::{load, save};
use filesquid_metadata::store::METADATA_MD;
use filesquid_metadata::MetadataNamespace;

#[test]
fn save_and_load_object_dir() {
    let dir = std::env::temp_dir().join("filesquid-metadata-store-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "creator", "George")
        .unwrap();
    save(&dir, &record).unwrap();
    assert!(dir.join(METADATA_MD).is_file());
    let loaded = load(&dir).unwrap();
    assert_eq!(loaded.get("dcterms:creator").unwrap().value, "George");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn csv_line_break_survives_save_and_load() {
    let dir = std::env::temp_dir().join(format!(
        "filesquid-metadata-store-newline-{}",
        std::process::id()
    ));
    if let Err(err) = std::fs::remove_dir_all(&dir) {
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}");
    }
    std::fs::create_dir_all(&dir).unwrap();
    let mut record = Record::default();
    filesquid_metadata::csv::apply_csv(
        "dcterms:description\n\"line one\nline two\"\n",
        &mut record,
    )
    .unwrap();
    save(&dir, &record).unwrap();
    let loaded = load(&dir).unwrap();
    assert_eq!(
        loaded.get("dcterms:description").unwrap().value,
        "line one\nline two"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The store runs over any `filesquid-ocfl` storage backend, not only a
/// local directory (IO-0.1, ADR 025).
mod backend_store {
    use filesquid_metadata::record::Record;
    use filesquid_metadata::store::ocfl::{load_from, metadata_key, save_to};
    use filesquid_metadata::store::METADATA_MD;
    use filesquid_metadata::MetadataNamespace;
    use filesquid_ocfl::storage::{complete_immediately, MemoryBackend, StorageBackend};

    #[test]
    fn metadata_key_joins_the_object_prefix() {
        assert_eq!(metadata_key("").unwrap().as_str(), METADATA_MD);
        assert_eq!(
            metadata_key("objects/a/").unwrap().as_str(),
            "objects/a/metadata.md"
        );
        assert_eq!(
            metadata_key("objects/a").unwrap().as_str(),
            "objects/a/metadata.md"
        );
        assert!(metadata_key("../a").is_err());
    }

    #[test]
    fn absolute_prefixes_are_rejected() {
        assert!(metadata_key("/").is_err());
        assert!(metadata_key("/objects/a").is_err());
    }

    #[test]
    fn records_larger_than_one_put_are_saved_through_multipart() {
        let backend = MemoryBackend::new();
        let key = metadata_key("objects/a").unwrap();
        let long = "x".repeat(9 * 1024 * 1024);
        let mut record = Record::default();
        record
            .set(MetadataNamespace::Dcterms, "description", &long)
            .unwrap();
        complete_immediately(save_to(&backend, &key, &record))
            .unwrap()
            .unwrap();
        let loaded = complete_immediately(load_from(&backend, &key))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.get("dcterms:description").unwrap().value, long);
    }

    #[test]
    fn missing_metadata_md_is_an_empty_record() {
        let backend = MemoryBackend::new();
        let key = metadata_key("objects/a").unwrap();
        let record = complete_immediately(load_from(&backend, &key))
            .unwrap()
            .unwrap();
        assert!(record.get("dcterms:title").is_none());
    }

    #[test]
    fn save_then_load_through_a_memory_backend() {
        let backend = MemoryBackend::new();
        let key = metadata_key("objects/a").unwrap();
        let mut record = Record::default();
        record
            .set(MetadataNamespace::Dcterms, "title", "Harbour")
            .unwrap();
        complete_immediately(save_to(&backend, &key, &record))
            .unwrap()
            .unwrap();
        assert!(complete_immediately(backend.exists(&key)).unwrap().unwrap());
        let loaded = complete_immediately(load_from(&backend, &key))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.get("dcterms:title").unwrap().value, "Harbour");
    }

    #[test]
    fn ocfl_store_implements_the_metadata_store_interface() {
        use filesquid_metadata::file_ref::FileRef;
        use filesquid_metadata::store::ocfl::OcflStore;
        use filesquid_metadata::store::MetadataStore;

        let store = OcflStore::new(MemoryBackend::new());
        let file = FileRef::new("objects/a", "photo.jpg")
            .unwrap()
            .with_version("v1");
        let mut record = Record::default();
        record
            .set(MetadataNamespace::Dcterms, "title", "Harbour")
            .unwrap();
        complete_immediately(store.save(&file, &record))
            .unwrap()
            .unwrap();
        let loaded = complete_immediately(store.load(&file)).unwrap().unwrap();
        assert_eq!(loaded.get("dcterms:title").unwrap().value, "Harbour");
        let other = FileRef::new("objects/b", "photo.jpg").unwrap();
        let empty = complete_immediately(store.load(&other)).unwrap().unwrap();
        assert!(empty.statements.is_empty());
    }

    #[test]
    fn ocfl_store_rejects_an_absolute_object_prefix() {
        use filesquid_metadata::file_ref::FileRef;
        use filesquid_metadata::store::ocfl::OcflStore;
        use filesquid_metadata::store::{MetadataStore, StoreError};

        let store = OcflStore::new(MemoryBackend::new());
        let file = FileRef::new("/objects/a", "photo.jpg").unwrap();
        let result = complete_immediately(store.load(&file)).unwrap();
        assert!(matches!(result, Err(StoreError::InvalidPath)));
    }
}
