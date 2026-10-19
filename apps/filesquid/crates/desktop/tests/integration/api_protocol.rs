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
    // The content route's only credential is a content handle; its denial is tested below.
    let protected = filesquid_api::route_actions()
        .into_iter()
        .filter(|(_, action)| {
            !matches!(
                action,
                filesquid_api::auth::Action::Public | filesquid_api::auth::Action::ReadContent
            )
        });
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

/// The privacy headers every content-route answer carries (API-S-018).
const PRIVACY_HEADERS: [(&str, &str); 4] = [
    ("referrer-policy", "no-referrer"),
    ("cache-control", "private, no-store"),
    ("x-content-type-options", "nosniff"),
    ("content-security-policy", "default-src 'none'; sandbox"),
];

/// DE-S-005: the scheme passes a content-route request to the router without a token and
/// hands back its answer, headers included, unchanged: the one denial for an unusable
/// handle, and `206` with `Content-Range` for a real handle's range.
#[tokio::test]
async fn content_route_answers_pass_through_the_scheme_unchanged() {
    let (state, token) = launched();
    let unknown =
        "filesquid://localhost/api/v1/content/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let denied = handle(
        state.clone(),
        get_with(unknown, Some("tauri://localhost"), None),
    )
    .await;
    assert_eq!(denied.status(), http::StatusCode::FORBIDDEN);
    let json: serde_json::Value = serde_json::from_slice(denied.body()).expect("json");
    assert_eq!(json["reason"], "invalid_credentials");
    for (name, value) in PRIVACY_HEADERS {
        assert_eq!(denied.headers()[name], value, "{name}");
    }
    let url = content::minted_url(&state, token.secret()).await;
    let mut request = get_with(&format!("filesquid://localhost{url}"), None, None);
    let range = http::HeaderValue::from_static("bytes=2-5");
    request.headers_mut().insert(http::header::RANGE, range);
    let answer = handle(state.clone(), request).await;
    assert_eq!(answer.status(), http::StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(answer.headers()["accept-ranges"], "bytes");
    assert_eq!(answer.headers()["content-type"], "video/webm");
    assert_eq!(answer.body(), b"2345");
    for (name, value) in PRIVACY_HEADERS {
        assert_eq!(answer.headers()[name], value, "{name}");
    }
    let mut head = get_with(&format!("filesquid://localhost{url}"), None, None);
    *head.method_mut() = http::Method::HEAD;
    let head = handle(state, head).await;
    assert_eq!(head.status(), http::StatusCode::OK);
    assert_eq!(head.headers()["accept-ranges"], "bytes");
    assert_eq!(head.headers()["content-type"], "video/webm");
    assert_eq!(head.headers()["content-length"], "10");
    assert!(head.body().is_empty(), "a HEAD answer has no body");
}

/// A file service over an in-memory OCFL root whose object holds `clip.webm`, and a content
/// handle minted for it.
mod content {
    use filesquid_api::files::{FileLibrary, FileService};
    use filesquid_api::ApiState;
    use filesquid_core::LibraryId;
    use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
    use filesquid_ocfl::storage::MemoryBackend;
    use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
    use serde_json::json;
    use std::sync::Arc;

    const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
    const OBJECT: &str = "urn:uuid:clip";

    /// Install the file service on `state`, commit the clip, and mint a handle with `token`
    /// through the scheme; answer its content URL.
    pub async fn minted_url(state: &ApiState, token: &str) -> String {
        let root = Arc::new(MemoryBackend::new());
        let leases = Arc::new(
            LeaseStore::new(
                MemoryBackend::new(),
                "leases",
                SystemClock,
                LeaseConfig::default(),
            )
            .expect("leases"),
        );
        commit(&root, &leases).await;
        let files = FileService::new();
        let library = FileLibrary {
            backend: root,
            reservations: leases,
            staging: Arc::new(MemoryBackend::new()),
            staging_prefix: "api".into(),
        };
        assert!(files.open_library(&LibraryId::new(LIBRARY).expect("id"), library));
        assert!(state.install_files(Arc::new(files)));
        mint(state, token).await
    }

    /// Write the storage-root declaration and commit `clip.webm` at v1.
    async fn commit(root: &MemoryBackend, leases: &LeaseStore<MemoryBackend, SystemClock>) {
        use filesquid_ocfl::storage::{StorageBackend, StorageKey};
        let key = StorageKey::new("0=ocfl_1.1").expect("key");
        root.put(&key, b"ocfl_1.1\n").await.expect("declaration");
        let staging = MemoryBackend::new();
        let area = StagingArea::new(&staging, "fixture").expect("area");
        let model = ChangeModel::new(root, leases, "fixture", "objects/clip", OBJECT, area);
        model
            .stage_bytes("clip.webm", b"0123456789")
            .await
            .expect("stage");
        let meta = VersionMeta::new("2026-10-18T00:00:00Z");
        model.commit(meta).await.expect("commit");
    }

    /// `POST /api/v1/content-handles` through the scheme; answer the handle's URL.
    async fn mint(state: &ApiState, token: &str) -> String {
        let body = json!({
            "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "path": "clip.webm"
        });
        let request = http::Request::post("filesquid://localhost/api/v1/content-handles")
            .header(http::header::AUTHORIZATION, format!("Bearer {token}"))
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(body.to_string().into_bytes())
            .expect("request");
        let answer = filesquid_desktop_lib::api_protocol::handle(state.clone(), request).await;
        assert_eq!(answer.status(), http::StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(answer.body()).expect("json");
        json["url"].as_str().expect("url").to_string()
    }
}
