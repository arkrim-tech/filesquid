//! Deny-by-default authorisation (API-S-004, SRS-F-056, ADR 030 Decision 5).
//!
//! Every route the router serves, its fallbacks and the host routes (guarded with
//! [`auth::require`]) refuse a call that does not carry a credential the installed
//! [`Authoriser`] accepts, before any handler runs. Denials are typed [`ApiError`]s the UI can
//! show, and every denial is journaled without the presented credential.

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request};
use axum::routing::get;
use axum::Router;
use filesquid_api::auth::{
    self, generate_token, presented_credential, Action, Authenticator, Authoriser, Credential,
    DenialReason, Guard, HostPolicy, LaunchToken, Policy, Principal, Resource, TokenAuthenticator,
    BEARER_SUBPROTOCOL_PREFIX, EVENTS_SUBPROTOCOL,
};
use filesquid_api::journal::{JournalQuery, JournalStore, Level, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::{route_actions, routed_paths, router, ApiError, ApiState, ErrorCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

/// A token that has the shape of an opaque identifier, so the journal would keep it if it
/// were ever passed in a journal field.
const PRESENTED: &str = "ben-member-token";

/// One request through the router with an optional bearer token: status and JSON body.
async fn call(state: &ApiState, method: Method, path: &str, token: Option<&str>) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router(state.clone())
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("router answers");
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.expect("body");
    let json = serde_json::from_slice(&body.to_bytes()).unwrap_or(Value::Null);
    (status, json)
}

/// API state whose authoriser accepts `token` as the desktop owner's per-launch token.
fn owner_state() -> (ApiState, LaunchToken) {
    let token = LaunchToken::generate().expect("random token");
    let state = ApiState::new();
    assert!(state.install_authoriser(Authoriser::desktop_owner(&token)));
    (state, token)
}

/// Owner state with a running monitor over an in-memory journal.
fn journaled_owner_state() -> (ApiState, LaunchToken, Monitor, Arc<MemoryJournal>) {
    let (state, token) = owner_state();
    let store = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        store.clone(),
        MonitorConfig::new("desktop", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    (state, token, monitor, store)
}

/// Every routed path that requires a credential: all but the deliberately public ones.
fn protected_paths() -> Vec<String> {
    route_actions()
        .into_iter()
        .filter(|(_, action)| *action != Action::Public)
        .map(|(path, _)| path)
        .collect()
}

/// Assert `json` is a non-retryable denial with `code` and `reason`.
fn assert_denial(json: &Value, code: &str, reason: &str, at: &str) {
    assert_eq!(json["code"], code, "{at}: {json}");
    assert_eq!(json["reason"], reason, "{at}: {json}");
    assert_eq!(json["retryable"], false, "{at}: {json}");
    let message = json["message"].as_str().unwrap_or_default();
    assert!(!message.is_empty(), "{at}: {json}");
}

// ---- Deny by default on every route ----

#[tokio::test]
async fn without_an_installed_authoriser_every_route_is_denied() {
    let state = ApiState::new();
    for path in protected_paths() {
        let (status, json) = call(&state, Method::GET, &path, None).await;
        assert_eq!(status, 401, "{path}");
        assert_denial(&json, "unauthorized", "missing_credentials", &path);
        let (status, json) = call(&state, Method::GET, &path, Some(PRESENTED)).await;
        assert_eq!(status, 403, "{path}");
        assert_denial(&json, "forbidden", "invalid_credentials", &path);
    }
}

#[tokio::test]
async fn every_route_denies_a_call_without_the_per_launch_token() {
    let (state, _token) = owner_state();
    let paths = protected_paths();
    assert!(paths.iter().any(|p| p == "/api/v1/health"));
    assert!(paths.iter().any(|p| p == "/api/v1/openapi.json"));
    for path in paths {
        let (status, json) = call(&state, Method::GET, &path, None).await;
        assert_eq!(status, 401, "{path}");
        assert_denial(&json, "unauthorized", "missing_credentials", &path);
    }
}

#[tokio::test]
async fn every_route_denies_a_wrong_token() {
    let (state, token) = owner_state();
    let other = LaunchToken::generate().expect("random token");
    let near_miss = format!("{}0", token.secret());
    for path in protected_paths() {
        for wrong in [PRESENTED, other.secret(), near_miss.as_str(), ""] {
            let (status, json) = call(&state, Method::GET, &path, Some(wrong)).await;
            assert_eq!(status, 403, "{path} with {wrong:?}");
            assert_denial(&json, "forbidden", "invalid_credentials", &path);
        }
    }
}

#[tokio::test]
async fn every_route_answers_the_per_launch_token_holder() {
    let (state, token, monitor, _) = journaled_owner_state();
    for path in routed_paths() {
        let (status, json) = call(&state, Method::GET, &path, Some(token.secret())).await;
        assert_eq!(status, 200, "{path}: {json}");
    }
    monitor.shutdown();
}

#[tokio::test]
async fn unknown_paths_and_wrong_methods_authenticate_before_routing() {
    let (state, token) = owner_state();
    let (status, json) = call(&state, Method::GET, "/api/v1/no-such-route", None).await;
    assert_eq!(status, 401);
    assert_denial(&json, "unauthorized", "missing_credentials", "unknown path");
    let (status, json) = call(&state, Method::DELETE, "/api/v1/health", None).await;
    assert_eq!(status, 401);
    assert_denial(&json, "unauthorized", "missing_credentials", "wrong method");
    let secret = Some(token.secret());
    let (status, _) = call(&state, Method::GET, "/api/v1/no-such-route", secret).await;
    assert_eq!(status, 404);
    let (status, _) = call(&state, Method::DELETE, "/api/v1/health", secret).await;
    assert_eq!(status, 405);
}

/// A policy that grants everything except reading the monitor.
struct NoMonitor;

impl Policy for NoMonitor {
    fn permits(&self, _principal: &Principal, action: Action, _resource: &Resource) -> bool {
        action != Action::ReadMonitor
    }
}

#[tokio::test]
async fn a_principal_the_policy_does_not_permit_is_forbidden() {
    let token = LaunchToken::generate().expect("random token");
    let state = ApiState::new();
    let authenticator = TokenAuthenticator::new(token.secret(), Principal::Owner);
    state.install_authoriser(Authoriser::new(
        Arc::new(authenticator),
        Arc::new(NoMonitor),
    ));
    let secret = Some(token.secret());
    let (status, json) = call(&state, Method::GET, "/api/v1/admin/health", secret).await;
    assert_eq!(status, 403);
    assert_denial(&json, "forbidden", "not_permitted", "admin health");
    let (status, _) = call(&state, Method::GET, "/api/v1/health", secret).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn an_authoriser_can_be_installed_only_once() {
    let (state, token) = owner_state();
    let other = LaunchToken::generate().expect("random token");
    assert!(!state.install_authoriser(Authoriser::desktop_owner(&other)));
    let (status, _) = call(&state, Method::GET, "/api/v1/health", Some(other.secret())).await;
    assert_eq!(status, 403);
    let (status, _) = call(&state, Method::GET, "/api/v1/health", Some(token.secret())).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn a_denial_never_echoes_the_presented_credential() {
    let (state, _token) = owner_state();
    for path in protected_paths() {
        let (_, json) = call(&state, Method::GET, &path, Some(PRESENTED)).await;
        assert!(!json.to_string().contains(PRESENTED), "{path}: {json}");
    }
}

// ---- The one deliberately public route ----

/// Path of the public liveness probe.
const LIVE: &str = "/api/v1/health/live";

#[test]
fn only_the_liveness_route_is_public() {
    let public: Vec<String> = route_actions()
        .into_iter()
        .filter(|(_, action)| *action == Action::Public)
        .map(|(path, _)| path)
        .collect();
    assert_eq!(public, vec![LIVE.to_string()]);
    assert_eq!(protected_paths().len() + 1, routed_paths().len());
}

#[tokio::test]
async fn the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up() {
    let (state, token, monitor, store) = journaled_owner_state();
    for presented in [None, Some(PRESENTED), Some(token.secret())] {
        let (status, json) = call(&state, Method::GET, LIVE, presented).await;
        assert_eq!(status, 200, "{presented:?}");
        assert_eq!(json, serde_json::json!({ "status": "up" }));
    }
    let (status, json) = call(&ApiState::new(), Method::GET, LIVE, None).await;
    assert_eq!(status, 200, "public even without an installed authoriser");
    assert_eq!(json, serde_json::json!({ "status": "up" }));
    let refused = entries(&monitor, &store)
        .into_iter()
        .filter(|e| e.operation == "api.auth.denied")
        .count();
    assert_eq!(refused, 0, "a public call is never a refusal");
    monitor.shutdown();
}

#[tokio::test]
async fn the_liveness_route_still_refuses_an_anonymous_wrong_method() {
    let (state, _token) = owner_state();
    let (status, json) = call(&state, Method::POST, LIVE, None).await;
    assert_eq!(status, 401);
    assert_denial(&json, "unauthorized", "missing_credentials", "POST live");
}

// ---- Every route declares its action ----

#[test]
fn every_routed_path_declares_its_action() {
    let actions = route_actions();
    let paths: Vec<String> = actions.iter().map(|(path, _)| path.clone()).collect();
    assert_eq!(paths, routed_paths());
    for (path, action) in &actions {
        let expected = match path.as_str() {
            "/api/v1/health" => Action::ReadHealth,
            "/api/v1/health/live" => Action::Public,
            "/api/v1/openapi.json" => Action::ReadContract,
            admin if admin.starts_with("/api/v1/admin/") => Action::ReadMonitor,
            other => panic!("{other} has no expected action in this test"),
        };
        assert_eq!(*action, expected, "{path}");
    }
}

#[test]
fn action_names_are_unique_dotted_identifiers() {
    let names: Vec<&str> = Action::ALL.iter().map(|a| a.as_str()).collect();
    assert_eq!(
        names,
        [
            "health.read",
            "contract.read",
            "events.subscribe",
            "monitor.read",
            "public"
        ]
    );
}

#[test]
fn the_host_policy_grants_the_owner_and_the_administrator_every_action() {
    for principal in [Principal::Owner, Principal::Administrator] {
        for action in Action::ALL {
            assert!(HostPolicy.permits(&principal, action, &Resource::Host));
        }
    }
}

// ---- Host routes guarded by `auth::require` ----

/// A host route like the server's event channel, guarded the way hosts guard theirs.
fn host_router(state: &ApiState) -> Router {
    let guard = Guard::new(state.clone(), Action::SubscribeEvents);
    Router::new().route(
        "/api/v1/events",
        get(|| async { "subscribed" })
            .route_layer(axum::middleware::from_fn_with_state(guard, auth::require)),
    )
}

/// One GET through `router` with the given headers.
async fn host_call(router: Router, headers: &[(&str, &str)]) -> (u16, Value) {
    let mut request = Request::get("/api/v1/events");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("answer");
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.expect("body");
    let json = serde_json::from_slice(&body.to_bytes()).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn host_routes_are_denied_by_default() {
    let (state, token) = owner_state();
    let (status, json) = host_call(host_router(&state), &[]).await;
    assert_eq!(status, 401);
    assert_denial(&json, "unauthorized", "missing_credentials", "host route");
    let wrong = [("authorization", "Bearer nope")];
    let (status, json) = host_call(host_router(&state), &wrong).await;
    assert_eq!(status, 403);
    assert_denial(&json, "forbidden", "invalid_credentials", "host route");
    let bearer = format!("Bearer {}", token.secret());
    let (status, _) = host_call(host_router(&state), &[("authorization", &bearer)]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn a_host_route_cannot_be_made_public_through_the_guard() {
    let (state, token) = owner_state();
    let guard = Guard::new(state.clone(), Action::Public);
    let router = || {
        Router::new().route(
            "/api/v1/events",
            get(|| async { "open" }).route_layer(axum::middleware::from_fn_with_state(
                guard.clone(),
                auth::require,
            )),
        )
    };
    let (status, json) = host_call(router(), &[]).await;
    assert_eq!(status, 401, "the guard never bypasses authentication");
    assert_denial(&json, "unauthorized", "missing_credentials", "public guard");
    let bearer = format!("Bearer {}", token.secret());
    let (status, _) = host_call(router(), &[("authorization", &bearer)]).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn a_websocket_upgrade_may_carry_the_token_as_a_subprotocol() {
    let (state, token) = owner_state();
    let offered = format!(
        "{EVENTS_SUBPROTOCOL}, {BEARER_SUBPROTOCOL_PREFIX}{}",
        token.secret()
    );
    let upgrade = [
        ("upgrade", "websocket"),
        ("sec-websocket-protocol", offered.as_str()),
    ];
    let (status, _) = host_call(host_router(&state), &upgrade).await;
    assert_eq!(status, 200);
    let plain = [("sec-websocket-protocol", offered.as_str())];
    let (status, _) = host_call(host_router(&state), &plain).await;
    assert_eq!(
        status, 401,
        "the subprotocol carrier counts only on upgrades"
    );
}

// ---- Credentials ----

/// Headers from name/value pairs.
fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(*name, HeaderValue::from_str(value).expect("header value"));
    }
    map
}

#[test]
fn the_bearer_scheme_is_read_case_insensitively() {
    for value in ["Bearer abc", "bearer abc", "BEARER abc"] {
        let map = headers(&[("authorization", value)]);
        assert_eq!(
            presented_credential(&map),
            Credential::Bearer("abc"),
            "{value}"
        );
    }
    assert_eq!(presented_credential(&HeaderMap::new()), Credential::Absent);
}

#[test]
fn a_present_but_unusable_authorization_header_is_not_a_missing_one() {
    for value in ["Basic abc", "Bearer", "Token abc", "abc"] {
        let map = headers(&[("authorization", value)]);
        assert_eq!(presented_credential(&map), Credential::Unusable, "{value}");
    }
}

#[test]
fn an_unusable_header_never_falls_back_to_the_subprotocol() {
    let map = headers(&[
        ("authorization", "Basic abc"),
        ("upgrade", "websocket"),
        (
            "sec-websocket-protocol",
            "filesquid.events.v1, filesquid.bearer.abc",
        ),
    ]);
    assert_eq!(presented_credential(&map), Credential::Unusable);
}

#[tokio::test]
async fn a_non_bearer_authorization_header_is_refused_as_invalid_credentials() {
    let (state, token) = owner_state();
    for value in [format!("Basic {}", token.secret()), "Bearer".to_string()] {
        let request = Request::get("/api/v1/health")
            .header(header::AUTHORIZATION, value.as_str())
            .body(Body::empty())
            .expect("request");
        let response = router(state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        assert_eq!(response.status().as_u16(), 403, "{value}");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: Value = serde_json::from_slice(&body).expect("json");
        assert_denial(&json, "forbidden", "invalid_credentials", &value);
    }
}

#[tokio::test]
async fn an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol() {
    let (state, token) = owner_state();
    let offered = format!(
        "{EVENTS_SUBPROTOCOL}, {BEARER_SUBPROTOCOL_PREFIX}{}",
        token.secret()
    );
    let upgrade = [
        ("authorization", "Basic abc"),
        ("upgrade", "websocket"),
        ("sec-websocket-protocol", offered.as_str()),
    ];
    let (status, json) = host_call(host_router(&state), &upgrade).await;
    assert_eq!(status, 403);
    assert_denial(&json, "forbidden", "invalid_credentials", "unusable header");
}

#[test]
fn an_empty_bearer_token_is_presented_and_then_rejected() {
    let map = headers(&[("authorization", "Bearer ")]);
    assert_eq!(presented_credential(&map), Credential::Bearer(""));
    let authenticator = TokenAuthenticator::new("abc", Principal::Owner);
    assert!(authenticator.authenticate("").is_none());
}

#[test]
fn the_subprotocol_carrier_is_read_from_any_offered_entry() {
    let map = headers(&[
        ("upgrade", "WebSocket"),
        ("sec-websocket-protocol", "filesquid.events.v1"),
        ("sec-websocket-protocol", "filesquid.bearer.abc"),
    ]);
    assert_eq!(presented_credential(&map), Credential::Bearer("abc"));
}

#[test]
fn the_subprotocol_token_counts_only_next_to_the_events_subprotocol() {
    let alone = headers(&[
        ("upgrade", "websocket"),
        ("sec-websocket-protocol", "filesquid.bearer.abc"),
    ]);
    assert_eq!(presented_credential(&alone), Credential::Absent);
    let other = headers(&[
        ("upgrade", "websocket"),
        ("sec-websocket-protocol", "graphql-ws, filesquid.bearer.abc"),
    ]);
    assert_eq!(presented_credential(&other), Credential::Absent);
}

#[test]
fn the_token_authenticator_accepts_only_the_exact_token() {
    let authenticator = TokenAuthenticator::new("abc123", Principal::Administrator);
    assert_eq!(
        authenticator.authenticate("abc123"),
        Some(Principal::Administrator)
    );
    for wrong in ["abc12", "abc1234", "ABC123", "", "abc124"] {
        assert!(authenticator.authenticate(wrong).is_none(), "{wrong}");
    }
}

#[test]
fn launch_tokens_are_unique_256_bit_hex() {
    let a = LaunchToken::generate().expect("random token");
    let b = LaunchToken::generate().expect("random token");
    assert_ne!(a.secret(), b.secret());
    for token in [a.secret(), b.secret()] {
        assert_eq!(token.len(), 64);
        assert!(token
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
    let raw = generate_token().expect("random token");
    assert_eq!(raw.len(), 64);
}

#[test]
fn debug_output_never_shows_a_secret() {
    let token = LaunchToken::generate().expect("random token");
    assert!(!format!("{token:?}").contains(token.secret()));
    let authoriser = Authoriser::desktop_owner(&token);
    assert!(!format!("{authoriser:?}").contains(token.secret()));
    let authenticator = TokenAuthenticator::new(token.secret(), Principal::Owner);
    assert!(!format!("{authenticator:?}").contains(token.secret()));
}

// ---- Typed denials ----

#[test]
fn denials_map_reasons_to_codes_and_are_not_retryable() {
    let expected = [
        (DenialReason::MissingCredentials, ErrorCode::Unauthorized),
        (DenialReason::InvalidCredentials, ErrorCode::Forbidden),
        (DenialReason::NotPermitted, ErrorCode::Forbidden),
    ];
    assert_eq!(expected.len(), DenialReason::ALL.len());
    for (reason, code) in expected {
        let error = ApiError::denied(reason);
        assert_eq!(error.code(), code, "{reason:?}");
        assert_eq!(error.reason(), Some(reason));
        assert!(!error.retryable());
    }
    assert_eq!(ApiError::new(ErrorCode::Internal, "x").reason(), None);
}

#[test]
fn an_error_without_a_reason_omits_the_member() {
    let json = serde_json::to_value(ApiError::new(ErrorCode::NotFound, "Gone.")).expect("json");
    assert!(json.get("reason").is_none(), "{json}");
}

// ---- Journal ----

/// Every journal entry of `store`, oldest first.
fn entries(monitor: &Monitor, store: &MemoryJournal) -> Vec<filesquid_api::journal::JournalEntry> {
    monitor.journal().flush();
    store
        .query(&JournalQuery::default())
        .expect("query")
        .entries
}

#[tokio::test]
async fn every_denial_is_journaled_without_the_presented_credential() {
    let (state, _token, monitor, store) = journaled_owner_state();
    call(
        &state,
        Method::GET,
        "/api/v1/admin/journal",
        Some(PRESENTED),
    )
    .await;
    call(&state, Method::GET, "/api/v1/health", None).await;
    let denied: Vec<_> = entries(&monitor, &store)
        .into_iter()
        .filter(|e| e.operation == "api.auth.denied")
        .collect();
    assert_eq!(denied.len(), 2, "{denied:?}");
    assert_eq!(denied[0].error_code.as_deref(), Some("invalid_credentials"));
    assert_eq!(denied[0].target_ids, vec!["monitor.read".to_string()]);
    assert_eq!(denied[1].error_code.as_deref(), Some("missing_credentials"));
    assert_eq!(denied[1].target_ids, vec!["health.read".to_string()]);
    for entry in &denied {
        assert_eq!(entry.actor, "anonymous");
        assert_eq!(entry.outcome, Outcome::Failed);
        assert_eq!(entry.level, Level::Warn);
        let text = serde_json::to_string(entry).expect("json");
        assert!(!text.contains(PRESENTED), "{text}");
    }
    monitor.shutdown();
}

#[tokio::test]
async fn a_not_permitted_denial_names_the_principal_and_the_action() {
    let token = LaunchToken::generate().expect("random token");
    let state = ApiState::new();
    let authenticator = TokenAuthenticator::new(token.secret(), Principal::Owner);
    state.install_authoriser(Authoriser::new(
        Arc::new(authenticator),
        Arc::new(NoMonitor),
    ));
    let store = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        store.clone(),
        MonitorConfig::new("desktop", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    call(
        &state,
        Method::GET,
        "/api/v1/admin/metrics",
        Some(token.secret()),
    )
    .await;
    let denied: Vec<_> = entries(&monitor, &store)
        .into_iter()
        .filter(|e| e.operation == "api.auth.denied")
        .collect();
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].actor, "owner");
    assert_eq!(denied[0].error_code.as_deref(), Some("not_permitted"));
    assert_eq!(denied[0].target_ids, vec!["monitor.read".to_string()]);
    let text = serde_json::to_string(&denied[0]).expect("json");
    assert!(!text.contains(token.secret()), "{text}");
    monitor.shutdown();
}

#[test]
fn the_token_issued_entry_has_no_field_that_could_carry_a_secret() {
    let entry = auth::token_issued_entry(&Principal::Owner);
    assert_eq!(entry.operation, "api.auth.token_issued");
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(entry.target_ids, vec!["owner".to_string()]);
    let json = serde_json::to_value(entry.redacted()).expect("json");
    let mut fields: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "actor",
            "host",
            "level",
            "operation",
            "outcome",
            "target_ids",
            "time"
        ],
        "only fixed, opaque fields; no free text"
    );
}

#[tokio::test]
async fn fallback_refusals_are_journaled_with_the_no_route_target() {
    let (state, _token, monitor, store) = journaled_owner_state();
    call(&state, Method::GET, "/api/v1/no-such-route", None).await;
    call(&state, Method::DELETE, "/api/v1/health", None).await;
    let denied: Vec<_> = entries(&monitor, &store)
        .into_iter()
        .filter(|e| e.operation == "api.auth.denied")
        .collect();
    assert_eq!(denied.len(), 2);
    for entry in denied {
        assert_eq!(entry.target_ids, vec![auth::NO_ROUTE_TARGET.to_string()]);
    }
    monitor.shutdown();
}
