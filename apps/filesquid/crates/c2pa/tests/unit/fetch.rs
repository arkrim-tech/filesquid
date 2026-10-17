//! SRS-F-054 / C2PA-S-008: remote manifests, revocation data and timestamps are
//! fetched only through a caller-supplied fetcher, and every response is
//! bounded by the request's `max_bytes`.

use std::cell::RefCell;

use filesquid_c2pa::fetch::{
    fetch_bounded, FetchError, FetchPurpose, FetchRequest, FetchResponse, Fetcher, OfflineFetcher,
    DEFAULT_MAX_FETCH_BYTES,
};
use futures::executor::block_on;

/// A fetcher that answers every request with a fixed body and records the
/// requests it saw.
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
fn bounded_fetch_rejects_an_oversized_response() {
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
