//! The router is transport-agnostic and mounted unchanged by every host (API-S-003,
//! ADR 030 Decision 3): it answers the same for a path-only request (how `filesquid-server`
//! hands requests to it) and for the absolute custom-scheme URIs the desktop webviews use
//! (`filesquid://localhost/...` on macOS and Linux, `http://filesquid.localhost/...` on
//! Windows).

use axum::body::Body;
use axum::http::{Method, Request};
use filesquid_api::{routed_paths, router, ApiState};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// URI prefixes a host may hand to the router for the same API path.
const ORIGINS: [&str; 3] = ["", "filesquid://localhost", "http://filesquid.localhost"];

/// One request through a fresh router; returns status, content type and body.
async fn answer(state: &ApiState, method: Method, uri: &str) -> (u16, String, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("request");
    let response = router(state.clone())
        .oneshot(request)
        .await
        .expect("router answers");
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.into_body().collect().await.expect("body");
    (status, content_type, body.to_bytes().to_vec())
}

/// Assert every origin in [`ORIGINS`] gets the same answer for `method path`.
async fn assert_same_answer_everywhere(state: &ApiState, method: Method, path: &str) {
    let reference = answer(state, method.clone(), path).await;
    for origin in &ORIGINS[1..] {
        let uri = format!("{origin}{path}");
        let got = answer(state, method.clone(), &uri).await;
        assert_eq!(got, reference, "{method} {uri} differs from {path}");
    }
}

#[tokio::test]
async fn every_routed_path_answers_the_same_through_every_host_uri_form() {
    let state = ApiState::new();
    for path in routed_paths() {
        assert_same_answer_everywhere(&state, Method::GET, &path).await;
    }
}

#[tokio::test]
async fn fallbacks_answer_the_same_through_every_host_uri_form() {
    let state = ApiState::new();
    assert_same_answer_everywhere(&state, Method::GET, "/api/v1/no-such-route").await;
    assert_same_answer_everywhere(&state, Method::DELETE, "/api/v1/health").await;
}

#[tokio::test]
async fn the_health_route_answers_through_the_desktop_scheme() {
    let state = ApiState::new();
    let (status, content_type, body) =
        answer(&state, Method::GET, "filesquid://localhost/api/v1/health").await;
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["status"], "ok");
}
