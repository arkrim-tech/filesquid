//! Streamed single-file stage (IO-1.7; OCFL-S-026, OCFL-S-014, OCFL-S-015,
//! OCFL-S-018; PRD F-051, NFR-003).
//!
//! `ChangeModel::stage_file` stages one logical file from a file of any
//! backend in bounded chunks and answers its sha256 digest. Every scenario
//! reads its source from the in-memory backend and from `FsBackend`; the
//! write-order scenario also commits to both.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{
    write_whole, FsBackend, MemoryBackend, StorageBackend, MAX_CHUNK_BYTES,
};

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/change_rig.rs"]
mod change;
#[path = "../support/counting_backend.rs"]
mod counting;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/write_rig.rs"]
mod rig;

use change::{commit_files, meta, state_of};
use counting::CountingBackend;
use fault::{FaultBackend, Log};
use rig::{key, ready, Leases, Rig, OBJECT_ID, ROOT};

/// The staging prefix of the models these tests build.
const STAGING: &str = "changes/writer-a";

/// The key every source file is read from.
const SOURCE_KEY: &str = "incoming/photo.raw";

/// Scratch directory for one filesystem backend, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh `FsBackend` on its own scratch directory.
fn fs_backend(tag: &str) -> (FsBackend, Scratch) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-stage-file-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// Run `$scenario` with a source on the in-memory backend and on
/// `FsBackend`, each wrapped in a [`CountingBackend`].
macro_rules! on_both_sources {
    ($scenario:ident) => {{
        $scenario(&CountingBackend::new(MemoryBackend::new()));
        let (backend, _scratch) = fs_backend("source");
        $scenario(&CountingBackend::new(backend));
    }};
}

/// The staging backend of a model: in memory, its writes logged.
type Staging = FaultBackend<MemoryBackend>;

/// A staging backend whose writes go to a fresh log.
fn staging() -> (Staging, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    (
        FaultBackend::new(MemoryBackend::new(), "staging", log.clone()),
        log,
    )
}

/// A change model for `writer-a` on `rig`, staging on `staging`.
fn model<'r, B: StorageBackend>(
    rig: &'r Rig<B>,
    staging: &'r Staging,
) -> ChangeModel<'r, FaultBackend<B>, Leases, Staging> {
    let area = StagingArea::new(staging, STAGING).expect("prefix");
    ChangeModel::new(
        &rig.storage,
        &*rig.leases,
        "writer-a",
        ROOT,
        OBJECT_ID,
        area,
    )
}

/// The staging key of the blob of `digest`.
fn blob(digest: &str) -> String {
    format!("{STAGING}/blobs/{digest}")
}

/// The sha256 digest of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    DigestAlgorithm::Sha256.digest_hex(bytes)
}

/// The bytes of `raw` on `backend`, or `None` when absent.
fn stored<B: StorageBackend>(backend: &B, raw: &str) -> Option<Vec<u8>> {
    let k = key(raw);
    if !ready(backend.exists(&k)).expect("exists") {
        return None;
    }
    let bytes = filesquid_ocfl::storage::read_verified(backend, &k, MAX_CHUNK_BYTES);
    Some(ready(bytes).expect("read"))
}

/// Blob writes the staging log recorded (index writes left out).
fn blob_writes(log: &Log) -> Vec<String> {
    let log = log.lock().expect("lock");
    log.iter()
        .filter(|w| w.contains("/blobs/"))
        .cloned()
        .collect()
}

/// Bytes larger than two chunks, not a whole number of chunks.
fn large_bytes() -> Vec<u8> {
    let len = usize::try_from(2 * MAX_CHUNK_BYTES + 123).expect("fits");
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// The chunk reads of one whole pass over `len` bytes.
fn chunks(len: usize) -> usize {
    (len as u64).div_ceil(MAX_CHUNK_BYTES).max(1) as usize
}

/// Put `bytes` at the source key of `source`.
fn put_source<B: StorageBackend>(source: &CountingBackend<B>, bytes: &[u8]) {
    ready(write_whole(source.inner(), &key(SOURCE_KEY), bytes)).expect("source");
    source.reset_counts();
}

fn answers_the_digest_and_stages_what_stage_bytes_would<B: StorageBackend>(
    source: &CountingBackend<B>,
) {
    let rig = Rig::new(MemoryBackend::new());
    put_source(source, b"pixels");
    let (streamed, _) = staging();
    let digest = ready(model(&rig, &streamed).stage_file("photo.raw", source, SOURCE_KEY))
        .expect("stage_file");
    assert_eq!(digest, sha256(b"pixels"));
    let (from_bytes, _) = staging();
    ready(model(&rig, &from_bytes).stage_bytes("photo.raw", b"pixels")).expect("stage_bytes");
    let blob = blob(&digest);
    assert_eq!(stored(&streamed, &blob), Some(b"pixels".to_vec()));
    assert_eq!(stored(&streamed, &blob), stored(&from_bytes, &blob));
    let index = format!("{STAGING}/index.json");
    assert_eq!(stored(&streamed, &index), stored(&from_bytes, &index));
}

#[test]
fn stage_file_answers_the_digest_and_stages_the_blob_stage_bytes_would() {
    on_both_sources!(answers_the_digest_and_stages_what_stage_bytes_would);
}

fn reads_in_bounded_chunks<B: StorageBackend>(source: &CountingBackend<B>) {
    let rig = Rig::new(MemoryBackend::new());
    let bytes = large_bytes();
    put_source(source, &bytes);
    let (staging, _) = staging();
    let m = model(&rig, &staging);
    let digest = ready(m.stage_file("big.raw", source, SOURCE_KEY)).expect("stage_file");
    assert_eq!(digest, sha256(&bytes));
    assert!(source.largest_read() <= MAX_CHUNK_BYTES, "bounded reads");
    let pass = chunks(bytes.len());
    assert!(pass >= 3, "the file spans more than one chunk");
    assert_eq!(
        source.reads(),
        2 * pass,
        "one hashing pass and one copying pass"
    );
    assert_eq!(stored(&staging, &blob(&digest)), Some(bytes));
    let receipt = ready(m.commit(meta(&rig, "big"))).expect("commit");
    assert_eq!(
        state_of(&rig, &receipt.version.to_string())["big.raw"],
        digest
    );
}

#[test]
fn a_file_larger_than_one_chunk_is_read_in_bounded_chunks() {
    on_both_sources!(reads_in_bounded_chunks);
}

fn manifest_bytes_are_not_copied<B: StorageBackend>(source: &CountingBackend<B>) {
    let rig = Rig::new(MemoryBackend::new());
    commit_files(
        &change::model(&rig, "seed"),
        &rig,
        &[("a.txt", b"alpha")],
        "v1",
    )
    .expect("v1");
    put_source(source, b"alpha");
    let (staging, log) = staging();
    let m = model(&rig, &staging);
    let digest = ready(m.stage_file("copy-of-a.txt", source, SOURCE_KEY)).expect("stage_file");
    assert_eq!(digest, sha256(b"alpha"));
    assert_eq!(blob_writes(&log), Vec::<String>::new(), "nothing copied");
    assert_eq!(source.reads(), 1, "hashed once, never copied");
    let receipt = ready(m.commit(meta(&rig, "copy"))).expect("commit");
    assert_eq!(receipt.written, Vec::<String>::new(), "no new content");
    let state = state_of(&rig, "v2");
    assert_eq!(state["copy-of-a.txt"], state["a.txt"]);
}

#[test]
fn a_file_whose_digest_the_manifest_holds_is_not_copied() {
    on_both_sources!(manifest_bytes_are_not_copied);
}

fn a_held_blob_is_reused<B: StorageBackend>(source: &CountingBackend<B>) {
    let rig = Rig::new(MemoryBackend::new());
    put_source(source, b"bravo");
    let digest = sha256(b"bravo");
    let (staging, log) = staging();
    ready(staging.inner().put(&key(&blob(&digest)), b"bravo")).expect("pre-fill");
    let m = model(&rig, &staging);
    let answered = ready(m.stage_file("b.txt", source, SOURCE_KEY)).expect("stage_file");
    assert_eq!(answered, digest);
    assert_eq!(
        blob_writes(&log),
        Vec::<String>::new(),
        "not copied a second time"
    );
    assert_eq!(source.reads(), 1, "hashed once, never copied");
    let receipt = ready(m.commit(meta(&rig, "b"))).expect("commit");
    assert_eq!(
        state_of(&rig, &receipt.version.to_string())["b.txt"],
        digest
    );
    rig.assert_valid();
}

#[test]
fn a_blob_the_staging_area_holds_is_not_copied_again() {
    on_both_sources!(a_held_blob_is_reused);
}

fn a_corrupt_blob_is_replaced<B: StorageBackend>(source: &CountingBackend<B>) {
    let rig = Rig::new(MemoryBackend::new());
    put_source(source, b"charlie");
    let digest = sha256(b"charlie");
    let (staging, log) = staging();
    ready(staging.inner().put(&key(&blob(&digest)), b"stale bytes")).expect("pre-fill");
    let m = model(&rig, &staging);
    let answered = ready(m.stage_file("c.txt", source, SOURCE_KEY)).expect("stage_file");
    assert_eq!(answered, digest);
    assert_eq!(stored(&staging, &blob(&digest)), Some(b"charlie".to_vec()));
    assert_eq!(
        blob_writes(&log).len(),
        1,
        "replaced once: {:?}",
        blob_writes(&log)
    );
    let receipt = ready(m.commit(meta(&rig, "c"))).expect("commit");
    let version = receipt.version.to_string();
    assert_eq!(state_of(&rig, &version)["c.txt"], digest);
    assert_eq!(
        rig.bytes(&format!("obj/{version}/content/c.txt")),
        b"charlie"
    );
    rig.assert_valid();
}

#[test]
fn a_corrupt_or_stale_staged_blob_is_replaced_with_the_source_bytes() {
    on_both_sources!(a_corrupt_blob_is_replaced);
}

fn a_changed_source_is_refused<B: StorageBackend>(source: &CountingBackend<B>) {
    let rig = Rig::new(MemoryBackend::new());
    let bytes = large_bytes();
    put_source(source, &bytes);
    let mut changed = bytes.clone();
    changed[0] ^= 0xff;
    source.swap_after(chunks(bytes.len()), &key(SOURCE_KEY), changed.clone());
    let (staging, _) = staging();
    let m = model(&rig, &staging);
    let err = ready(m.stage_file("big.raw", source, SOURCE_KEY)).expect_err("refused");
    assert!(matches!(err, ChangeError::SourceChanged { .. }), "{err:?}");
    assert_eq!(
        stored(&staging, &blob(&sha256(&bytes))),
        None,
        "no partial blob"
    );
    assert_eq!(stored(&staging, &blob(&sha256(&changed))), None);
    assert_eq!(
        stored(&staging, &format!("{STAGING}/index.json")),
        None,
        "nothing staged"
    );
}

#[test]
fn a_file_that_changes_between_the_two_reads_is_refused() {
    on_both_sources!(a_changed_source_is_refused);
}

#[test]
fn an_invalid_logical_path_is_refused_before_the_source_is_read() {
    let rig = Rig::new(MemoryBackend::new());
    let source = CountingBackend::new(MemoryBackend::new());
    put_source(&source, b"pixels");
    let (staging, log) = staging();
    let err = ready(model(&rig, &staging).stage_file("../escape", &source, SOURCE_KEY))
        .expect_err("refused");
    assert!(matches!(err, ChangeError::InvalidPath { .. }), "{err:?}");
    assert_eq!(source.reads(), 0);
    assert!(log.lock().expect("lock").is_empty());
}

fn streamed_commit_follows_the_write_order<B: StorageBackend>(rig: &Rig<B>) {
    commit_files(
        &change::model(rig, "seed"),
        rig,
        &[("a.txt", b"alpha")],
        "v1",
    )
    .expect("v1");
    let source = CountingBackend::new(MemoryBackend::new());
    let bytes = large_bytes();
    put_source(&source, &bytes);
    let (staging, _) = staging();
    let m = model(rig, &staging);
    ready(m.stage_file("big.raw", &source, SOURCE_KEY)).expect("stage_file");
    rig.clear_log();
    ready(m.commit(meta(rig, "v2"))).expect("commit");
    let lease = rig.leases.lease_key(OBJECT_ID).expect("lease key");
    let reserve = format!("leases put_if {lease}");
    let writes = rig.writes();
    assert_eq!(
        writes.first(),
        Some(&reserve),
        "reserved before any vN byte"
    );
    assert_eq!(writes.last(), Some(&reserve), "released last");
    let storage: Vec<&str> = writes
        .iter()
        .filter_map(|w| w.strip_prefix("storage "))
        .collect();
    assert_eq!(
        storage,
        [
            "complete obj/v2/content/big.raw",
            "put_if obj/v2/inventory.json",
            "put_if obj/v2/inventory.json.sha256",
            "put_if obj/inventory.json",
            "put_if obj/inventory.json.sha256",
        ]
    );
    assert_eq!(rig.bytes("obj/v2/content/big.raw"), bytes);
    rig.assert_valid();
}

#[test]
fn the_commit_of_a_streamed_file_keeps_the_write_order() {
    streamed_commit_follows_the_write_order(&Rig::new(MemoryBackend::new()));
    let (backend, _scratch) = fs_backend("storage");
    streamed_commit_follows_the_write_order(&Rig::new(backend));
}
