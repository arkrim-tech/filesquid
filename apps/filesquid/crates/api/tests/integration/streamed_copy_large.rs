//! Bounded memory of the streamed copy (Task API-F2.26; API-S-011, NFR-003, OCFL-S-026): a
//! file of 600 MiB is copied through the router on file-system backends, and the process's
//! peak resident memory grows by far less than the file while it is copied. The test runs
//! alone in its own binary, so no other test's allocations count; the peak is read from
//! `/proc/self/status` (`VmHWM`) on Linux, and elsewhere the copy runs without that check.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/copy_fixture.rs"]
mod copy_fixture;

use axum::http::StatusCode;
use copy_fixture::{copy, import_source, staged_files, stored, Fill, MAX_READ, MAX_WRITE, MIB};
use files_fixture::FilesRig;

/// The size of the copied file: 600 MiB, above 512 MiB.
const FILE_BYTES: u64 = 600 * MIB;

/// The most the peak resident memory may grow while the file is copied: 256 MiB, under half
/// the file (holding it in memory alone would add all of it).
const MAX_GROWTH_BYTES: u64 = 256 * MIB;

/// The process's peak resident memory in bytes, where the platform reports it.
fn peak_resident() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[tokio::test]
async fn a_copy_above_512_mib_keeps_its_memory_bounded() {
    let rig = FilesRig::new().await;
    let source = import_source(&rig, "archive.bin", FILE_BYTES, Fill::Zeros).await;
    let before = peak_resident();
    let answer = copy(&rig, &source.file_id).await;
    let after = peak_resident();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let copy_id = answer.result()["file_id"].as_str().expect("id").to_string();
    let copied = stored(&rig, &copy_id).await;
    assert_eq!(copied.sha256, source.sha256, "the source's SHA-256");
    assert_eq!(copied.size, FILE_BYTES, "the source's size");
    assert!(rig.root.largest_read() <= MAX_READ);
    assert!(rig.api_staging.largest_write() <= MAX_WRITE);
    assert_eq!(staged_files(&rig), Vec::<std::path::PathBuf>::new());
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        assert!(
            growth <= MAX_GROWTH_BYTES,
            "the peak memory grew by {growth} bytes while {FILE_BYTES} were copied"
        );
    }
}
