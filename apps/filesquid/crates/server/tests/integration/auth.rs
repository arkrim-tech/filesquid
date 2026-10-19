//! Integration tests: deny-by-default authorisation on `filesquid-server` (API-S-004,
//! SRS-F-056, ADR 030 Decision 5). Every API route, the WebSocket event channel and the
//! Prometheus `/metrics` route refuse a call without the administrator token, before any
//! handler runs. The static distribution UI stays public: it holds no private content.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::Action;
use filesquid_api::{route_actions, ApiState};
use filesquid_server::{app, install_admin_authoriser, ServerConfig};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Request as WsRequest;
use tower::ServiceExt;

/// Administrator token configured for the test server.
const TOKEN: &str = "5ca1ab1e5ca1ab1e5ca1ab1e5ca1ab1e5ca1ab1e5ca1ab1e5ca1ab1e5ca1ab1e";

/// A fresh, empty data directory for one test.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-server-auth-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Server configuration with [`TOKEN`] and its own data directory.
fn config(name: &str) -> ServerConfig {
    ServerConfig {
        data_dir: data_dir(name),
        admin_token: Some(TOKEN.into()),
        ..ServerConfig::default()
    }
}

/// The server app with the administrator authoriser installed from `config`.
fn server(config: &ServerConfig) -> axum::Router {
    let state = ApiState::new();
    install_admin_authoriser(config, &state).expect("authoriser");
    app(config, state)
}

/// `GET path` with an optional bearer token: status and JSON body (null when not JSON).
async fn get(router: axum::Router, path: &str, token: Option<&str>) -> (u16, serde_json::Value) {
    let mut request = Request::get(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("answer");
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.expect("body");
    let json = serde_json::from_slice(&body.to_bytes()).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Every path the server answers that MUST be guarded by a Bearer credential: the API
/// router's routes except the public liveness probe and the content route (whose only
/// credential is a content handle, API-S-018), plus the event channel and `/metrics`.
fn guarded_paths() -> Vec<String> {
    let mut paths: Vec<String> = route_actions()
        .into_iter()
        .filter(|(_, action)| !matches!(action, Action::Public | Action::ReadContent))
        .map(|(path, _)| path)
        .collect();
    paths.push("/api/v1/events".into());
    paths.push("/metrics".into());
    paths
}

#[tokio::test]
async fn every_server_route_refuses_a_call_without_the_token() {
    let config = config("anonymous");
    for path in guarded_paths() {
        let (status, json) = get(server(&config), &path, None).await;
        assert_eq!(status, 401, "{path}");
        assert_eq!(json["code"], "unauthorized", "{path}");
        assert_eq!(json["reason"], "missing_credentials", "{path}");
    }
}

#[tokio::test]
async fn the_server_answers_the_content_route_with_its_one_denial() {
    let config = config("content");
    for path in [
        "/api/v1/content/AAAA",
        "/api/v1/content/",
        "/api/v1/content/a/b",
    ] {
        for token in [None, Some(TOKEN)] {
            let (status, json) = get(server(&config), path, token).await;
            assert_eq!(status, 403, "{path}");
            assert_eq!(json["code"], "forbidden", "{path}");
            assert_eq!(json["reason"], "invalid_credentials", "{path}");
        }
    }
}

#[tokio::test]
async fn every_server_route_refuses_a_wrong_token() {
    let config = config("wrong");
    for path in guarded_paths() {
        let (status, json) = get(server(&config), &path, Some("not-the-token")).await;
        assert_eq!(status, 403, "{path}");
        assert_eq!(json["reason"], "invalid_credentials", "{path}");
    }
}

#[tokio::test]
async fn a_server_without_an_installed_authoriser_refuses_every_route() {
    let router = app(&ServerConfig::default(), ApiState::new());
    for path in guarded_paths() {
        let (status, _) = get(router.clone(), &path, Some(TOKEN)).await;
        assert_eq!(status, 403, "{path}");
    }
}

#[tokio::test]
async fn the_liveness_probe_answers_without_a_token() {
    let (status, json) = get(server(&config("live")), "/api/v1/health/live", None).await;
    assert_eq!(status, 200);
    assert_eq!(json, serde_json::json!({ "status": "up" }));
    let router = app(&ServerConfig::default(), ApiState::new());
    let (status, _) = get(router, "/api/v1/health/live", None).await;
    assert_eq!(status, 200, "public even before an authoriser is installed");
}

#[tokio::test]
async fn the_static_ui_stays_public() {
    let dir = data_dir("ui");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("index.html"), "<!doctype html><title>UI</title>").expect("index");
    let config = ServerConfig {
        ui_dir: Some(dir.clone()),
        admin_token: Some(TOKEN.into()),
        ..ServerConfig::default()
    };
    let response = server(&config)
        .oneshot(Request::get("/").body(Body::empty()).expect("request"))
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::OK);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Start the server on an ephemeral port; returns its address and task.
async fn serve(config: &ServerConfig) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("port");
    let addr = listener.local_addr().expect("addr");
    let router = server(config);
    let task = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            panic!("server failed: {err}");
        }
    });
    (addr, task)
}

/// A WebSocket handshake request for the event channel at `addr`.
fn events_request(addr: SocketAddr) -> WsRequest<()> {
    format!("ws://{addr}/api/v1/events")
        .into_client_request()
        .expect("request")
}

#[tokio::test]
async fn the_event_channel_refuses_a_websocket_without_the_token() {
    let (addr, task) = serve(&config("ws-anonymous")).await;
    let refused = tokio_tungstenite::connect_async(events_request(addr)).await;
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) = refused else {
        panic!("handshake without the token was not refused");
    };
    assert_eq!(response.status().as_u16(), 401);
    task.abort();
}

#[tokio::test]
async fn a_browser_may_send_the_token_as_a_websocket_subprotocol() {
    let (addr, task) = serve(&config("ws-subprotocol")).await;
    let mut request = events_request(addr);
    let offered = format!("filesquid.events.v1, filesquid.bearer.{TOKEN}");
    request.headers_mut().insert(
        "sec-websocket-protocol",
        offered.parse().expect("header value"),
    );
    let (socket, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("handshake with the subprotocol token");
    let selected = response.headers()["sec-websocket-protocol"].to_str();
    assert_eq!(selected.expect("ascii"), "filesquid.events.v1");
    drop(socket);
    task.abort();
}

#[tokio::test]
async fn a_missing_admin_token_is_generated_into_the_data_dir_with_owner_only_access() {
    let dir = data_dir("generated");
    let config = ServerConfig {
        data_dir: dir.clone(),
        admin_token: None,
        ..ServerConfig::default()
    };
    let router = server(&config);
    let token = std::fs::read_to_string(dir.join("admin-token")).expect("token file");
    let token = token.trim().to_string();
    assert_eq!(token.len(), 64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(dir.join("admin-token")).expect("metadata");
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }
    let (status, _) = get(router.clone(), "/api/v1/health", Some(&token)).await;
    assert_eq!(status, 200);
    let again = ServerConfig {
        data_dir: dir.clone(),
        admin_token: None,
        ..ServerConfig::default()
    };
    let (status, _) = get(server(&again), "/api/v1/health", Some(&token)).await;
    assert_eq!(status, 200, "the stored token is reused on restart");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn other_methods_on_host_routes_are_refused_before_routing() {
    let config = config("methods");
    for path in ["/api/v1/events", "/metrics"] {
        let request = || Request::post(path);
        let anonymous = request().body(Body::empty()).expect("request");
        let response = server(&config).oneshot(anonymous).await.expect("answer");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let authorised = request()
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .body(Body::empty())
            .expect("request");
        let response = server(&config).oneshot(authorised).await.expect("answer");
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).expect("ApiError");
        assert_eq!(json["code"], "method_not_allowed", "{path}");
    }
}

#[cfg(unix)]
#[test]
fn a_group_or_world_readable_token_file_is_refused() {
    use filesquid_server::admin_token::load_or_create_token;
    use std::os::unix::fs::PermissionsExt;
    for mode in [0o644, 0o640, 0o604] {
        let dir = data_dir(&format!("mode-{mode:o}"));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("admin-token");
        std::fs::write(
            &path,
            "0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c",
        )
        .expect("token file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("chmod");
        let error = load_or_create_token(&dir, None).expect_err("refused");
        assert!(error.contains("chmod 600"), "{error}");
        assert!(
            !error.contains("0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c"),
            "{error}"
        );
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_token_file_is_refused() {
    use filesquid_server::admin_token::load_or_create_token;
    let dir = data_dir("symlink");
    std::fs::create_dir_all(&dir).expect("dir");
    let target = dir.join("elsewhere");
    std::fs::write(
        &target,
        "0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c",
    )
    .expect("target");
    std::os::unix::fs::symlink(&target, dir.join("admin-token")).expect("symlink");
    let error = load_or_create_token(&dir, None).expect_err("refused");
    assert!(error.contains("symbolic link"), "{error}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn an_owner_only_token_file_is_reused() {
    use filesquid_server::admin_token::load_or_create_token;
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir("owner-only");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("admin-token");
    std::fs::write(
        &path,
        "0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c\n",
    )
    .expect("token file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    let (token, created) = load_or_create_token(&dir, None).expect("reused");
    assert_eq!(
        (token.as_str(), created),
        (
            "0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c0ddba11c",
            false
        )
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn concurrent_first_starts_agree_on_one_token() {
    use filesquid_server::admin_token::load_or_create_token;
    for round in 0..20 {
        let dir = data_dir(&format!("race-{round}"));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let dir = dir.clone();
                std::thread::spawn(move || load_or_create_token(&dir, None).expect("token").0)
            })
            .collect();
        let tokens: Vec<String> = threads
            .into_iter()
            .map(|t| t.join().expect("thread"))
            .collect();
        let stored = std::fs::read_to_string(dir.join("admin-token")).expect("stored");
        assert!(tokens.iter().all(|t| *t == stored.trim()), "round {round}");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}

#[cfg(unix)]
#[test]
fn a_blank_owner_only_token_file_left_by_a_crash_is_replaced() {
    use filesquid_server::admin_token::load_or_create_token;
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir("blank");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("admin-token");
    std::fs::write(&path, "").expect("blank file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    let (token, created) = load_or_create_token(&dir, None).expect("replaced");
    assert!(created);
    assert_eq!(token.len(), 64);
    let stored = std::fs::read_to_string(&path).expect("stored");
    assert_eq!(stored.trim(), token);
    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let leftovers = std::fs::read_dir(&dir).expect("dir").count();
    assert_eq!(leftovers, 1, "no temporary file is left behind");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn a_blank_token_file_open_to_others_is_still_refused() {
    use filesquid_server::admin_token::load_or_create_token;
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir("blank-open");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("admin-token");
    std::fs::write(&path, "").expect("blank file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let error = load_or_create_token(&dir, None).expect_err("refused");
    assert!(error.contains("chmod 600"), "{error}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn every_start_journals_the_administrator_token_issue() {
    use filesquid_api::journal::JournalQuery;
    use filesquid_server::start_server;
    let dir = data_dir("issued");
    let config = ServerConfig {
        data_dir: dir.clone(),
        admin_token: None,
        ..ServerConfig::default()
    };
    for start in 0..2 {
        let state = ApiState::new();
        let host = start_server(&config, &state, None).expect("server");
        host.monitor().journal().flush();
        let query = JournalQuery {
            operation: Some("api.auth.token_issued".into()),
            ..Default::default()
        };
        let page = host.monitor().store().query(&query).expect("query");
        assert_eq!(page.entries.len(), start + 1, "one issue record per start");
        for entry in &page.entries {
            assert_eq!(entry.target_ids, vec!["administrator".to_string()]);
        }
        let stored = std::fs::read_to_string(dir.join("admin-token")).expect("token file");
        let journaled = serde_json::to_string(&page.entries).expect("json");
        assert!(
            !journaled.contains(stored.trim()),
            "the administrator token is never journaled"
        );
        host.stop();
        drop(host);
        drop(state);
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_configured_token_must_have_the_generated_format() {
    for weak in [
        "weak-secret-value",
        "abc",
        &"a".repeat(63),
        &"a".repeat(65),
        &"g".repeat(64),
    ] {
        let config = ServerConfig {
            data_dir: data_dir("weak"),
            admin_token: Some(weak.to_string()),
            ..ServerConfig::default()
        };
        let error = install_admin_authoriser(&config, &ApiState::new()).expect_err("refused");
        assert!(error.contains("FILESQUID_ADMIN_TOKEN"), "{error}");
        assert!(error.contains("64 hexadecimal"), "{error}");
        assert!(!error.contains(weak), "the value is never echoed: {error}");
    }
    let strong = ServerConfig {
        data_dir: data_dir("strong"),
        admin_token: Some(TOKEN.to_ascii_uppercase()),
        ..ServerConfig::default()
    };
    assert!(install_admin_authoriser(&strong, &ApiState::new()).is_ok());
}

#[cfg(unix)]
#[test]
fn a_stored_token_that_lacks_the_generated_format_is_refused() {
    use filesquid_server::admin_token::load_or_create_token;
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir("weak-stored");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("admin-token");
    std::fs::write(&path, "hand-written").expect("token file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    let error = load_or_create_token(&dir, None).expect_err("refused");
    assert!(error.contains("64 hexadecimal"), "{error}");
    assert!(!error.contains("hand-written"), "{error}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}
