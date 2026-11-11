//! The navigation routes (API-S-021): `GET /api/v1/navigation` and
//! `GET /api/v1/navigation/destination` (action `navigation.read`).
//!
//! The router's guard authorises the caller and journals a denial before the handler runs
//! (API-S-004). A refusal after that (an unusable page request, an unknown or unreadable
//! node, a failing port) is journaled once as [`NAVIGATE_OPERATION`], or
//! [`DESTINATION_OPERATION`] for the destination lookup, with its error code and no node,
//! name or path; a successful read is not journaled (API-S-017).

use super::error::NavigationError;
use super::view::{DestinationRequest, DestinationView, NavigationRequest};
use super::{NavigationService, DESTINATION_OPERATION, NAVIGATE_OPERATION};
use crate::auth::{journal_entry, Action, Principal};
use crate::error::ApiError;
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::{ApiState, ErrorCode};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json};
use std::sync::Arc;

/// The navigation route, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/navigation", get(navigation), Action::ReadNavigation),
        (
            "/navigation/destination",
            get(destination),
            Action::ReadNavigation,
        ),
    ]
}

/// The installed service, or `unavailable` until the host installs one.
fn installed(state: &ApiState) -> Result<Arc<NavigationService>, NavigationError> {
    state.navigation().ok_or_else(NavigationError::unavailable)
}

/// The query, or `invalid_request` for an unknown, missing or malformed parameter.
fn query<T>(request: Result<Query<T>, QueryRejection>) -> Result<T, NavigationError> {
    request
        .map(|Query(q)| q)
        .map_err(|_| NavigationError::invalid("The query names unknown or malformed parameters."))
}

/// `GET /api/v1/navigation`.
async fn navigation(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<NavigationRequest>, QueryRejection>,
) -> Response {
    match answer(&state, &principal, request).await {
        Ok(page) => Json(page).into_response(),
        Err(err) => refuse(&state, NAVIGATE_OPERATION, &principal, err).await,
    }
}

/// `GET /api/v1/navigation/destination`.
async fn destination(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<DestinationRequest>, QueryRejection>,
) -> Response {
    match resolve_destination(&state, &principal, request).await {
        Ok(found) => Json(found).into_response(),
        Err(err) => refuse(&state, DESTINATION_OPERATION, &principal, err).await,
    }
}

/// Resolve on a blocking thread: the catalog and the folder index may block on storage.
async fn resolve_destination(
    state: &ApiState,
    principal: &Principal,
    request: Result<Query<DestinationRequest>, QueryRejection>,
) -> Result<DestinationView, NavigationError> {
    let service = installed(state)?;
    let request = query(request)?;
    let principal = principal.clone();
    let read = move || service.destination(&principal, &request.node);
    tokio::task::spawn_blocking(read).await.map_err(|join| {
        tracing::info!(target: super::LOG_TARGET, error = %join, "destination task failed");
        NavigationError::internal()
    })?
}

/// List on a blocking thread: the catalog and the folder index may block on storage.
async fn answer(
    state: &ApiState,
    principal: &Principal,
    request: Result<Query<NavigationRequest>, QueryRejection>,
) -> Result<super::NavigationPage, NavigationError> {
    let service = installed(state)?;
    let request = query(request)?;
    let principal = principal.clone();
    let listed = tokio::task::spawn_blocking(move || service.list(&principal, &request)).await;
    listed.map_err(|join| {
        tracing::info!(target: super::LOG_TARGET, error = %join, "navigation task failed");
        NavigationError::internal()
    })?
}

/// Journal the refusal of `operation` once and answer the typed error.
async fn refuse(
    state: &ApiState,
    operation: &'static str,
    principal: &Principal,
    err: NavigationError,
) -> Response {
    let level = match err.api_code() {
        ErrorCode::Internal | ErrorCode::Unavailable => Level::Error,
        _ => Level::Warn,
    };
    let entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(principal.actor())
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::from(err).into_response()
}
