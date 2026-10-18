//! Unit tests for the admin API routes (K9, K10).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::journal::{JournalEntry, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::registry::ProcessSpec;
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

/// API state with `authoriser` installed and a running monitor over an in-memory journal.
fn state_with_monitor(authoriser: Authoriser) -> (ApiState, Monitor) {
    let state = ApiState::new();
    state.install_authoriser(authoriser);
    let dir = std::env::temp_dir();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        MonitorConfig::new("host-a", dir),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    (state, monitor)
}

/// Desktop state: the owner's per-launch token, as text for the `Authorization` header.
fn owner_state() -> (ApiState, Monitor, String) {
    let token = LaunchToken::generate().expect("random token");
    let (state, monitor) = state_with_monitor(Authoriser::desktop_owner(&token));
    (state, monitor, token.secret().to_string())
}

async fn get(
    state: &ApiState,
    path: &str,
    token: Option<&str>,
) -> (StatusCode, String, Option<String>) {
    let mut req = Request::get(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let res = router(state.clone())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let ctype = res
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap(), ctype)
}

#[tokio::test]
async fn admin_routes_need_the_admin_token_on_the_server() {
    let (state, monitor) = state_with_monitor(Authoriser::server_administrator("s3cret"));
    assert_eq!(
        get(&state, "/api/v1/admin/health", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&state, "/api/v1/admin/health", Some("wrong")).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, body, _) = get(&state, "/api/v1/admin/health", Some("s3cret")).await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(["ok", "degraded", "failing"].contains(&json["status"].as_str().unwrap()));
    assert!(json["reasons"].is_array());
    monitor.shutdown();
}

#[tokio::test]
async fn the_desktop_owner_reaches_admin_routes_with_the_per_launch_token() {
    let (state, monitor, token) = owner_state();
    assert_eq!(
        get(&state, "/api/v1/admin/metrics", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&state, "/api/v1/admin/metrics", Some(&token)).await.0,
        StatusCode::OK
    );
    monitor.shutdown();
}

#[tokio::test]
async fn admin_routes_report_unavailable_before_the_monitor_starts() {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator("s3cret"));
    assert_eq!(
        get(&state, "/api/v1/admin/health", Some("s3cret")).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn journal_query_filters_pages_and_exports_json_lines() {
    let (state, monitor, token) = owner_state();
    let sender = monitor.journal();
    for _ in 0..3 {
        sender.send(JournalEntry::new("ocfl.commit", Outcome::Failed));
        sender.send(JournalEntry::new("import", Outcome::Succeeded));
    }
    sender.flush();
    let (status, body, _) = get(
        &state,
        "/api/v1/admin/journal?operation=ocfl.commit&outcome=failed&limit=2",
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(page["entries"].as_array().unwrap().len(), 2);
    let cursor = page["next_cursor"].as_i64().expect("cursor");
    let (_, body2, _) = get(
        &state,
        &format!(
            "/api/v1/admin/journal?operation=ocfl.commit&outcome=failed&limit=2&after={cursor}"
        ),
        Some(&token),
    )
    .await;
    let page2: serde_json::Value = serde_json::from_str(&body2).unwrap();
    assert_eq!(page2["entries"].as_array().unwrap().len(), 1);
    let (status, export, ctype) = get(
        &state,
        "/api/v1/admin/journal/export?operation=ocfl.commit",
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype.as_deref(), Some("application/x-ndjson"));
    assert_eq!(export.lines().count(), 3);
    for line in export.lines() {
        let e: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(e["operation"], "ocfl.commit");
    }
    monitor.shutdown();
}

#[tokio::test]
async fn processes_lists_registered_processes() {
    let (state, monitor, token) = owner_state();
    let handle = monitor
        .registry()
        .register(ProcessSpec::new("c2pa.scan", 30_000));
    handle.progress(2, 8);
    let (status, body, _) = get(&state, "/api/v1/admin/processes", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    let scan = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "c2pa.scan")
        .expect("scan listed");
    assert_eq!(scan["state"], "running");
    assert_eq!(scan["done"], 2);
    assert_eq!(scan["total"], 8);
    handle.complete();
    monitor.shutdown();
}

#[test]
fn the_write_probe_never_touches_an_existing_file() {
    let dir = std::env::temp_dir().join(format!("fs-admin-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".filesquid-write-probe"), b"keep me").unwrap();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        MonitorConfig::new("host-a", &dir),
        filesquid_api::EventBus::new(),
    );
    let report = monitor.health_report();
    assert!(!report
        .reasons
        .iter()
        .any(|r| r.code == "data_dir_not_writable"));
    assert_eq!(
        std::fs::read(dir.join(".filesquid-write-probe")).unwrap(),
        b"keep me"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    monitor.shutdown();
    drop(monitor);
    std::fs::remove_dir_all(dir).unwrap();
}
