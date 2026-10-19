//! The installation UUID (DE-0.10; DE-S-012; ADR 040 Decision 4; ADR 021):
//! generated once, stored in `<data dir>/installation.json` and in the
//! operational store, read back at every start, restored from whichever
//! copy survives, and refused when malformed without naming a path.

use std::path::{Path, PathBuf};

use filesquid_native::installation::{
    load_or_create, InstallationError, InstallationSource, INSTALLATION_FILE,
};

/// A fresh data directory for one test.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-installation-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    dir
}

/// The installation file of `dir`.
fn file(dir: &Path) -> PathBuf {
    dir.join(INSTALLATION_FILE)
}

#[test]
fn the_first_start_creates_one_uuid_v4_and_later_starts_read_it() {
    let dir = data_dir("first");
    let first = load_or_create(&dir).expect("first start");
    assert_eq!(first.source, InstallationSource::Created);
    let uuid = first.id.as_str().to_string();
    assert_eq!(uuid.len(), 36);
    assert_eq!(&uuid[14..15], "4", "version 4");
    for _ in 0..3 {
        let again = load_or_create(&dir).expect("restart");
        assert_eq!(again.id, first.id, "stable across restarts");
        assert_eq!(again.source, InstallationSource::Stored);
    }
    let text = std::fs::read_to_string(file(&dir)).expect("installation.json");
    assert!(text.contains(&uuid), "{text}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn two_installations_get_different_uuids() {
    let a = data_dir("a");
    let b = data_dir("b");
    assert_ne!(
        load_or_create(&a).unwrap().id,
        load_or_create(&b).unwrap().id
    );
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}

#[test]
fn a_lost_file_is_restored_from_the_operational_store() {
    let dir = data_dir("lost-file");
    let first = load_or_create(&dir).unwrap();
    std::fs::remove_file(file(&dir)).unwrap();
    let again = load_or_create(&dir).expect("restored");
    assert_eq!(again.id, first.id);
    assert_eq!(again.source, InstallationSource::RestoredFile);
    assert!(file(&dir).is_file(), "the file copy is written back");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_lost_operational_store_is_restored_from_the_file() {
    let dir = data_dir("lost-store");
    let first = load_or_create(&dir).unwrap();
    for name in [
        "filesquid.sqlite",
        "filesquid.sqlite-wal",
        "filesquid.sqlite-shm",
    ] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let again = load_or_create(&dir).expect("restored");
    assert_eq!(again.id, first.id);
    assert_eq!(again.source, InstallationSource::RestoredStore);
    assert_eq!(
        load_or_create(&dir).unwrap().source,
        InstallationSource::Stored
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_malformed_copy_is_refused_without_naming_a_path() {
    let dir = data_dir("malformed");
    std::fs::write(file(&dir), "{\"installation_id\":\"not-a-uuid\"}").unwrap();
    let err = load_or_create(&dir).expect_err("malformed");
    assert!(
        matches!(err, InstallationError::Malformed { .. }),
        "{err:?}"
    );
    let text = err.to_string();
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
    assert!(!text.contains("not-a-uuid"), "{text}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn copies_that_disagree_are_refused() {
    let a = data_dir("disagree-a");
    let b = data_dir("disagree-b");
    load_or_create(&a).unwrap();
    load_or_create(&b).unwrap();
    std::fs::copy(file(&b), file(&a)).unwrap();
    let err = load_or_create(&a).expect_err("disagree");
    assert!(matches!(err, InstallationError::Mismatch), "{err:?}");
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}

#[cfg(unix)]
#[test]
fn the_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir("mode");
    load_or_create(&dir).unwrap();
    let mode = std::fs::metadata(file(&dir)).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    std::fs::remove_dir_all(dir).unwrap();
}
