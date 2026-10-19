//! Step definitions for `content_handles_F-056.feature` (Task API-F1.3; API-S-004, API-S-005,
//! API-S-017, API-S-018, DE-S-005): content handles minted for the shared 100 MB video root,
//! read through the router as a media element reads them, with the clock moved by hand.
//!
//! The server scenarios stand in for `filesquid-server` accounts until API-F1.9 with a
//! credential whose acceptance and permission the test withdraws ([`Switchable`]): logout
//! and session end end the credential's handles, rotation stops accepting the credential,
//! and a restart is a new API state.

use crate::files_fixture::{
    pattern, send, Answer, FilesRig, Switchable, LIBRARY, OBJECT, VIDEO_LEN,
};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::auth::Authoriser;
use filesquid_api::content::{
    ContentTarget, ABSOLUTE_LIFETIME_MS, IDLE_TIMEOUT_MS, ISSUED_OPERATION,
    MAX_HANDLES_PER_CREDENTIAL,
};
use filesquid_api::files::MAX_ANSWER_BYTES;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::instrument::WithSubscriber;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// The headers every content-route answer carries, denials included.
const PRIVACY_HEADERS: [(&str, &str); 4] = [
    ("referrer-policy", "no-referrer"),
    ("cache-control", "private, no-store"),
    ("x-content-type-options", "nosniff"),
    ("content-security-policy", "default-src 'none'; sandbox"),
];

/// An unknown but well-formed handle.
const UNKNOWN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// What the content-handle scenarios keep between steps.
#[derive(Default)]
pub struct ContentWorld {
    rig: Option<FilesRig>,
    switch: Option<Arc<Switchable>>,
    minted: Value,
    url: Option<String>,
    urls: Vec<String>,
    answer: Option<Answer>,
    answers: Vec<Answer>,
    reference: Option<Answer>,
    presented: Option<String>,
    reads_before: u64,
    logs: Captured,
}

impl ContentWorld {
    /// The scenario's rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    /// The last answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("an answer")
    }

    /// The last minted content URL.
    fn url(&self) -> String {
        self.url.clone().expect("a minted handle")
    }
}

/// Every field of every `tracing` event seen while a request ran.
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

/// Mint a handle for `path` at `version` with the rig's credential.
async fn mint(rig: &FilesRig, path: &str, version: &str) -> Answer {
    let body = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "version": version, "path": path,
    });
    rig.post("/api/v1/content-handles", body).await
}

/// Mint a handle for `path` at v2 and keep its URL.
async fn mint_and_keep(w: &mut ContentWorld, path: &str) {
    let answer = mint(w.rig(), path, "v2").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    w.minted = answer.json();
    w.url = w.minted["url"].as_str().map(str::to_string);
    w.answer = Some(answer);
}

/// A request through `url` as a media element sends it: no `Authorization` header. Every
/// `tracing` event it emits is captured in `w.logs`.
async fn fetch(w: &ContentWorld, method: Method, url: &str, range: Option<&str>) -> Answer {
    let headers: Vec<(&str, &str)> = range.map(|r| ("range", r)).into_iter().collect();
    let subscriber = tracing::Dispatch::new(tracing_subscriber::registry().with(w.logs.clone()));
    // Scenarios run concurrently: drop any interest cached before this subscriber existed.
    tracing::callsite::rebuild_interest_cache();
    send(&w.rig().state, method, url, &headers, Vec::new())
        .with_subscriber(subscriber)
        .await
}

/// A Switchable server credential for Ana over the shared video root.
async fn server_rig(w: &mut ContentWorld) {
    let switch = Switchable::new("ana-token");
    let authoriser = Authoriser::new(switch.clone(), switch.clone());
    w.rig = Some(FilesRig::video_with_authoriser(authoriser, "ana-token".into()).await);
    w.switch = Some(switch);
}

/// A number written with thousands separators, such as `3,600`.
fn number(text: &str) -> i64 {
    text.replace(',', "").parse().expect("a number")
}

// ---- Background ----

#[given(expr = "the owner's library holds a 100 MB video {string} at version v2")]
async fn video_library(w: &mut MonitorWorld, _name: String) {
    w.content.rig = Some(FilesRig::with_video().await);
}

#[given(expr = "the owner's client holds a valid credential for the library")]
async fn valid_credential(w: &mut MonitorWorld) {
    let answer = w.content.rig().get("/api/v1/health").await;
    assert_eq!(answer.status, StatusCode::OK);
}

// ---- Minting ----

#[when(expr = "the client mints a content handle for {string} at version v2")]
async fn client_mints(w: &mut MonitorWorld, path: String) {
    mint_and_keep(&mut w.content, &path).await;
}

#[given(expr = "the client has minted a content handle for {string} at version v2")]
async fn client_has_minted(w: &mut MonitorWorld, path: String) {
    mint_and_keep(&mut w.content, &path).await;
}

#[given(expr = "the client has minted a content handle for {string} at version v2 on the desktop")]
async fn client_mints_on_desktop(w: &mut MonitorWorld, path: String) {
    mint_and_keep(&mut w.content, &path).await;
}

#[then(
    expr = "the answer should carry a handle, its content URL and an absolute expiry in RFC 3339 UTC"
)]
fn mint_answer(w: &mut MonitorWorld) {
    let json = &w.content.minted;
    let handle = json["handle"].as_str().expect("handle");
    assert_eq!(json["url"], format!("/api/v1/content/{handle}"));
    let expires = json["expires_at"].as_str().expect("expiry");
    let parsed =
        time::OffsetDateTime::parse(expires, &time::format_description::well_known::Rfc3339)
            .expect("RFC 3339");
    assert_eq!(parsed.offset(), time::UtcOffset::UTC);
    assert!(expires.ends_with('Z'));
    let answer = w.content.answer();
    for (name, value) in PRIVACY_HEADERS {
        assert_eq!(answer.header(name), value, "the mint answer carries {name}");
    }
}

#[then(expr = "the handle should be 43 base64url characters without padding")]
fn handle_shape(w: &mut MonitorWorld) {
    let handle = w.content.minted["handle"].as_str().expect("handle");
    assert_eq!(handle.len(), 43);
    assert!(handle
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
}

#[then(expr = "the content URL should contain neither a filesystem path nor the client's token")]
fn url_is_opaque(w: &mut MonitorWorld) {
    let url = w.content.url();
    let rig = w.content.rig();
    assert!(!url.contains(&rig.token));
    assert!(!url.contains(rig.dir.to_str().expect("dir")));
    assert!(!url.contains("holiday"), "{url}");
    assert!(!url.contains("objects/"), "{url}");
}

#[then(expr = "the activity journal should record one {string} entry without the handle")]
fn issued_without_handle(w: &mut MonitorWorld, operation: String) {
    assert_eq!(operation, ISSUED_OPERATION);
    let rig = w.content.rig();
    let issued = rig.journaled(&operation);
    assert_eq!(issued.len(), 1);
    let handle = w.content.minted["handle"].as_str().expect("handle");
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    assert!(!journal.contains(handle));
}

// ---- Reading through the content URL ----

#[when(
    regex = r"^a media element requests bytes (\d+) to (\d+) through the content URL( without an Authorization header)?$"
)]
async fn request_bytes(w: &mut MonitorWorld, first: u64, last: u64, _anonymous: String) {
    let url = w.content.url();
    let range = format!("bytes={first}-{last}");
    w.content.answer = Some(fetch(&w.content, Method::GET, &url, Some(&range)).await);
}

#[when(expr = "a media element sends HEAD through the content URL")]
async fn head_request(w: &mut MonitorWorld) {
    let url = w.content.url();
    w.content.answer = Some(fetch(&w.content, Method::HEAD, &url, None).await);
}

#[then(expr = "the answer should be {int} with {string} {string}")]
fn answer_with_header(w: &mut MonitorWorld, status: u16, name: String, value: String) {
    let answer = w.content.answer();
    assert_eq!(answer.status.as_u16(), status);
    assert_eq!(answer.header(&name.to_lowercase()), value);
}

#[then(expr = "the answer should carry {string} {string}")]
fn answer_carries(w: &mut MonitorWorld, name: String, value: String) {
    assert_eq!(w.content.answer().header(&name.to_lowercase()), value);
}

#[then(expr = "the answer should be {int} with an empty body")]
fn answer_empty(w: &mut MonitorWorld, status: u16) {
    let answer = w.content.answer();
    assert_eq!(answer.status.as_u16(), status);
    assert!(answer.body.is_empty());
    assert_eq!(answer.header("content-length"), VIDEO_LEN.to_string());
}

#[when(expr = "a media element makes a {word} request through the content URL")]
async fn request_of_kind(w: &mut MonitorWorld, kind: String) {
    let url = w.content.url();
    let unsatisfiable = format!("bytes={VIDEO_LEN}-");
    let (method, url, range) = match kind.as_str() {
        "ranged" => (Method::GET, url, Some("bytes=0-99".to_string())),
        "HEAD" => (Method::HEAD, url, None),
        "unsatisfiable" => (Method::GET, url, Some(unsatisfiable)),
        _ => (Method::GET, format!("/api/v1/content/{UNKNOWN}"), None),
    };
    let answer = fetch(&w.content, method, &url, range.as_deref()).await;
    w.content.answer = Some(answer);
}

// ---- Expiry ----

#[when(expr = "a media element requests a range every {int} seconds for {int} minutes")]
async fn steady_playback(w: &mut MonitorWorld, every: i64, minutes: i64) {
    let url = w.content.url();
    let steps = minutes * 60 / every;
    for _ in 0..steps {
        w.content.rig().clock.advance(every * 1_000);
        let answer = fetch(&w.content, Method::GET, &url, Some("bytes=0-1023")).await;
        w.content.answers.push(answer);
    }
}

#[then(expr = "every range request should be answered with {int}")]
fn every_answered(w: &mut MonitorWorld, status: u16) {
    assert!(!w.content.answers.is_empty());
    for answer in &w.content.answers {
        assert_eq!(answer.status.as_u16(), status);
    }
}

#[when(expr = "{int} seconds pass without a request through the content URL")]
fn idle_seconds(w: &mut MonitorWorld, seconds: i64) {
    w.content.rig().clock.advance(seconds * 1_000);
}

#[then(
    expr = "the next request through the content URL should be refused as forbidden with reason {string}"
)]
async fn next_refused(w: &mut MonitorWorld, reason: String) {
    let url = w.content.url();
    let answer = fetch(&w.content, Method::GET, &url, Some("bytes=0-9")).await;
    assert_refused(&answer, &reason);
}

#[when(regex = r"^a media element requests a range every (\d+) seconds for ([\d,]+) seconds$")]
async fn playback_for_seconds(w: &mut MonitorWorld, every: String, total: String) {
    let (every, total) = (number(&every), number(&total));
    let url = w.content.url();
    for _ in 0..total / every + 1 {
        w.content.rig().clock.advance(every * 1_000);
        let answer = fetch(&w.content, Method::GET, &url, Some("bytes=0-9")).await;
        w.content.answers.push(answer);
    }
}

#[then(
    regex = r#"^the first request after ([\d,]+) seconds should be refused as forbidden with reason "([^"]+)"$"#
)]
fn first_after_refused(w: &mut MonitorWorld, seconds: String, reason: String) {
    assert_eq!(number(&seconds) * 1_000, ABSOLUTE_LIFETIME_MS);
    let answers = &w.content.answers;
    let allowed = (ABSOLUTE_LIFETIME_MS / 60_000 - 1) as usize;
    for answer in &answers[..allowed] {
        assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    }
    assert_refused(&answers[allowed], &reason);
}

#[given(
    expr = "a media element has started a 16 MiB range read 1 second before the handle expires"
)]
fn started_before_expiry(w: &mut MonitorWorld) {
    w.content.rig().clock.advance(IDLE_TIMEOUT_MS - 1_000);
}

#[when(expr = "the handle expires while the answer is being sent")]
async fn expires_while_sending(w: &mut MonitorWorld) {
    let clock = w.content.rig().clock.clone();
    w.content
        .rig()
        .root
        .on_read(Box::new(move || clock.advance(ABSOLUTE_LIFETIME_MS)));
    let url = w.content.url();
    let answer = fetch(&w.content, Method::GET, &url, Some("bytes=0-")).await;
    w.content.answer = Some(answer);
}

#[then(expr = "the media element should receive all 16 MiB of the answer")]
async fn received_all(w: &mut MonitorWorld) {
    let answer = w.content.answer();
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.body.len(), MAX_ANSWER_BYTES);
    assert_eq!(answer.body, pattern(MAX_ANSWER_BYTES));
    let url = w.content.url();
    let after = fetch(&w.content, Method::GET, &url, Some("bytes=0-9")).await;
    assert_refused(&after, "invalid_credentials");
}

// ---- Bound ----

#[given(regex = r"^the client has minted ([\d,]+) content handles$")]
async fn minted_many(w: &mut MonitorWorld, count: String) {
    let count = number(&count) as usize;
    assert_eq!(count, MAX_HANDLES_PER_CREDENTIAL);
    mint_and_keep(&mut w.content, "holiday.mp4").await;
    w.content.urls.push(w.content.url());
    let credential = crate::files_fixture::sha256(&w.content.rig().token);
    let target = ContentTarget {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        version: "v2".into(),
        path: "holiday.mp4".into(),
    };
    for _ in 1..count {
        let minted = w
            .content
            .rig()
            .state
            .content_handles()
            .mint(credential, target.clone());
        let handle = minted.expect("mint").handle;
        w.content.urls.push(format!("/api/v1/content/{handle}"));
    }
}

#[when(expr = "the client mints one more content handle")]
async fn mint_one_more(w: &mut MonitorWorld) {
    mint_and_keep(&mut w.content, "holiday.mp4").await;
}

#[then(expr = "the least recently used handle should be refused as forbidden with reason {string}")]
async fn lru_refused(w: &mut MonitorWorld, reason: String) {
    let oldest = w.content.urls[0].clone();
    let answer = fetch(&w.content, Method::GET, &oldest, None).await;
    assert_refused(&answer, &reason);
}

#[then(regex = r"^the other ([\d,]+) handles should still be answered$")]
async fn others_answered(w: &mut MonitorWorld, count: String) {
    let live = w.content.rig().state.content_handles().live();
    assert_eq!(live, number(&count) as usize);
    let newest = w.content.url();
    let urls = &w.content.urls;
    for url in [&urls[1], &urls[512], &urls[urls.len() - 1], &newest] {
        let answer = fetch(&w.content, Method::HEAD, url, None).await;
        assert_eq!(answer.status, StatusCode::OK, "{url}");
    }
}

// ---- One identical denial ----

#[when(expr = "a media element requests a range through {}")]
async fn request_through(w: &mut MonitorWorld, kind: String) {
    let url = unusable_url(&mut w.content, &kind).await;
    let reference = format!("/api/v1/content/{UNKNOWN}");
    w.content.reference = Some(fetch(&w.content, Method::GET, &reference, Some("bytes=0-9")).await);
    w.content.reads_before = w.content.rig().root.reads();
    w.content.presented = url.strip_prefix("/api/v1/content/").map(str::to_string);
    w.content.answer = Some(fetch(&w.content, Method::GET, &url, Some("bytes=0-9")).await);
}

/// The content URL of the unusable handle `kind` names, minting or ending one as needed.
async fn unusable_url(w: &mut ContentWorld, kind: &str) -> String {
    match kind {
        "an unknown handle" => format!("/api/v1/content/{}", UNKNOWN.replace('A', "B")),
        "a malformed handle" => "/api/v1/content/not+a+handle".to_string(),
        "a missing handle (an empty path segment)" => "/api/v1/content/".to_string(),
        "an extra path segment after the handle" => {
            mint_and_keep(w, "holiday.mp4").await;
            format!("{}/extra", w.url())
        }
        "an expired handle" => {
            mint_and_keep(w, "holiday.mp4").await;
            w.rig().clock.advance(IDLE_TIMEOUT_MS);
            w.url()
        }
        "a handle for a file its minter may no longer read" => {
            server_rig(w).await;
            mint_and_keep(w, "holiday.mp4").await;
            let switch = w.switch.as_ref().expect("switch");
            switch.permitting.store(false, Ordering::SeqCst);
            w.url()
        }
        "a handle whose issuing credential has ended" => {
            mint_and_keep(w, "holiday.mp4").await;
            w.rig().state.end_credential(&w.rig().token);
            w.url()
        }
        other => panic!("unknown handle kind {other}"),
    }
}

#[then(expr = "the answer should be 403 forbidden with reason {string} and not retryable")]
fn answer_forbidden(w: &mut MonitorWorld, reason: String) {
    assert_refused(w.content.answer(), &reason);
}

#[then(expr = "the answer should never be 401 or 404")]
fn never_401_or_404(w: &mut MonitorWorld) {
    let status = w.content.answer().status.as_u16();
    assert!(status != 401 && status != 404, "{status}");
}

#[then(expr = "the answer should be byte for byte the same as for an unknown handle")]
fn same_as_unknown(w: &mut MonitorWorld) {
    let reference = w.content.reference.as_ref().expect("reference");
    let answer = w.content.answer();
    assert_eq!(answer.status, reference.status);
    assert_eq!(answer.headers, reference.headers);
    assert_eq!(answer.body, reference.body);
}

#[then(expr = "no file should have been read")]
fn nothing_read(w: &mut MonitorWorld) {
    assert_eq!(w.content.rig().root.reads(), w.content.reads_before);
}

#[then(expr = "the activity journal should record one {string} entry with reason {string}")]
fn one_denial(w: &mut MonitorWorld, operation: String, reason: String) {
    let denied = w.content.rig().journaled(&operation);
    let last = denied.last().expect("a denial");
    assert_eq!(last.error_code.as_deref(), Some(reason.as_str()));
    assert_eq!(last.target_ids, ["content.read"]);
    let content_denials = denied.iter().filter(|e| e.target_ids == ["content.read"]);
    let expected = 1 + usize::from(w.content.reference.is_some());
    assert_eq!(content_denials.count(), expected);
}

#[then(expr = "no log line or journal entry should contain the presented handle")]
fn handle_redacted(w: &mut MonitorWorld) {
    let Some(handle) = w.content.presented.clone().filter(|h| h.len() >= 8) else {
        return;
    };
    let handle = handle.trim_end_matches("/extra");
    let logs = w.content.logs.0.lock().expect("capture").join("\n");
    assert!(!logs.is_empty(), "the refusal was logged");
    assert!(!logs.contains(handle), "a log line carries the handle");
    let journal = serde_json::to_string(&w.content.rig().journal()).expect("json");
    assert!(!journal.contains(handle), "the journal carries the handle");
}

/// Assert `answer` is the content denial with `reason`.
fn assert_refused(answer: &Answer, reason: &str) {
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let json = answer.json();
    assert_eq!(json["code"], "forbidden", "{json}");
    assert_eq!(json["reason"], reason, "{json}");
    assert_eq!(json["retryable"], false, "{json}");
    for (name, value) in PRIVACY_HEADERS {
        assert_eq!(answer.header(name), value, "{name}");
    }
}

// ---- Nothing beyond the file version ----

#[when(expr = "a client presents the handle as a Bearer token on another API route")]
async fn handle_as_bearer(w: &mut MonitorWorld) {
    let handle = w.content.minted["handle"]
        .as_str()
        .expect("handle")
        .to_string();
    let bearer = format!("Bearer {handle}");
    let headers = [("authorization", bearer.as_str())];
    let answer = send(
        &w.content.rig().state,
        Method::GET,
        "/api/v1/health",
        &headers,
        Vec::new(),
    )
    .await;
    w.content.answer = Some(answer);
}

#[then(expr = "the call should be refused as forbidden with reason {string}")]
fn call_refused(w: &mut MonitorWorld, reason: String) {
    let json = w.content.answer().json();
    assert_eq!(w.content.answer().status, StatusCode::FORBIDDEN);
    assert_eq!(json["reason"], reason.as_str());
}

#[then(expr = "the content URL should refuse a write method")]
async fn refuses_writes(w: &mut MonitorWorld) {
    let url = w.content.url();
    for method in [Method::PUT, Method::POST, Method::DELETE] {
        let answer = fetch(&w.content, method, &url, None).await;
        assert_refused(&answer, "invalid_credentials");
    }
}

#[given(expr = "a client whose credential may not read {string}")]
async fn client_may_not_read(w: &mut MonitorWorld, _path: String) {
    server_rig(&mut w.content).await;
    let switch = w.content.switch.as_ref().expect("switch");
    switch.permitting.store(false, Ordering::SeqCst);
}

#[when(expr = "that client tries to mint a content handle for {string} at version v2")]
async fn refused_mint(w: &mut MonitorWorld, path: String) {
    w.content.answer = Some(mint(w.content.rig(), &path, "v2").await);
}

#[then(
    expr = "the minting should get the same refusal as a ranged read of {string} by that client"
)]
async fn same_refusal_as_read(w: &mut MonitorWorld, path: String) {
    let query = crate::files_fixture::query(&format!("version=v2&path={path}"));
    let uri = format!("/api/v1/files/content?{query}");
    let rig = w.content.rig();
    let read = rig
        .call(Method::GET, &uri, &[("range", "bytes=0-9")], Vec::new())
        .await;
    let minted = w.content.answer();
    assert_eq!(minted.status, StatusCode::FORBIDDEN);
    assert_eq!(minted.status, read.status);
    assert_eq!(minted.body, read.body);
}

#[when(expr = "the desktop app is relaunched")]
async fn relaunch(w: &mut MonitorWorld) {
    w.content.rig = Some(FilesRig::with_video().await);
}

#[then(
    expr = "a request through the old content URL should be refused as forbidden with reason {string}"
)]
async fn old_url_refused(w: &mut MonitorWorld, reason: String) {
    let url = w.content.url();
    let answer = fetch(&w.content, Method::GET, &url, None).await;
    assert_refused(&answer, &reason);
}

#[when(expr = "a media element sends HEAD through an unknown handle")]
async fn head_unknown(w: &mut MonitorWorld) {
    let url = format!("/api/v1/content/{UNKNOWN}");
    w.content.reference = Some(fetch(&w.content, Method::GET, &url, None).await);
    w.content.answer = Some(fetch(&w.content, Method::HEAD, &url, None).await);
}

#[then(expr = "the answer should be 403 with the same headers as a GET denial and an empty body")]
fn head_denial(w: &mut MonitorWorld) {
    let reference = w.content.reference.as_ref().expect("GET denial");
    let answer = w.content.answer();
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(answer.headers, reference.headers);
    assert!(answer.body.is_empty());
}

// ---- filesquid-server ----

#[given(
    expr = "a filesquid-server where Ana has minted a content handle for {string} at version v2"
)]
async fn ana_minted(w: &mut MonitorWorld, path: String) {
    server_rig(&mut w.content).await;
    mint_and_keep(&mut w.content, &path).await;
}

#[when(expr = "Ana logs out")]
fn ana_logs_out(w: &mut MonitorWorld) {
    end_ana_credential(w);
}

#[when(expr = "Ana's session ends")]
fn ana_session_ends(w: &mut MonitorWorld) {
    end_ana_credential(w);
}

/// End Ana's credential, as logout and session end do: its one handle ends with it.
fn end_ana_credential(w: &mut MonitorWorld) {
    let rig = w.content.rig();
    assert_eq!(rig.state.end_credential(&rig.token), 1);
}

#[when(expr = "the server token that minted the handle is rotated")]
fn token_rotated(w: &mut MonitorWorld) {
    let switch = w.content.switch.as_ref().expect("switch");
    switch.accepting.store(false, Ordering::SeqCst);
}

#[when(expr = "the server restarts")]
async fn server_restarts(w: &mut MonitorWorld) {
    let url = w.content.url();
    server_rig(&mut w.content).await;
    w.content.url = Some(url);
}

#[when(expr = "Ana's permission to read {string} is withdrawn")]
fn permission_withdrawn(w: &mut MonitorWorld, _path: String) {
    let switch = w.content.switch.as_ref().expect("switch");
    switch.permitting.store(false, Ordering::SeqCst);
}

#[then(
    expr = "Ana's next request through the content URL should be refused as forbidden with reason {string}"
)]
async fn ana_refused(w: &mut MonitorWorld, reason: String) {
    let url = w.content.url();
    let answer = fetch(&w.content, Method::GET, &url, None).await;
    assert_refused(&answer, &reason);
    w.content.answer = Some(answer);
}
