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

/// Concurrent launches over one data directory agree on a single installation ID: the
/// store's immediate transaction serialises the read, both writes and any restore.
#[test]
fn concurrent_launches_agree_on_one_installation() {
    let dir = std::env::temp_dir().join(format!("fs-installation-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let dir = dir.clone();
            std::thread::spawn(move || load_or_create(&dir).map(|i| i.id))
        })
        .collect();
    let ids: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread").expect("launch"))
        .collect();
    assert!(ids.windows(2).all(|w| w[0] == w[1]), "{ids:?}");
    let again = load_or_create(&dir).expect("next launch");
    assert_eq!(again.id, ids[0]);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// The file copy of `dir`, parsed.
fn file_json(dir: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(file(dir)).expect("installation.json");
    serde_json::from_str(&text).expect("installation.json is JSON")
}

/// DE-S-012: the file is `{"schema": 1, "installation_id": "<uuid>", "created": "<RFC 3339
/// UTC>"}`, and a restart leaves it as it was.
#[test]
fn the_file_holds_schema_1_the_uuid_and_its_creation_time() {
    let dir = data_dir("schema");
    let first = load_or_create(&dir).expect("first start");
    let json = file_json(&dir);
    assert_eq!(json["schema"], 1, "{json}");
    assert_eq!(json["installation_id"], first.id.as_str(), "{json}");
    let created = json["created"].as_str().expect("created");
    assert!(created.ends_with('Z'), "UTC: {created}");
    assert!(
        filesquid_api::journal::parse_rfc3339_ms(created).is_some(),
        "RFC 3339: {created}"
    );
    load_or_create(&dir).expect("restart");
    assert_eq!(file_json(&dir), json, "a restart rewrites nothing");
    std::fs::remove_dir_all(dir).unwrap();
}

/// DE-S-012: a copy restored from the other keeps the surviving copy's creation time, so the
/// installation is not reported as created again.
#[test]
fn a_restored_copy_keeps_the_creation_time() {
    let dir = data_dir("created-kept");
    load_or_create(&dir).expect("first start");
    let json = file_json(&dir);
    std::fs::remove_file(file(&dir)).unwrap();
    load_or_create(&dir).expect("file restored from the store");
    assert_eq!(file_json(&dir), json, "the store kept the creation time");
    for name in [
        "filesquid.sqlite",
        "filesquid.sqlite-wal",
        "filesquid.sqlite-shm",
    ] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    load_or_create(&dir).expect("store restored from the file");
    std::fs::remove_file(file(&dir)).unwrap();
    load_or_create(&dir).expect("file restored from the restored store");
    assert_eq!(file_json(&dir), json, "the restored store kept it too");
    std::fs::remove_dir_all(dir).unwrap();
}

/// DE-S-012: a file of another schema is malformed, refused naming no path.
#[test]
fn a_file_of_another_schema_is_refused_without_naming_a_path() {
    let dir = data_dir("other-schema");
    let uuid = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
    let text = format!(
        "{{\"schema\":2,\"installation_id\":\"{uuid}\",\"created\":\"2026-10-31T00:00:00.000Z\"}}"
    );
    std::fs::write(file(&dir), text).unwrap();
    let err = load_or_create(&dir).expect_err("another schema");
    assert!(
        matches!(err, InstallationError::Malformed { copy: "file" }),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(
        !message.contains(dir.to_string_lossy().as_ref()),
        "{message}"
    );
    assert!(!message.contains(uuid), "{message}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// A file an earlier build wrote (no `schema`, no `created`) is still read as that
/// installation, so an upgrade never mints a second one.
#[test]
fn a_file_from_an_earlier_build_is_still_read() {
    let dir = data_dir("earlier-build");
    let uuid = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
    std::fs::write(file(&dir), format!("{{\"installation_id\":\"{uuid}\"}}\n")).unwrap();
    let found = load_or_create(&dir).expect("read");
    assert_eq!(found.id.as_str(), uuid);
    assert_eq!(found.source, InstallationSource::RestoredStore);
    std::fs::remove_dir_all(dir).unwrap();
}
