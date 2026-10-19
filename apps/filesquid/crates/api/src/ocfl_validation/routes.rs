//! The OCFL validation admin routes (API-S-022): `GET /api/v1/admin/ocfl/findings`
//! (`monitor.read`) and `POST /api/v1/admin/ocfl/validation-runs` (`ocfl.validation.run`).
//!
//! The router's guard authorises the desktop owner or a server administrator before a
//! handler runs, and journals every denial. A finding's location is shown only to a caller
//! who may read the object: the desktop owner. A server administrator does not own the
//! libraries, so it gets no locations until per-object permissions arrive (API-F1.9,
//! SV-F1.2). A refused run request is journaled by the service or, for a malformed body,
//! here.

use super::error::ValidationError;
use super::service::failed;
use super::view::RunRequest;
use super::RUN_OPERATION;
use crate::auth::{Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::routes::Route;
use crate::ApiState;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json};

/// The validation routes, relative to the API prefix.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/admin/ocfl/findings", get(findings), Action::ReadMonitor),
        (
            "/admin/ocfl/validation-runs",
            post(start_runs),
            Action::RunValidation,
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
fn installed(state: &ApiState) -> Result<std::sync::Arc<dyn super::OcflValidation>, ApiError> {
    state.validation().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "OCFL validation is starting. Try again in a moment.",
        )
    })
}

/// `GET /admin/ocfl/findings`.
async fn findings(State(state): State<ApiState>, Extension(principal): Extension<Principal>) -> Response {
    let validation = match installed(&state) {
        Ok(v) => v,
        Err(error) => return error.into_response(),
    };
    let show = may_read_objects(&principal);
    let answer = tokio::task::spawn_blocking(move || validation.findings_view(show)).await;
    match answer {
        Ok(Ok(view)) => Json(view).into_response(),
        Ok(Err(err)) => ApiError::from(err).into_response(),
        Err(join) => ApiError::internal("api.admin.ocfl.findings", &join.to_string()).into_response(),
    }
}

/// `POST /admin/ocfl/validation-runs`: an empty body runs every open library from its
/// cursor.
async fn start_runs(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    body: Bytes,
) -> Response {
    let validation = match installed(&state) {
        Ok(v) => v,
        Err(error) => return error.into_response(),
    };
    let request = match parse(&body) {
        Ok(request) => request,
        Err(err) => {
            if let Some(monitor) = state.monitor() {
                monitor
                    .journal()
                    .send(failed(RUN_OPERATION, &err).actor(principal.actor()));
            }
            return ApiError::from(err).into_response();
        }
    };
    match validation.start_runs(request, principal.actor()) {
        Ok(accepted) => Json(accepted).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// The run request in `body`; an empty body is the default request.
fn parse(body: &[u8]) -> Result<RunRequest, ValidationError> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(RunRequest::default());
    }
    serde_json::from_slice(body).map_err(|_| ValidationError::Invalid {
        code: "invalid_request",
        message: "The run request is not valid JSON of the documented shape.",
    })
}
