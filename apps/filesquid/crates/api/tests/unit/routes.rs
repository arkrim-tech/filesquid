//! The router is transport-agnostic and mounted unchanged by every host (API-S-003,
//! ADR 030 Decision 3): it answers the same for a path-only request (how `filesquid-server`
//! hands requests to it) and for the absolute custom-scheme URIs the desktop webviews use
//! (`filesquid://localhost/...` on macOS and Linux, `http://filesquid.localhost/...` on
//! Windows).

use axum::body::Body;
use axum::http::{header, Method, Request};
use filesquid_api::auth::{Action, Authoriser};
use filesquid_api::{route_actions, routed_paths, router, ApiState};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// URI prefixes a host may hand to the router for the same API path.
const ORIGINS: [&str; 3] = ["", "filesquid://localhost", "http://filesquid.localhost"];

/// Administrator token accepted by [`authorised_state`].
const TOKEN: &str = "routes-token";

/// API state whose authoriser accepts [`TOKEN`].
fn authorised_state() -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    state
}

/// One authorised request through a fresh router; returns status, content type and body.
async fn answer(state: &ApiState, method: Method, uri: &str) -> (u16, String, Vec<u8>) {
    answer_as(state, method, uri, Some(TOKEN)).await
}

/// One request through a fresh router with an optional bearer token; returns status,
/// content type and body.
async fn answer_as(
    state: &ApiState,
    method: Method,
    uri: &str,
    token: Option<&str>,
) -> (u16, String, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = request.body(Body::empty()).expect("request");
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
    let state = authorised_state();
    for path in routed_paths() {
        assert_same_answer_everywhere(&state, Method::GET, &path).await;
    }
}

#[tokio::test]
async fn denials_answer_the_same_through_every_host_uri_form() {
    let state = authorised_state();
    let mut cases: Vec<(Method, String)> = route_actions()
        .into_iter()
        .filter(|(_, action)| !matches!(action, Action::Public | Action::ReadContent))
        .map(|(path, _)| (Method::GET, path))
        .collect();
    cases.push((Method::GET, "/api/v1/no-such-route".to_string()));
    cases.push((Method::DELETE, "/api/v1/health".to_string()));
    for (method, path) in cases {
        let reference = answer_as(&state, method.clone(), &path, None).await;
        assert_eq!(reference.0, 401, "{method} {path}");
        let json: serde_json::Value = serde_json::from_slice(&reference.2).expect("ApiError");
        assert_eq!(json["reason"], "missing_credentials", "{method} {path}");
        for origin in &ORIGINS[1..] {
            let uri = format!("{origin}{path}");
            let got = answer_as(&state, method.clone(), &uri, None).await;
            assert_eq!(got, reference, "{method} {uri} differs from {path}");
        }
    }
}

#[tokio::test]
async fn fallbacks_answer_the_same_through_every_host_uri_form() {
    let state = authorised_state();
    assert_same_answer_everywhere(&state, Method::GET, "/api/v1/no-such-route").await;
    assert_same_answer_everywhere(&state, Method::DELETE, "/api/v1/health").await;
}

#[tokio::test]
async fn the_health_route_answers_through_the_desktop_scheme() {
    let state = authorised_state();
    let (status, content_type, body) =
        answer(&state, Method::GET, "filesquid://localhost/api/v1/health").await;
    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["status"], "ok");
}

/// The headers a content denial MUST carry (API-S-018).
const DENIAL_HEADERS: [&str; 5] = [
    "content-type",
    "cache-control",
    "referrer-policy",
    "x-content-type-options",
    "content-security-policy",
];

/// One unauthenticated request through a fresh router; returns status, the
/// [`DENIAL_HEADERS`] it carries and the body.
async fn denial_answer(
    state: &ApiState,
    method: Method,
    uri: &str,
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let request = Request::builder().method(method).uri(uri);
    let request = request.body(Body::empty()).expect("request");
    let response = router(state.clone())
        .oneshot(request)
        .await
        .expect("router answers");
    let status = response.status().as_u16();
    let headers = DENIAL_HEADERS
        .iter()
        .map(|name| {
            let value = response.headers().get(*name);
            let value = value.and_then(|v| v.to_str().ok()).unwrap_or_default();
            (name.to_string(), value.to_string())
        })
        .collect();
    let body = response.into_body().collect().await.expect("body");
    (status, headers, body.to_bytes().to_vec())
}

#[tokio::test]
async fn the_content_denial_answers_the_same_through_every_host_uri_form() {
    let state = authorised_state();
    for path in [
        "/api/v1/content/{handle}",
        "/api/v1/content/",
        "/api/v1/content/a/b",
    ] {
        let reference = denial_answer(&state, Method::GET, path).await;
        assert_eq!(reference.0, 403, "{path}");
        assert!(reference.1.iter().all(|(_, v)| !v.is_empty()), "{path}");
        let json: serde_json::Value = serde_json::from_slice(&reference.2).expect("ApiError");
        assert_eq!(json["code"], "forbidden", "{path}");
        assert_eq!(json["reason"], "invalid_credentials", "{path}");
        let head = denial_answer(&state, Method::HEAD, path).await;
        assert_eq!((head.0, &head.1), (403, &reference.1), "HEAD {path}");
        assert!(head.2.is_empty(), "HEAD {path} has no body");
        for origin in &ORIGINS[1..] {
            let uri = format!("{origin}{path}");
            let got = denial_answer(&state, Method::GET, &uri).await;
            assert_eq!(got, reference, "{uri} differs from {path}");
            let got_head = denial_answer(&state, Method::HEAD, &uri).await;
            assert_eq!(got_head, head, "HEAD {uri} differs from {path}");
        }
    }
}
