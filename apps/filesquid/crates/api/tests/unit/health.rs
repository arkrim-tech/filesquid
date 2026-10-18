//! Unit tests for `GET /api/v1/health` on the shared FileSquid router.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::Authoriser;
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Send one GET request with the administrator token to a fresh router; return status and
/// body text.
async fn get(path: &str) -> (StatusCode, String) {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator("t"));
    let request = Request::get(path)
        .header(header::AUTHORIZATION, "Bearer t")
        .body(Body::empty())
        .expect("request");
    let response = router(state)
        .oneshot(request)
        .await
        .expect("router answers");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

#[tokio::test]
async fn health_is_ok_and_names_filesquid() {
    let (status, body) = get("/api/v1/health").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["product"], "filesquid");
    assert_eq!(json["schema_version"], 1);
}

#[tokio::test]
async fn unknown_api_path_is_not_found() {
    let (status, _) = get("/api/v1/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
