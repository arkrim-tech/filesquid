//! Integration tests: `filesquid-server` serves a distribution UI from `FILESQUID_UI_DIR`,
//! including HTTP `Range` reads (needed for previews and video seeking).

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use filesquid_api::auth::Authoriser;
use filesquid_api::ApiState;
use filesquid_server::{app, ServerConfig};
use std::path::PathBuf;
use tower::ServiceExt;

/// Create a throwaway UI directory with an `index.html` and a 1 KiB asset.
fn ui_fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("filesquid-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).expect("fixture dir");
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>FileSquid</title>",
    )
    .expect("index");
    std::fs::write(dir.join("assets/blob.bin"), vec![7u8; 1024]).expect("asset");
    dir
}

/// Build the server app with the given UI directory.
fn server(ui_dir: PathBuf) -> axum::Router {
    let config = ServerConfig {
        ui_dir: Some(ui_dir),
        ..ServerConfig::default()
    };
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator("ui-token"));
    app(&config, state)
}

#[tokio::test]
async fn root_serves_the_distribution_index() {
    let dir = ui_fixture("index");
    let response = server(dir.clone())
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("<title>FileSquid</title>"));
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn range_requests_return_partial_content() {
    let dir = ui_fixture("range");
    let response = server(dir.clone())
        .oneshot(
            Request::get("/assets/blob.bin")
                .header(header::RANGE, "bytes=0-99")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    assert_eq!(bytes.len(), 100);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn api_routes_take_precedence_over_static_files() {
    let dir = ui_fixture("precedence");
    let response = server(dir.clone())
        .oneshot(
            Request::get("/api/v1/health")
                .header(header::AUTHORIZATION, "Bearer ui-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    std::fs::remove_dir_all(dir).expect("cleanup");
}
