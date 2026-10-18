//! Integration tests for the desktop transport: the FileSquid API served through the
//! `filesquid://` custom URI scheme, without a TCP listener (ADR 030, FileSquid T-049).

use filesquid_api::auth::LaunchToken;
use filesquid_api::ApiState;
use filesquid_desktop_lib::api_protocol::{
    api_error_response, app_origins, handle, into_scheme_response, launch_token_script,
    DEV_ORIGINS, MAX_SCHEME_RESPONSE_BYTES, RELEASE_ORIGINS, SCHEME,
};

#[path = "../support/journal.rs"]
mod journal;
use filesquid_desktop_lib::{describe_desktop_host, install_launch_token};
use journal::{block_on, journaled};

/// API state with a freshly generated per-launch token installed, as the desktop host does.
fn launched() -> (ApiState, LaunchToken) {
    let state = ApiState::new();
    let token = install_launch_token(&state).expect("per-launch token");
    (state, token)
}

/// Build a GET request the way the webview delivers it to the scheme handler, carrying the
/// per-launch token when one is given.
fn get_with(uri: &str, origin: Option<&str>, token: Option<&str>) -> http::Request<Vec<u8>> {
    let mut builder = http::Request::get(uri);
    if let Some(origin) = origin {
        builder = builder.header(http::header::ORIGIN, origin);
    }
    if let Some(token) = token {
        builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Vec::new()).expect("request")
}

#[test]
fn scheme_name_is_filesquid() {
    assert_eq!(SCHEME, "filesquid");
}

#[tokio::test]
async fn health_is_served_through_the_custom_scheme() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/health";
    let response = handle(state, get_with(uri, None, Some(token.secret()))).await;
    assert_eq!(response.status(), http::StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json["product"], "filesquid");
}

/// API-S-020: the desktop describes itself as `desktop`, with no operator name and not
/// enrolled, through the custom scheme, whatever the request claims.
#[tokio::test]
async fn the_host_descriptor_is_served_through_the_custom_scheme() {
    let (state, token) = launched();
    describe_desktop_host(&state).expect("described");
    assert!(describe_desktop_host(&state).is_err(), "described once");
    for uri in [
        "filesquid://localhost/api/v1/host",
        "http://filesquid.localhost/api/v1/host",
    ] {
        let mut request = get_with(uri, Some("tauri://localhost"), Some(token.secret()));
        request.headers_mut().insert(
            http::header::USER_AGENT,
            "server browser".parse().expect("value"),
        );
        let response = handle(state.clone(), request).await;
        assert_eq!(response.status(), http::StatusCode::OK, "{uri}");
        let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
        assert_eq!(
            json,
            serde_json::json!({
                "schema_version": 1,
                "host_type": "desktop",
                "operator_name": null,
                "hosted_service": {
                    "enrolled": false,
                    "backup_enabled": false,
                    "sync_enabled": false,
                    "service_host": null
                }
            })
        );
    }
    let anonymous = handle(
        state,
        get_with("filesquid://localhost/api/v1/host", None, None),
    )
    .await;
    assert_eq!(anonymous.status(), http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn windows_style_scheme_urls_are_served_too() {
    let (state, token) = launched();
    let uri = "http://filesquid.localhost/api/v1/health";
    let response = handle(state, get_with(uri, None, Some(token.secret()))).await;
    assert_eq!(response.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn app_origins_receive_cors_headers() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/health";
    let request = get_with(uri, Some("tauri://localhost"), Some(token.secret()));
    let response = handle(state, request).await;
    assert_eq!(
        response.headers()[http::header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
}

#[tokio::test]
async fn foreign_origins_receive_no_cors_headers() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/health";
    let request = get_with(uri, Some("https://evil.example"), Some(token.secret()));
    let response = handle(state, request).await;
    assert!(response
        .headers()
        .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}

#[tokio::test]
async fn unknown_paths_are_not_found() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/nope";
    let response = handle(state, get_with(uri, None, Some(token.secret()))).await;
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_desktop_owner_reaches_admin_routes_with_the_per_launch_token() {
    use filesquid_api::journal::MemoryJournal;
    use filesquid_api::monitor::{Monitor, MonitorConfig};
    let (state, token) = launched();
    let monitor = Monitor::start(
        std::sync::Arc::new(MemoryJournal::new("desktop")),
        std::sync::Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    let uri = "filesquid://localhost/api/v1/admin/health";
    let request = get_with(uri, Some("tauri://localhost"), Some(token.secret()));
    let response = handle(state, request).await;
    assert_eq!(response.status(), http::StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert!(json["status"].is_string());
    monitor.shutdown();
}

#[tokio::test]
async fn a_call_without_the_per_launch_token_is_refused() {
    let (state, _token) = launched();
    let protected = filesquid_api::route_actions()
        .into_iter()
        .filter(|(_, action)| *action != filesquid_api::auth::Action::Public);
    for (path, _) in protected {
        let uri = format!("filesquid://localhost{path}");
        let request = get_with(&uri, Some("tauri://localhost"), None);
        let response = handle(state.clone(), request).await;
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED, "{path}");
        let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
        assert_eq!(json["reason"], "missing_credentials", "{path}");
    }
}

#[tokio::test]
async fn the_liveness_probe_needs_no_token() {
    let (state, _token) = launched();
    let uri = "filesquid://localhost/api/v1/health/live";
    let response = handle(state, get_with(uri, None, None)).await;
    assert_eq!(response.status(), http::StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json, serde_json::json!({ "status": "up" }));
}

#[tokio::test]
async fn a_token_from_another_launch_is_refused() {
    let (state, _token) = launched();
    let other = LaunchToken::generate().expect("random token");
    let uri = "filesquid://localhost/api/v1/health";
    let response = handle(state, get_with(uri, None, Some(other.secret()))).await;
    assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
}

#[test]
fn each_launch_installs_a_new_token_once() {
    let (state, first) = launched();
    assert!(
        install_launch_token(&state).is_err(),
        "a second token is refused"
    );
    let (_, second) = launched();
    assert_ne!(first.secret(), second.secret());
}

/// A CORS preflight the webview sends before a cross-origin request with `Authorization`.
fn preflight(origin: &str) -> http::Request<Vec<u8>> {
    http::Request::builder()
        .method(http::Method::OPTIONS)
        .uri("filesquid://localhost/api/v1/health")
        .header(http::header::ORIGIN, origin)
        .header(http::header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(
            http::header::ACCESS_CONTROL_REQUEST_HEADERS,
            "authorization",
        )
        .body(Vec::new())
        .expect("request")
}

#[tokio::test]
async fn app_origins_may_preflight_the_authorization_header() {
    let (state, _token) = launched();
    let response = handle(state, preflight("tauri://localhost")).await;
    assert_eq!(response.status(), http::StatusCode::NO_CONTENT);
    let headers = response.headers();
    assert_eq!(
        headers[http::header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "tauri://localhost"
    );
    let allowed = headers[http::header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .expect("ascii");
    assert!(allowed.contains("authorization"), "{allowed}");
    assert!(response.body().is_empty());
}

#[tokio::test]
async fn foreign_origins_get_no_preflight_grant() {
    let (state, _token) = launched();
    let response = handle(state, preflight("https://evil.example")).await;
    assert!(response
        .headers()
        .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    assert_ne!(response.status(), http::StatusCode::NO_CONTENT);
}

#[test]
fn release_builds_trust_only_the_apps_own_scheme_origins() {
    assert_eq!(
        RELEASE_ORIGINS,
        [
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost"
        ]
    );
    for origin in DEV_ORIGINS {
        assert!(!RELEASE_ORIGINS.contains(origin), "{origin}");
        assert!(origin.starts_with("http://127.0.0.1:"), "{origin}");
    }
}

#[test]
fn dev_server_origins_are_trusted_only_in_debug_builds() {
    let trusted = app_origins();
    for origin in RELEASE_ORIGINS {
        assert!(trusted.contains(origin), "{origin}");
    }
    for origin in DEV_ORIGINS {
        assert_eq!(trusted.contains(origin), cfg!(debug_assertions), "{origin}");
    }
}

#[test]
fn the_init_script_hands_the_token_only_to_app_origins() {
    let token = LaunchToken::generate().expect("random token");
    let script = launch_token_script(&token);
    assert!(script.contains(token.secret()));
    assert!(script.contains("__FILESQUID_LAUNCH_TOKEN__"));
    assert!(script.contains("window.location.origin"));
    for origin in app_origins() {
        assert!(script.contains(origin), "{origin}");
    }
    for origin in DEV_ORIGINS {
        assert_eq!(script.contains(origin), cfg!(debug_assertions), "{origin}");
    }
    assert_eq!(
        script.matches(token.secret()).count(),
        1,
        "the secret appears once, inside the origin-guarded defineProperty"
    );
    let guard = script.find("indexOf(window.location.origin) === -1) return;");
    let secret_at = script.find(token.secret());
    assert!(guard.expect("origin guard") < secret_at.expect("secret"));
    assert!(script.contains("writable: false"));
}

#[tokio::test]
async fn app_origin_responses_expose_the_journal_cursor_header() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/health";
    let request = get_with(uri, Some("tauri://localhost"), Some(token.secret()));
    let response = handle(state, request).await;
    let exposed = response.headers()[http::header::ACCESS_CONTROL_EXPOSE_HEADERS]
        .to_str()
        .expect("ascii")
        .to_ascii_lowercase();
    assert!(exposed.contains("x-journal-next-cursor"), "{exposed}");
}

#[tokio::test]
async fn app_origins_may_preflight_the_range_header() {
    let (state, _token) = launched();
    let response = handle(state, preflight("tauri://localhost")).await;
    let allowed = response.headers()[http::header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .expect("ascii");
    assert!(allowed.contains("range"), "{allowed}");
}

#[tokio::test]
async fn app_origin_responses_expose_the_range_headers() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/api/v1/health";
    let request = get_with(uri, Some("tauri://localhost"), Some(token.secret()));
    let response = handle(state, request).await;
    let exposed = response.headers()[http::header::ACCESS_CONTROL_EXPOSE_HEADERS]
        .to_str()
        .expect("ascii")
        .to_ascii_lowercase();
    assert!(exposed.contains("content-range"), "{exposed}");
    assert!(exposed.contains("accept-ranges"), "{exposed}");
}

#[tokio::test]
async fn windows_style_scheme_urls_allow_and_expose_the_range_headers() {
    let (state, token) = launched();
    let uri = "http://filesquid.localhost/api/v1/health";
    let preflight = http::Request::builder()
        .method(http::Method::OPTIONS)
        .uri(uri)
        .header(http::header::ORIGIN, "http://tauri.localhost")
        .header(http::header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(
            http::header::ACCESS_CONTROL_REQUEST_HEADERS,
            "authorization, range",
        )
        .body(Vec::new())
        .expect("request");
    let answer = handle(state.clone(), preflight).await;
    assert_eq!(answer.status(), http::StatusCode::NO_CONTENT);
    let allowed = answer.headers()[http::header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .expect("ascii");
    assert!(allowed.contains("range"), "{allowed}");
    let request = get_with(uri, Some("http://tauri.localhost"), Some(token.secret()));
    let response = handle(state, request).await;
    let exposed = response.headers()[http::header::ACCESS_CONTROL_EXPOSE_HEADERS]
        .to_str()
        .expect("ascii")
        .to_ascii_lowercase();
    assert!(exposed.contains("content-range"), "{exposed}");
    assert!(exposed.contains("accept-ranges"), "{exposed}");
}

/// A router-style response with `bytes` as its body and `status`.
fn body_response(status: u16, bytes: Vec<u8>) -> http::Response<axum::body::Body> {
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_RANGE, "bytes 0-9/100")
        .body(axum::body::Body::from(bytes))
        .expect("response")
}

#[tokio::test]
async fn a_partial_response_passes_through_the_scheme_unchanged() {
    let bytes: Vec<u8> = (0..10).collect();
    let response = into_scheme_response(body_response(206, bytes.clone()), 10).await;
    assert_eq!(response.status(), http::StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[http::header::CONTENT_RANGE],
        "bytes 0-9/100"
    );
    assert_eq!(response.body(), &bytes);
}

#[tokio::test]
async fn a_response_over_the_buffer_cap_is_a_typed_internal_error() {
    let response = into_scheme_response(body_response(206, vec![7; 11]), 10).await;
    assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers()[http::header::CONTENT_TYPE],
        "application/json"
    );
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json["code"], "internal");
    assert_eq!(json["retryable"], false);
    assert!(response
        .headers()
        .get(http::header::CONTENT_RANGE)
        .is_none());
}

#[test]
fn the_scheme_buffers_at_most_sixteen_mebibytes() {
    assert_eq!(MAX_SCHEME_RESPONSE_BYTES, 16 * 1024 * 1024);
}

#[test]
fn host_errors_carry_the_api_error_body() {
    let error = filesquid_api::ApiError::new(
        filesquid_api::ErrorCode::InvalidRequest,
        "The request address is not valid.",
    );
    let response = api_error_response(&error);
    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()[http::header::CONTENT_TYPE],
        "application/json"
    );
    let json: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(json["code"], "invalid_request");
    assert_eq!(json["message"], "The request address is not valid.");
}

#[cfg(not(feature = "test-webdriver"))]
#[tokio::test]
async fn default_builds_serve_no_test_fixture() {
    let (state, token) = launched();
    let uri = "filesquid://localhost/__filesquid_test__/range-fixture";
    let response = handle(state, get_with(uri, None, Some(token.secret()))).await;
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
}

#[test]
fn an_oversized_response_is_journaled_without_its_detail() {
    let entries = journaled(|| {
        let private = b"/Users/owner/Pictures/private.jpg".to_vec();
        block_on(into_scheme_response(body_response(200, private), 4));
    });
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.operation, "desktop.scheme.response");
    assert_eq!(entry.error_code.as_deref(), Some("internal"));
    let text = serde_json::to_string(entry).expect("json");
    assert!(!text.contains("private"), "{text}");
    assert!(!text.contains("Users"), "{text}");
}
