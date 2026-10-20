//! Unit tests for the client-events route (API-S-030; ADR 033 Decision 7): the request
//! bounds, one journal entry per delivered event after a `429` and retry, de-duplication by
//! client event ID, the principal check, the overflow schema and its rate-bound exemption,
//! and the redaction of every new event type.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::client_events::{
    ERROR_OPERATION, MAX_BODY_BYTES, MAX_EVENTS, MAX_OVERFLOW_COUNT, OVERFLOW_OPERATION,
    RATE_LIMIT, RATE_WINDOW_MS, WARN_OPERATION,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, Level, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

const PATH: &str = "/api/v1/client-events";

/// A desktop host whose clock the test moves, with the owner's token.
struct Host {
    state: ApiState,
    monitor: Monitor,
    token: String,
    now: Arc<AtomicI64>,
}

impl Host {
    fn start() -> Self {
        let now = Arc::new(AtomicI64::new(1_800_000_000_000));
        let clock = now.clone();
        let state = ApiState::with_clock(Arc::new(move || clock.load(Ordering::SeqCst)));
        let token = LaunchToken::generate().expect("token");
        state.install_authoriser(Authoriser::desktop_owner(&token));
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("host-a")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        Self {
            state,
            monitor,
            token: token.secret().to_string(),
            now,
        }
    }

    fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    async fn post_bytes(&self, body: Vec<u8>) -> (StatusCode, Option<String>, Value) {
        let request = Request::post(PATH)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("request");
        let response = router(self.state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let retry = response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().expect("ascii").to_string());
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, retry, value)
    }

    async fn post(&self, body: Value) -> (StatusCode, Option<String>, Value) {
        self.post_bytes(serde_json::to_vec(&body).expect("json"))
            .await
    }

    /// Every stored entry whose operation starts with `ui.console.`.
    fn console_entries(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            limit: 1000,
            ..JournalQuery::default()
        };
        self.monitor
            .store()
            .query(&query)
            .expect("query")
            .entries
            .into_iter()
            .filter(|e| e.operation.starts_with("ui.console."))
            .collect()
    }

    fn entries_named(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }
}

/// The `n`th client event ID: a well-formed UUID v4.
fn uuid(n: u32) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}

fn console(n: u32, level: &str) -> Value {
    json!({
        "kind": "console",
        "id": uuid(n),
        "level": level,
        "component": "fs-file-browser",
        "error_code": "fetch_failed",
        "ids": ["urn:uuid:5f0c3e44-2a7b-4c1d-9e8f-0123456789ab"]
    })
}

fn overflow(n: u32, count: Value) -> Value {
    json!({ "kind": "overflow", "id": uuid(n), "component": "fs-console-queue", "count": count })
}

fn batch(events: Vec<Value>) -> Value {
    json!({ "events": events })
}

#[tokio::test]
async fn a_full_batch_is_journaled_one_entry_per_event() {
    let host = Host::start();
    let events: Vec<Value> = (0..MAX_EVENTS as u32)
        .map(|n| console(n, if n % 2 == 0 { "warn" } else { "error" }))
        .collect();
    let (status, _, ack) = host.post(batch(events)).await;
    assert_eq!(status, StatusCode::OK, "{ack}");
    assert_eq!(ack["journaled"], MAX_EVENTS);
    assert_eq!(
        ack["acknowledged"].as_array().expect("ids").len(),
        MAX_EVENTS
    );
    assert_eq!(ack["principal"], "owner");
    let entries = host.console_entries();
    assert_eq!(entries.len(), MAX_EVENTS);
    let warns = entries.iter().filter(|e| e.operation == WARN_OPERATION);
    assert!(warns.clone().all(|e| e.level == Level::Warn));
    assert_eq!(warns.count(), MAX_EVENTS / 2);
    let errors = entries.iter().filter(|e| e.operation == ERROR_OPERATION);
    assert!(errors.clone().all(|e| e.level == Level::Error));
    assert_eq!(errors.count(), MAX_EVENTS / 2);
    let first = &entries[0];
    assert_eq!(first.actor, "owner");
    assert_eq!(first.outcome, Outcome::Info);
    assert_eq!(first.error_code.as_deref(), Some("fetch_failed"));
    assert_eq!(first.client_event_id.as_deref(), Some(uuid(0).as_str()));
    assert!(first
        .target_ids
        .contains(&"component:fs-file-browser".to_string()));
    host.monitor.shutdown();
}

#[tokio::test]
async fn more_than_twenty_events_are_refused_with_an_error_code() {
    let host = Host::start();
    let events = (0..=MAX_EVENTS as u32)
        .map(|n| console(n, "warn"))
        .collect();
    let (status, _, body) = host.post(batch(events)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_request");
    assert!(host.console_entries().is_empty());
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_body_over_sixteen_kibibytes_is_refused_with_an_error_code() {
    let host = Host::start();
    let mut body = serde_json::to_vec(&batch(vec![console(1, "warn")])).expect("json");
    body.pop();
    body.extend(std::iter::repeat_n(b' ', MAX_BODY_BYTES));
    body.push(b'}');
    let (status, _, answer) = host.post_bytes(body).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(answer["code"], "payload_too_large");
    assert!(host.console_entries().is_empty());
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_body_of_exactly_sixteen_kibibytes_is_accepted() {
    let host = Host::start();
    let mut body = serde_json::to_vec(&batch(vec![console(1, "warn")])).expect("json");
    body.pop();
    let pad = MAX_BODY_BYTES - body.len() - 1;
    body.extend(std::iter::repeat_n(b' ', pad));
    body.push(b'}');
    assert_eq!(body.len(), MAX_BODY_BYTES);
    let (status, _, answer) = host.post_bytes(body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_sixty_first_request_in_a_minute_gets_429_and_every_event_is_journaled_after_retry() {
    let host = Host::start();
    for n in 0..RATE_LIMIT as u32 {
        let (status, _, _) = host.post(batch(vec![console(n, "warn")])).await;
        assert_eq!(status, StatusCode::OK, "request {n}");
    }
    let late = batch(vec![console(1000, "error"), console(1001, "warn")]);
    let (status, retry, body) = host.post(late.clone()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "rate_limited");
    assert_eq!(body["retryable"], true);
    let seconds: i64 = retry.expect("Retry-After").parse().expect("seconds");
    assert!((1..=60).contains(&seconds), "Retry-After {seconds}");
    assert_eq!(host.console_entries().len(), RATE_LIMIT);
    host.advance(seconds * 1000);
    let (status, _, ack) = host.post(late).await;
    assert_eq!(status, StatusCode::OK, "{ack}");
    let entries = host.console_entries();
    assert_eq!(
        entries.len(),
        RATE_LIMIT + 2,
        "one entry per delivered event"
    );
    let ids: std::collections::BTreeSet<_> =
        entries.iter().map(|e| e.client_event_id.clone()).collect();
    assert_eq!(ids.len(), entries.len(), "no event merged or duplicated");
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_rate_bound_rolls_with_the_window() {
    let host = Host::start();
    for n in 0..RATE_LIMIT as u32 {
        host.post(batch(vec![console(n, "warn")])).await;
        host.advance(RATE_WINDOW_MS / RATE_LIMIT as i64);
    }
    let (status, _, _) = host.post(batch(vec![console(500, "warn")])).await;
    assert_eq!(status, StatusCode::OK, "the oldest request left the window");
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_resent_batch_is_acknowledged_without_new_entries() {
    let host = Host::start();
    let events = batch(vec![console(1, "warn"), console(2, "error")]);
    let (status, _, first) = host.post(events.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["journaled"], 2);
    let (status, _, again) = host.post(events).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["journaled"], 0);
    assert_eq!(again["acknowledged"], json!([uuid(1), uuid(2)]));
    assert_eq!(host.console_entries().len(), 2);
    host.monitor.shutdown();
}

#[tokio::test]
async fn one_id_twice_in_one_request_is_journaled_once() {
    let host = Host::start();
    let (status, _, ack) = host
        .post(batch(vec![console(7, "warn"), console(7, "warn")]))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ack["journaled"], 1);
    assert_eq!(host.console_entries().len(), 1);
    host.monitor.shutdown();
}

#[tokio::test]
async fn an_event_raised_by_another_principal_is_refused_and_journaled() {
    let host = Host::start();
    let mut event = console(1, "warn");
    event["principal"] = json!("administrator");
    let (status, _, body) = host.post(batch(vec![console(2, "warn"), event])).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["reason"], "not_permitted");
    assert!(host.console_entries().is_empty(), "nothing of the batch");
    let denied = host.entries_named("api.auth.denied");
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].target_ids, ["client_events.write"]);
    host.monitor.shutdown();
}

#[tokio::test]
async fn an_event_raised_by_the_same_principal_is_accepted() {
    let host = Host::start();
    let mut event = console(1, "warn");
    event["principal"] = json!("owner");
    let (status, _, _) = host.post(batch(vec![event])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(host.console_entries().len(), 1);
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_overflow_count_must_be_an_integer_from_one_to_a_million() {
    assert_eq!(MAX_OVERFLOW_COUNT, 1_000_000, "API-S-030 pins the bound");
    let host = Host::start();
    let refused = [
        json!(0),
        json!(1_000_001),
        json!(-1),
        json!(2.5),
        json!("12"),
        Value::Null,
    ];
    for (n, count) in refused.into_iter().enumerate() {
        let (status, _, body) = host
            .post(batch(vec![overflow(n as u32, count.clone())]))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "count {count}");
        assert_eq!(body["code"], "invalid_request");
    }
    for (n, count) in [1, 1_000_000].into_iter().enumerate() {
        let (status, _, _) = host
            .post(batch(vec![overflow(100 + n as u32, json!(count))]))
            .await;
        assert_eq!(status, StatusCode::OK, "count {count}");
    }
    let stored = host.entries_named(OVERFLOW_OPERATION);
    assert_eq!(stored.len(), 2);
    assert!(stored[1].target_ids.contains(&"count:1000000".to_string()));
    assert_eq!(stored[0].level, Level::Warn);
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_overflow_event_carries_nothing_but_its_schema() {
    let host = Host::start();
    let mut with_code = overflow(1, json!(3));
    with_code["error_code"] = json!("quota");
    let mut with_ids = overflow(2, json!(3));
    with_ids["ids"] = json!(["urn:uuid:x"]);
    let mut with_level = overflow(3, json!(3));
    with_level["level"] = json!("warn");
    let mut without_component = overflow(4, json!(3));
    without_component
        .as_object_mut()
        .expect("object")
        .remove("component");
    for event in [with_code, with_ids, with_level, without_component] {
        let (status, _, _) = host.post(batch(vec![event.clone()])).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{event}");
    }
    assert!(host.entries_named(OVERFLOW_OPERATION).is_empty());
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_lone_overflow_event_is_exempt_from_the_rate_bound() {
    let host = Host::start();
    for n in 0..RATE_LIMIT as u32 {
        host.post(batch(vec![console(n, "warn")])).await;
    }
    let (status, _, _) = host.post(batch(vec![overflow(900, json!(4))])).await;
    assert_eq!(status, StatusCode::OK, "a lone overflow is never refused");
    let mixed = batch(vec![overflow(901, json!(4)), console(902, "warn")]);
    let (status, _, _) = host.post(mixed).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "a mixed request counts"
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn lone_overflow_requests_do_not_count_toward_the_rate_bound() {
    let host = Host::start();
    for n in 0..(RATE_LIMIT as u32 + 5) {
        let (status, _, _) = host.post(batch(vec![overflow(n, json!(1))])).await;
        assert_eq!(status, StatusCode::OK);
    }
    for n in 0..RATE_LIMIT as u32 {
        let (status, _, _) = host.post(batch(vec![console(1000 + n, "warn")])).await;
        assert_eq!(status, StatusCode::OK, "console request {n}");
    }
    host.monitor.shutdown();
}

#[tokio::test]
async fn message_text_urls_file_names_and_stacks_never_reach_the_journal() {
    let host = Host::start();
    let mut event = console(1, "error");
    event["message"] = json!("Could not load Grandma's birthday.jpg");
    event["stack"] = json!("at load (https://example.org/app.js:1:2)");
    event["url"] = json!("https://example.org/library/Grandma");
    event["file_name"] = json!("birthday.jpg");
    let (status, _, _) = host.post(batch(vec![event])).await;
    assert_eq!(status, StatusCode::OK);
    let entries = host.console_entries();
    assert_eq!(entries.len(), 1);
    let stored = serde_json::to_string(&entries[0]).expect("json");
    for secret in ["Grandma", "birthday", "example.org", "app.js"] {
        assert!(
            !stored.contains(secret),
            "{secret} reached the journal: {stored}"
        );
    }
    host.monitor.shutdown();
}

#[tokio::test]
async fn structural_fields_that_are_not_opaque_tokens_are_refused() {
    let host = Host::start();
    let mut spaced = console(1, "warn");
    spaced["component"] = json!("file browser");
    let mut pathy = console(2, "warn");
    pathy["error_code"] = json!("/Users/me/photo.jpg");
    let mut id_text = console(3, "warn");
    id_text["ids"] = json!(["Grandma's birthday"]);
    let mut many_ids = console(4, "warn");
    many_ids["ids"] = json!((0..9).map(|n| format!("id-{n}")).collect::<Vec<_>>());
    let mut level = console(5, "warn");
    level["level"] = json!("info");
    let mut not_v4 = console(6, "warn");
    not_v4["id"] = json!("00000006-0000-1000-8000-000000000000");
    let mut not_uuid = console(7, "warn");
    not_uuid["id"] = json!("event-7");
    let mut kind = console(8, "warn");
    kind["kind"] = json!("log");
    let mut code_missing = console(9, "warn");
    code_missing
        .as_object_mut()
        .expect("object")
        .remove("error_code");
    for event in [
        spaced,
        pathy,
        id_text,
        many_ids,
        level,
        not_v4,
        not_uuid,
        kind,
        code_missing,
    ] {
        let (status, _, body) = host.post(batch(vec![event.clone()])).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{event}");
        assert_eq!(body["code"], "invalid_request");
        let message = body["message"].as_str().expect("message");
        assert!(!message.contains("Grandma") && !message.contains("/Users"));
    }
    assert!(host.console_entries().is_empty());
    host.monitor.shutdown();
}

#[tokio::test]
async fn malformed_json_and_an_empty_batch_are_refused() {
    let host = Host::start();
    let (status, _, _) = host.post_bytes(b"{not json".to_vec()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = host.post(batch(Vec::new())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_caller_without_a_credential_is_refused() {
    let host = Host::start();
    let request = Request::post(PATH)
        .body(Body::from(
            serde_json::to_vec(&batch(vec![console(1, "warn")])).expect("json"),
        ))
        .expect("request");
    let response = router(host.state.clone())
        .oneshot(request)
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(host.console_entries().is_empty());
    host.monitor.shutdown();
}

#[tokio::test]
async fn an_event_time_is_kept_and_a_far_future_time_is_refused() {
    let host = Host::start();
    let mut event = console(1, "warn");
    event["time"] = json!("2026-10-19T10:00:00.000Z");
    let (status, _, _) = host.post(batch(vec![event])).await;
    assert_eq!(status, StatusCode::OK);
    let stored = host.console_entries();
    assert_eq!(
        serde_json::to_value(&stored[0]).expect("json")["time"],
        "2026-10-19T10:00:00.000Z"
    );
    let mut future = console(2, "warn");
    future["time"] = json!("2999-01-01T00:00:00Z");
    let (status, _, _) = host.post(batch(vec![future])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    host.monitor.shutdown();
}

#[tokio::test]
async fn every_new_client_event_type_is_redacted_in_the_journal() {
    let host = Host::start();
    host.post(batch(vec![console(1, "warn"), console(2, "error")]))
        .await;
    host.post(batch(vec![overflow(3, json!(9))])).await;
    let entries = host.console_entries();
    assert_eq!(entries.len(), 3);
    for entry in entries {
        assert_eq!(entry, entry.clone().redacted(), "{}", entry.operation);
        assert!(entry.duration_ms.is_none());
        assert!(entry.process_id.is_none());
    }
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_acknowledgement_follows_the_durable_write() {
    let host = Host::start();
    let (status, _, _) = host.post(batch(vec![console(1, "warn")])).await;
    assert_eq!(status, StatusCode::OK);
    // No flush: the route wrote through the store before answering.
    let query = JournalQuery {
        operation: Some(WARN_OPERATION.to_string()),
        ..JournalQuery::default()
    };
    assert_eq!(host.monitor.store().count(&query).expect("count"), 1);
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_lone_overflow_event_naming_another_principal_is_counted_toward_the_rate_bound() {
    let host = Host::start();
    let mut foreign = overflow(1, json!(2));
    foreign["principal"] = json!("administrator");
    for _ in 0..RATE_LIMIT {
        let (status, _, _) = host.post(batch(vec![foreign.clone()])).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _, _) = host.post(batch(vec![foreign])).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "refusals are bounded too"
    );
    assert_eq!(host.entries_named("api.auth.denied").len(), RATE_LIMIT);
    host.monitor.shutdown();
}

#[tokio::test]
async fn malformed_and_oversized_requests_count_toward_the_rate_bound() {
    let host = Host::start();
    for n in 0..RATE_LIMIT {
        let body = if n % 2 == 0 {
            b"{not json".to_vec()
        } else {
            vec![b' '; MAX_BODY_BYTES + 1]
        };
        let (status, _, _) = host.post_bytes(body).await;
        assert!(
            [StatusCode::BAD_REQUEST, StatusCode::PAYLOAD_TOO_LARGE].contains(&status),
            "{status}"
        );
    }
    let (status, retry, _) = host.post(batch(vec![console(1, "warn")])).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(retry.is_some());
    let (status, _, _) = host.post_bytes(b"{not json".to_vec()).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "a refused request over the bound"
    );
    let (status, _, _) = host.post(batch(vec![overflow(2, json!(1))])).await;
    assert_eq!(status, StatusCode::OK, "a lone overflow is still exempt");
    host.monitor.shutdown();
}
