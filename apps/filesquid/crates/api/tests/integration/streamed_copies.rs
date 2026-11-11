//! Integration tests of Task API-F2.26 (API-S-011, NFR-003, OCFL-S-026) through the router,
//! against a real OCFL storage root and a staging area on the local filesystem: a copy of a
//! file above the former 64 MiB bound streams through bounded ranged reads and writes and has
//! its source's SHA-256 and size; its staged bytes go to the staging area on disk, which is
//! empty afterwards; the library's write lock is held for the commit only, so another write
//! completes while a copy is staging; and a source that moved on, changed or cannot be read
//! fails with nothing committed and nothing left staged. The copy above 512 MiB and its
//! memory bound are in `integration_streamed_copy_large`, which runs alone in its binary.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/copy_fixture.rs"]
mod copy_fixture;

use axum::http::StatusCode;
use copy_fixture::{
    content_file, copy, import_source, object_count, staged_files, stored, Fill, MAX_READ,
    MAX_WRITE, MIB,
};
use files_fixture::{digest, pattern, Answer, FilesRig, LIBRARY};
use filesquid_api::journal::Outcome;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A source above the former copy bound: 65 MiB.
const ABOVE_FORMER_BOUND: u64 = 65 * MIB;

/// A source of more than one ranged read: 9 MiB.
const TWO_READS: u64 = 9 * MIB;

/// The longest a test waits for the other side of a hand-over.
const HAND_OVER: Duration = Duration::from_secs(30);

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// Assert a refused copy of `rig` committed nothing beyond `objects` objects, left nothing
/// staged and was journaled once as failed.
fn assert_nothing_copied(rig: &FilesRig, objects: usize) {
    assert_eq!(object_count(rig), objects, "no object was committed");
    assert_eq!(staged_files(rig), Vec::<std::path::PathBuf>::new());
    let journaled = rig.journaled("file.copy");
    assert_eq!(journaled.len(), 1, "journaled once");
    assert_eq!(journaled[0].outcome, Outcome::Failed);
}

/// Run `change` once, at the next read of `rig`'s root that answers at least 1 MiB: the
/// first chunk of a stored original, which only a copy reads here.
fn at_first_chunk(rig: &FilesRig, change: impl Fn() + Send + Sync + 'static) {
    let root = rig.root.clone();
    let before = root.chunk_reads();
    let done = AtomicBool::new(false);
    rig.root.on_read(Box::new(move || {
        if root.chunk_reads() > before && !done.swap(true, Ordering::SeqCst) {
            change();
        }
    }));
}

#[tokio::test]
async fn a_copy_above_64_mib_streams_in_bounded_reads_and_writes() {
    let rig = FilesRig::new().await;
    let source = import_source(&rig, "film.mov", ABOVE_FORMER_BOUND, Fill::Pattern).await;
    let staged_before = rig.api_staging.written();
    let answer = copy(&rig, &source.file_id).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let result = answer.result();
    let copy_id = result["file_id"].as_str().expect("copy id");
    assert_ne!(copy_id, source.file_id, "a new FileId");
    assert_eq!(result["version"], "v1");
    assert_eq!(result["source"], format!("urn:uuid:{}@v1", source.file_id));
    let copied = stored(&rig, copy_id).await;
    assert_eq!(copied.sha256, source.sha256, "the source's SHA-256");
    assert_eq!(copied.size, ABOVE_FORMER_BOUND, "the source's size");
    assert_eq!(stored(&rig, &source.file_id).await.sha256, source.sha256);
    assert!(
        rig.root.largest_read() <= MAX_READ,
        "a read of {} bytes",
        rig.root.largest_read()
    );
    for backend in [&rig.root, &rig.api_staging] {
        assert!(
            backend.largest_write() <= MAX_WRITE,
            "a write of {} bytes",
            backend.largest_write()
        );
    }
    let staged = rig.api_staging.written() - staged_before;
    assert!(
        staged >= ABOVE_FORMER_BOUND,
        "{staged} bytes were staged on disk"
    );
    assert_eq!(staged_files(&rig), Vec::<std::path::PathBuf>::new());
}

#[tokio::test]
async fn a_small_copy_is_streamed_too_and_cites_its_content() {
    let rig = FilesRig::new().await;
    let source = import_source(&rig, "notes.txt", MIB, Fill::Pattern).await;
    assert_eq!(source.sha256, digest(&pattern(MIB as usize)));
    let answer = copy(&rig, &source.file_id).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let copy_id = answer.result()["file_id"].as_str().expect("id").to_string();
    let copied = stored(&rig, &copy_id).await;
    assert_eq!(copied.sha256, source.sha256);
    assert_eq!(copied.size, MIB);
    let facts = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={copy_id}"
        ))
        .await;
    assert_eq!(facts.status, StatusCode::OK, "{:?}", facts.json());
    let text = facts.json().to_string();
    assert!(
        text.contains(&source.sha256),
        "the record cites the content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_write_completes_while_a_copy_stages_and_the_stale_copy_is_refused() {
    let rig = Arc::new(FilesRig::new().await);
    let source = import_source(&rig, "film.mov", TWO_READS, Fill::Pattern).await;
    let objects = object_count(&rig);
    let (staging, staging_seen) = mpsc::channel::<()>();
    let (renamed, renamed_seen) = mpsc::channel::<()>();
    let hand_over = Mutex::new((staging, renamed_seen));
    at_first_chunk(&rig, move || {
        let (staging, renamed_seen) = &*hand_over.lock().expect("hand-over");
        staging.send(()).expect("the test waits for the stage");
        renamed_seen
            .recv_timeout(HAND_OVER)
            .expect("the rename completes while the copy is staging");
    });
    let copying = {
        let (rig, file_id) = (rig.clone(), source.file_id.clone());
        tokio::spawn(async move { copy(&rig, &file_id).await })
    };
    let waiting = tokio::task::spawn_blocking(move || staging_seen.recv_timeout(HAND_OVER));
    waiting
        .await
        .expect("joined")
        .expect("the copy reached its stage");
    let rename = json!({
        "library_id": LIBRARY, "file_id": source.file_id, "display_name": "Final cut.mov"
    });
    let rename = rig.command("/api/v1/files/renames", rename);
    let answer = tokio::time::timeout(HAND_OVER, rename)
        .await
        .expect("the write lock is free while a copy is staging");
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    renamed.send(()).expect("the copy waits for the rename");
    let refused = copying.await.expect("joined");
    assert_error(&refused, 409, "conflict");
    assert_nothing_copied(&rig, objects);
}

#[tokio::test]
async fn a_source_that_changes_while_it_is_read_fails_with_nothing_staged() {
    let rig = FilesRig::new().await;
    let source = import_source(&rig, "film.mov", TWO_READS, Fill::Pattern).await;
    let objects = object_count(&rig);
    let content = content_file(&rig, TWO_READS);
    at_first_chunk(&rig, move || {
        std::fs::remove_file(&content).expect("remove the stored content");
        std::fs::write(&content, vec![7u8; TWO_READS as usize]).expect("other content");
    });
    let refused = copy(&rig, &source.file_id).await;
    assert_error(&refused, 409, "conflict");
    let message = refused.json()["message"].to_string();
    assert!(message.contains("source_changed"), "{message}");
    assert_nothing_copied(&rig, objects);
}

#[tokio::test]
async fn a_source_that_cannot_be_read_fails_with_nothing_staged() {
    let rig = FilesRig::new().await;
    let source = import_source(&rig, "film.mov", TWO_READS, Fill::Pattern).await;
    let objects = object_count(&rig);
    let content = content_file(&rig, TWO_READS);
    at_first_chunk(&rig, move || {
        std::fs::remove_file(&content).expect("remove the stored content");
    });
    let refused = copy(&rig, &source.file_id).await;
    assert_error(&refused, 409, "conflict");
    let message = refused.json()["message"].to_string();
    assert!(message.contains("source_unreadable"), "{message}");
    assert_nothing_copied(&rig, objects);
}
