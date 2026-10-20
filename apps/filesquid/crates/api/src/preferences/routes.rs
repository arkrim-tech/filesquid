//! The preference routes: `GET` and `PUT /api/v1/preferences` (action `preferences.manage`).
//!
//! Each caller reads and replaces only their own preferences, keyed by the principal's opaque
//! actor name. A save is journaled once as [`UPDATE_OPERATION`] with the actor only, never
//! the values; a refusal is journaled once with its error code; a successful read is not
//! journaled (API-S-017).

use super::model::Preferences;
use super::store::{PreferenceStore, PreferenceStoreError};
use super::{LOG_TARGET, READ_OPERATION, UPDATE_OPERATION};
use crate::auth::{journal_entry, Action, Principal};
use crate::error::ApiError;
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::{ApiState, ErrorCode};
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json};
use std::sync::Arc;

/// The preference routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![(
        "/preferences",
        get(read).put(update),
        Action::ManagePreferences,
    )]
}

/// `GET /api/v1/preferences`: the caller's saved preferences, or the defaults.
async fn read(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Response {
    let user = principal.actor();
    let loaded = match installed(&state) {
        Ok(store) => blocking(move || store.load(user)).await,
        Err(err) => Err(err),
    };
    match loaded {
        Ok(saved) => Json(saved.unwrap_or_default()).into_response(),
        Err(err) => refuse(&state, READ_OPERATION, user, err).await,
    }
}

/// `PUT /api/v1/preferences`: replace the caller's preferences with the body.
async fn update(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Preferences>, JsonRejection>,
) -> Response {
    let user = principal.actor();
    match save(&state, user, request).await {
        Ok(saved) => {
            let entry = JournalEntry::new(UPDATE_OPERATION, Outcome::Succeeded).actor(user);
            journal_entry(&state, entry).await;
            Json(saved).into_response()
        }
        Err(err) => refuse(&state, UPDATE_OPERATION, user, err).await,
    }
}

/// Check the body and save it for `user` on a blocking thread; answers what was saved.
async fn save(
    state: &ApiState,
    user: &'static str,
    request: Result<Json<Preferences>, JsonRejection>,
) -> Result<Preferences, ApiError> {
    let store = installed(state)?;
    let Json(preferences) = request.map_err(|_| {
        invalid("The JSON body has unknown, missing or malformed preference fields.")
    })?;
    preferences.check().map_err(invalid)?;
    let saved = preferences.clone();
    blocking(move || store.save(user, &saved)).await?;
    Ok(preferences)
}

/// The installed store, or `unavailable` until the host installs one.
fn installed(state: &ApiState) -> Result<Arc<dyn PreferenceStore>, ApiError> {
    state.preferences().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "This host does not keep preferences yet.",
        )
    })
}

/// Run a store call on a blocking thread: hosts keep preferences in a database.
async fn blocking<T: Send + 'static>(
    call: impl FnOnce() -> Result<T, PreferenceStoreError> + Send + 'static,
) -> Result<T, ApiError> {
    let joined = tokio::task::spawn_blocking(call).await.map_err(|join| {
        tracing::info!(target: LOG_TARGET, error = %join, "preference task failed");
        ApiError::new(
            ErrorCode::Internal,
            "The preferences could not be read or saved.",
        )
    })?;
    joined.map_err(store_error)
}

/// The API error of a store refusal: `unavailable` (retryable) when the store cannot be
/// reached, `conflict` (not retryable) when it keeps no more users.
fn store_error(err: PreferenceStoreError) -> ApiError {
    match err {
        PreferenceStoreError::Unavailable(detail) => {
            tracing::info!(target: LOG_TARGET, %detail, "preference store unavailable");
            ApiError::new(
                ErrorCode::Unavailable,
                "The preferences cannot be reached right now; try again shortly.",
            )
        }
        PreferenceStoreError::Full => ApiError::new(
            ErrorCode::Conflict,
            "This host keeps preferences for no more users.",
        ),
    }
}

/// `invalid_request` with `message`.
fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::InvalidRequest, message)
}

/// Journal the refusal once, with its code, and answer the typed error.
async fn refuse(state: &ApiState, operation: &str, actor: &str, err: ApiError) -> Response {
    let level = match err.code() {
        ErrorCode::Internal | ErrorCode::Unavailable => Level::Error,
        _ => Level::Warn,
    };
    let entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.code().as_str());
    journal_entry(state, entry).await;
    err.into_response()
}
