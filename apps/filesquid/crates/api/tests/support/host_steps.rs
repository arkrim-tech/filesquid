//! Steps for `host_descriptor_API-S-020.feature` (API-S-020) that run on the API router: the
//! descriptor on each host type, who may read it, the host type that never follows a
//! request, the `host.change` event, rereading after a resync, and redaction. The
//! `@server-start` scenarios run in the `filesquid-server` harness.

use crate::MonitorWorld;
use axum::body::Body;
use axum::http::Request;
use cucumber::{given, then, when};
use filesquid_api::auth::{Authoriser, DenyAll, LaunchToken, Principal, TokenAuthenticator};
use filesquid_api::events::{topic, EVENT_BUFFER};
use filesquid_api::host::{EnrolmentPort, HostIdentity, HostedService, OperatorName};
use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiEvent, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

/// Credential of the stand-in server member: accepted, granted nothing but what every
/// credential gets.
const MEMBER_TOKEN: &str = "member-token";

/// Per-scenario state of the host descriptor steps.
#[derive(Default)]
pub struct HostWorld {
    port: Option<Arc<SwitchablePort>>,
    token: Option<String>,
    answers: Vec<Value>,
    resynced: bool,
}

/// An enrolment port the steps switch, as the enrolment code of API-F4.1 will.
#[derive(Default)]
pub struct SwitchablePort(Mutex<Option<HostedService>>);

impl SwitchablePort {
    /// Replace the hosted-service state the port reports.
    fn set(&self, service: HostedService) {
        *self.0.lock().expect("port lock") = Some(service);
    }
}

impl EnrolmentPort for SwitchablePort {
    fn hosted_service(&self) -> HostedService {
        let current = self.0.lock().expect("port lock").clone();
        current.unwrap_or_else(HostedService::not_enrolled)
    }
}

/// Start an in-memory monitor for `state`, so refusals are journaled.
fn start_monitor(w: &mut MonitorWorld, state: &ApiState) {
    let store = Arc::new(MemoryJournal::new("host"));
    let monitor = Monitor::start(
        store.clone(),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("host"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    w.store = Some(store);
    w.monitor = Some(monitor);
}

/// A desktop host with a per-launch token and a switchable enrolment port.
fn desktop_host(w: &mut MonitorWorld) -> Arc<SwitchablePort> {
    let token = LaunchToken::generate().expect("random token");
    let state = ApiState::new();
    state.install_authoriser(Authoriser::desktop_owner(&token));
    let port = Arc::new(SwitchablePort::default());
    assert!(state.describe_host(HostIdentity::desktop(), port.clone()));
    start_monitor(w, &state);
    w.host.token = Some(token.secret().to_string());
    w.host.port = Some(port.clone());
    w.state = Some(state);
    port
}

/// `GET /api/v1/host` with `token` (if any) and extra `headers`; records status and body.
async fn read_host(w: &mut MonitorWorld, token: Option<&str>, headers: &[(&str, &str)]) {
    let mut request = Request::get("/api/v1/host");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router(w.state.clone().expect("host"))
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("answer");
    w.statuses.push(response.status().as_u16());
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    w.json = serde_json::from_slice(&body).expect("JSON body");
    w.host.answers.push(w.json.clone());
}

/// The owner's per-launch token of this scenario.
fn owner_token(w: &MonitorWorld) -> String {
    w.host.token.clone().expect("per-launch token")
}

#[given(expr = "a FileSquid desktop host that describes itself to the API")]
fn desktop_described(w: &mut MonitorWorld) {
    desktop_host(w);
}

#[given(
    expr = "a FileSquid desktop host enrolled with the hosted service at {string} with backup {word} and sync {word}"
)]
fn desktop_enrolled(w: &mut MonitorWorld, service_host: String, backup: String, sync: String) {
    let port = desktop_host(w);
    let service = HostedService::enrolled_at(&service_host, backup == "on", sync == "on")
        .expect("service host");
    port.set(service);
    let host = w.state.as_ref().expect("host").host().expect("described");
    assert!(host.refresh_hosted_service());
}

#[given(expr = "a FileSquid server host whose operator name is set to {string}")]
fn server_named(w: &mut MonitorWorld, configured: String) {
    let operator = OperatorName::parse(&configured).expect("usable operator name");
    let member = TokenAuthenticator::new(MEMBER_TOKEN, Principal::Administrator);
    let state = ApiState::new();
    state.install_authoriser(Authoriser::new(Arc::new(member), Arc::new(DenyAll)));
    let port = Arc::new(SwitchablePort::default());
    assert!(state.describe_host(HostIdentity::server(operator), port));
    start_monitor(w, &state);
    w.state = Some(state);
}

#[when(expr = "the owner's webview reads the host descriptor with the per-launch token")]
async fn owner_reads(w: &mut MonitorWorld) {
    let token = owner_token(w);
    read_host(w, Some(&token), &[]).await;
}

/// The member's credential is accepted but its policy grants nothing else: it cannot read
/// process health, and it still reads the descriptor.
#[when(expr = "a signed-in member who is not an administrator reads the host descriptor")]
async fn member_reads(w: &mut MonitorWorld) {
    let request = Request::get("/api/v1/health")
        .header("authorization", format!("Bearer {MEMBER_TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let health = router(w.state.clone().expect("host"))
        .oneshot(request)
        .await
        .expect("answer");
    assert_eq!(
        health.status().as_u16(),
        403,
        "a member is not an administrator"
    );
    read_host(w, Some(MEMBER_TOKEN), &[]).await;
}

#[when(
    expr = "a caller reads the host descriptor with headers that claim a server origin and a browser user agent"
)]
async fn spoofed_read(w: &mut MonitorWorld) {
    let token = owner_token(w);
    let headers = [
        ("origin", "https://files.example.org"),
        ("host", "files.example.org"),
        ("user-agent", "Mozilla/5.0 (Windows NT 10.0) Firefox/131.0"),
        ("x-forwarded-host", "files.example.org"),
        ("x-forwarded-proto", "https"),
    ];
    read_host(w, Some(&token), &headers).await;
}

#[when(expr = "a client reads the host descriptor without a credential")]
async fn anonymous_read(w: &mut MonitorWorld) {
    read_host(w, None, &[]).await;
}

#[then(expr = "the descriptor should say host type {string} with no operator name")]
fn type_without_operator(w: &mut MonitorWorld, host_type: String) {
    assert_eq!(w.statuses.last(), Some(&200), "{}", w.json);
    assert_eq!(w.json["schema_version"], 1);
    assert_eq!(w.json["host_type"], host_type.as_str());
    assert_eq!(w.json["operator_name"], Value::Null);
}

#[then(expr = "the descriptor should say host type {string} with operator name {string}")]
fn type_with_operator(w: &mut MonitorWorld, host_type: String, operator: String) {
    assert_eq!(w.statuses.last(), Some(&200), "{}", w.json);
    assert_eq!(w.json["host_type"], host_type.as_str());
    assert_eq!(w.json["operator_name"], operator.as_str());
    crate::shutdown_monitor(w);
}

#[then(expr = "the descriptor should still say host type {string}")]
fn still_type(w: &mut MonitorWorld, host_type: String) {
    type_without_operator(w, host_type);
    crate::shutdown_monitor(w);
}

#[then(expr = "the hosted service should be reported as not enrolled")]
fn reported_not_enrolled(w: &mut MonitorWorld) {
    let expected = serde_json::to_value(HostedService::not_enrolled()).expect("JSON");
    assert_eq!(w.json["hosted_service"], expected);
    crate::shutdown_monitor(w);
}

#[then(expr = "the hosted service should be enrolled at {string}")]
fn reported_enrolled(w: &mut MonitorWorld, service_host: String) {
    assert_eq!(w.json["hosted_service"]["enrolled"], true);
    assert_eq!(
        w.json["hosted_service"]["service_host"],
        service_host.as_str()
    );
}

#[then(expr = "backup should be reported on and sync off, as two separate values")]
fn backup_and_sync_separate(w: &mut MonitorWorld) {
    assert_eq!(w.json["hosted_service"]["backup_enabled"], true);
    assert_eq!(w.json["hosted_service"]["sync_enabled"], false);
    crate::shutdown_monitor(w);
}

#[given(
    expr = "an owner's webview and a second authenticated client subscribed to the event channel"
)]
fn two_subscribers(w: &mut MonitorWorld) {
    let bus = w.state.as_ref().expect("host").events.clone();
    w.subscriptions.push(bus.subscription().expect("slot"));
    w.subscriptions.push(bus.subscription().expect("slot"));
}

/// Switch the scenario's enrolment port and let the host publish the change.
fn switch(w: &mut MonitorWorld, service: HostedService) {
    w.host.port.as_ref().expect("port").set(service);
    let host = w.state.as_ref().expect("host").host().expect("described");
    assert!(host.refresh_hosted_service(), "the state changed");
}

#[when(expr = "the host enrols with the hosted service")]
fn host_enrols(w: &mut MonitorWorld) {
    let service = HostedService::enrolled_at("backup.filesquid.tech", true, false).expect("host");
    switch(w, service);
}

#[when(expr = "the host then turns sync on")]
fn sync_on(w: &mut MonitorWorld) {
    let service = HostedService::enrolled_at("backup.filesquid.tech", true, true).expect("host");
    switch(w, service);
}

/// Every event already queued on each open subscription, appended per subscriber.
async fn drain_all(w: &mut MonitorWorld) -> Vec<Vec<ApiEvent>> {
    let mut per_subscriber = Vec::new();
    for subscription in &mut w.subscriptions {
        let mut events = Vec::new();
        while let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(20), subscription.next()).await
        {
            events.push(event);
        }
        per_subscriber.push(events);
    }
    per_subscriber
}

#[then(expr = "each subscriber should receive two {string} events in that order")]
async fn two_events_each(w: &mut MonitorWorld, name: String) {
    let per_subscriber = drain_all(w).await;
    assert_eq!(per_subscriber.len(), 2);
    for events in &per_subscriber {
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(events.iter().all(|e| e.topic == name), "{events:?}");
    }
    w.received = per_subscriber.concat();
}

#[then(
    expr = "the first should carry the enrolled descriptor with sync off and the second the descriptor with sync on"
)]
async fn whole_descriptors_in_order(w: &mut MonitorWorld) {
    let token = owner_token(w);
    read_host(w, Some(&token), &[]).await;
    for pair in w.received.chunks(2) {
        let (first, second) = (&pair[0].data, &pair[1].data);
        assert_eq!(first["hosted_service"]["enrolled"], true);
        assert_eq!(first["hosted_service"]["sync_enabled"], false);
        assert_eq!(second["hosted_service"]["sync_enabled"], true);
        assert_eq!(
            *second, w.json,
            "the event carries the whole new descriptor"
        );
    }
    crate::shutdown_monitor(w);
}

/// The subscriber falls more than the buffer behind while the host enrols, so it receives a
/// resync hint instead of the `host.change` event it missed.
#[given(expr = "a subscriber that received an {string} hint")]
async fn resynced_subscriber(w: &mut MonitorWorld, hint: String) {
    desktop_host(w);
    let bus = w.state.as_ref().expect("host").events.clone();
    let mut subscription = bus.subscription().expect("slot");
    host_enrols(w);
    for n in 0..=EVENT_BUFFER {
        bus.publish(ApiEvent::new(
            topic::PROCESS_UPDATE,
            serde_json::json!({ "n": n }),
        ));
    }
    let first = subscription.next().await.expect("an event");
    assert_eq!(first.topic, hint);
    w.host.resynced = true;
    w.subscriptions.push(subscription);
}

#[when(expr = "it recovers its state")]
async fn recovers(w: &mut MonitorWorld) {
    assert!(w.host.resynced);
    let token = owner_token(w);
    read_host(w, Some(&token), &[]).await;
}

/// The reread answers the state the missed event carried, and the contract tells clients to
/// reread the route after `events.resync`.
#[then(expr = "it should read the host descriptor through the request route")]
fn reread_through_route(w: &mut MonitorWorld) {
    assert_eq!(w.statuses.last(), Some(&200));
    assert_eq!(w.json["hosted_service"]["enrolled"], true);
    let contract: Value = serde_json::from_str(filesquid_api::CONTRACT).expect("contract");
    let route = contract["paths"]["/api/v1/host"]["get"]["description"]
        .as_str()
        .expect("documented route");
    assert!(
        route.contains("after events.resync or a reconnect"),
        "{route}"
    );
    crate::shutdown_monitor(w);
}

#[then(expr = "it should be refused as {string} because of {string}")]
fn refused_because(w: &mut MonitorWorld, code: String, reason: String) {
    assert_eq!(w.statuses.last(), Some(&401));
    assert_eq!(w.json["code"], code.as_str());
    assert_eq!(w.json["reason"], reason.as_str());
}

#[then(expr = "the activity journal should record one {string} entry")]
fn one_entry(w: &mut MonitorWorld, operation: String) {
    let monitor = w.monitor.as_ref().expect("monitor");
    monitor.journal().flush();
    let query = JournalQuery {
        operation: Some(operation),
        ..JournalQuery::default()
    };
    let count = w
        .store
        .as_ref()
        .expect("journal")
        .count(&query)
        .expect("count");
    assert_eq!(count, 1);
    crate::shutdown_monitor(w);
}

#[when(expr = "the descriptor and a {string} payload are inspected")]
async fn inspect_both(w: &mut MonitorWorld, name: String) {
    let bus = w.state.as_ref().expect("host").events.clone();
    let mut subscription = bus.subscription().expect("slot");
    switch(w, HostedService::not_enrolled());
    switch(
        w,
        HostedService::enrolled_at("backup.filesquid.tech", true, true).expect("host"),
    );
    let token = owner_token(w);
    read_host(w, Some(&token), &[]).await;
    for _ in 0..2 {
        let event = subscription.next().await.expect("event");
        assert_eq!(event.topic, name);
        w.host.answers.push(event.data);
    }
}

#[then(
    expr = "neither should contain a token, content handle, key or key identifier, account or device identifier, e-mail address, quota, object count, size, timestamp, path or file name"
)]
fn nothing_private(w: &mut MonitorWorld) {
    let token = owner_token(w);
    assert!(w.host.answers.len() >= 3);
    for answer in &w.host.answers {
        assert_only_descriptor_fields(answer);
        let text = answer.to_string();
        assert!(!text.contains(&token), "the per-launch token leaked");
        for forbidden in ['/', '\\', '@'] {
            assert!(!text.contains(forbidden), "{text}");
        }
    }
    crate::shutdown_monitor(w);
}

/// Exactly the documented keys, booleans for the switches, and no number but the version.
fn assert_only_descriptor_fields(answer: &Value) {
    let keys: Vec<&String> = answer.as_object().expect("object").keys().collect();
    assert_eq!(
        keys,
        [
            "host_type",
            "hosted_service",
            "operator_name",
            "schema_version"
        ]
    );
    let service = answer["hosted_service"].as_object().expect("object");
    let keys: Vec<&String> = service.keys().collect();
    assert_eq!(
        keys,
        ["backup_enabled", "enrolled", "service_host", "sync_enabled"]
    );
    assert_eq!(answer["schema_version"], 1);
    for flag in ["backup_enabled", "enrolled", "sync_enabled"] {
        assert!(service[flag].is_boolean());
    }
}
