//! Steps for `event_channel_F-056.feature` (API-S-003): the router answers the same through
//! every host's address form, and the event channel is bounded and tells a client that
//! missed events to reread.

use crate::MonitorWorld;
use axum::body::Body;
use axum::http::Request;
use cucumber::{given, then, when};
use filesquid_api::events::{topic, EventResync, EVENT_BUFFER, MAX_EVENT_BYTES, MAX_SUBSCRIBERS};
use filesquid_api::{router, ApiEvent, ErrorCode, EventBus};
use http_body_util::BodyExt;
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;

/// Address forms a host hands to the router: server path, macOS/Linux and Windows schemes.
const HEALTH_URIS: [&str; 3] = [
    "/api/v1/health",
    "filesquid://localhost/api/v1/health",
    "http://filesquid.localhost/api/v1/health",
];

#[when(expr = "a client asks for the health report through each host's address form")]
async fn health_everywhere(w: &mut MonitorWorld) {
    let state = w.state.clone().expect("router");
    for uri in HEALTH_URIS {
        let request = Request::get(uri)
            .header("authorization", format!("Bearer {}", crate::ADMIN_TOKEN))
            .body(Body::empty())
            .expect("request");
        let response = router(state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status().as_u16();
        let body = response.into_body().collect().await.expect("body");
        w.answers.push((status, body.to_bytes().to_vec()));
    }
}

#[then(expr = "every address form should get the same answer")]
fn same_answer(w: &mut MonitorWorld) {
    assert_eq!(w.answers.len(), HEALTH_URIS.len());
    assert_eq!(w.answers[0].0, 200);
    assert!(w.answers.iter().all(|a| *a == w.answers[0]));
}

#[given(expr = "an event channel with one subscriber")]
fn one_subscriber(w: &mut MonitorWorld) {
    open_one_subscription(w);
}

#[given(expr = "an event channel with one subscriber that is not reading")]
fn one_idle_subscriber(w: &mut MonitorWorld) {
    open_one_subscription(w);
}

/// Create a channel with one open subscription; nothing reads it until a `Then` step.
fn open_one_subscription(w: &mut MonitorWorld) {
    let bus = EventBus::new();
    w.subscriptions.push(bus.subscription().expect("slot"));
    w.channel = Some(bus);
}

#[when(expr = "a process update is published")]
fn publish_process_update(w: &mut MonitorWorld) {
    let bus = w.channel.as_ref().expect("channel");
    bus.publish(ApiEvent::new(topic::PROCESS_UPDATE, json!({ "done": 1 })));
}

#[when(expr = "as many events as the buffer holds are published")]
fn publish_buffer_full(w: &mut MonitorWorld) {
    publish_numbered(w, EVENT_BUFFER);
}

#[when(expr = "one more event than the buffer holds is published")]
fn publish_buffer_plus_one(w: &mut MonitorWorld) {
    publish_numbered(w, EVENT_BUFFER + 1);
}

#[when(expr = "an event one byte over the size cap is published")]
fn publish_oversized(w: &mut MonitorWorld) {
    let empty = ApiEvent::new(topic::JOURNAL_ENTRY, json!(""));
    let overhead = serde_json::to_vec(&empty).expect("encode").len();
    let fill = "x".repeat(MAX_EVENT_BYTES + 1 - overhead);
    let bus = w.channel.as_ref().expect("channel");
    bus.publish(ApiEvent::new(topic::JOURNAL_ENTRY, json!(fill)));
}

/// Publish `count` `process.update` events numbered from 0.
fn publish_numbered(w: &mut MonitorWorld, count: usize) {
    let bus = w.channel.as_ref().expect("channel");
    (0..count).for_each(|n| {
        bus.publish(ApiEvent::new(topic::PROCESS_UPDATE, json!({ "n": n })));
    });
}

/// Read up to `count` events from the first subscription into `w.received`.
async fn receive(w: &mut MonitorWorld, count: usize) {
    let sub = w.subscriptions.first_mut().expect("subscriber");
    for _ in 0..count {
        let next = tokio::time::timeout(Duration::from_secs(1), sub.next()).await;
        w.received
            .push(next.expect("event available").expect("bus open"));
    }
}

#[then(expr = "the subscriber should receive a {string} event")]
async fn receives_topic(w: &mut MonitorWorld, expected: String) {
    receive(w, 1).await;
    assert_eq!(w.received[0].topic, expected);
}

#[then(expr = "the subscriber should receive every event in order")]
async fn receives_all_in_order(w: &mut MonitorWorld) {
    receive(w, EVENT_BUFFER).await;
    let order: Vec<_> = w.received.iter().map(|e| e.data["n"].clone()).collect();
    let expected: Vec<_> = (0..EVENT_BUFFER).map(|n| json!(n)).collect();
    assert_eq!(order, expected);
}

#[then(expr = "the subscriber should first receive an {string} hint for {int} missed event")]
async fn receives_hint(w: &mut MonitorWorld, expected: String, missed: u64) {
    receive(w, 1).await;
    assert_eq!(w.received[0].topic, expected);
    let hint: EventResync = serde_json::from_value(w.received[0].data.clone()).expect("hint");
    assert_eq!(hint.missed, missed);
}

#[then(expr = "then the newest events the buffer holds")]
async fn receives_newest(w: &mut MonitorWorld) {
    receive(w, EVENT_BUFFER).await;
    assert_eq!(w.received[1].data["n"], 1);
    assert_eq!(w.received[EVENT_BUFFER].data["n"], EVENT_BUFFER);
}

#[given(expr = "an event channel with as many subscribers as the cap allows")]
fn full_channel(w: &mut MonitorWorld) {
    let bus = EventBus::new();
    w.subscriptions = (0..MAX_SUBSCRIBERS)
        .map(|_| bus.subscription().expect("slot"))
        .collect();
    w.channel = Some(bus);
}

#[when(expr = "one more client subscribes")]
fn one_more(w: &mut MonitorWorld) {
    let bus = w.channel.as_ref().expect("channel");
    w.refused = bus.subscription().err();
}

#[then(expr = "the subscription should be refused with a retryable {string} error")]
fn refused(w: &mut MonitorWorld, code: String) {
    let error = w.refused.as_ref().expect("refused");
    assert_eq!(json!(error.code()), json!(code));
    assert_eq!(error.code(), ErrorCode::Unavailable);
    assert!(error.retryable());
}

#[when(expr = "one subscriber disconnects")]
fn disconnect(w: &mut MonitorWorld) {
    w.subscriptions.pop();
}

#[then(expr = "a new client should be able to subscribe")]
fn resubscribe(w: &mut MonitorWorld) {
    let bus = w.channel.as_ref().expect("channel");
    w.subscriptions
        .push(bus.subscription().expect("freed slot"));
    assert_eq!(bus.open_subscriptions(), MAX_SUBSCRIBERS);
}
