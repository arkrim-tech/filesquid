//! Unit tests for the import of one source file (Task DE-1.2; F-002, SRS-F-002, DE-S-008,
//! MD-S-013, ADR 038 Decision 1): the original is stored under `<file-uuid>.<ext>` with its
//! metadata record in the same version of its own object; the record keeps the original
//! name, the facts read before the bytes and the citation of the bytes; a size that does
//! not match the bytes and a file too large to read are refused with nothing published.

use filesquid_api::canonical::{read_file_original, OwnerReadable};
use filesquid_api::files::SourceImport;
use filesquid_api::journal::MemoryJournal;
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::EventBus;
use filesquid_core::{FileId, LibraryId};
use filesquid_metadata::fact::Fact;
use filesquid_metadata::import::{ByteStorage, ImportMethod};
use filesquid_native::import::facts::gather_source;
use filesquid_native::import::file::{import_file, FileImport, ImportFailure};
use filesquid_native::import::host::{gather_host, Build};
use filesquid_native::{open_fs_library, HostFiles};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/// The library every test imports into.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "filesquid-native-import-file-{name}-{}",
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

/// This test build.
fn build() -> Build {
    Build {
        version: "0.1.0".into(),
        build: "test".into(),
        distribution: "filesquid".into(),
    }
}

/// An empty library storage root at `<dir>/root`, served by a new file service.
async fn library(dir: &Path) -> (HostFiles, FsBackend) {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store,
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let root = dir.join("root");
    std::fs::create_dir_all(&root).expect("root");
    let backend = FsBackend::new(&root, dir.join("root-parts")).expect("backend");
    let id = LibraryId::new(LIBRARY).expect("library");
    LocalLibraryService::new(writer.sender(), Arc::new(MemoryRepairLedger::new()))
        .create_root("owner", &id, &backend)
        .await
        .expect("create root");
    writer.shutdown();
    let files = HostFiles::new();
    open_fs_library(&files, &id, &root, &dir.join("data")).expect("opened");
    (files, backend)
}

/// A source file `name` in `<dir>/source` holding `bytes`, modified at a known time.
fn source(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let folder = dir.join("source");
    std::fs::create_dir_all(&folder).expect("source dir");
    let path = folder.join(name);
    std::fs::write(&path, bytes).expect("write");
    let modified = UNIX_EPOCH + Duration::new(1_792_490_400, 5);
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_modified(modified).expect("time");
    path
}

/// The import of `path` from its folder.
fn request<'a>(path: &'a Path, build: &'a Build) -> FileImport<'a> {
    FileImport {
        library_id: LIBRARY,
        path,
        import_root: path.parent().expect("folder"),
        method: ImportMethod::Folder,
        job_id: Some("job-1".into()),
        operation_id: Some("op-1".into()),
        build,
        actor: "owner",
    }
}

#[tokio::test]
async fn an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version() {
    let dir = TempDir::new("one");
    let (files, backend) = library(&dir.0).await;
    let build = build();
    let path = source(&dir.0, "Holiday photo.JPG", b"jpeg bytes");
    let view = import_file(&files, &request(&path, &build))
        .await
        .expect("imported");
    assert_eq!(view.version, "v1");
    assert_eq!(view.object_id, format!("urn:uuid:{}", view.file_id));
    let file_id = FileId::new(&view.file_id).expect("id");
    let original = read_file_original(&backend, &OwnerReadable, &file_id)
        .await
        .expect("read")
        .expect("original");
    assert_eq!(original.path, format!("original/{}.JPG", view.file_id));
    assert!(
        !original.path.contains("Holiday"),
        "no owner name in the path"
    );
    let record = original.record;
    assert_eq!(record.imports().len(), 1);
    let entry = &record.imports()[0];
    assert_eq!(entry.acquisition.method, ImportMethod::Folder);
    assert_eq!(entry.acquisition.storage, ByteStorage::Copied);
    assert_eq!(entry.acquisition.object_version, "v1");
    assert_eq!(entry.acquisition.size, 10);
    assert_eq!(entry.acquisition.job_id, Fact::value("job-1".to_string()));
    assert_eq!(entry.source.verified_size, 10);
    assert_eq!(entry.source.reported_size, Fact::value(10));
    assert_eq!(entry.source.original_name.as_bytes(), b"Holiday photo.JPG");
    assert_eq!(
        entry.source.modified_time,
        Fact::value("2026-10-20T10:00:00.000000005Z".to_string())
    );
    assert_eq!(entry.host.filesquid_version, "0.1.0");
    let markdown = record.to_markdown();
    assert!(markdown.contains("Holiday photo.JPG"), "{markdown}");
}

#[tokio::test]
async fn a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published() {
    let dir = TempDir::new("mismatch");
    let (files, backend) = library(&dir.0).await;
    let path = source(&dir.0, "a.txt", b"twelve bytes");
    let mut gathered = gather_source(&path, path.parent().expect("folder")).expect("facts");
    gathered.file.reported_size = Fact::value(99);
    let import = SourceImport {
        library_id: LIBRARY.into(),
        file_id: None,
        method: ImportMethod::FilePicker,
        job_id: None,
        operation_id: None,
        started_at: "2026-10-20T10:00:00Z".into(),
        source: gathered.file,
        volume: gathered.volume,
        attributes: gathered.attributes,
        host: gather_host(&build()),
    };
    let err = files
        .import_original(import, b"twelve bytes", "owner")
        .await
        .expect_err("refused");
    assert!(
        err.message().starts_with("size_mismatch"),
        "{}",
        err.message()
    );
    let objects = backend.root().join("files");
    assert!(!objects.exists(), "nothing published");
}

#[tokio::test]
async fn a_file_above_the_in_memory_bound_is_refused_before_it_is_read() {
    let dir = TempDir::new("large");
    let (files, _) = library(&dir.0).await;
    let path = source(&dir.0, "huge.bin", b"");
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_len(513 * 1024 * 1024).expect("sparse");
    let build = build();
    let failure = import_file(&files, &request(&path, &build))
        .await
        .expect_err("refused");
    assert_eq!(failure, ImportFailure::TooLarge);
    assert_eq!(failure.error_code(), "too_large_for_this_build");
    assert!(failure.retryable(), "a later build may import it");
    assert!(!failure.retry_now(), "no attempt within this job can");
}

#[tokio::test]
async fn a_missing_source_is_refused_without_retry() {
    let dir = TempDir::new("missing");
    let (files, _) = library(&dir.0).await;
    let build = build();
    let path = dir.0.join("gone.txt");
    let mut missing = request(&path, &build);
    missing.import_root = &dir.0;
    let failure = import_file(&files, &missing).await.expect_err("refused");
    assert_eq!(failure.error_code(), "source_missing");
    assert!(!failure.retryable());
    assert!(!failure.retry_now());
}
