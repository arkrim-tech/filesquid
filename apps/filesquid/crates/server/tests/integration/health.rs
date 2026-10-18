//! Integration tests: the FileSquid API health route through the server host.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use filesquid_api::auth::Authoriser;
use filesquid_api::ApiState;
use filesquid_server::{app, ServerConfig};
use tower::ServiceExt;

/// Administrator token the test server accepts.
const TOKEN: &str = "health-token";

/// The server app with an administrator authoriser for [`TOKEN`].
fn server() -> axum::Router {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    app(&ServerConfig::default(), state)
}

/// `GET path` with the administrator token.
fn authorised(path: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn api_health_returns_filesquid_json() {
    let response = server()
        .oneshot(authorised("/api/v1/health"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let body = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert!(body.contains("\"status\":\"ok\""));
    assert!(body.contains("\"product\":\"filesquid\""));
}

#[tokio::test]
async fn metrics_are_exposed_for_prometheus() {
    let response = server()
        .oneshot(authorised("/metrics"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    assert!(String::from_utf8_lossy(&bytes).contains("filesquid_up 1"));
}
