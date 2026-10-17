//! Integration tests: the activity journal, process registry and admin routes on
//! `filesquid-server` (SRS-F-061), with the journal in a real SQLite file.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::monitor::HostMonitor;
use filesquid_api::ApiState;
use filesquid_server::{app, install_admin_authoriser, start_host_monitor, ServerConfig};
use http_body_util::BodyExt;
use std::path::PathBuf;
use tower::ServiceExt;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-server-monitor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn get(router: axum::Router, path: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::get(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let res = router
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn admin_health_journal_and_processes_work_with_the_configured_token() {
    let dir = data_dir("token");
    let config = ServerConfig {
        data_dir: dir.clone(),
        admin_token: Some("t0ken".into()),
        ..ServerConfig::default()
    };
    let state = ApiState::new();
    install_admin_authoriser(&config, &state).expect("authoriser");
    let host = start_host_monitor(&config, &state, None).expect("monitor");
    let router = app(&config, state.clone());
    assert_eq!(
        get(router.clone(), "/api/v1/admin/health", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = get(router.clone(), "/api/v1/admin/health", Some("t0ken")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    host.tick();
    host.monitor().journal().flush();
    let (_, journal) = get(
        router.clone(),
        "/api/v1/admin/journal?operation=host.started",
        Some("t0ken"),
    )
    .await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&journal).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (_, procs) = get(router.clone(), "/api/v1/admin/processes", Some("t0ken")).await;
    assert!(procs.contains("\"kind\":\"host\""));
    host.stop();
    assert!(dir.join("filesquid.sqlite").exists());
    release(host, router, state);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn prometheus_metrics_match_the_admin_json_route() {
    let dir = data_dir("metrics");
    let config = ServerConfig {
        data_dir: dir.clone(),
        admin_token: Some("t".into()),
        ..ServerConfig::default()
    };
    let state = ApiState::new();
    install_admin_authoriser(&config, &state).expect("authoriser");
    let host = start_host_monitor(&config, &state, None).expect("monitor");
    host.monitor().journal().flush();
    let router = app(&config, state.clone());
    let (status, _) = get(router.clone(), "/metrics", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // The refusal is journaled; let it land so both reads see the same counters.
    host.monitor().journal().flush();
    let (_, text) = get(router.clone(), "/metrics", Some("t")).await;
    let (_, json) = get(router.clone(), "/api/v1/admin/metrics", Some("t")).await;
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    for (name, value) in json.as_object().unwrap() {
        let line = text
            .lines()
            .find(|l| l.starts_with(&format!("{name} ")))
            .unwrap_or_else(|| panic!("{name}"));
        assert_eq!(line.split_whitespace().nth(1).unwrap(), value.to_string());
    }
    host.stop();
    release(host, router, state);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn the_monitor_writes_no_log_files_into_the_data_dir() {
    let dir = data_dir("nolog");
    let config = ServerConfig {
        data_dir: dir.clone(),
        admin_token: Some("t".into()),
        ..ServerConfig::default()
    };
    let state = ApiState::new();
    install_admin_authoriser(&config, &state).expect("authoriser");
    let host = start_host_monitor(&config, &state, None).expect("monitor");
    let router = app(&config, state.clone());
    assert_eq!(
        get(router.clone(), "/api/v1/admin/health", Some("t"))
            .await
            .0,
        StatusCode::OK
    );
    host.stop();
    let logs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .collect();
    assert!(logs.is_empty());
    release(host, router, state);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Drop every handle that keeps the SQLite connection open, so the data directory can be
/// removed on Windows, where open files cannot be deleted.
fn release(host: HostMonitor, router: axum::Router, state: ApiState) {
    drop(router);
    drop(state);
    drop(host);
}
