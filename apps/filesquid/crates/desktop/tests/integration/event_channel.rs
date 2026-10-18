//! Integration tests for the desktop event channel: the API event bus forwarded to the app's
//! own webviews through one counted subscription (ADR 030 Decision 4, DE-S-005).

use filesquid_api::events::{topic, EVENT_BUFFER, MAX_EVENT_BYTES, MAX_SUBSCRIBERS};
use filesquid_api::journal::MemoryJournal;
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::registry::ProcessState;
use filesquid_api::{ApiEvent, ApiState, EventBus};
use filesquid_desktop_lib::event_channel::{
    is_app_url, navigation_allowed, open_subscription, pump, register_forwarder, EventSink,
    Forwarding, PumpEnd, EVENT_NAME, FORWARDER_HEARTBEAT, FORWARDER_KIND, FORWARDER_TIMEOUT_MS,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Url;

#[path = "../support/journal.rs"]
mod journal;
use journal::{block_on, journaled};

/// A sink that records what it is given, and fails the first `fail_first` deliveries.
#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<ApiEvent>>,
    fail_first: usize,
}

impl EventSink for RecordingSink {
    fn deliver(&self, event: &ApiEvent) -> Result<(), String> {
        let mut events = self.events.lock().expect("sink lock");
        events.push(event.clone());
        if events.len() <= self.fail_first {
            return Err("webview gone at /Users/owner/secret".to_string());
        }
        Ok(())
    }
}

impl RecordingSink {
    fn topics(&self) -> Vec<String> {
        let events = self.events.lock().expect("sink lock");
        events.iter().map(|e| e.topic.clone()).collect()
    }
}

/// Forwarding settings with a fast heartbeat, no process and a fresh stop flag.
fn forwarding() -> Forwarding {
    Forwarding {
        stop: Arc::new(AtomicBool::new(false)),
        heartbeat: Duration::from_millis(10),
        process: None,
    }
}

fn marker(n: u64) -> ApiEvent {
    ApiEvent::new("test.marker", serde_json::json!({ "n": n }))
}

#[test]
fn the_event_name_is_the_one_the_ui_listens_to() {
    assert_eq!(EVENT_NAME, "filesquid-event");
}

#[tokio::test]
async fn published_events_reach_the_webview_in_order_until_the_bus_closes() {
    let state = ApiState::new();
    let subscription = open_subscription(&state.events).expect("subscription");
    for n in 0..3 {
        state.events.publish(marker(n));
    }
    drop(state);
    let sink = RecordingSink::default();
    let report = pump(subscription, &sink, forwarding()).await;
    assert_eq!(report.end, PumpEnd::BusClosed);
    assert_eq!(report.delivered, 3);
    let events = sink.events.lock().expect("lock");
    let numbers: Vec<u64> = events
        .iter()
        .map(|e| e.data["n"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, [0, 1, 2]);
}

#[tokio::test]
async fn a_webview_that_fell_behind_is_told_to_reread() {
    let bus = EventBus::new();
    let subscription = open_subscription(&bus).expect("subscription");
    for n in 0..=(EVENT_BUFFER as u64) {
        bus.publish(marker(n));
    }
    drop(bus);
    let sink = RecordingSink::default();
    pump(subscription, &sink, forwarding()).await;
    let events = sink.events.lock().expect("lock");
    assert_eq!(events[0].topic, topic::RESYNC);
    assert_eq!(events[0].data["missed"], 1);
    assert_eq!(events.len(), EVENT_BUFFER + 1);
}

#[tokio::test]
async fn an_oversized_event_reaches_the_webview_as_a_resync_hint() {
    let bus = EventBus::new();
    let subscription = open_subscription(&bus).expect("subscription");
    let big = "x".repeat(MAX_EVENT_BYTES);
    bus.publish(ApiEvent::new(
        "test.marker",
        serde_json::json!({ "big": big }),
    ));
    drop(bus);
    let sink = RecordingSink::default();
    pump(subscription, &sink, forwarding()).await;
    assert_eq!(sink.topics(), [topic::RESYNC]);
}

#[tokio::test]
async fn the_forwarder_counts_against_the_subscriber_cap() {
    let bus = EventBus::new();
    let subscription = open_subscription(&bus).expect("subscription");
    assert_eq!(bus.open_subscriptions(), 1);
    drop(subscription);
    assert_eq!(bus.open_subscriptions(), 0);
}

#[test]
fn a_refused_subscription_is_journaled_as_an_error() {
    let bus = EventBus::new();
    let held: Vec<_> = (0..MAX_SUBSCRIBERS)
        .map(|_| bus.subscription().expect("slot"))
        .collect();
    let mut refused = None;
    let entries = journaled(|| refused = Some(open_subscription(&bus).is_none()));
    assert_eq!(refused, Some(true));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].operation, "desktop.events.subscription");
    assert_eq!(entries[0].error_code.as_deref(), Some("unavailable"));
    drop(held);
}

#[test]
fn a_failed_delivery_is_journaled_without_its_detail_and_forwarding_goes_on() {
    let bus = EventBus::new();
    let subscription = open_subscription(&bus).expect("subscription");
    bus.publish(marker(1));
    bus.publish(marker(2));
    drop(bus);
    let sink = RecordingSink {
        fail_first: 1,
        ..RecordingSink::default()
    };
    let mut report = None;
    let entries = journaled(|| report = Some(block_on(pump(subscription, &sink, forwarding()))));
    let report = report.expect("report");
    assert_eq!((report.delivered, report.failed), (1, 1));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].operation, "desktop.events.forward");
    let text = serde_json::to_string(&entries[0]).expect("json");
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("test.marker"), "{text}");
}

#[tokio::test]
async fn the_forwarder_stops_when_the_app_exits() {
    let state = ApiState::new();
    let subscription = open_subscription(&state.events).expect("subscription");
    let settings = forwarding();
    let stop = settings.stop.clone();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        stop.store(true, Ordering::SeqCst);
    });
    let sink = RecordingSink::default();
    let report = tokio::time::timeout(Duration::from_secs(5), pump(subscription, &sink, settings))
        .await
        .expect("pump ends after the stop flag");
    assert_eq!(report.end, PumpEnd::Stopped);
    stopper.await.expect("stopper");
    drop(state);
}

#[tokio::test]
async fn the_forwarder_is_a_registered_process_with_a_heartbeat() {
    assert_eq!(FORWARDER_KIND, "event-forwarder");
    assert_eq!(FORWARDER_HEARTBEAT, Duration::from_secs(5));
    assert_eq!(FORWARDER_TIMEOUT_MS, 30_000);
    let state = ApiState::new();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("desktop")),
        MonitorConfig::new("desktop", std::env::temp_dir()),
        state.events.clone(),
    );
    let subscription = open_subscription(&state.events).expect("subscription");
    let mut settings = forwarding();
    settings.process = Some(Arc::new(register_forwarder(monitor.registry())));
    let running = monitor.registry().snapshot();
    let view = running
        .iter()
        .find(|p| p.kind == FORWARDER_KIND)
        .expect("registered");
    assert_eq!(view.state, ProcessState::Running);
    settings.stop.store(true, Ordering::SeqCst);
    pump(subscription, &RecordingSink::default(), settings).await;
    let finished = monitor.registry().snapshot();
    let view = finished
        .iter()
        .find(|p| p.kind == FORWARDER_KIND)
        .expect("listed");
    assert_eq!(view.state, ProcessState::Completed);
    monitor.shutdown();
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[test]
fn only_the_apps_own_pages_are_app_urls() {
    assert!(is_app_url(&url("tauri://localhost/index.html")));
    assert!(is_app_url(&url("http://tauri.localhost/")));
    assert!(is_app_url(&url(
        "https://tauri.localhost/library?view=grid"
    )));
    assert!(!is_app_url(&url("https://evil.example/")));
    assert!(!is_app_url(&url("http://tauri.localhost.evil.example/")));
    assert!(!is_app_url(&url("filesquid://localhost/api/v1/health")));
    assert_eq!(
        is_app_url(&url("http://127.0.0.1:5173/")),
        cfg!(debug_assertions)
    );
    assert!(!is_app_url(&url("http://127.0.0.1:5175/")));
}

#[test]
fn app_webviews_may_open_blank_documents() {
    assert!(navigation_allowed(&url("about:blank")));
    assert!(navigation_allowed(&url("about:srcdoc")));
    assert!(navigation_allowed(&url("tauri://localhost/index.html")));
}

#[test]
fn navigation_to_a_foreign_origin_is_refused_and_journaled_without_the_address() {
    let mut allowed = None;
    let entries = journaled(|| {
        allowed = Some(navigation_allowed(&url(
            "https://evil.example/collect?token=abc&file=private.jpg",
        )))
    });
    assert_eq!(allowed, Some(false));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].operation, "desktop.navigation.refused");
    assert_eq!(entries[0].error_code.as_deref(), Some("foreign_origin"));
    let text = serde_json::to_string(&entries[0]).expect("json");
    assert!(!text.contains("evil"), "{text}");
    assert!(!text.contains("private"), "{text}");
    assert!(!text.contains("abc"), "{text}");
}
