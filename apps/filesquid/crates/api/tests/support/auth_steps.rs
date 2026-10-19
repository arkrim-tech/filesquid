//! Steps for `deny_by_default_authorisation_F-056.feature` (API-S-004): every route, the
//! fallbacks and the host routes refuse a call that the installed authoriser does not
//! accept, with a typed, journaled denial that never contains the presented credential.

use crate::MonitorWorld;
use axum::body::Body;
use axum::http::Request;
use axum::routing::get;
use axum::Router;
use cucumber::{given, then, when};
use filesquid_api::auth::{
    self, Action, Authoriser, Guard, LaunchToken, Policy, Principal, Resource, TokenAuthenticator,
    BEARER_SUBPROTOCOL_PREFIX, EVENTS_SUBPROTOCOL,
};
use filesquid_api::host::{HostIdentity, NotEnrolled};
use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::{route_actions, router, ApiState};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

/// Every routed path that requires a credential (all but the public liveness probe), with
/// its path parameters naming the sample history's file and version.
fn protected_paths() -> Vec<String> {
    route_actions()
        .into_iter()
        .filter(|(_, action)| *action != Action::Public)
        .map(|(path, _)| crate::history_fixture::concrete(&path))
        .collect()
}

/// Start a monitor over an in-memory journal for `state` and keep both in the world.
fn start_monitor(w: &mut MonitorWorld, state: &ApiState) {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        store.clone(),
        std::sync::Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    let validation: filesquid_api::ocfl_validation::ValidationService<
        filesquid_ocfl::storage::MemoryBackend,
    > = filesquid_api::ocfl_validation::ValidationService::new(
        Arc::new(filesquid_api::ocfl_validation::MemoryFindingsStore::new()),
        monitor.clone(),
    );
    state.install_validation(Arc::new(validation));
    w.store = Some(store);
    w.monitor = Some(monitor);
}

/// A fresh per-launch token.
fn launch_token() -> LaunchToken {
    LaunchToken::generate().expect("random token")
}

#[given(expr = "a desktop FileSquid API with a per-launch token")]
fn desktop_api(w: &mut MonitorWorld) {
    let token = launch_token();
    let state = ApiState::new();
    state.install_authoriser(Authoriser::desktop_owner(&token));
    state.describe_host(HostIdentity::desktop(), Arc::new(NotEnrolled));
    crate::history_fixture::install_sample_history(&state);
    start_monitor(w, &state);
    w.secret = Some(token.secret().to_string());
    w.state = Some(state);
}

#[given(expr = "a FileSquid API router with no authoriser installed")]
fn no_authoriser(w: &mut MonitorWorld) {
    crate::shutdown_monitor(w);
    let state = ApiState::new();
    start_monitor(w, &state);
    w.state = Some(state);
}

/// Grants everything except reading the monitor.
struct NoMonitor;

impl Policy for NoMonitor {
    fn permits(&self, _principal: &Principal, action: Action, _resource: &Resource) -> bool {
        action != Action::ReadMonitor
    }
}

#[given(expr = "a desktop FileSquid API whose policy does not permit reading the monitor")]
fn restricted_api(w: &mut MonitorWorld) {
    crate::shutdown_monitor(w);
    let token = launch_token();
    let state = ApiState::new();
    let authenticator = TokenAuthenticator::new(token.secret(), Principal::Owner);
    state.install_authoriser(Authoriser::new(
        Arc::new(authenticator),
        Arc::new(NoMonitor),
    ));
    start_monitor(w, &state);
    w.secret = Some(token.secret().to_string());
    w.state = Some(state);
}

/// Send one request through `service` and record its status and body in `answers`.
async fn record(w: &mut MonitorWorld, service: Router, request: Request<Body>) {
    let response = service.oneshot(request).await.expect("answer");
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.expect("body");
    w.answers.push((status, body.to_bytes().to_vec()));
}

/// Call `path` through the API router with an optional bearer token: `POST` (an empty body)
/// and `PUT` (the default settings) for the validation command routes, `GET` for every other
/// route.
async fn call(w: &mut MonitorWorld, path: &str, token: Option<String>) {
    let (mut request, body) = match path {
        "/api/v1/admin/ocfl/validation-runs" => (Request::post(path), ""),
        "/api/v1/admin/ocfl/validation-settings" => (
            Request::put(path),
            r#"{"interval_ms":604800000,"object_cap":100000}"#,
        ),
        _ => (Request::get(path), ""),
    };
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let service = router(w.state.clone().expect("state"));
    record(w, service, request.body(Body::from(body)).expect("request")).await;
}

/// The event channel as hosts mount it: a host route guarded by [`auth::require`].
fn event_channel(state: &ApiState) -> Router {
    let guard = Guard::new(state.clone(), Action::SubscribeEvents);
    let layer = axum::middleware::from_fn_with_state(guard, auth::require);
    Router::new().route(
        "/api/v1/events",
        get(|| async { "subscribed" }).route_layer(layer),
    )
}

#[when(expr = "the owner's webview calls every protected API route with the per-launch token")]
async fn call_all_with_token(w: &mut MonitorWorld) {
    for path in protected_paths() {
        call(w, &path, w.secret.clone()).await;
    }
}

#[when(expr = "a client calls every protected API route without a token")]
async fn call_all_anonymous(w: &mut MonitorWorld) {
    for path in protected_paths() {
        call(w, &path, None).await;
    }
}

#[when(expr = "a client calls every protected API route with a token from another launch")]
async fn call_all_with_other_token(w: &mut MonitorWorld) {
    let other = launch_token().secret().to_string();
    w.presented = Some(other.clone());
    for path in protected_paths() {
        call(w, &path, Some(other.clone())).await;
    }
}

#[when(expr = "a client asks for the health status without a token")]
async fn health_anonymous(w: &mut MonitorWorld) {
    call(w, "/api/v1/health", None).await;
}

#[when(expr = "a client asks for an API path that does not exist without a token")]
async fn unknown_anonymous(w: &mut MonitorWorld) {
    call(w, "/api/v1/no-such-route", None).await;
}

#[when(expr = "the owner's webview asks for the monitor health with the per-launch token")]
async fn monitor_health(w: &mut MonitorWorld) {
    call(w, "/api/v1/admin/health", w.secret.clone()).await;
}

#[when(expr = "a client opens the event channel without a token")]
async fn events_anonymous(w: &mut MonitorWorld) {
    let service = event_channel(w.state.as_ref().expect("state"));
    let request = Request::get("/api/v1/events").body(Body::empty());
    record(w, service, request.expect("request")).await;
}

#[when(expr = "the owner's webview opens the event channel offering the token as a subprotocol")]
async fn events_with_subprotocol(w: &mut MonitorWorld) {
    let secret = w.secret.clone().expect("token");
    let offered = format!("{EVENTS_SUBPROTOCOL}, {BEARER_SUBPROTOCOL_PREFIX}{secret}");
    let service = event_channel(w.state.as_ref().expect("state"));
    let request = Request::get("/api/v1/events")
        .header("upgrade", "websocket")
        .header("sec-websocket-protocol", offered)
        .body(Body::empty());
    record(w, service, request.expect("request")).await;
}

#[when(expr = "a monitoring probe asks whether the process is up without a token")]
async fn liveness_probe(w: &mut MonitorWorld) {
    call(w, "/api/v1/health/live", None).await;
}

#[then(expr = "the probe should be told only that the process is up")]
fn probe_told_up(w: &mut MonitorWorld) {
    let (status, body) = w.answers.last().expect("answer");
    assert_eq!(*status, 200);
    let json: serde_json::Value = serde_json::from_slice(body).expect("JSON");
    assert_eq!(json, serde_json::json!({ "status": "up" }));
    crate::shutdown_monitor(w);
}

#[then(expr = "every call should be answered")]
fn all_answered(w: &mut MonitorWorld) {
    assert!(!w.answers.is_empty());
    for (status, body) in &w.answers {
        assert_eq!(*status, 200, "{}", String::from_utf8_lossy(body));
    }
    crate::shutdown_monitor(w);
}

#[then(expr = "every call should be refused as {string} because of {string}")]
fn all_refused(w: &mut MonitorWorld, code: String, reason: String) {
    assert!(!w.answers.is_empty());
    for (status, body) in &w.answers {
        let json: serde_json::Value = serde_json::from_slice(body).expect("ApiError body");
        assert!(*status == 401 || *status == 403, "{status}");
        assert_eq!(json["code"], code.as_str(), "{json}");
        assert_eq!(json["reason"], reason.as_str(), "{json}");
        assert!(json["message"].as_str().is_some_and(|m| !m.is_empty()));
    }
    crate::shutdown_monitor(w);
}

#[then(expr = "no refusal should be retryable")]
fn none_retryable(w: &mut MonitorWorld) {
    for (_, body) in &w.answers {
        let json: serde_json::Value = serde_json::from_slice(body).expect("ApiError body");
        assert_eq!(json["retryable"], false, "{json}");
    }
}

#[then(expr = "the activity journal should record one {string} entry per refused call")]
fn journal_records_denials(w: &mut MonitorWorld, operation: String) {
    let monitor = w.monitor.as_ref().expect("monitor");
    monitor.journal().flush();
    let store = w.store.as_ref().expect("store");
    let page = store.query(&JournalQuery::default()).expect("query");
    let denied = page.entries.iter().filter(|e| e.operation == operation);
    assert_eq!(denied.count(), w.answers.len());
}

#[when(expr = "the desktop host issues its per-launch token at launch")]
async fn desktop_issues_token(w: &mut MonitorWorld) {
    let monitor = w.monitor.clone().expect("monitor");
    auth::record_token_issue(&monitor, &Principal::Owner);
    monitor.journal().flush();
}

#[then(expr = "the activity journal should record one {string} entry for the owner")]
fn journal_records_issue(w: &mut MonitorWorld, operation: String) {
    let store = w.store.as_ref().expect("store");
    let page = store.query(&JournalQuery::default()).expect("query");
    let issued: Vec<_> = page
        .entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].target_ids, vec!["owner".to_string()]);
}

#[then(expr = "no journal entry should contain the per-launch token")]
fn journal_free_of_launch_token(w: &mut MonitorWorld) {
    let store = w.store.as_ref().expect("store");
    let page = store.query(&JournalQuery::default()).expect("query");
    let text = serde_json::to_string(&page.entries).expect("json");
    let secret = w.secret.clone().expect("per-launch token");
    assert!(!text.contains(&secret));
    crate::shutdown_monitor(w);
}

#[then(expr = "no journal entry should contain the presented token")]
fn journal_redacted(w: &mut MonitorWorld) {
    let store = w.store.as_ref().expect("store");
    let page = store.query(&JournalQuery::default()).expect("query");
    let text = serde_json::to_string(&page.entries).expect("json");
    let presented = w.presented.clone().expect("presented token");
    assert!(!text.contains(&presented));
    crate::shutdown_monitor(w);
}
