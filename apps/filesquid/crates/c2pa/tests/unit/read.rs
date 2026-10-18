//! C2PA-S-001 / C2PA-S-008: reading a manifest store from a caller-supplied
//! byte source (a sidecar `.c2pa` file) and from a remote reference through
//! the caller's fetcher.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::cell::{Cell, RefCell};

use filesquid_c2pa::fetch::{
    FetchError, FetchPurpose, FetchRequest, FetchResponse, Fetcher, OfflineFetcher,
};
use filesquid_c2pa::io::{ByteIoError, ByteSource, MemorySource};
use filesquid_c2pa::manifest::{ParseLimits, StoreError};
use filesquid_c2pa::read::{fetch_remote_manifest_store, read_manifest_store, ReadError};
use fixtures::*;
use futures::executor::block_on;

/// A source that reports `size` and counts reads, optionally failing them.
struct CountingSource {
    /// Reported size.
    size: u64,
    /// Number of `read_at` calls.
    reads: Cell<usize>,
    /// Error returned by `read_at`, if any.
    fail: Option<ByteIoError>,
}

impl ByteSource for CountingSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Ok(self.size)
    }

    async fn read_at(&mut self, _offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        self.reads.set(self.reads.get() + 1);
        match &self.fail {
            Some(err) => Err(err.clone()),
            None => {
                buf.fill(0);
                Ok(buf.len())
            }
        }
    }
}

/// Limits with `max_store_bytes` set.
fn limits(max_store_bytes: u64) -> ParseLimits {
    ParseLimits {
        max_store_bytes,
        ..ParseLimits::default()
    }
}

#[test]
fn a_sidecar_store_is_read_from_a_byte_source() {
    let mut source = MemorySource::new(sample_store());
    let store = block_on(read_manifest_store(&mut source, &ParseLimits::default())).expect("store");
    assert_eq!(store.manifests().len(), 2);
    assert_eq!(store.active_manifest().label, ACTIVE_LABEL);
}

#[test]
fn store_size_limit_holds_at_the_boundary_before_reading() {
    let bytes = sample_store();
    let size = bytes.len() as u64;
    let mut exact = MemorySource::new(bytes);
    assert!(block_on(read_manifest_store(&mut exact, &limits(size))).is_ok());
    let mut over = CountingSource {
        size: size + 1,
        reads: Cell::new(0),
        fail: None,
    };
    let err = block_on(read_manifest_store(&mut over, &limits(size))).expect_err("too large");
    assert_eq!(
        err,
        ReadError::TooLarge {
            size: size + 1,
            max: size
        }
    );
    assert_eq!(over.reads.get(), 0, "nothing is read or allocated");
}

#[test]
fn a_huge_reported_size_is_rejected_without_allocating() {
    let mut source = CountingSource {
        size: u64::MAX,
        reads: Cell::new(0),
        fail: None,
    };
    let err = block_on(read_manifest_store(&mut source, &limits(u64::MAX))).expect_err("too large");
    assert!(
        matches!(err, ReadError::TooLarge { size: u64::MAX, .. }),
        "{err:?}"
    );
    assert_eq!(source.reads.get(), 0);
}

#[test]
fn byte_source_errors_propagate() {
    let mut failing = CountingSource {
        size: 64,
        reads: Cell::new(0),
        fail: Some(ByteIoError::Backend("disk".into())),
    };
    let err = block_on(read_manifest_store(&mut failing, &ParseLimits::default())).expect_err("io");
    assert_eq!(err, ReadError::Io(ByteIoError::Backend("disk".into())));
    assert_eq!(err.status_code(), None);
}

#[test]
fn a_malformed_sidecar_is_a_store_error() {
    let mut source = MemorySource::new(b"not a manifest store".to_vec());
    let err = block_on(read_manifest_store(&mut source, &ParseLimits::default())).expect_err("bad");
    assert!(
        matches!(err, ReadError::Store(StoreError::Jumbf(_))),
        "{err:?}"
    );
    let mut empty = MemorySource::new(Vec::new());
    assert!(matches!(
        block_on(read_manifest_store(&mut empty, &ParseLimits::default())),
        Err(ReadError::Store(_))
    ));
}

/// A fetcher returning a fixed body and recording requests.
struct CannedFetcher {
    /// Body for every request.
    body: Vec<u8>,
    /// Requests seen.
    seen: RefCell<Vec<FetchRequest>>,
}

impl Fetcher for CannedFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        self.seen.borrow_mut().push(request.clone());
        Ok(FetchResponse {
            content_type: Some("application/c2pa".into()),
            body: self.body.clone(),
        })
    }
}

/// A canned fetcher for `body`.
fn canned(body: Vec<u8>) -> CannedFetcher {
    CannedFetcher {
        body,
        seen: RefCell::new(Vec::new()),
    }
}

#[test]
fn a_remote_store_is_fetched_through_the_caller_fetcher() {
    let fetcher = canned(sample_store());
    let url = "https://cdn.example.org/asset.c2pa";
    let store =
        block_on(fetch_remote_manifest_store(&fetcher, url, &limits(1 << 20))).expect("store");
    assert_eq!(store.active_manifest().label, ACTIVE_LABEL);
    let seen = fetcher.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].url, url);
    assert_eq!(seen[0].purpose, FetchPurpose::RemoteManifest);
    assert_eq!(
        seen[0].max_bytes,
        1 << 20,
        "the store limit bounds the fetch"
    );
    assert_eq!(seen[0].body, None, "a GET");
}

#[test]
fn http_and_mixed_case_schemes_are_accepted() {
    for url in ["http://example.org/a.c2pa", "HTTPS://example.org/a.c2pa"] {
        let fetcher = canned(sample_store());
        assert!(
            block_on(fetch_remote_manifest_store(
                &fetcher,
                url,
                &ParseLimits::default()
            ))
            .is_ok(),
            "{url}"
        );
    }
}

#[test]
fn offline_remote_references_are_manifest_inaccessible() {
    let err = block_on(fetch_remote_manifest_store(
        &OfflineFetcher,
        "https://example.org/a.c2pa",
        &ParseLimits::default(),
    ))
    .expect_err("offline");
    assert!(
        matches!(err, ReadError::Fetch(FetchError::Offline { .. })),
        "{err:?}"
    );
    assert_eq!(err.status_code(), Some("manifest.inaccessible"));
}

#[test]
fn fetch_error_messages_do_not_repeat_the_url() {
    let err = block_on(fetch_remote_manifest_store(
        &OfflineFetcher,
        "https://user:SECRET@example.org/a.c2pa?token=SECRET",
        &ParseLimits::default(),
    ))
    .expect_err("offline");
    assert_eq!(
        err.to_string(),
        "remote manifest store unavailable (offline)"
    );
}

#[test]
fn oversized_remote_stores_are_rejected_at_the_boundary() {
    let body = sample_store();
    let size = body.len() as u64;
    let fetcher = canned(body);
    assert!(block_on(fetch_remote_manifest_store(
        &fetcher,
        "https://e.org/a",
        &limits(size)
    ))
    .is_ok());
    let err = block_on(fetch_remote_manifest_store(
        &fetcher,
        "https://e.org/a",
        &limits(size - 1),
    ))
    .expect_err("too large");
    assert!(
        matches!(err, ReadError::Fetch(FetchError::TooLarge { .. })),
        "{err:?}"
    );
    assert_eq!(err.status_code(), Some("manifest.inaccessible"));
}

#[test]
fn non_http_references_are_refused_before_fetching() {
    let fetcher = canned(sample_store());
    for url in [
        "file:///etc/passwd",
        "ftp://example.org/a.c2pa",
        "a.c2pa",
        "data:application/c2pa,AA",
    ] {
        let err = block_on(fetch_remote_manifest_store(
            &fetcher,
            url,
            &ParseLimits::default(),
        ))
        .expect_err("refused");
        assert_eq!(err, ReadError::UnsupportedUrl, "{url}");
    }
    assert!(fetcher.seen.borrow().is_empty());
}

#[test]
fn references_into_an_asset_need_a_format_handler() {
    let fetcher = canned(sample_store());
    let err = block_on(fetch_remote_manifest_store(
        &fetcher,
        "https://c2pa.org/image.jpg#jumbf=c2pa",
        &ParseLimits::default(),
    ))
    .expect_err("embedded");
    assert_eq!(err, ReadError::EmbeddedReference);
    assert!(fetcher.seen.borrow().is_empty());
}

#[test]
fn a_malformed_remote_store_is_a_store_error() {
    let fetcher = canned(b"<html>not found</html>".to_vec());
    let err = block_on(fetch_remote_manifest_store(
        &fetcher,
        "https://e.org/a",
        &ParseLimits::default(),
    ))
    .expect_err("bad");
    assert!(matches!(err, ReadError::Store(_)), "{err:?}");
    assert_eq!(err.status_code(), None);
}

#[test]
fn urls_without_a_host_are_refused_before_fetching() {
    let fetcher = canned(sample_store());
    for url in [
        "http:///path",
        "http://?x",
        "https://#frag",
        "https://user@/a.c2pa",
        "https://exa mple.org/a.c2pa",
        "https://example.org\t/a.c2pa",
    ] {
        let err = block_on(fetch_remote_manifest_store(
            &fetcher,
            url,
            &ParseLimits::default(),
        ))
        .expect_err("refused");
        assert_eq!(err, ReadError::UnsupportedUrl, "{url:?}");
    }
    assert!(fetcher.seen.borrow().is_empty());
}
