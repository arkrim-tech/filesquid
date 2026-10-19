//! The OCFL validation admin routes (API-S-022):
//!
//! - `GET /api/v1/admin/ocfl/findings` (`monitor.read`): the findings, settings and runs;
//! - `POST /api/v1/admin/ocfl/validation-runs` (`ocfl.validation.run`): start runs;
//! - `PUT /api/v1/admin/ocfl/validation-settings` (`ocfl.validation.configure`): change the
//!   schedule and cap.
//!
//! The router's guard authorises the desktop owner or a server administrator before a
//! handler runs, and journals every denial. A finding's location is shown only to a caller
//! who may read the object: the desktop owner. A server administrator does not own the
//! libraries, so it gets no locations until per-object permissions arrive (API-F1.9,
//! SV-F1.2). A refused request is journaled by the service or, for a body that cannot be
//! read or parsed, here.

use super::error::ValidationError;
use super::service::{failed, ValidationConfig};
use super::view::RunRequest;
use super::{OcflValidation, CONFIGURE_OPERATION, RUN_OPERATION};
use crate::auth::{Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::routes::Route;
use crate::ApiState;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json};
use serde::de::DeserializeOwned;
use std::sync::Arc;

/// The validation routes, relative to the API prefix.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/admin/ocfl/findings", get(findings), Action::ReadMonitor),
        (
            "/admin/ocfl/validation-runs",
            post(start_runs),
            Action::RunValidation,
        ),
        (
            "/admin/ocfl/validation-settings",
            put(configure),
            Action::ConfigureValidation,
        ),
    ]
}

/// Whether `principal` may read the objects of the open libraries, and so see where a
/// finding is. Adding a principal makes this fail to compile until that is decided.
fn may_read_objects(principal: &Principal) -> bool {
    match principal {
        Principal::Owner => true,
        Principal::Administrator => false,
    }
}

/// The installed validation, or `unavailable` before the host installs it.
fn installed(state: &ApiState) -> Result<Arc<dyn OcflValidation>, ApiError> {
    state.validation().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "OCFL validation is starting. Try again in a moment.",
        )
    })
}

/// `GET /admin/ocfl/findings`.
async fn findings(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Response {
    let validation = match installed(&state) {
        Ok(v) => v,
        Err(error) => return error.into_response(),
    };
    let show = may_read_objects(&principal);
    let answer = tokio::task::spawn_blocking(move || validation.findings_view(show)).await;
    match answer {
        Ok(Ok(view)) => Json(view).into_response(),
        Ok(Err(err)) => ApiError::from(err).into_response(),
        Err(join) => {
            ApiError::internal("api.admin.ocfl.findings", &join.to_string()).into_response()
        }
    }
}

/// `POST /admin/ocfl/validation-runs`: an empty body runs every open library from its
/// cursor.
async fn start_runs(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let validation = match installed(&state) {
        Ok(v) => v,
        Err(error) => return error.into_response(),
    };
    let request = match read_body::<RunRequest>(body, true) {
        Ok(request) => request,
        Err(err) => return refuse(&state, RUN_OPERATION, &principal, err),
    };
    // Starting runs reads each library's last run from the findings store (SQLite on the
    // hosts), so it runs on a blocking thread, like the findings view.
    let actor = principal.actor().to_string();
    let answer = tokio::task::spawn_blocking(move || validation.start_runs(request, &actor)).await;
    match answer {
        Ok(Ok(accepted)) => Json(accepted).into_response(),
        Ok(Err(err)) => ApiError::from(err).into_response(),
        Err(join) => {
            ApiError::internal("api.admin.ocfl.validation_runs", &join.to_string()).into_response()
        }
    }
}

/// `PUT /admin/ocfl/validation-settings`: answers the settings now in force.
async fn configure(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let validation = match installed(&state) {
        Ok(v) => v,
        Err(error) => return error.into_response(),
    };
    let config = match read_body::<ValidationConfig>(body, false) {
        Ok(config) => config,
        Err(err) => return refuse(&state, CONFIGURE_OPERATION, &principal, err),
    };
    match validation.configure(principal.actor(), config) {
        Ok(()) => Json(config).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// Journal the refused `operation` for `principal` and answer `err`.
fn refuse(
    state: &ApiState,
    operation: &str,
    principal: &Principal,
    err: ValidationError,
) -> Response {
    if let Some(monitor) = state.monitor() {
        monitor
            .journal()
            .send(failed(operation, &err).actor(principal.actor()));
    }
    ApiError::from(err).into_response()
}

/// The JSON body as `T`. An empty body is `T::default()` when `empty_is_default`. A body
/// that cannot be read (too large, cut off) or parsed is `invalid_request`.
fn read_body<T: DeserializeOwned + Default>(
    body: Result<Bytes, BytesRejection>,
    empty_is_default: bool,
) -> Result<T, ValidationError> {
    let invalid = ValidationError::Invalid {
        code: "invalid_request",
        message: "The request body is not valid JSON of the documented shape.",
    };
    let bytes = body.map_err(|_| invalid.clone())?;
    if empty_is_default && bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid)
}
