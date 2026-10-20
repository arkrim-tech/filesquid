//! Admin routes of the FileSquid API: health, processes, journal (query, one entry, and the
//! JSON Lines or CSV export; [`journal`]) and metrics (SRS-F-061, API-S-031). Only the
//! desktop owner or a server administrator may call them: every route performs
//! [`Action::ReadMonitor`], which the [`crate::auth`] layer authorises before the handler
//! runs.
//!
//! Every failure answers with a typed [`ApiError`] (API-S-009).

pub mod journal;

pub use journal::{
    JournalEntryView, JournalNames, JournalViewPage, EXPORT_OPERATION, NEXT_CURSOR_HEADER,
};

use crate::auth::Action;
use crate::error::{ApiError, ErrorCode};
use crate::monitor::Monitor;
use crate::routes::Route;
use crate::ApiState;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;

/// Admin routes, relative to the API prefix.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/admin/health", get(health), Action::ReadMonitor),
        ("/admin/processes", get(processes), Action::ReadMonitor),
        ("/admin/journal", get(journal::page), Action::ReadMonitor),
        (
            "/admin/journal/entries/{entry}",
            get(journal::one),
            Action::ReadMonitor,
        ),
        (
            "/admin/journal/export",
            get(journal::export),
            Action::ReadMonitor,
        ),
        ("/admin/metrics", get(metrics), Action::ReadMonitor),
    ]
}

/// Resolve the installed monitor (`unavailable` before the monitor starts). The caller was
/// already authorised for [`Action::ReadMonitor`] by the route's guard.
pub(crate) fn installed_monitor(state: &ApiState) -> Result<Monitor, ApiError> {
    state.monitor().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "The monitor is starting. Try again in a moment.",
        )
    })
}

/// Run blocking store or filesystem work off the async executor; a panicked task becomes
/// an `internal` error for `operation`.
pub(crate) async fn blocking<T: Send + 'static>(
    operation: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::internal(operation, &e.to_string()))
}

async fn health(State(state): State<ApiState>) -> Response {
    let monitor = match installed_monitor(&state) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };
    match blocking("api.admin.health", move || monitor.health_report()).await {
        Ok(report) => Json(report).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn processes(State(state): State<ApiState>) -> Response {
    match installed_monitor(&state) {
        Ok(m) => Json(m.registry().snapshot()).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn metrics(State(state): State<ApiState>) -> Response {
    match installed_monitor(&state) {
        Ok(m) => Json(m.metrics().snapshot()).into_response(),
        Err(error) => error.into_response(),
    }
}
