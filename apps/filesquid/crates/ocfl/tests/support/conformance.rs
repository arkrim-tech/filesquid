//! Storage backend conformance suite (OCFL-S-007), shared by the in-memory
//! test target (no features) and the filesystem test target (`fs`). Each
//! generic case runs against any [`StorageBackend`].

#![allow(dead_code)]

use filesquid_ocfl::storage::{
    ByteRange, ChunkReader, ETag, MultipartWriter, Precondition, StorageBackend, StorageError,
    StorageKey, MAX_CHUNK_BYTES, MAX_LIST_PAGE, MAX_PART_BYTES, MAX_PUT_BYTES,
};

/// Parse a key that the test knows is valid.
pub fn key(s: &str) -> StorageKey {
    StorageKey::new(s).expect("valid key")
}

pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

pub async fn read_all<B: StorageBackend>(backend: &B, k: &StorageKey, chunk: u64) -> Vec<u8> {
    let mut reader = ChunkReader::new(backend, k.clone(), chunk).expect("reader");
    let mut out = Vec::new();
    while let Some(bytes) = reader.next_chunk().await.expect("chunk") {
        assert!(bytes.len() as u64 <= chunk, "chunk over bound");
        out.extend_from_slice(&bytes);
    }
    out
}

pub async fn put_get_head_exists<B: StorageBackend>(b: &B) {
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

pub async fn put_replaces<B: StorageBackend>(b: &B) {
    let k = key("a.txt");
    b.put(&k, b"one").await.expect("put");
    b.put(&k, b"two!").await.expect("put");
    assert_eq!(read_all(b, &k, 2).await, b"two!");
    assert_eq!(b.head(&k).await.expect("head").size, 4);
}

pub async fn missing_key_is_not_found<B: StorageBackend>(b: &B) {
    let k = key("nope/missing.bin");
    let range = ByteRange::new(0, 1).expect("range");
    for err in [
        b.head(&k).await.expect_err("head"),
        b.get_range(&k, range).await.expect_err("get"),
    ] {
        assert!(matches!(err, StorageError::NotFound { .. }), "{err:?}");
    }
}

pub async fn compare_and_swap<B: StorageBackend>(b: &B) {
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

pub async fn list_pages_in_lexical_order<B: StorageBackend>(b: &B) {
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

pub async fn delete_is_idempotent<B: StorageBackend>(b: &B) {
    let k = key("staging/a/b/c.txt");
    b.put(&k, b"c").await.expect("put");
    b.delete(&k).await.expect("delete");
    assert!(!b.exists(&k).await.expect("exists"));
    b.delete(&k).await.expect("delete again");
    let page = b.list("", None, 10).await.expect("list");
    assert!(page.keys.is_empty());
}

pub async fn single_put_is_bounded<B: StorageBackend>(b: &B) {
    let big = vec![0u8; MAX_PUT_BYTES as usize + 1];
    let err = b.put(&key("big.bin"), &big).await.expect_err("too big");
    assert!(matches!(err, StorageError::LimitExceeded { .. }), "{err:?}");
    assert!(!b.exists(&key("big.bin")).await.expect("exists"));
}

pub async fn multipart_round_trip_streams_in_chunks<B: StorageBackend>(b: &B) {
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

pub async fn multipart_abort_leaves_nothing<B: StorageBackend>(b: &B) {
    let k = key("objects/a/v2/content/aborted.bin");
    let mut writer = MultipartWriter::start(b, k.clone()).await.expect("start");
    writer.write_part(b"partial").await.expect("part");
    writer.abort().await.expect("abort");
    assert!(!b.exists(&k).await.expect("exists"));
}

pub async fn multipart_rejects_bad_parts<B: StorageBackend>(b: &B) {
    let k = key("mp.bin");
    let upload = b.create_multipart(&k).await.expect("create");
    let oversize = vec![0u8; MAX_PART_BYTES as usize + 1];
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

pub async fn multipart_complete_honours_cas<B: StorageBackend>(b: &B) {
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

pub async fn chunk_reader_bounds<B: StorageBackend>(b: &B) {
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

pub async fn parts_above_the_read_chunk_are_accepted<B: StorageBackend>(b: &B) {
    let k = key("objects/a/v1/content/large-part.bin");
    let data = pattern(MAX_CHUNK_BYTES as usize + 5);
    let mut writer = MultipartWriter::start(b, k.clone()).await.expect("start");
    writer
        .write_part(&data)
        .await
        .expect("part above the read chunk");
    writer.finish(None).await.expect("finish");
    assert_eq!(read_all(b, &k, MAX_CHUNK_BYTES).await, data);
}

pub async fn lookalike_temporary_names_are_listed<B: StorageBackend>(b: &B) {
    b.put(&key("x.fsquid-tmp-y"), b"kept").await.expect("put");
    let page = b.list("", None, 10).await.expect("list");
    let names: Vec<&str> = page.keys.iter().map(StorageKey::as_str).collect();
    assert_eq!(names, ["x.fsquid-tmp-y"]);
}

pub async fn listing_under_a_file_is_empty<B: StorageBackend>(b: &B) {
    b.put(&key("foo"), b"file").await.expect("put");
    let page = b.list("foo/", None, 10).await.expect("list under a file");
    assert!(page.keys.is_empty());
    assert!(page.next.is_none());
}

pub async fn stream_with_limit<B: StorageBackend>(
    b: &B,
    k: &StorageKey,
    chunk: u64,
    limit: u64,
) -> (Vec<Vec<u8>>, Option<StorageError>) {
    let mut reader = ChunkReader::with_limit(b, k.clone(), chunk, limit).expect("reader");
    let mut chunks = Vec::new();
    for _ in 0..16 {
        match reader.next_chunk().await {
            Ok(Some(bytes)) => chunks.push(bytes),
            Ok(None) => return (chunks, None),
            Err(err) => return (chunks, Some(err)),
        }
    }
    panic!("reader did not finish");
}

pub async fn chunk_reader_stops_exactly_at_the_limit<B: StorageBackend>(b: &B) {
    let k = key("exact.bin");
    b.put(&k, b"01234567").await.expect("put");
    let (chunks, err) = stream_with_limit(b, &k, 4, 8).await;
    assert_eq!(chunks, [b"0123".to_vec(), b"4567".to_vec()]);
    assert!(err.is_none(), "{err:?}");
}

pub async fn chunk_reader_never_returns_bytes_past_the_limit<B: StorageBackend>(b: &B) {
    let k = key("over.bin");
    b.put(&k, b"0123456789").await.expect("put");
    let (chunks, err) = stream_with_limit(b, &k, 3, 8).await;
    assert_eq!(chunks.concat(), b"01234567");
    assert!(chunks.iter().all(|c| c.len() <= 3));
    assert!(
        matches!(err, Some(StorageError::LimitExceeded { .. })),
        "{err:?}"
    );
}

pub async fn key_below_a_file_is_not_found<B: StorageBackend>(b: &B) {
    b.put(&key("foo"), b"file").await.expect("put");
    let below = key("foo/bar");
    let range = ByteRange::new(0, 4).expect("range");
    for err in [
        b.get_range(&below, range).await.expect_err("get"),
        b.head(&below).await.expect_err("head"),
    ] {
        assert!(matches!(err, StorageError::NotFound { .. }), "{err:?}");
    }
    assert!(!b.exists(&below).await.expect("exists"));
}

/// Generate one `#[tokio::test]` per conformance case. `$make` builds a
/// fixture whose field `.0` is the backend; the fixture lives for the test.
#[macro_export]
macro_rules! conformance_tests {
    ($make:expr) => {
        #[tokio::test]
        async fn key_below_a_file_is_not_found() {
            let fixture = $make;
            $crate::suite::key_below_a_file_is_not_found(&fixture.0).await;
        }
        #[tokio::test]
        async fn put_get_head_exists() {
            let fixture = $make;
            $crate::suite::put_get_head_exists(&fixture.0).await;
        }

        #[tokio::test]
        async fn parts_above_the_read_chunk_are_accepted() {
            let fixture = $make;
            $crate::suite::parts_above_the_read_chunk_are_accepted(&fixture.0).await;
        }

        #[tokio::test]
        async fn lookalike_temporary_names_are_listed() {
            let fixture = $make;
            $crate::suite::lookalike_temporary_names_are_listed(&fixture.0).await;
        }

        #[tokio::test]
        async fn listing_under_a_file_is_empty() {
            let fixture = $make;
            $crate::suite::listing_under_a_file_is_empty(&fixture.0).await;
        }

        #[tokio::test]
        async fn chunk_reader_stops_exactly_at_the_limit() {
            let fixture = $make;
            $crate::suite::chunk_reader_stops_exactly_at_the_limit(&fixture.0).await;
        }

        #[tokio::test]
        async fn chunk_reader_never_returns_bytes_past_the_limit() {
            let fixture = $make;
            $crate::suite::chunk_reader_never_returns_bytes_past_the_limit(&fixture.0).await;
        }

        #[tokio::test]
        async fn put_replaces() {
            let fixture = $make;
            $crate::suite::put_replaces(&fixture.0).await;
        }

        #[tokio::test]
        async fn missing_key_is_not_found() {
            let fixture = $make;
            $crate::suite::missing_key_is_not_found(&fixture.0).await;
        }

        #[tokio::test]
        async fn compare_and_swap() {
            let fixture = $make;
            $crate::suite::compare_and_swap(&fixture.0).await;
        }

        #[tokio::test]
        async fn list_pages_in_lexical_order() {
            let fixture = $make;
            $crate::suite::list_pages_in_lexical_order(&fixture.0).await;
        }

        #[tokio::test]
        async fn delete_is_idempotent() {
            let fixture = $make;
            $crate::suite::delete_is_idempotent(&fixture.0).await;
        }

        #[tokio::test]
        async fn single_put_is_bounded() {
            let fixture = $make;
            $crate::suite::single_put_is_bounded(&fixture.0).await;
        }

        #[tokio::test]
        async fn multipart_round_trip_streams_in_chunks() {
            let fixture = $make;
            $crate::suite::multipart_round_trip_streams_in_chunks(&fixture.0).await;
        }

        #[tokio::test]
        async fn multipart_abort_leaves_nothing() {
            let fixture = $make;
            $crate::suite::multipart_abort_leaves_nothing(&fixture.0).await;
        }

        #[tokio::test]
        async fn multipart_rejects_bad_parts() {
            let fixture = $make;
            $crate::suite::multipart_rejects_bad_parts(&fixture.0).await;
        }

        #[tokio::test]
        async fn multipart_complete_honours_cas() {
            let fixture = $make;
            $crate::suite::multipart_complete_honours_cas(&fixture.0).await;
        }

        #[tokio::test]
        async fn chunk_reader_bounds() {
            let fixture = $make;
            $crate::suite::chunk_reader_bounds(&fixture.0).await;
        }
    };
}
