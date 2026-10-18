//! Ranged reads of a 100 MB file through the `filesquid://` scheme (ADR 030 S1 item (a), at the
//! handler). The file is the test-instrumented build's fixture, so these tests need the
//! `test-webdriver` feature; the packaged-window test under `tests/e2e/` repeats the read in a
//! real webview.

use filesquid_api::auth::LaunchToken;
use filesquid_api::ApiState;
use filesquid_desktop_lib::api_protocol::{handle, MAX_SCHEME_RESPONSE_BYTES};
use filesquid_desktop_lib::install_launch_token;
use filesquid_desktop_lib::test_fixture::{
    fixture_byte, FIXTURE_LEN, FIXTURE_MAX_RANGE, FIXTURE_PATH,
};
use http::header::{ACCEPT_RANGES, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use http::StatusCode;

/// API state with a per-launch token installed, as the desktop host does.
fn launched() -> (ApiState, LaunchToken) {
    let state = ApiState::new();
    let token = install_launch_token(&state).expect("per-launch token");
    (state, token)
}

/// A fixture read the way an app page sends it: app origin, bearer token, optional `Range`.
fn read(token: Option<&str>, range: Option<&str>) -> http::Request<Vec<u8>> {
    let mut builder = http::Request::get(format!("filesquid://localhost{FIXTURE_PATH}"))
        .header(http::header::ORIGIN, "tauri://localhost");
    if let Some(token) = token {
        builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(range) = range {
        builder = builder.header(RANGE, range);
    }
    builder.body(Vec::new()).expect("request")
}

/// The fixture's bytes from `start` to `end` inclusive.
fn expected(start: u64, end: u64) -> Vec<u8> {
    (start..=end).map(fixture_byte).collect()
}

#[test]
fn the_fixture_is_a_100_mb_file_read_in_ranges_under_the_scheme_cap() {
    assert_eq!(FIXTURE_LEN, 104_857_600);
    assert_eq!(FIXTURE_MAX_RANGE, 8 * 1024 * 1024);
    assert!(FIXTURE_MAX_RANGE as usize <= MAX_SCHEME_RESPONSE_BYTES);
}

#[tokio::test]
async fn the_last_bytes_of_a_100_mb_file_are_read_as_partial_content() {
    let (state, token) = launched();
    let response = handle(
        state,
        read(Some(token.secret()), Some("bytes=104857500-104857599")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let headers = response.headers();
    assert_eq!(
        headers[CONTENT_RANGE],
        "bytes 104857500-104857599/104857600"
    );
    assert_eq!(headers[ACCEPT_RANGES], "bytes");
    assert_eq!(headers[CONTENT_TYPE], "application/octet-stream");
    assert_eq!(response.body(), &expected(104_857_500, 104_857_599));
}

#[tokio::test]
async fn a_suffix_range_reads_the_end_of_the_file() {
    let (state, token) = launched();
    let response = handle(state, read(Some(token.secret()), Some("bytes=-100"))).await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[CONTENT_RANGE],
        "bytes 104857500-104857599/104857600"
    );
}

#[tokio::test]
async fn an_open_ended_range_is_capped_so_memory_stays_bounded() {
    let (state, token) = launched();
    let response = handle(state, read(Some(token.secret()), Some("bytes=0-"))).await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[CONTENT_RANGE],
        "bytes 0-8388607/104857600"
    );
    assert_eq!(response.body().len() as u64, FIXTURE_MAX_RANGE);
}

#[tokio::test]
async fn the_whole_100_mb_file_is_read_through_capped_ranges() {
    let (state, token) = launched();
    let (mut next, mut total, mut requests) = (0u64, 0u64, 0u32);
    while next < FIXTURE_LEN && requests < 20 {
        let range = format!("bytes={next}-");
        let response = handle(state.clone(), read(Some(token.secret()), Some(&range))).await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let body = response.body();
        assert_eq!(body[0], fixture_byte(next));
        assert_eq!(
            body[body.len() - 1],
            fixture_byte(next + body.len() as u64 - 1)
        );
        total += body.len() as u64;
        next += body.len() as u64;
        requests += 1;
    }
    assert_eq!(total, FIXTURE_LEN);
    assert_eq!(requests, 13);
}

#[tokio::test]
async fn an_unsatisfiable_range_is_refused_with_the_file_length() {
    let (state, token) = launched();
    let response = handle(state, read(Some(token.secret()), Some("bytes=104857600-"))).await;
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[CONTENT_RANGE], "bytes */104857600");
}

#[tokio::test]
async fn a_read_without_a_range_is_an_invalid_request() {
    let (state, token) = launched();
    let response = handle(state, read(Some(token.secret()), None)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json["code"], "invalid_request");
}

#[tokio::test]
async fn a_ranged_read_without_the_per_launch_token_is_refused() {
    let (state, _token) = launched();
    let response = handle(state, read(None, Some("bytes=0-99"))).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json["reason"], "missing_credentials");
}

#[tokio::test]
async fn app_pages_can_read_the_range_headers() {
    let (state, token) = launched();
    let response = handle(state, read(Some(token.secret()), Some("bytes=0-9"))).await;
    let headers = response.headers();
    assert_eq!(
        headers[http::header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
    let exposed = headers[http::header::ACCESS_CONTROL_EXPOSE_HEADERS]
        .to_str()
        .expect("ascii")
        .to_ascii_lowercase();
    assert!(exposed.contains("content-range"), "{exposed}");
}
