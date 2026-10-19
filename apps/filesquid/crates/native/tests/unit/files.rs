//! Unit tests for the file service the hosts install (Task API-F1.3): one install per API
//! state, libraries opened over local OCFL roots with their leases and staging areas in the
//! data directory (never inside the root), and a write through the API's change model.

use filesquid_api::files::{FileOperations, ListingQuery, UploadQuery};
use filesquid_api::journal::MemoryJournal;
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::{ApiState, EventBus};
use filesquid_core::LibraryId;
use filesquid_native::{install_file_service, open_fs_library, HostFiles};
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The library every test opens.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The object every test names.
const OBJECT: &str = "urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11";

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "filesquid-native-files-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove the test directory: {err}");
        }
    }
}

fn library() -> LibraryId {
    LibraryId::new(LIBRARY).expect("library id")
}

/// An OCFL storage root at `root` whose object holds `a.txt` at v1.
async fn ocfl_root(root: &Path, scratch: &Path) {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store,
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let backend = FsBackend::new(root, scratch.join("root-parts")).expect("root");
    LocalLibraryService::new(writer.sender(), Arc::new(MemoryRepairLedger::new()))
        .create_root("owner", &library(), &backend)
        .await
        .expect("create root");
    writer.shutdown();
    let staging = FsBackend::new(scratch.join("staging"), scratch.join("parts")).expect("staging");
    let lease_root =
        FsBackend::new(scratch.join("leases"), scratch.join("lease-parts")).expect("leases");
    let leases =
        LeaseStore::new(lease_root, "leases", SystemClock, LeaseConfig::default()).expect("leases");
    let area = StagingArea::new(&staging, "fixture").expect("area");
    let model = ChangeModel::new(&backend, &leases, "fixture", "objects/a", OBJECT, area);
    model.stage_bytes("a.txt", b"a").await.expect("stage");
    model
        .commit(VersionMeta::new("2026-10-18T00:00:00Z"))
        .await
        .expect("commit");
}

fn listing() -> ListingQuery {
    ListingQuery {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        version: None,
        prefix: None,
        cursor: None,
        limit: None,
    }
}

#[test]
fn a_state_takes_one_file_service() {
    let state = ApiState::new();
    install_file_service(&state).expect("first install");
    assert!(install_file_service(&state).is_err());
    assert!(state.files().is_some());
}

#[tokio::test]
async fn an_opened_library_is_served_and_keeps_its_state_outside_the_root() {
    let temp = TempDir::new("open");
    let (root, data, scratch) = (
        temp.0.join("root"),
        temp.0.join("data"),
        temp.0.join("scratch"),
    );
    ocfl_root(&root, &scratch).await;
    let files: HostFiles = HostFiles::new();
    assert!(
        files.listing(listing()).await.is_err(),
        "nothing is open yet"
    );
    assert!(open_fs_library(&files, &library(), &root, &data).expect("open"));
    assert!(!open_fs_library(&files, &library(), &root, &data).expect("again"));
    let page = files.listing(listing()).await.expect("listing");
    assert_eq!(page.entries.len(), 1);
    let library_dir = data
        .join("files")
        .join(filesquid_native::files::library_dir_name(&library()));
    assert!(library_dir.join("leases").is_dir());
    assert!(library_dir.join("staging").is_dir());
    assert!(!root.join("leases").exists() && !root.join("staging").exists());
}

#[tokio::test]
async fn a_write_through_the_host_service_publishes_one_version() {
    let temp = TempDir::new("write");
    let (root, data, scratch) = (
        temp.0.join("root"),
        temp.0.join("data"),
        temp.0.join("scratch"),
    );
    ocfl_root(&root, &scratch).await;
    let files = HostFiles::new();
    open_fs_library(&files, &library(), &root, &data).expect("open");
    let upload = UploadQuery {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        path: "b.txt".into(),
        message: None,
    };
    let commit = FileOperations::upload(&files, upload, b"bee".to_vec().into(), "owner".into())
        .await
        .expect("upload");
    assert_eq!(commit.version, "v2");
    let stored = root.join("objects/a/v2/content/b.txt");
    assert_eq!(std::fs::read(stored).expect("read back"), b"bee");
}
