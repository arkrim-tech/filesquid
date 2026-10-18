//! The `sidecar` store keeps `<file>.metadata.md` beside a plain file.

use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::record::Record;
use filesquid_metadata::store::sidecar::SidecarStore;
use filesquid_metadata::store::{MetadataStore, StoreError};
use filesquid_metadata::MetadataNamespace;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("filesquid-sidecar-{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn titled(title: &str) -> Record {
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", title)
        .unwrap();
    record
}

#[tokio::test]
async fn save_then_load_round_trips_beside_the_file() {
    let dir = scratch("roundtrip");
    std::fs::create_dir_all(dir.join("photos")).unwrap();
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "photos/harbour.jpg").unwrap();
    store.save(&file, &titled("Harbour")).await.unwrap();
    assert!(dir.join("photos/harbour.jpg.metadata.md").is_file());
    assert!(!dir.join("photos/harbour.jpg.metadata.md.tmp").exists());
    let loaded = store.load(&file).await.unwrap();
    assert_eq!(loaded.get("dcterms:title").unwrap().value, "Harbour");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn a_file_without_a_sidecar_loads_an_empty_record() {
    let dir = scratch("missing");
    let store = SidecarStore::new(&dir);
    std::fs::write(dir.join("none.jpg"), b"pixels").unwrap();
    let file = FileRef::new("", "none.jpg").unwrap();
    assert!(store.load(&file).await.unwrap().statements.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn save_replaces_the_earlier_record() {
    let dir = scratch("replace");
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "a.jpg").unwrap();
    store.save(&file, &titled("One")).await.unwrap();
    store.save(&file, &titled("Two")).await.unwrap();
    let loaded = store.load(&file).await.unwrap();
    assert_eq!(loaded.get("dcterms:title").unwrap().value, "Two");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn a_git_working_tree_is_refused() {
    let dir = scratch("git");
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "a.jpg").unwrap();
    assert!(matches!(store.load(&file).await, Err(StoreError::Git)));
    assert!(matches!(
        store.save(&file, &titled("x")).await,
        Err(StoreError::Git)
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn a_missing_parent_directory_is_a_backend_error() {
    let dir = scratch("noparent");
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "absent/a.jpg").unwrap();
    assert!(matches!(
        store.save(&file, &titled("x")).await,
        Err(StoreError::Backend(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_directory_cannot_leave_the_root() {
    let dir = scratch("symlink");
    let outside = scratch("symlink-outside");
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("photos")).unwrap();
    let store = SidecarStore::new(&root);
    let file = FileRef::new("", "photos/a.jpg").unwrap();
    assert!(matches!(
        store.save(&file, &titled("x")).await,
        Err(StoreError::InvalidPath)
    ));
    assert!(!outside.join("a.jpg.metadata.md").exists());
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_dir_all(&outside).unwrap();
}

#[tokio::test]
async fn a_sidecar_that_is_not_utf8_is_an_error_not_a_lossy_record() {
    let dir = scratch("utf8");
    std::fs::write(dir.join("a.jpg.metadata.md"), [0xff, 0xfe, 0x00]).unwrap();
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "a.jpg").unwrap();
    assert!(matches!(
        store.load(&file).await,
        Err(StoreError::Backend(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}
