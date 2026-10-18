//! Unit tests for the in-process event channel that hosts forward to UIs (API-S-003,
//! ADR 030). Every cap is tested at its boundary: the per-subscriber buffer
//! ([`EVENT_BUFFER`]), the encoded event size ([`MAX_EVENT_BYTES`]) and the number of client
//! subscriptions ([`MAX_SUBSCRIBERS`]).

use filesquid_api::events::{
    topic, EventResync, EventSubscription, EVENT_BUFFER, MAX_EVENT_BYTES, MAX_SUBSCRIBERS,
};
use filesquid_api::{ApiEvent, ApiState, ErrorCode, EventBus};
use serde_json::{json, Value};

/// A `journal.entry` event whose JSON encoding is exactly `bytes` long.
fn event_of_encoded_size(bytes: usize) -> ApiEvent {
    let empty = ApiEvent::new(topic::JOURNAL_ENTRY, json!(""));
    let overhead = serde_json::to_vec(&empty).expect("encode").len();
    let event = ApiEvent::new(topic::JOURNAL_ENTRY, json!("x".repeat(bytes - overhead)));
    assert_eq!(serde_json::to_vec(&event).expect("encode").len(), bytes);
    event
}

/// A small numbered `process.update` event.
fn numbered(n: usize) -> ApiEvent {
    ApiEvent::new(topic::PROCESS_UPDATE, json!({ "n": n }))
}

/// Read `count` events from `sub`; every read must be immediately available.
async fn take(sub: &mut EventSubscription, count: usize) -> Vec<ApiEvent> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let next = tokio::time::timeout(std::time::Duration::from_secs(1), sub.next())
            .await
            .expect("event available");
        out.push(next.expect("bus open"));
    }
    out
}

fn missed(event: &ApiEvent) -> u64 {
    assert_eq!(event.topic, topic::RESYNC, "expected a resync hint");
    let hint: EventResync = serde_json::from_value(event.data.clone()).expect("EventResync");
    hint.missed
}

#[tokio::test]
async fn subscribers_receive_published_events() {
    let state = ApiState::new();
    let mut rx = state.events.subscribe();
    state.events.publish(ApiEvent::new(
        topic::PROCESS_UPDATE,
        serde_json::json!({ "done": 1, "total": 2 }),
    ));
    let event = rx.recv().await.expect("event delivered");
    assert_eq!(event.topic, topic::PROCESS_UPDATE);
    assert_eq!(event.data["done"], 1);
}

#[test]
fn publishing_without_subscribers_is_not_an_error() {
    let state = ApiState::new();
    let delivered = state
        .events
        .publish(ApiEvent::new(topic::HEALTH_CHANGE, Value::Null));
    assert_eq!(delivered, 0);
}

#[test]
fn the_documented_limits_have_their_declared_values() {
    assert_eq!(EVENT_BUFFER, 256);
    assert_eq!(MAX_EVENT_BYTES, 65_536);
    assert_eq!(MAX_SUBSCRIBERS, 64);
    assert_eq!(
        topic::ALL,
        [
            "process.update",
            "journal.entry",
            "health.change",
            "events.resync",
            "host.change"
        ]
    );
}

#[tokio::test]
async fn a_subscriber_exactly_one_buffer_behind_loses_nothing() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    (0..EVENT_BUFFER).for_each(|n| {
        bus.publish(numbered(n));
    });
    let got = take(&mut sub, EVENT_BUFFER).await;
    let order: Vec<Value> = got.iter().map(|e| e.data["n"].clone()).collect();
    let expected: Vec<Value> = (0..EVENT_BUFFER).map(|n| json!(n)).collect();
    assert_eq!(order, expected);
}

#[tokio::test]
async fn one_event_past_the_buffer_becomes_a_resync_hint_then_the_newest_events() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    (0..=EVENT_BUFFER).for_each(|n| {
        bus.publish(numbered(n));
    });
    let got = take(&mut sub, EVENT_BUFFER + 1).await;
    assert_eq!(missed(&got[0]), 1);
    assert_eq!(
        got[1].data["n"], 1,
        "oldest retained event follows the hint"
    );
    assert_eq!(got[EVENT_BUFFER].data["n"], EVENT_BUFFER);
}

#[tokio::test]
async fn a_slow_subscriber_cannot_block_publishers_or_grow_memory() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    let total = 10_000;
    (0..total).for_each(|n| {
        bus.publish(numbered(n));
    });
    let got = take(&mut sub, EVENT_BUFFER + 1).await;
    assert_eq!(missed(&got[0]), (total - EVENT_BUFFER) as u64);
    assert_eq!(got[EVENT_BUFFER].data["n"], total - 1);
}

#[tokio::test]
async fn an_event_at_the_size_cap_is_delivered_unchanged() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    let event = event_of_encoded_size(MAX_EVENT_BYTES);
    assert_eq!(bus.publish(event.clone()), 1);
    assert_eq!(take(&mut sub, 1).await, vec![event]);
    assert_eq!(bus.oversized_events(), 0);
}

#[tokio::test]
async fn an_event_one_byte_over_the_size_cap_becomes_a_resync_hint() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    assert_eq!(bus.publish(event_of_encoded_size(MAX_EVENT_BYTES + 1)), 1);
    let got = take(&mut sub, 1).await;
    assert_eq!(missed(&got[0]), 1);
    assert_eq!(bus.oversized_events(), 1);
}

#[test]
fn the_subscriber_cap_refuses_one_more_with_a_retryable_error() {
    let bus = EventBus::new();
    let open: Vec<EventSubscription> = (0..MAX_SUBSCRIBERS)
        .map(|_| bus.subscription().expect("slot under the cap"))
        .collect();
    assert_eq!(bus.open_subscriptions(), MAX_SUBSCRIBERS);
    let refused = bus.subscription().expect_err("cap reached");
    assert_eq!(refused.code(), ErrorCode::Unavailable);
    assert!(refused.retryable());
    assert!(!refused.message().is_empty());
    drop(open);
}

#[test]
fn dropping_a_subscription_frees_its_slot_and_its_receiver() {
    let bus = EventBus::new();
    let mut open: Vec<EventSubscription> = (0..MAX_SUBSCRIBERS)
        .map(|_| bus.subscription().expect("slot"))
        .collect();
    assert_eq!(bus.subscriber_count(), MAX_SUBSCRIBERS);
    open.pop();
    assert_eq!(bus.open_subscriptions(), MAX_SUBSCRIBERS - 1);
    assert_eq!(bus.subscriber_count(), MAX_SUBSCRIBERS - 1);
    open.push(bus.subscription().expect("freed slot is reusable"));
    open.clear();
    assert_eq!(bus.open_subscriptions(), 0);
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn concurrent_subscribers_never_exceed_the_cap() {
    let bus = EventBus::new();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let bus = bus.clone();
            std::thread::spawn(move || {
                (0..MAX_SUBSCRIBERS)
                    .filter_map(|_| bus.subscription().ok())
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let held: Vec<EventSubscription> = threads
        .into_iter()
        .flat_map(|t| t.join().expect("thread"))
        .collect();
    assert_eq!(held.len(), MAX_SUBSCRIBERS);
    assert_eq!(bus.open_subscriptions(), MAX_SUBSCRIBERS);
}

#[tokio::test]
async fn a_closed_bus_ends_the_subscription() {
    let bus = EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    bus.publish(numbered(7));
    drop(bus);
    assert_eq!(
        sub.next().await.map(|e| e.data["n"].clone()),
        Some(json!(7))
    );
    assert_eq!(sub.next().await, None);
}

#[test]
fn a_resync_hint_carries_only_the_missed_count() {
    let hint = ApiEvent::resync(3);
    assert_eq!(hint.topic, topic::RESYNC);
    assert_eq!(hint.data, json!({ "missed": 3 }));
}
