//! Unit tests for the host descriptor (API-S-020): `GET /api/v1/host` through the router on
//! every host type, who may read it, the host type that never follows a request, the
//! not-enrolled default and its invariants, the operator-name bounds, the `host.change`
//! event, and redaction of the answer and the event payload.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use filesquid_api::auth::{Action, Authoriser, DenyAll, Principal, TokenAuthenticator};
use filesquid_api::events::topic;
use filesquid_api::host::{
    EnrolmentPort, HostIdentity, HostType, HostedService, NotEnrolled, OperatorName,
    OperatorNameError, ServiceHostError, DESCRIPTOR_SCHEMA_VERSION, MAX_OPERATOR_NAME_CHARS,
};
use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{route_actions, router, ApiEvent, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Bearer token accepted by every state built here.
const TOKEN: &str = "host-token";

/// API state accepting [`TOKEN`] as the server administrator, described as `identity`.
fn described(identity: HostIdentity) -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    assert!(state.describe_host(identity, Arc::new(NotEnrolled)));
    state
}

/// `GET uri` through a fresh router with `headers`; returns status and JSON body.
async fn read(state: &ApiState, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, Value) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = router(state.clone())
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("router answers");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// `GET /api/v1/host` with [`TOKEN`].
async fn read_host(state: &ApiState) -> (StatusCode, Value) {
    let bearer = format!("Bearer {TOKEN}");
    read(state, "/api/v1/host", &[("authorization", &bearer)]).await
}

/// The not-enrolled `hosted_service` value.
fn not_enrolled() -> Value {
    json!({
        "enrolled": false,
        "backup_enabled": false,
        "sync_enabled": false,
        "service_host": null
    })
}

fn operator(name: &str) -> OperatorName {
    OperatorName::parse(name)
        .expect("valid operator name")
        .expect("not blank")
}

// ---- The route on every host type ----

#[tokio::test]
async fn the_desktop_describes_itself_with_no_operator_and_not_enrolled() {
    let (status, body) = read_host(&described(HostIdentity::desktop())).await;
    assert_eq!(status, StatusCode::OK);
    let expected = json!({
        "schema_version": 1,
        "host_type": "desktop",
        "operator_name": null,
        "hosted_service": not_enrolled()
    });
    assert_eq!(body, expected);
}

#[tokio::test]
async fn a_server_describes_itself_with_its_operator() {
    let identity = HostIdentity::server(Some(operator("Garderen Family")));
    let (status, body) = read_host(&described(identity)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["host_type"], "server");
    assert_eq!(body["operator_name"], "Garderen Family");
    assert_eq!(body["hosted_service"], not_enrolled());
}

#[tokio::test]
async fn an_unnamed_server_reports_a_null_operator() {
    let (_, body) = read_host(&described(HostIdentity::server(None))).await;
    assert_eq!(body["host_type"], "server");
    assert_eq!(body["operator_name"], Value::Null);
}

#[tokio::test]
async fn the_browser_host_describes_itself_with_no_operator() {
    let (_, body) = read_host(&described(HostIdentity::browser())).await;
    assert_eq!(body["host_type"], "browser");
    assert_eq!(body["operator_name"], Value::Null);
}

#[test]
fn only_a_server_identity_can_carry_an_operator() {
    assert_eq!(HostIdentity::desktop().operator_name(), None);
    assert_eq!(HostIdentity::browser().operator_name(), None);
    let named = HostIdentity::server(Some(operator("Ops")));
    assert_eq!(named.operator_name().map(OperatorName::as_str), Some("Ops"));
    assert_eq!(named.host_type(), HostType::Server);
}

#[tokio::test]
async fn the_descriptor_has_exactly_its_documented_fields() {
    let (_, body) = read_host(&described(HostIdentity::desktop())).await;
    let keys: Vec<&str> = body
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys.len(),
        4,
        "schema_version, host_type, operator_name, hosted_service: {keys:?}"
    );
    let service = body["hosted_service"].as_object().expect("object");
    assert_eq!(service.len(), 4, "{service:?}");
    assert_eq!(body["schema_version"], DESCRIPTOR_SCHEMA_VERSION);
    assert_eq!(DESCRIPTOR_SCHEMA_VERSION, 1);
}

#[tokio::test]
async fn before_a_host_describes_itself_the_route_is_unavailable() {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    let (status, body) = read_host(&state).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "unavailable");
    assert_eq!(body["retryable"], true);
}

#[tokio::test]
async fn a_host_describes_itself_once_and_the_first_description_stays() {
    let state = described(HostIdentity::desktop());
    assert!(!state.describe_host(HostIdentity::server(None), Arc::new(NotEnrolled)));
    let (_, body) = read_host(&state).await;
    assert_eq!(body["host_type"], "desktop");
}

#[test]
fn the_route_is_host_read() {
    assert_eq!(Action::ReadHost.as_str(), "host.read");
    assert!(Action::ALL.contains(&Action::ReadHost));
    assert!(route_actions().contains(&("/api/v1/host".to_string(), Action::ReadHost)));
}

// ---- Who may read it ----

#[tokio::test]
async fn the_desktop_owner_reads_it_with_the_per_launch_token() {
    let token = filesquid_api::auth::LaunchToken::generate().expect("token");
    let state = ApiState::new();
    state.install_authoriser(Authoriser::desktop_owner(&token));
    state.describe_host(HostIdentity::desktop(), Arc::new(NotEnrolled));
    let bearer = format!("Bearer {}", token.secret());
    let (status, _) = read(&state, "/api/v1/host", &[("authorization", &bearer)]).await;
    assert_eq!(status, StatusCode::OK);
}

/// A credential the host accepts but whose policy grants nothing, the shape of a server
/// member who is not an administrator: it reads the descriptor and nothing else.
#[tokio::test]
async fn every_accepted_credential_reads_it_even_one_granted_nothing_else() {
    // No member principal exists before API-F1.9: the label is inert, `DenyAll` grants nothing.
    let member = TokenAuthenticator::new(TOKEN, Principal::Administrator);
    let state = ApiState::new();
    state.install_authoriser(Authoriser::new(Arc::new(member), Arc::new(DenyAll)));
    state.describe_host(HostIdentity::server(None), Arc::new(NotEnrolled));
    let (status, body) = read_host(&state).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bearer = format!("Bearer {TOKEN}");
    let (health, body) = read(&state, "/api/v1/health", &[("authorization", &bearer)]).await;
    assert_eq!(health, StatusCode::FORBIDDEN);
    assert_eq!(body["reason"], "not_permitted");
}

#[test]
fn host_read_is_granted_to_every_authenticated_principal_whatever_the_policy() {
    let authoriser = Authoriser::new(Arc::new(DenyAll), Arc::new(DenyAll));
    let resource = filesquid_api::auth::Resource::Host;
    for principal in [Principal::Owner, Principal::Administrator] {
        assert_eq!(
            authoriser.authorise(&principal, Action::ReadHost, &resource),
            Ok(())
        );
        assert!(authoriser
            .authorise(&principal, Action::ReadHealth, &resource)
            .is_err());
    }
}

#[tokio::test]
async fn a_missing_credential_is_refused() {
    let (status, body) = read(&described(HostIdentity::desktop()), "/api/v1/host", &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");
    assert_eq!(body["reason"], "missing_credentials");
}

#[tokio::test]
async fn a_wrong_credential_is_refused() {
    let state = described(HostIdentity::desktop());
    let (status, body) = read(&state, "/api/v1/host", &[("authorization", "Bearer nope")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["reason"], "invalid_credentials");
}

// ---- The host type never follows a request ----

#[tokio::test]
async fn the_host_type_never_follows_a_request_header_or_address() {
    let state = described(HostIdentity::desktop());
    let bearer = format!("Bearer {TOKEN}");
    let spoofed: [(&str, &str); 8] = [
        ("authorization", &bearer),
        ("origin", "https://server.example"),
        (
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64) Chrome/130.0 browser",
        ),
        ("host", "filesquid.example:8083"),
        ("x-forwarded-proto", "https"),
        ("x-forwarded-host", "server.example"),
        ("sec-fetch-site", "cross-site"),
        ("x-filesquid-host-type", "server"),
    ];
    for uri in [
        "/api/v1/host",
        "https://server.example/api/v1/host",
        "filesquid://localhost/api/v1/host",
        "http://filesquid.localhost/api/v1/host",
    ] {
        let (status, body) = read(&state, uri, &spoofed).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body["host_type"], "desktop", "{uri}");
        assert_eq!(body["operator_name"], Value::Null, "{uri}");
    }
}

// ---- Operator-name bounds ----

#[test]
fn an_operator_name_is_trimmed() {
    assert_eq!(operator("  Garderen Family \t").as_str(), "Garderen Family");
}

#[test]
fn a_blank_operator_name_is_no_operator_name() {
    for blank in ["", " ", "\t", " \t  \t", "\n"] {
        assert_eq!(OperatorName::parse(blank), Ok(None), "{blank:?}");
    }
}

#[test]
fn an_operator_name_holds_1_to_256_characters() {
    assert_eq!(MAX_OPERATOR_NAME_CHARS, 256);
    assert_eq!(operator("A").as_str(), "A");
    let longest = "é".repeat(256);
    assert_eq!(operator(&longest).as_str(), longest);
    let padded = format!("  {}  ", "x".repeat(256));
    assert_eq!(operator(&padded).as_str().chars().count(), 256);
    assert_eq!(
        OperatorName::parse(&"é".repeat(257)),
        Err(OperatorNameError::TooLong)
    );
}

#[test]
fn an_operator_name_with_a_control_character_is_refused() {
    for bad in [
        "Ops\u{0}Team",
        "Ops\u{7}",
        "Ops\nTeam",
        "Ops\u{1b}[31m",
        "Ops\u{85}x",
    ] {
        assert_eq!(
            OperatorName::parse(bad),
            Err(OperatorNameError::ControlCharacter),
            "{bad:?}"
        );
    }
}

#[test]
fn an_operator_name_error_never_contains_the_value() {
    let long = "secret-operator-".repeat(20);
    for (value, error) in [
        (long.as_str(), OperatorName::parse(&long).unwrap_err()),
        (
            "Ops\u{7}Team",
            OperatorName::parse("Ops\u{7}Team").unwrap_err(),
        ),
    ] {
        let text = error.to_string();
        assert!(!text.contains(value), "{text}");
        assert!(!text.contains("secret-operator"), "{text}");
        assert!(!text.contains("Ops"), "{text}");
    }
}

// ---- Hosted service: not-enrolled default and enrolled invariants ----

#[test]
fn the_default_port_reports_exactly_not_enrolled() {
    let service = NotEnrolled.hosted_service();
    assert_eq!(service, HostedService::not_enrolled());
    assert!(!service.enrolled());
    assert!(!service.backup_enabled());
    assert!(!service.sync_enabled());
    assert_eq!(service.service_host(), None);
    assert_eq!(
        serde_json::to_value(&service).expect("json"),
        not_enrolled()
    );
}

#[test]
fn an_enrolled_service_reports_backup_and_sync_separately() {
    let service = HostedService::enrolled_at("backup.filesquid.tech", true, false).expect("valid");
    assert_eq!(
        serde_json::to_value(&service).expect("json"),
        json!({
            "enrolled": true,
            "backup_enabled": true,
            "sync_enabled": false,
            "service_host": "backup.filesquid.tech"
        })
    );
    let sync_only = HostedService::enrolled_at("sync.example.org", false, true).expect("valid");
    assert!(!sync_only.backup_enabled());
    assert!(sync_only.sync_enabled());
}

#[test]
fn the_service_host_is_stored_lowercase() {
    let service = HostedService::enrolled_at("Backup.FileSquid.Tech", false, false).expect("valid");
    assert_eq!(service.service_host(), Some("backup.filesquid.tech"));
}

#[test]
fn the_service_host_is_a_bare_dns_host_name() {
    let label64 = format!("{}.tech", "a".repeat(64));
    let too_long = vec!["abcdefghi"; 26].join(".");
    let refused = [
        "",
        "https://backup.filesquid.tech",
        "backup.filesquid.tech:443",
        "backup.filesquid.tech/path",
        "user@backup.filesquid.tech",
        "backup.filesquid.tech?x=1",
        "backup.filesquid.tech#top",
        "backup filesquid.tech",
        "-backup.filesquid.tech",
        "backup-.filesquid.tech",
        "backup..filesquid.tech",
        ".filesquid.tech",
        "backup.filesquid.tech.",
        "back_up.filesquid.tech",
        "192.168.0.1",
        "[::1]",
        label64.as_str(),
        too_long.as_str(),
    ];
    for host in refused {
        assert_eq!(
            HostedService::enrolled_at(host, true, true),
            Err(ServiceHostError::NotAHostName),
            "{host:?}"
        );
    }
    for host in [
        "localhost",
        "a.b",
        "x1-y2.filesquid.tech",
        "backup.white-label.example",
    ] {
        assert!(
            HostedService::enrolled_at(host, true, true).is_ok(),
            "{host}"
        );
    }
}

#[test]
fn a_service_host_error_never_contains_the_value() {
    let text = HostedService::enrolled_at("https://user:pw@secret.example/x", true, true)
        .unwrap_err()
        .to_string();
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("pw"), "{text}");
}

// ---- host.change ----

/// An enrolment port whose state a test changes, like the enrolment code of API-F4.1.
#[derive(Default)]
struct SwitchablePort(Mutex<Option<HostedService>>);

impl SwitchablePort {
    fn set(&self, service: HostedService) {
        *self.0.lock().expect("port") = Some(service);
    }
}

impl EnrolmentPort for SwitchablePort {
    fn hosted_service(&self) -> HostedService {
        self.0
            .lock()
            .expect("port")
            .clone()
            .unwrap_or_else(HostedService::not_enrolled)
    }
}

/// A desktop state over a [`SwitchablePort`].
fn switchable() -> (ApiState, Arc<SwitchablePort>) {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    let port = Arc::new(SwitchablePort::default());
    assert!(state.describe_host(HostIdentity::desktop(), port.clone()));
    (state, port)
}

/// Every event already queued on `subscription`.
async fn drain(subscription: &mut filesquid_api::EventSubscription) -> Vec<ApiEvent> {
    let mut events = Vec::new();
    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_millis(20), subscription.next()).await
    {
        events.push(event);
    }
    events
}

#[tokio::test]
async fn each_change_sends_one_host_change_with_the_whole_descriptor_to_every_subscriber() {
    let (state, port) = switchable();
    let host = state.host().expect("described");
    let mut first = state.events.subscription().expect("slot");
    let mut second = state.events.subscription().expect("slot");
    port.set(HostedService::enrolled_at("backup.filesquid.tech", true, false).expect("valid"));
    assert!(host.refresh_hosted_service());
    let (_, enrolled) = read_host(&state).await;
    port.set(HostedService::enrolled_at("backup.filesquid.tech", true, true).expect("valid"));
    assert!(host.refresh_hosted_service());
    let (_, sync_on) = read_host(&state).await;
    for subscription in [&mut first, &mut second] {
        let events = drain(subscription).await;
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(events.iter().all(|e| e.topic == topic::HOST_CHANGE));
        assert_eq!(events[0].data, enrolled);
        assert_eq!(events[1].data, sync_on);
        assert_eq!(events[1].data["hosted_service"]["sync_enabled"], true);
    }
}

#[tokio::test]
async fn a_refresh_without_a_change_sends_nothing() {
    let (state, port) = switchable();
    let host = state.host().expect("described");
    let mut subscription = state.events.subscription().expect("slot");
    assert!(!host.refresh_hosted_service());
    port.set(HostedService::not_enrolled());
    assert!(!host.refresh_hosted_service());
    assert!(drain(&mut subscription).await.is_empty());
}

/// A client that reconnects gets no replay of what it missed: the new subscription starts
/// empty, and only rereading the route shows the current state (API-S-003, API-S-020).
#[tokio::test]
async fn a_client_that_reconnects_reads_the_current_descriptor_from_the_route() {
    let (state, port) = switchable();
    let host = state.host().expect("described");
    let disconnected = state.events.subscription().expect("slot");
    drop(disconnected);
    port.set(HostedService::enrolled_at("backup.filesquid.tech", true, false).expect("valid"));
    assert!(host.refresh_hosted_service());
    let mut reconnected = state.events.subscription().expect("slot");
    assert!(
        drain(&mut reconnected).await.is_empty(),
        "nothing is replayed"
    );
    let (status, body) = read_host(&state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hosted_service"]["enrolled"], true);
    let contract: Value = serde_json::from_str(filesquid_api::CONTRACT).expect("contract");
    let route = contract["paths"]["/api/v1/host"]["get"]["description"]
        .as_str()
        .expect("documented");
    assert!(
        route.contains("after events.resync or a reconnect"),
        "{route}"
    );
}

#[test]
fn host_change_is_a_documented_topic() {
    assert_eq!(topic::HOST_CHANGE, "host.change");
    assert!(topic::ALL.contains(&topic::HOST_CHANGE));
}

// ---- Not journaled ----

#[tokio::test]
async fn reading_the_descriptor_is_not_journaled() {
    let state = described(HostIdentity::desktop());
    let store = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        store.clone(),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    monitor.journal().flush();
    let before = store.count(&JournalQuery::default()).expect("count");
    for _ in 0..5 {
        assert_eq!(read_host(&state).await.0, StatusCode::OK);
    }
    monitor.journal().flush();
    assert_eq!(
        store.count(&JournalQuery::default()).expect("count"),
        before
    );
    monitor.shutdown();
}

// ---- Redaction ----

/// Assert `value` carries nothing API-S-020 forbids (a token, content handle, key or key
/// identifier, account or device identifier, e-mail address, quota, object count, size,
/// timestamp, logical or filesystem path or file name): only the documented keys, booleans
/// for the switches, no number but the schema version (so no quota, size or object count),
/// and no string shaped like a token, handle, key, identifier, e-mail address, path, file
/// name, timestamp or URL.
fn assert_nothing_private(value: &Value) {
    let top = value.as_object().expect("descriptor object");
    let keys: Vec<&str> = top.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "host_type",
            "hosted_service",
            "operator_name",
            "schema_version"
        ],
        "top-level keys"
    );
    let service = top["hosted_service"].as_object().expect("hosted_service");
    let keys: Vec<&str> = service.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["backup_enabled", "enrolled", "service_host", "sync_enabled"],
        "hosted_service keys"
    );
    assert_eq!(top["schema_version"], 1);
    for flag in ["backup_enabled", "enrolled", "sync_enabled"] {
        assert!(service[flag].is_boolean(), "{flag}");
    }
    for text in [
        &top["host_type"],
        &top["operator_name"],
        &service["service_host"],
    ]
    .into_iter()
    .filter_map(Value::as_str)
    {
        assert_plain_text(text);
    }
}

/// A string that is not a token, key, path, e-mail, URL, timestamp or file name.
fn assert_plain_text(text: &str) {
    for forbidden in ['/', '\\', '@', ':', '?', '#', '='] {
        assert!(!text.contains(forbidden), "{text:?} contains {forbidden:?}");
    }
    let hex_run = text
        .split(|c: char| !c.is_ascii_hexdigit())
        .map(str::len)
        .max()
        .unwrap_or(0);
    assert!(hex_run < 16, "{text:?} looks like a token or key");
    assert!(
        !text.chars().any(|c| c.is_ascii_digit()),
        "{text:?} looks like a size, count or timestamp"
    );
    let file_like = [".jpg", ".png", ".pdf", ".txt", ".json", ".sqlite", ".mov"];
    assert!(
        file_like.iter().all(|ext| !text.ends_with(ext)),
        "{text:?} looks like a file name"
    );
}

#[tokio::test]
async fn the_answer_and_the_event_carry_nothing_private() {
    let (state, port) = switchable();
    let host = state.host().expect("described");
    let mut subscription = state.events.subscription().expect("slot");
    let (_, before) = read_host(&state).await;
    assert_nothing_private(&before);
    port.set(HostedService::enrolled_at("backup.filesquid.tech", true, true).expect("valid"));
    host.refresh_hosted_service();
    let (_, after) = read_host(&state).await;
    assert_nothing_private(&after);
    let events = drain(&mut subscription).await;
    assert_eq!(events.len(), 1);
    assert_nothing_private(&events[0].data);
    let named = described(HostIdentity::server(Some(operator("Garderen Family"))));
    assert_nothing_private(&read_host(&named).await.1);
}
