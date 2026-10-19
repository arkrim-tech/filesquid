//! Integration tests of content handles for media elements (Task API-F1.3; API-S-004,
//! API-S-005, API-S-017, API-S-018; ADR 030 Decision 5) through the router, against a real
//! OCFL storage root on the local filesystem, with an injected clock: minting, the content
//! route's ranged reads and `HEAD`, the privacy headers, sliding and absolute expiry, a read
//! that outlives its handle, eviction at 1,024 per credential, invalidation when the
//! credential or the permission ends, one identical denial for every unusable handle, and
//! redaction of the handle from answers, the journal and the logs.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{pattern, send, Answer, FilesRig, Switchable, LIBRARY, OBJECT};
use filesquid_api::auth::Authoriser;
use filesquid_api::content::{
    ContentTarget, ABSOLUTE_LIFETIME_MS, IDLE_TIMEOUT_MS, ISSUED_OPERATION,
    MAX_HANDLES_PER_CREDENTIAL,
};
use filesquid_api::files::MAX_ANSWER_BYTES;
use filesquid_api::journal::Outcome;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// The video every test reads: 3 MiB plus a little, so ranges and the length are distinct.
const VIDEO_LEN: usize = 3 * 1024 * 1024 + 4321;

/// The headers every content-route answer carries, denials included (API-S-018).
const PRIVACY_HEADERS: [(&str, &str); 4] = [
    ("referrer-policy", "no-referrer"),
    ("cache-control", "private, no-store"),
    ("x-content-type-options", "nosniff"),
    ("content-security-policy", "default-src 'none'; sandbox"),
];

/// A rig whose object holds `holiday.mp4` at v1 and a changed one at v2.
async fn video_rig() -> FilesRig {
    let rig = FilesRig::with_files(&[("holiday.mp4", &pattern(VIDEO_LEN))]).await;
    rig.commit(&[("holiday.mp4", &pattern(VIDEO_LEN + 1))])
        .await;
    rig
}

/// Mint a handle for `path` at `version` with the rig's credential.
async fn mint(rig: &FilesRig, version: &str, path: &str) -> Answer {
    let request = json!({
        "library_id": LIBRARY,
        "object_id": OBJECT,
        "version": version,
        "path": path,
    });
    rig.post("/api/v1/content-handles", request).await
}

/// Mint a handle for `holiday.mp4` at v2 and return its URL.
async fn minted_url(rig: &FilesRig) -> String {
    let answer = mint(rig, "v2", "holiday.mp4").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()["url"].as_str().expect("url").to_string()
}

/// A request through a content URL: no `Authorization` header, as a media element sends it.
async fn fetch(rig: &FilesRig, method: Method, url: &str, range: Option<&str>) -> Answer {
    let headers: Vec<(&str, &str)> = range.map(|r| ("range", r)).into_iter().collect();
    send(&rig.state, method, url, &headers, Vec::new()).await
}

/// Assert `answer` carries the four privacy headers.
fn assert_privacy_headers(answer: &Answer, at: &str) {
    for (name, value) in PRIVACY_HEADERS {
        assert_eq!(answer.header(name), value, "{at}: {name}");
    }
}

/// Assert `answer` is the one content-handle denial.
fn assert_denied(answer: &Answer, at: &str) {
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{at}");
    let json = answer.json();
    assert_eq!(json["code"], "forbidden", "{at}: {json}");
    assert_eq!(json["reason"], "invalid_credentials", "{at}: {json}");
    assert_eq!(json["retryable"], false, "{at}: {json}");
    assert_privacy_headers(answer, at);
}

// ---- Minting ----

#[tokio::test]
async fn a_mint_answers_a_43_character_handle_its_url_and_an_rfc3339_expiry() {
    let rig = video_rig().await;
    let answer = mint(&rig, "v2", "holiday.mp4").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let json = answer.json();
    let handle = json["handle"].as_str().expect("handle");
    assert_eq!(handle.len(), 43);
    assert!(handle
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    assert_eq!(json["url"], format!("/api/v1/content/{handle}"));
    assert_eq!(json["expires_at"], "2026-10-18T01:00:00Z");
    assert!(!json["url"].as_str().expect("url").contains(&rig.token));
    assert!(
        !json.to_string().contains("holiday"),
        "no path in the answer"
    );
}

#[tokio::test]
async fn every_handle_is_new_random_bytes() {
    let rig = video_rig().await;
    let first = mint(&rig, "v2", "holiday.mp4").await.json();
    let second = mint(&rig, "v2", "holiday.mp4").await.json();
    assert_ne!(first["handle"], second["handle"]);
}

#[tokio::test]
async fn a_mint_is_journaled_without_the_handle() {
    let rig = video_rig().await;
    let json = mint(&rig, "v2", "holiday.mp4").await.json();
    let handle = json["handle"].as_str().expect("handle").to_string();
    let issued = rig.journaled(ISSUED_OPERATION);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].outcome, Outcome::Succeeded);
    assert_eq!(issued[0].actor, "owner");
    let targets = &issued[0].target_ids;
    assert!(targets.contains(&OBJECT.to_string()), "{targets:?}");
    assert!(targets.contains(&"v2".to_string()), "{targets:?}");
    assert!(
        targets.contains(&"expires:2026-10-18T01:00:00Z".to_string()),
        "{targets:?}"
    );
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    assert!(!journal.contains(&handle));
}

#[tokio::test]
async fn a_mint_for_a_missing_file_or_version_is_not_found() {
    let rig = video_rig().await;
    for (version, path) in [("v2", "missing.mp4"), ("v9", "holiday.mp4")] {
        let answer = mint(&rig, version, path).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{version} {path}");
    }
    let refused = rig.journaled(ISSUED_OPERATION);
    assert_eq!(refused.len(), 2);
    assert!(refused.iter().all(|e| e.outcome == Outcome::Failed));
}

#[tokio::test]
async fn a_mint_must_name_one_explicit_version() {
    let rig = video_rig().await;
    for version in ["head", "", "2"] {
        let answer = mint(&rig, version, "holiday.mp4").await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{version:?}");
    }
}

#[tokio::test]
async fn minting_is_refused_exactly_like_the_ranged_read() {
    let switch = Switchable::new("ana-token");
    let authoriser = Authoriser::new(switch.clone(), switch.clone());
    let rig = FilesRig::with_authoriser(authoriser, "ana-token".to_string()).await;
    rig.commit(&[("holiday.mp4", &pattern(100))]).await;
    switch.permitting.store(false, Ordering::SeqCst);
    let minted = mint(&rig, "v1", "holiday.mp4").await;
    let read_uri = format!(
        "/api/v1/files/content?{}",
        files_fixture::query("path=holiday.mp4&version=v1")
    );
    let read = rig.get(&read_uri).await;
    assert_eq!(minted.status, StatusCode::FORBIDDEN);
    assert_eq!(minted.status, read.status);
    assert_eq!(minted.body, read.body);
    let denied = rig.journaled("api.auth.denied");
    assert_eq!(denied.len(), 2);
    assert!(denied.iter().all(|e| e.target_ids == ["files.read"]));
}

#[tokio::test]
async fn a_content_handle_cannot_mint_another_or_act_as_a_bearer_token() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let handle = url.rsplit('/').next().expect("handle").to_string();
    let bearer = format!("Bearer {handle}");
    let body = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "version": "v2", "path": "holiday.mp4"
    });
    let headers = [
        ("authorization", bearer.as_str()),
        ("content-type", "application/json"),
    ];
    let remint = send(
        &rig.state,
        Method::POST,
        "/api/v1/content-handles",
        &headers,
        body.to_string().into_bytes(),
    )
    .await;
    assert_eq!(remint.status, StatusCode::FORBIDDEN);
    assert_eq!(remint.json()["reason"], "invalid_credentials");
    let other = send(
        &rig.state,
        Method::GET,
        "/api/v1/health",
        &headers[..1],
        Vec::new(),
    )
    .await;
    assert_eq!(other.status, StatusCode::FORBIDDEN);
}

// ---- The content route ----

#[tokio::test]
async fn a_media_element_reads_a_range_through_the_handle() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let answer = fetch(&rig, Method::GET, &url, Some("bytes=0-1048575")).await;
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    let length = VIDEO_LEN + 1;
    assert_eq!(
        answer.header("content-range"),
        format!("bytes 0-1048575/{length}")
    );
    assert_eq!(answer.header("accept-ranges"), "bytes");
    assert_eq!(answer.header("content-type"), "video/mp4");
    assert_eq!(answer.body, pattern(length)[..1_048_576]);
    assert_privacy_headers(&answer, "ranged");
}

#[tokio::test]
async fn head_through_the_handle_describes_the_file_without_a_body() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let answer = fetch(&rig, Method::HEAD, &url, None).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.is_empty());
    assert_eq!(answer.header("content-length"), (VIDEO_LEN + 1).to_string());
    assert_eq!(answer.header("accept-ranges"), "bytes");
    assert_eq!(answer.header("content-type"), "video/mp4");
    assert_privacy_headers(&answer, "HEAD");
}

#[tokio::test]
async fn an_unsatisfiable_range_through_the_handle_keeps_its_416() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let length = VIDEO_LEN + 1;
    let range = format!("bytes={length}-{}", length + 100);
    let answer = fetch(&rig, Method::GET, &url, Some(&range)).await;
    assert_eq!(answer.status.as_u16(), 416);
    assert_eq!(answer.header("content-range"), format!("bytes */{length}"));
    assert_privacy_headers(&answer, "unsatisfiable");
}

#[tokio::test]
async fn a_handle_serves_only_the_version_it_names() {
    let rig = video_rig().await;
    let old = mint(&rig, "v1", "holiday.mp4").await.json();
    let url = old["url"].as_str().expect("url");
    let answer = fetch(&rig, Method::GET, url, Some("bytes=0-")).await;
    let expected = format!("bytes 0-{}/{VIDEO_LEN}", VIDEO_LEN - 1);
    assert_eq!(answer.header("content-range"), expected);
    assert_eq!(answer.body, pattern(VIDEO_LEN));
    rig.commit(&[("holiday.mp4", b"replaced")]).await;
    let after = fetch(&rig, Method::GET, url, Some("bytes=0-")).await;
    assert_eq!(after.body, pattern(VIDEO_LEN), "still v1 after a new head");
}

#[tokio::test]
async fn the_content_route_refuses_every_other_method() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let reference = fetch(&rig, Method::GET, "/api/v1/content/unknown", None).await;
    for method in [Method::POST, Method::PUT, Method::DELETE, Method::PATCH] {
        let answer = fetch(&rig, method.clone(), &url, None).await;
        assert_denied(&answer, method.as_str());
        assert_eq!(answer.body, reference.body, "{method}");
    }
}

#[tokio::test]
async fn an_authorization_header_never_replaces_the_handle() {
    let rig = video_rig().await;
    let bearer = format!("Bearer {}", rig.token);
    let url = format!("/api/v1/content/{}", rig.token);
    let answer = send(
        &rig.state,
        Method::GET,
        &url,
        &[("authorization", &bearer)],
        Vec::new(),
    )
    .await;
    assert_denied(&answer, "token in place of a handle");
}

// ---- Expiry ----

#[tokio::test]
async fn a_handle_expires_after_300_idle_seconds() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    rig.clock.advance(IDLE_TIMEOUT_MS - 1);
    let alive = fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await;
    assert_eq!(alive.status, StatusCode::PARTIAL_CONTENT);
    rig.clock.advance(IDLE_TIMEOUT_MS);
    assert_denied(
        &fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await,
        "idle",
    );
}

#[tokio::test]
async fn playback_keeps_the_handle_alive_within_the_idle_window() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    for minute in 0..12 {
        rig.clock.advance(240_000);
        let answer = fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await;
        assert_eq!(
            answer.status,
            StatusCode::PARTIAL_CONTENT,
            "after {minute} steps"
        );
    }
}

#[tokio::test]
async fn a_handle_expires_3600_seconds_after_minting_even_in_use() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let steps = ABSOLUTE_LIFETIME_MS / 60_000;
    for step in 1..steps {
        rig.clock.advance(60_000);
        let answer = fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await;
        assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT, "step {step}");
    }
    rig.clock.advance(60_000);
    assert_denied(
        &fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await,
        "absolute",
    );
}

#[tokio::test]
async fn a_range_read_that_outlives_its_handle_is_not_cut_off() {
    let len = MAX_ANSWER_BYTES + 10;
    let rig = FilesRig::with_files(&[("big.mp4", &pattern(len))]).await;
    let url = mint(&rig, "v1", "big.mp4").await.json()["url"]
        .as_str()
        .expect("url")
        .to_string();
    rig.clock.advance(IDLE_TIMEOUT_MS - 1_000);
    let clock = rig.clock.clone();
    rig.root
        .on_read(Box::new(move || clock.advance(ABSOLUTE_LIFETIME_MS)));
    let answer = fetch(&rig, Method::GET, &url, Some("bytes=0-")).await;
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.body.len(), MAX_ANSWER_BYTES, "all 16 MiB arrive");
    assert_eq!(answer.body, pattern(len)[..MAX_ANSWER_BYTES]);
    assert_denied(
        &fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await,
        "next request",
    );
}

// ---- Bound and invalidation ----

#[tokio::test]
async fn the_1025th_handle_evicts_the_least_recently_used_one() {
    let rig = video_rig().await;
    let mut urls = vec![minted_url(&rig).await];
    // The middle of the bound is filled straight in the store, for speed; the first and the
    // boundary handles go through the router.
    let target = ContentTarget {
        library_id: LIBRARY.to_string(),
        object_id: OBJECT.to_string(),
        version: "v2".to_string(),
        path: "holiday.mp4".to_string(),
    };
    let credential = files_fixture::sha256(&rig.token);
    for _ in 2..MAX_HANDLES_PER_CREDENTIAL {
        let minted = rig.state.content_handles().mint(credential, target.clone());
        urls.push(format!("/api/v1/content/{}", minted.expect("mint").handle));
    }
    urls.push(minted_url(&rig).await);
    assert_eq!(
        rig.state.content_handles().live(),
        MAX_HANDLES_PER_CREDENTIAL
    );
    rig.clock.advance(1);
    let used = fetch(&rig, Method::HEAD, &urls[0], None).await;
    assert_eq!(
        used.status,
        StatusCode::OK,
        "the oldest handle is used again"
    );
    let newest = minted_url(&rig).await;
    assert_denied(&fetch(&rig, Method::GET, &urls[1], None).await, "evicted");
    assert_eq!(
        rig.state.content_handles().live(),
        MAX_HANDLES_PER_CREDENTIAL
    );
    let sample = [&urls[0], &urls[2], &urls[512], &urls[1023], &newest];
    for url in sample {
        assert_eq!(
            fetch(&rig, Method::HEAD, url, None).await.status,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn expired_handles_are_removed_when_minting() {
    let rig = video_rig().await;
    for _ in 0..10 {
        minted_url(&rig).await;
    }
    assert_eq!(rig.state.content_handles().live(), 10);
    rig.clock.advance(IDLE_TIMEOUT_MS);
    minted_url(&rig).await;
    assert_eq!(rig.state.content_handles().live(), 1);
}

#[tokio::test]
async fn a_handle_ends_with_the_credential_that_minted_it() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    rig.state.end_credential(&rig.token);
    assert_denied(
        &fetch(&rig, Method::GET, &url, None).await,
        "ended credential",
    );
}

#[tokio::test]
async fn a_relaunch_ends_every_handle() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let relaunched = FilesRig::new().await;
    let answer = send(&relaunched.state, Method::GET, &url, &[], Vec::new()).await;
    assert_denied(&answer, "relaunch");
}

#[tokio::test]
async fn the_credential_and_the_policy_are_checked_again_at_every_request() {
    for withdraw in ["rotated", "permission"] {
        let switch = Switchable::new("ana-token");
        let authoriser = Authoriser::new(switch.clone(), switch.clone());
        let rig = FilesRig::with_authoriser(authoriser, "ana-token".to_string()).await;
        rig.commit(&[("holiday.mp4", &pattern(100))]).await;
        let url = mint(&rig, "v1", "holiday.mp4").await.json()["url"]
            .as_str()
            .expect("url")
            .to_string();
        assert_eq!(
            fetch(&rig, Method::HEAD, &url, None).await.status,
            StatusCode::OK
        );
        match withdraw {
            "rotated" => switch.accepting.store(false, Ordering::SeqCst),
            _ => switch.permitting.store(false, Ordering::SeqCst),
        }
        assert_denied(&fetch(&rig, Method::GET, &url, None).await, withdraw);
        let denied = rig.journaled("api.auth.denied");
        assert_eq!(denied.len(), 1, "{withdraw}");
        assert_eq!(denied[0].error_code.as_deref(), Some("invalid_credentials"));
    }
}

// ---- One identical denial ----

#[tokio::test]
async fn every_unusable_handle_gets_one_identical_denial_and_no_file_is_read() {
    let rig = video_rig().await;
    let expired = minted_url(&rig).await;
    rig.clock.advance(IDLE_TIMEOUT_MS);
    let revoked_rig = video_rig().await;
    let revoked = minted_url(&revoked_rig).await;
    revoked_rig.state.end_credential(&revoked_rig.token);
    let reference = fetch(
        &rig,
        Method::GET,
        "/api/v1/content/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        None,
    )
    .await;
    assert_denied(&reference, "unknown");
    let cases = [
        ("malformed", "/api/v1/content/not+a+handle".to_string()),
        ("too long", format!("/api/v1/content/{}", "A".repeat(44))),
        ("missing", "/api/v1/content/".to_string()),
        ("extra segment", format!("{expired}/x")),
        ("expired", expired.clone()),
        ("double slash", "/api/v1/content//x".to_string()),
    ];
    let before = rig.root.reads();
    for (name, url) in &cases {
        let answer = fetch(&rig, Method::GET, url, Some("bytes=0-9")).await;
        assert_denied(&answer, name);
        assert_eq!(answer.headers, reference.headers, "{name}");
        assert_eq!(answer.body, reference.body, "{name}");
        let head = fetch(&rig, Method::HEAD, url, None).await;
        assert_eq!(head.status, reference.status, "{name}");
        assert_eq!(head.headers, reference.headers, "HEAD {name}");
        assert!(head.body.is_empty(), "HEAD {name}");
    }
    assert_eq!(
        rig.root.reads(),
        before,
        "no file is read before a handle is accepted"
    );
    let other = fetch(&revoked_rig, Method::GET, &revoked, None).await;
    assert_eq!(other.headers, reference.headers, "revoked");
    assert_eq!(other.body, reference.body, "revoked");
    let denied = rig.journaled("api.auth.denied");
    assert_eq!(denied.len(), 2 * cases.len() + 1);
    assert!(denied
        .iter()
        .all(|e| e.error_code.as_deref() == Some("invalid_credentials")
            && e.target_ids == ["content.read"]));
}

#[tokio::test]
async fn the_denial_is_never_401_or_404_even_without_any_handle() {
    let rig = video_rig().await;
    for url in [
        "/api/v1/content/",
        "/api/v1/content/a/b/c",
        "/api/v1/content/%2F",
    ] {
        let answer = fetch(&rig, Method::GET, url, None).await;
        assert_denied(&answer, url);
    }
}

// ---- Redaction ----

/// Every field value and message of every `tracing` event, as text.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> Layer<S> for Captured {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut text = Text(String::new());
        event.record(&mut text);
        self.0.lock().expect("capture").push(text.0);
    }
}

/// Collects every field of one event.
struct Text(String);

impl Visit for Text {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!(" {}={value:?}", field.name()));
    }
}

#[tokio::test]
async fn no_log_line_journal_entry_or_debug_output_carries_a_handle() {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::registry().with(captured.clone());
    let _guard = tracing::subscriber::set_default(subscriber);
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let handle = url.rsplit('/').next().expect("handle").to_string();
    fetch(&rig, Method::GET, &url, Some("bytes=0-9")).await;
    rig.clock.advance(IDLE_TIMEOUT_MS);
    fetch(&rig, Method::GET, &url, None).await;
    fetch(&rig, Method::GET, &format!("{url}/extra"), None).await;
    let lines = captured.0.lock().expect("capture").join("\n");
    assert!(!lines.is_empty(), "the refusals were logged");
    assert!(!lines.contains(&handle), "a handle reached a log line");
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    assert!(!journal.contains(&handle), "a handle reached the journal");
    assert!(!format!("{:?}", rig.state).contains(&handle));
    assert!(!format!("{:?}", rig.state.content_handles()).contains(&handle));
}

#[tokio::test]
async fn an_error_answer_never_echoes_the_handle() {
    let rig = video_rig().await;
    let url = minted_url(&rig).await;
    let handle = url.rsplit('/').next().expect("handle").to_string();
    rig.clock.advance(ABSOLUTE_LIFETIME_MS);
    let answer = fetch(&rig, Method::GET, &url, None).await;
    let text = String::from_utf8_lossy(&answer.body).to_string();
    assert!(!text.contains(&handle));
    let headers = format!("{:?}", answer.headers);
    assert!(!headers.contains(&handle));
    let _: Value = answer.json();
}
