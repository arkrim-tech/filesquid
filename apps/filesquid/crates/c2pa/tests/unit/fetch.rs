//! SRS-F-054 / C2PA-S-008: remote manifests, revocation data and timestamps are
//! fetched only through a caller-supplied fetcher, and every response is
//! bounded by the request's `max_bytes`.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::{Arc, Mutex};

use filesquid_c2pa::fetch::{
    fetch_bounded, redacted_origin, BoundedBody, FetchError, FetchPurpose, FetchRequest,
    FetchResponse, Fetcher, OfflineFetcher, DEFAULT_MAX_FETCH_BYTES, UNPARSED_URL,
};
use futures::executor::block_on;

/// A fetcher that ignores `max_bytes` (a contract violation) and answers
/// every request with a fixed body, recording the requests it saw.
struct CannedFetcher {
    /// Body returned for every request.
    body: Vec<u8>,
    /// URLs requested so far.
    seen: RefCell<Vec<String>>,
}

impl Fetcher for CannedFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        self.seen.borrow_mut().push(request.url.clone());
        Ok(FetchResponse {
            content_type: Some("application/c2pa".to_string()),
            body: self.body.clone(),
        })
    }
}

fn canned(body: &[u8]) -> CannedFetcher {
    CannedFetcher {
        body: body.to_vec(),
        seen: RefCell::new(Vec::new()),
    }
}

#[test]
fn get_request_carries_purpose_and_bound() {
    let request = FetchRequest::get(
        "https://example.org/m.c2pa",
        FetchPurpose::RemoteManifest,
        1024,
    );
    assert_eq!(request.url, "https://example.org/m.c2pa");
    assert_eq!(request.purpose, FetchPurpose::RemoteManifest);
    assert_eq!(request.max_bytes, 1024);
    assert_eq!(request.body, None);
    assert_eq!(request.content_type, None);
}

#[test]
fn post_request_carries_body_and_content_type() {
    let request = FetchRequest::post(
        "https://ocsp.example.org/",
        FetchPurpose::Revocation,
        "application/ocsp-request",
        vec![0x30, 0x00],
        4096,
    );
    assert_eq!(request.body.as_deref(), Some(&[0x30, 0x00][..]));
    assert_eq!(
        request.content_type.as_deref(),
        Some("application/ocsp-request")
    );
    assert_eq!(request.purpose, FetchPurpose::Revocation);
}

#[test]
fn default_bound_is_sixteen_mebibytes() {
    assert_eq!(DEFAULT_MAX_FETCH_BYTES, 16 * 1024 * 1024);
}

#[test]
fn offline_fetcher_refuses_every_request() {
    let request = FetchRequest::get(
        "https://example.org/m.c2pa",
        FetchPurpose::RemoteManifest,
        8,
    );
    let err = block_on(fetch_bounded(&OfflineFetcher, &request)).expect_err("offline");
    assert_eq!(
        err,
        FetchError::Offline {
            url: "https://example.org/m.c2pa".to_string()
        }
    );
}

#[test]
fn bounded_fetch_returns_a_response_within_the_bound() {
    let fetcher = canned(b"manifest");
    let request = FetchRequest::get("https://example.org/a", FetchPurpose::RemoteManifest, 8);
    let response = block_on(fetch_bounded(&fetcher, &request)).expect("fetch");
    assert_eq!(response.body, b"manifest");
    assert_eq!(fetcher.seen.borrow().as_slice(), ["https://example.org/a"]);
}

#[test]
fn bounded_fetch_rejects_a_fetcher_that_ignores_the_cap() {
    let fetcher = canned(b"manifest!");
    let request = FetchRequest::get("https://example.org/a", FetchPurpose::RemoteManifest, 8);
    let err = block_on(fetch_bounded(&fetcher, &request)).expect_err("too large");
    assert_eq!(
        err,
        FetchError::TooLarge {
            url: "https://example.org/a".to_string(),
            max_bytes: 8,
            actual: 9
        }
    );
}

/// A fetcher that honours the contract: it reads a large response in 4-byte
/// chunks through a `BoundedBody` and stops at the cap.
struct StreamingFetcher {
    /// Total bytes the "server" would send.
    server_bytes: usize,
    /// Chunks actually read.
    chunks_read: Cell<usize>,
}

impl Fetcher for StreamingFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        let mut body = BoundedBody::new(request);
        let server = vec![7u8; self.server_bytes];
        for chunk in server.chunks(4) {
            self.chunks_read.set(self.chunks_read.get() + 1);
            body.push(chunk)?;
        }
        Ok(FetchResponse {
            content_type: None,
            body: body.into_bytes(),
        })
    }
}

#[test]
fn streaming_fetcher_stops_reading_at_the_cap() {
    let fetcher = StreamingFetcher {
        server_bytes: 1_000_000,
        chunks_read: Cell::new(0),
    };
    let request = FetchRequest::get("https://example.org/big", FetchPurpose::Revocation, 10);
    let err = block_on(fetch_bounded(&fetcher, &request)).expect_err("too large");
    assert_eq!(err.category(), "too_large");
    assert_eq!(
        fetcher.chunks_read.get(),
        3,
        "stops at the third 4-byte chunk"
    );
}

#[test]
fn bounded_body_accepts_exactly_the_cap() {
    let request = FetchRequest::get("https://example.org/a", FetchPurpose::Timestamp, 8);
    let mut body = BoundedBody::new(&request);
    assert!(body.is_empty());
    body.push(b"abcd").expect("first");
    body.push(b"efgh").expect("second");
    assert_eq!(body.len(), 8);
    assert_eq!(body.into_bytes(), b"abcdefgh");
}

#[test]
fn bounded_body_keeps_nothing_of_an_overflowing_chunk() {
    let request = FetchRequest::get("https://example.org/a", FetchPurpose::Timestamp, 5);
    let mut body = BoundedBody::new(&request);
    body.push(b"abc").expect("first");
    let err = body.push(b"def").expect_err("over");
    assert_eq!(
        err,
        FetchError::TooLarge {
            url: "https://example.org/a".to_string(),
            max_bytes: 5,
            actual: 6
        }
    );
    assert_eq!(body.into_bytes(), b"abc");
}

/// A fetcher whose transport fails with sensitive text.
struct FailingFetcher;

impl Fetcher for FailingFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        Err(FetchError::Failed {
            url: request.url.clone(),
            reason: "proxy auth failed for token=SECRET-REASON".to_string(),
        })
    }
}

#[test]
fn caller_failures_pass_through_with_a_category() {
    let request = FetchRequest::get("https://example.org/a", FetchPurpose::Revocation, 8);
    let err = block_on(fetch_bounded(&FailingFetcher, &request)).expect_err("failed");
    assert_eq!(err.category(), "failed");
    assert!(matches!(err, FetchError::Failed { .. }));
}

#[test]
fn offline_and_too_large_have_categories() {
    let offline = FetchError::Offline { url: "u".into() };
    assert_eq!(offline.category(), "offline");
}

#[test]
fn redacted_origin_drops_userinfo_path_query_and_fragment() {
    assert_eq!(
        redacted_origin("https://user:pw@cdn.example.org:8443/m.c2pa?sig=abc#frag"),
        "https://cdn.example.org:8443"
    );
    assert_eq!(
        redacted_origin("http://ocsp.example.org"),
        "http://ocsp.example.org"
    );
    assert_eq!(
        redacted_origin("https://h.example?x=1"),
        "https://h.example"
    );
}

#[test]
fn redacted_origin_hides_text_that_is_not_a_url() {
    assert_eq!(redacted_origin("token=SECRET"), UNPARSED_URL);
    assert_eq!(redacted_origin("://nohost"), UNPARSED_URL);
    assert_eq!(redacted_origin("https:///path"), UNPARSED_URL);
    assert_eq!(redacted_origin("a b://host"), UNPARSED_URL);
}

/// A `tracing` writer that appends to a shared buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn failure_logs_carry_no_secrets() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();
    let request = FetchRequest::get(
        "https://user:SECRET-PW@cdn.example.org/m.c2pa?sig=SECRET-SIG",
        FetchPurpose::RemoteManifest,
        8,
    );
    tracing::subscriber::with_default(subscriber, || {
        block_on(fetch_bounded(&FailingFetcher, &request)).expect_err("failed");
    });
    let log = String::from_utf8(captured.0.lock().expect("log buffer").clone()).expect("utf-8");
    assert!(log.contains("https://cdn.example.org"), "{log}");
    assert!(log.contains("category=\"failed\""), "{log}");
    for secret in ["SECRET-PW", "SECRET-SIG", "SECRET-REASON", "m.c2pa"] {
        assert!(!log.contains(secret), "{secret} leaked: {log}");
    }
}
