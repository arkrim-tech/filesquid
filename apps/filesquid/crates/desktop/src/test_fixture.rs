//! A 100 MB byte-range fixture, compiled only into the test-instrumented build
//! (`--features test-webdriver`), never into a shipping one.
//!
//! ADR 030 S1 item (a) asks for `Range` reads of a 100 MB file through the custom scheme in
//! a real desktop webview. The file-content routes arrive with API-F1.3, so the
//! packaged-window test reads this fixture instead: [`FIXTURE_LEN`] deterministic bytes
//! ([`fixture_byte`]) at [`FIXTURE_PATH`] on the `filesquid://` scheme. It behaves like a
//! capped file-content route: the per-launch token is required, one `Range` per request is
//! answered with `206 Partial Content`, each answer is capped at [`FIXTURE_MAX_RANGE`], and a
//! range past the end is answered `416` with the length. The bytes are generated per request,
//! so no file is written and memory stays bounded.

use crate::api_protocol::api_error_response;
use filesquid_api::auth::check_caller;
use filesquid_api::{ApiError, ApiState, ErrorCode};
use http::header::{ACCEPT_RANGES, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use http::{HeaderValue, Request, Response, StatusCode};

/// Environment variable naming the OCFL storage root that the packaged-window tests open as
/// [`E2E_LIBRARY_ID`]: a media library their harness writes before the run, read through the
/// API's file and content routes (API-F1.3, ADR 030 S1 item (a)).
pub const E2E_LIBRARY_VARIABLE: &str = "FILESQUID_E2E_LIBRARY";

/// The library ID the packaged-window tests' media library is opened as.
pub const E2E_LIBRARY_ID: &str = "5e2e0000-0000-4000-8000-00000000e2e0";

/// Open the storage root named by [`E2E_LIBRARY_VARIABLE`] as [`E2E_LIBRARY_ID`] through
/// `files`, with its leases and staging area under `data_dir`. Does nothing when the variable
/// is unset. Creates directories.
///
/// # Errors
///
/// A message when the root or its leases and staging cannot be prepared.
pub fn open_e2e_library(
    files: &filesquid_native::HostFiles,
    data_dir: &std::path::Path,
) -> Result<(), String> {
    let Some(root) = std::env::var_os(E2E_LIBRARY_VARIABLE) else {
        return Ok(());
    };
    let library = filesquid_core::LibraryId::new(E2E_LIBRARY_ID).map_err(|e| e.to_string())?;
    filesquid_native::open_fs_library(files, &library, std::path::Path::new(&root), data_dir)
        .map(|_| ())
}

/// Scheme path of the fixture (outside `/api/v1/`, so it never shadows an API route).
pub const FIXTURE_PATH: &str = "/__filesquid_test__/range-fixture";

/// Fixture length in bytes: 100 MiB.
pub const FIXTURE_LEN: u64 = 104_857_600;

/// Most bytes one fixture answer carries (8 MiB), below the scheme's buffer cap.
pub const FIXTURE_MAX_RANGE: u64 = 8 * 1024 * 1024;

/// The fixture byte at `offset`: `offset mod 251`, a pattern that does not repeat on
/// power-of-two boundaries, so a misplaced range shows. Pure.
pub fn fixture_byte(offset: u64) -> u8 {
    (offset % 251) as u8
}

/// True when `request` asks for the fixture.
pub fn is_fixture_request(request: &Request<Vec<u8>>) -> bool {
    request.uri().path() == FIXTURE_PATH
}

/// Answer a fixture read: refuse a caller without the per-launch token (journaled by the API
/// authoriser), require one `Range`, and answer the capped range or `416`.
pub async fn serve(state: &ApiState, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if let Err(denial) = check_caller(state, request.headers()).await {
        return api_error_response(&denial);
    }
    let Some(header) = request.headers().get(RANGE) else {
        let error = ApiError::new(ErrorCode::InvalidRequest, "A Range header is required.");
        return api_error_response(&error);
    };
    let text = header.to_str().unwrap_or_default();
    match parse_range(text, FIXTURE_LEN, FIXTURE_MAX_RANGE) {
        Some((start, end)) => partial_response(start, end),
        None => not_satisfiable_response(),
    }
}

/// Parse one `bytes=` range (`a-b`, `a-` or `-n`) against a resource of `len` bytes and cap it
/// at `cap` bytes. Returns the inclusive `(start, end)`, or `None` when the range is malformed,
/// lists several ranges, or starts at or past the end. Pure.
fn parse_range(text: &str, len: u64, cap: u64) -> Option<(u64, u64)> {
    let spec = text.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 || cap == 0 {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let (start, end) = if first.is_empty() {
        let suffix: u64 = last.parse().ok().filter(|n| *n > 0)?;
        (len.saturating_sub(suffix), len - 1)
    } else {
        let start: u64 = first.parse().ok()?;
        let end = match last {
            "" => len - 1,
            text => text.parse::<u64>().ok()?.min(len - 1),
        };
        (start, end)
    };
    if start >= len || start > end {
        return None;
    }
    Some((start, end.min(start + cap - 1)))
}

/// The `206` answer carrying fixture bytes `start..=end`.
fn partial_response(start: u64, end: u64) -> Response<Vec<u8>> {
    let body: Vec<u8> = (start..=end).map(fixture_byte).collect();
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    let range = format!("bytes {start}-{end}/{FIXTURE_LEN}");
    set_range_headers(&mut response, &range);
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response
}

/// The `416` answer naming the fixture length.
fn not_satisfiable_response() -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
    set_range_headers(&mut response, &format!("bytes */{FIXTURE_LEN}"));
    response
}

/// Set `Accept-Ranges: bytes` and `Content-Range: <range>` (ASCII digits only, so the value is
/// always valid; an invalid one would be left out, and the tests would see it missing).
fn set_range_headers(response: &mut Response<Vec<u8>>, range: &str) {
    let headers = response.headers_mut();
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(value) = HeaderValue::from_str(range) {
        headers.insert(CONTENT_RANGE, value);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges_are_parsed_and_capped() {
        assert_eq!(parse_range("bytes=0-9", 100, 50), Some((0, 9)));
        assert_eq!(parse_range("bytes=90-", 100, 50), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100, 50), Some((90, 99)));
        assert_eq!(parse_range("bytes=-500", 100, 50), Some((0, 49)));
        assert_eq!(parse_range("bytes=0-", 100, 50), Some((0, 49)));
        assert_eq!(parse_range("bytes=95-500", 100, 50), Some((95, 99)));
    }

    #[test]
    fn malformed_or_unsatisfiable_ranges_are_refused() {
        for text in [
            "bytes=100-",
            "bytes=9-0",
            "bytes=-0",
            "bytes=0-1,4-5",
            "items=0-9",
            "bytes=a-b",
            "bytes=",
        ] {
            assert_eq!(parse_range(text, 100, 50), None, "{text}");
        }
    }
}
