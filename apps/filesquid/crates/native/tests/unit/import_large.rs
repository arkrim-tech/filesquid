//! Bounded memory of the streamed import (Task DE-1.9; DE-S-019, OCFL-S-026, NFR-003): a file
//! of 600 MiB is imported through `ChangeModel::stage_file`, read in chunks of at most 8 MiB,
//! and the process's peak resident memory grows by far less than the file. The test runs
//! alone in its own binary, so no other test's allocations count; the peak is read from
//! `/proc/self/status` (`VmHWM`) on Linux, and elsewhere the import runs without the check.

use filesquid_api::canonical::{read_file_original, OwnerReadable};
use filesquid_api::journal::MemoryJournal;
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::EventBus;
use filesquid_core::{FileId, LibraryId};
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::file::{import_file, FileImport};
use filesquid_native::import::host::Build;
use filesquid_native::{open_fs_library, HostFiles};
use filesquid_ocfl::storage::FsBackend;
use std::path::Path;
use std::sync::Arc;

/// The library the test imports into.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The size of the imported file: 600 MiB.
const FILE_BYTES: u64 = 600 * 1024 * 1024;

/// The most the peak resident memory may grow while the file is imported: 256 MiB, under
/// half the file (an in-memory read alone would add all of it).
const MAX_GROWTH_BYTES: u64 = 256 * 1024 * 1024;

/// The process's peak resident memory in bytes, where the platform reports it.
fn peak_resident() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// The SHA-256 of `length` zero bytes, hashed in 8 MiB steps. Pure.
fn zeros_sha256(length: u64) -> String {
    use sha2::Digest as _;
    let chunk = vec![0u8; 8 * 1024 * 1024];
    let mut hasher = sha2::Sha256::new();
    let mut left = length;
    for _ in 0..=length / chunk.len() as u64 {
        let take = left.min(chunk.len() as u64) as usize;
        hasher.update(&chunk[..take]);
        left -= take as u64;
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An empty library at `<dir>/root`, served by a new file service, and its storage root.
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

#[tokio::test]
async fn a_600_mib_file_is_imported_within_bounded_memory() {
    let dir = std::env::temp_dir().join(format!("fs-native-import-large-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("source")).expect("dir");
    let (files, backend) = library(&dir).await;
    let path = dir.join("source").join("big.raw");
    let file = std::fs::File::create(&path).expect("create");
    file.set_len(FILE_BYTES).expect("sparse");
    let build = Build {
        version: "0.1.0".into(),
        build: "test".into(),
        distribution: "filesquid".into(),
    };
    let request = FileImport {
        library_id: LIBRARY,
        file_id: "0b1c2d3e-4f5a-4b6c-8d7e-9f0a1b2c3d4e",
        path: &path,
        import_root: path.parent().expect("folder"),
        method: ImportMethod::FilePicker,
        job_id: Some("job-1".into()),
        operation_id: Some("op-1".into()),
        build: &build,
        actor: "owner",
    };
    let before = peak_resident();
    let imported = import_file(&files, &request).await;
    let after = peak_resident();
    let original = match &imported {
        Ok(view) => match FileId::new(&view.file_id) {
            Ok(file_id) => read_file_original(&backend, &OwnerReadable, &file_id)
                .await
                .ok(),
            Err(_) => None,
        },
        Err(_) => None,
    };
    std::fs::remove_dir_all(&dir).expect("scratch removed");
    imported.expect("imported, with no too_large_for_this_build refusal");
    let original = original.flatten().expect("the original and its record");
    let entry = &original.record.imports()[0];
    assert_eq!(entry.acquisition.size, FILE_BYTES);
    assert_eq!(entry.source.verified_size, FILE_BYTES);
    assert_eq!(entry.acquisition.sha256, zeros_sha256(FILE_BYTES));
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        assert!(
            growth < MAX_GROWTH_BYTES,
            "peak resident memory grew by {growth} bytes for a {FILE_BYTES}-byte file"
        );
    }
}
