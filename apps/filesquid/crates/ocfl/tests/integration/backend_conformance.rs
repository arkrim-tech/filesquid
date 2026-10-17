//! Storage backend conformance (OCFL-S-007). One generic suite runs against
//! the in-memory backend and against the local filesystem backend on a real
//! directory, so both backends obey the same contract.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::storage::{
    ByteRange, ChunkReader, ETag, FsBackend, MemoryBackend, MultipartWriter, Precondition,
    StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES, MAX_LIST_PAGE, MAX_PUT_BYTES,
};

/// A scratch directory under Cargo's per-target temporary directory, removed
/// on drop so every test closes what it opened.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ocfl-backend-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(s: &str) -> StorageKey {
    StorageKey::new(s).expect("valid key")
}

fn fs_backend(label: &str) -> (FsBackend, Scratch) {
    let scratch = Scratch::new(label);
    let backend =
        FsBackend::new(scratch.0.join("root"), scratch.0.join("staging")).expect("fs backend");
    (backend, scratch)
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

async fn read_all<B: StorageBackend>(backend: &B, k: &StorageKey, chunk: u64) -> Vec<u8> {
    let mut reader = ChunkReader::new(backend, k.clone(), chunk).expect("reader");
    let mut out = Vec::new();
    while let Some(bytes) = reader.next_chunk().await.expect("chunk") {
        assert!(bytes.len() as u64 <= chunk, "chunk over bound");
        out.extend_from_slice(&bytes);
    }
    out
}

async fn put_get_head_exists<B: StorageBackend>(b: &B) {
    let k = key("objects/a/inventory.json");
    assert!(!b.exists(&k).await.expect("exists"));
    let meta = b.put(&k, b"hello OCFL").await.expect("put");
    assert_eq!(meta.size, 10);
    assert_eq!(meta.etag, ETag::sha256_of(b"hello OCFL"));
    assert!(b.exists(&k).await.expect("exists"));
    assert_eq!(b.head(&k).await.expect("head"), meta);
    let range = ByteRange::new(0, 1024).expect("range");
    assert_eq!(b.get_range(&k, range).await.expect("get"), b"hello OCFL");
    let mid = ByteRange::new(6, 2).expect("range");
    assert_eq!(b.get_range(&k, mid).await.expect("get"), b"OC");
    let past = ByteRange::new(100, 2).expect("range");
    assert!(b.get_range(&k, past).await.expect("get").is_empty());
}

async fn put_replaces<B: StorageBackend>(b: &B) {
    let k = key("a.txt");
    b.put(&k, b"one").await.expect("put");
    b.put(&k, b"two!").await.expect("put");
    assert_eq!(read_all(b, &k, 2).await, b"two!");
    assert_eq!(b.head(&k).await.expect("head").size, 4);
}

async fn missing_key_is_not_found<B: StorageBackend>(b: &B) {
    let k = key("nope/missing.bin");
    let range = ByteRange::new(0, 1).expect("range");
    for err in [
        b.head(&k).await.expect_err("head"),
        b.get_range(&k, range).await.expect_err("get"),
    ] {
        assert!(matches!(err, StorageError::NotFound { .. }), "{err:?}");
    }
}

async fn compare_and_swap<B: StorageBackend>(b: &B) {
    let k = key("objects/a/inventory.json");
    let first = b
        .put_if(&k, b"v1", &Precondition::IfAbsent)
        .await
        .expect("create");
    let again = b.put_if(&k, b"other", &Precondition::IfAbsent).await;
    assert!(matches!(
        again,
        Err(StorageError::PreconditionFailed { .. })
    ));
    b.put(&k, b"v2").await.expect("competing writer");
    let stale = b
        .put_if(&k, b"v3", &Precondition::IfMatch(first.etag.clone()))
        .await;
    assert!(matches!(
        stale,
        Err(StorageError::PreconditionFailed { .. })
    ));
    assert_eq!(read_all(b, &k, 64).await, b"v2");
    let current = b.head(&k).await.expect("head").etag;
    b.put_if(&k, b"v3", &Precondition::IfMatch(current))
        .await
        .expect("fresh cas");
    assert_eq!(read_all(b, &k, 64).await, b"v3");
    let absent = key("objects/b/inventory.json");
    let missing = b
        .put_if(&absent, b"x", &Precondition::IfMatch(first.etag))
        .await;
    assert!(matches!(
        missing,
        Err(StorageError::PreconditionFailed { .. })
    ));
    assert!(!b.exists(&absent).await.expect("exists"));
}

async fn list_pages_in_lexical_order<B: StorageBackend>(b: &B) {
    for name in [
        "objects/e",
        "objects/b",
        "objects/d/x",
        "objects/a",
        "objects/c",
    ] {
        b.put(&key(name), name.as_bytes()).await.expect("put");
    }
    b.put(&key("other/z"), b"z").await.expect("put");
    let mut seen = Vec::new();
    let mut after: Option<StorageKey> = None;
    let mut pages = 0;
    loop {
        let page = b.list("objects/", after.as_ref(), 2).await.expect("list");
        assert!(page.keys.len() <= 2);
        pages += 1;
        seen.extend(page.keys.iter().map(|k| k.as_str().to_string()));
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
        assert!(pages < 10, "listing did not terminate");
    }
    assert_eq!(pages, 3);
    assert_eq!(
        seen,
        [
            "objects/a",
            "objects/b",
            "objects/c",
            "objects/d/x",
            "objects/e"
        ]
    );
    let all = b.list("", None, MAX_LIST_PAGE).await.expect("list all");
    assert_eq!(all.keys.len(), 6);
    assert!(all.next.is_none());
    let too_many = b.list("", None, MAX_LIST_PAGE + 1).await;
    assert!(matches!(too_many, Err(StorageError::LimitExceeded { .. })));
    let bad = b.list("../", None, 2).await;
    assert!(matches!(bad, Err(StorageError::InvalidKey { .. })));
}

async fn delete_is_idempotent<B: StorageBackend>(b: &B) {
    let k = key("staging/a/b/c.txt");
    b.put(&k, b"c").await.expect("put");
    b.delete(&k).await.expect("delete");
    assert!(!b.exists(&k).await.expect("exists"));
    b.delete(&k).await.expect("delete again");
    let page = b.list("", None, 10).await.expect("list");
    assert!(page.keys.is_empty());
}

async fn single_put_is_bounded<B: StorageBackend>(b: &B) {
    let big = vec![0u8; MAX_PUT_BYTES as usize + 1];
    let err = b.put(&key("big.bin"), &big).await.expect_err("too big");
    assert!(matches!(err, StorageError::LimitExceeded { .. }), "{err:?}");
    assert!(!b.exists(&key("big.bin")).await.expect("exists"));
}

async fn multipart_round_trip_streams_in_chunks<B: StorageBackend>(b: &B) {
    let k = key("objects/a/v1/content/big.bin");
    let data = pattern(3 * 1024 * 1024 + 17);
    let mut writer = MultipartWriter::start(b, k.clone()).await.expect("start");
    for part in data.chunks(1024 * 1024) {
        writer.write_part(part).await.expect("part");
    }
    let meta = writer.finish(None).await.expect("finish");
    assert_eq!(meta.size, data.len() as u64);
    assert_eq!(meta.etag, ETag::sha256_of(&data));
    assert_eq!(read_all(b, &k, 256 * 1024).await, data);
}

async fn multipart_abort_leaves_nothing<B: StorageBackend>(b: &B) {
    let k = key("objects/a/v2/content/aborted.bin");
    let mut writer = MultipartWriter::start(b, k.clone()).await.expect("start");
    writer.write_part(b"partial").await.expect("part");
    writer.abort().await.expect("abort");
    assert!(!b.exists(&k).await.expect("exists"));
}

async fn multipart_rejects_bad_parts<B: StorageBackend>(b: &B) {
    let k = key("mp.bin");
    let upload = b.create_multipart(&k).await.expect("create");
    let oversize = vec![0u8; MAX_CHUNK_BYTES as usize + 1];
    let err = b.upload_part(&upload, 1, &oversize).await.expect_err("big");
    assert!(matches!(err, StorageError::LimitExceeded { .. }), "{err:?}");
    let err = b.upload_part(&upload, 0, b"x").await.expect_err("zero");
    assert!(
        matches!(err, StorageError::InvalidArgument { .. }),
        "{err:?}"
    );
    let p1 = b.upload_part(&upload, 1, b"aa").await.expect("p1");
    let p2 = b.upload_part(&upload, 2, b"bb").await.expect("p2");
    let unordered = b
        .complete_multipart(&upload, &[p2.clone(), p1.clone()], None)
        .await;
    assert!(matches!(
        unordered,
        Err(StorageError::InvalidArgument { .. })
    ));
    let mut forged = p2.clone();
    forged.etag = ETag::sha256_of(b"zz");
    let tampered = b
        .complete_multipart(&upload, &[p1.clone(), forged], None)
        .await;
    assert!(matches!(tampered, Err(StorageError::PartMismatch { .. })));
    b.complete_multipart(&upload, &[p1, p2], None)
        .await
        .expect("complete");
    assert_eq!(read_all(b, &k, 3).await, b"aabb");
    let gone = b.abort_multipart(&upload).await;
    assert!(matches!(gone, Err(StorageError::UnknownUpload { .. })));
}

async fn multipart_complete_honours_cas<B: StorageBackend>(b: &B) {
    let k = key("objects/a/inventory.json");
    b.put(&k, b"head").await.expect("put");
    let upload = b.create_multipart(&k).await.expect("create");
    let p1 = b.upload_part(&upload, 1, b"new").await.expect("part");
    let result = b
        .complete_multipart(&upload, &[p1], Some(&Precondition::IfAbsent))
        .await;
    assert!(matches!(
        result,
        Err(StorageError::PreconditionFailed { .. })
    ));
    assert_eq!(read_all(b, &k, 16).await, b"head");
    b.abort_multipart(&upload).await.expect("abort");
}

async fn chunk_reader_bounds<B: StorageBackend>(b: &B) {
    let k = key("x.bin");
    b.put(&k, b"0123456789").await.expect("put");
    let err = ChunkReader::new(b, k.clone(), MAX_CHUNK_BYTES + 1).expect_err("over");
    assert!(matches!(err, StorageError::LimitExceeded { .. }));
    assert!(ChunkReader::new(b, k.clone(), 0).is_err());
    let mut reader = ChunkReader::new(b, k, 4).expect("reader");
    let mut sizes = Vec::new();
    while let Some(chunk) = reader.next_chunk().await.expect("chunk") {
        sizes.push(chunk.len());
    }
    assert_eq!(sizes, [4, 4, 2]);
    assert!(reader.next_chunk().await.expect("done").is_none());
}

macro_rules! conformance {
    ($($name:ident),* $(,)?) => {
        mod memory {
            use super::*;
            $(
                #[tokio::test]
                async fn $name() {
                    super::$name(&MemoryBackend::new()).await;
                }
            )*
        }
        mod filesystem {
            use super::*;
            $(
                #[tokio::test]
                async fn $name() {
                    let (backend, _scratch) = fs_backend(stringify!($name));
                    super::$name(&backend).await;
                }
            )*
        }
    };
}

conformance!(
    put_get_head_exists,
    put_replaces,
    missing_key_is_not_found,
    compare_and_swap,
    list_pages_in_lexical_order,
    delete_is_idempotent,
    single_put_is_bounded,
    multipart_round_trip_streams_in_chunks,
    multipart_abort_leaves_nothing,
    multipart_rejects_bad_parts,
    multipart_complete_honours_cas,
    chunk_reader_bounds,
);

#[test]
fn fs_staging_must_be_outside_the_storage_root() {
    let scratch = Scratch::new("staging-inside");
    let root = scratch.0.join("root");
    let err = FsBackend::new(&root, root.join("staging")).expect_err("inside");
    assert!(matches!(err, StorageError::InvalidConfig { .. }), "{err:?}");
}

#[tokio::test]
async fn fs_backend_writes_plain_files_under_the_root() {
    let (backend, scratch) = fs_backend("plain");
    backend
        .put(&key("objects/a/0=ocfl_object_1.1"), b"ocfl_object_1.1\n")
        .await
        .expect("put");
    let on_disk = scratch.0.join("root/objects/a/0=ocfl_object_1.1");
    assert_eq!(
        std::fs::read(on_disk).expect("plain file"),
        b"ocfl_object_1.1\n"
    );
}

#[tokio::test]
async fn fs_backend_leaves_no_temporary_files_or_staging() {
    let (backend, scratch) = fs_backend("tidy");
    let k = key("objects/a/v1/content/f.bin");
    backend.put(&k, b"abc").await.expect("put");
    let mut writer = MultipartWriter::start(&backend, k.clone())
        .await
        .expect("start");
    writer.write_part(b"def").await.expect("part");
    writer.finish(None).await.expect("finish");
    let names: Vec<String> = std::fs::read_dir(scratch.0.join("root/objects/a/v1/content"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["f.bin"]);
    let staged = std::fs::read_dir(scratch.0.join("staging"))
        .expect("staging")
        .count();
    assert_eq!(staged, 0, "multipart staging must be cleaned up");
}

#[tokio::test]
async fn fs_delete_prunes_empty_directories() {
    let (backend, scratch) = fs_backend("prune");
    backend.put(&key("a/b/c/d.txt"), b"d").await.expect("put");
    backend.put(&key("a/keep.txt"), b"k").await.expect("put");
    backend.delete(&key("a/b/c/d.txt")).await.expect("delete");
    assert!(!scratch.0.join("root/a/b").exists(), "empty dirs remain");
    assert!(scratch.0.join("root/a/keep.txt").exists());
}
