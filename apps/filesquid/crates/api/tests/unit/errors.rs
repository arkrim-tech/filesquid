//! Typed API errors (API-S-009, ADR 030 Decision 6): every failed request answers with an
//! `ApiError` JSON body the UI can show, and the router keeps answering afterwards.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::IntoResponse;
use filesquid_api::admin::AdminAuth;
use filesquid_api::journal::{
    JournalEntry, JournalError, JournalPage, JournalQuery, JournalStore, MemoryJournal,
    RetentionPolicy,
};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::{router, ApiError, ApiState, ErrorCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

/// Send `GET path` with an optional bearer token; return status, content type and JSON body.
async fn get(state: &ApiState, path: &str, token: Option<&str>) -> (StatusCode, String, Value) {
    let mut request = Request::get(path);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router(state.clone())
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("router answers");
    let status = response.status();
    let ctype = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().expect("ascii").to_string())
        .unwrap_or_default();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, ctype, json)
}

fn server_state() -> (ApiState, Monitor) {
    let state = ApiState::new();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("server")),
        MonitorConfig::new("server", std::env::temp_dir(), AdminAuth::bearer("s3cret")),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    (state, monitor)
}

/// Assert a typed error body with the given code and the code's retryability.
fn assert_api_error(json: &Value, ctype: &str, code: &str) {
    assert_eq!(ctype, "application/json");
    assert_eq!(json["code"], code, "{json}");
    assert!(
        json["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{json}"
    );
    assert_eq!(json["retryable"], code == "unavailable", "{json}");
}

#[tokio::test]
async fn api_error_renders_as_json_with_the_code_status() {
    let response = ApiError::new(ErrorCode::Conflict, "Someone else changed it.").into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        json,
        serde_json::json!({"code": "conflict", "message": "Someone else changed it.", "retryable": false})
    );
}

#[test]
fn api_error_takes_retryability_from_its_code() {
    assert!(ApiError::new(ErrorCode::Unavailable, "later").retryable());
    assert!(!ApiError::new(ErrorCode::Forbidden, "no").retryable());
    assert_eq!(
        ApiError::new(ErrorCode::NotFound, "gone").code(),
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn admin_routes_before_the_monitor_starts_are_unavailable_and_retryable() {
    let (status, ctype, json) = get(&ApiState::new(), "/api/v1/admin/health", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_api_error(&json, &ctype, "unavailable");
}

#[tokio::test]
async fn missing_and_wrong_credentials_are_typed_errors() {
    let (state, monitor) = server_state();
    let (status, ctype, json) = get(&state, "/api/v1/admin/journal", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_api_error(&json, &ctype, "unauthorized");
    let (status, ctype, json) = get(&state, "/api/v1/admin/journal", Some("wrong")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_api_error(&json, &ctype, "forbidden");
    monitor.shutdown();
}

#[tokio::test]
async fn invalid_and_malformed_query_parameters_are_invalid_requests() {
    let (state, monitor) = server_state();
    for path in [
        "/api/v1/admin/journal?outcome=exploded",
        "/api/v1/admin/journal?after=not-a-number",
        "/api/v1/admin/journal/export?order=sideways",
        "/api/v1/admin/journal/export?after=1.5",
        "/api/v1/admin/journal?limit=0",
        "/api/v1/admin/journal?limit=1001",
    ] {
        let (status, ctype, json) = get(&state, path, Some("s3cret")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_api_error(&json, &ctype, "invalid_request");
    }
    monitor.shutdown();
}

#[tokio::test]
async fn the_router_keeps_answering_after_a_failed_request() {
    let (state, monitor) = server_state();
    let (status, _, _) = get(&state, "/api/v1/admin/journal?after=x", Some("s3cret")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, json) = get(&state, "/api/v1/admin/journal", Some("s3cret")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["entries"].is_array());
    monitor.shutdown();
}

/// Detail that a failing store reports: a path and SQL that MUST NOT reach a caller.
const SECRET_DETAIL: &str = "/home/owner/library/filesquid.sqlite: SELECT * FROM journal";

/// A journal store whose reads always fail with [`SECRET_DETAIL`]; appends succeed.
struct FailingStore;

impl JournalStore for FailingStore {
    fn host(&self) -> &str {
        "server"
    }
    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        Ok((1..=entries.len() as i64).collect())
    }
    fn query(&self, _query: &JournalQuery) -> Result<JournalPage, JournalError> {
        Err(JournalError(SECRET_DETAIL.into()))
    }
    fn count(&self, _query: &JournalQuery) -> Result<u64, JournalError> {
        Err(JournalError(SECRET_DETAIL.into()))
    }
    fn apply_retention(&self, _policy: RetentionPolicy, _now: i64) -> Result<u64, JournalError> {
        Ok(0)
    }
    fn ping(&self) -> Result<(), JournalError> {
        Ok(())
    }
}

/// Assert an `internal` error body that does not reveal [`SECRET_DETAIL`] (API-S-005).
fn assert_redacted_internal(json: &Value) {
    assert_eq!(json["code"], "internal", "{json}");
    assert_eq!(json["retryable"], false, "{json}");
    let message = json["message"].as_str().expect("message");
    assert_eq!(message, "FileSquid could not complete the request.");
    for leak in ["/home", "filesquid.sqlite", "SELECT", "journal error"] {
        assert!(
            !json.to_string().contains(leak),
            "body leaks {leak}: {json}"
        );
    }
}

#[test]
fn internal_errors_keep_the_detail_out_of_the_message() {
    let error = ApiError::internal("api.test", SECRET_DETAIL);
    assert_eq!(error.code(), ErrorCode::Internal);
    assert!(!error.retryable());
    assert_redacted_internal(&serde_json::to_value(&error).expect("serialize"));
}

#[tokio::test]
async fn a_failing_store_answers_a_redacted_internal_error_through_the_router() {
    let state = ApiState::new();
    let monitor = Monitor::start(
        Arc::new(FailingStore),
        MonitorConfig::new("server", std::env::temp_dir(), AdminAuth::bearer("s3cret")),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    for path in ["/api/v1/admin/journal", "/api/v1/admin/journal/export"] {
        let (status, ctype, json) = get(&state, path, Some("s3cret")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{path}");
        assert_eq!(ctype, "application/json", "{path}");
        assert_redacted_internal(&json);
    }
    monitor.shutdown();
}
