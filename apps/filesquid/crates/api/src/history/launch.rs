//! The history backfill route (API-S-029; ADR 040 Decision 7):
//! `POST /api/v1/admin/history/backfill` (`history.backfill`, the desktop owner or a server
//! administrator) starts one backfill over every storage root the host serves, in the
//! background, and answers `200` at once.
//!
//! The backfill itself is the API's [`super::backfill::Backfill`] job: each run registers in
//! the process registry, checkpoints per object, stops at its cap or on cancel, skips
//! versions that have a record and commits one provenance-only version per object. Which
//! roots it visits and on which thread it runs is the host's, behind [`BackfillLauncher`],
//! installed with [`crate::ApiState::install_history_backfill`]; until then the route
//! answers `503 unavailable`. A request while a backfill runs is refused `409 conflict`.
//!
//! The route journals the request once as [`BACKFILL_REQUEST_OPERATION`]: `succeeded` when the
//! backfill started, `failed` with its error code when refused (an authorisation refusal is
//! journaled by the guard as `api.auth.denied`). Each run journals itself.

use crate::auth::{journal_entry, Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

/// Journal operation of a backfill request.
pub const BACKFILL_REQUEST_OPERATION: &str = "history.backfill.request";

/// Why a host did not start a backfill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// A backfill is already running on this host.
    Running,
    /// The host cannot start one now (no thread, its stores unreachable).
    Unavailable,
}

impl LaunchError {
    /// The journal error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            LaunchError::Running => "backfill_running",
            LaunchError::Unavailable => "unavailable",
        }
    }
}

/// What a host runs for the backfill route.
pub trait BackfillLauncher: Send + Sync {
    /// Start one backfill over every storage root the host serves, in the background, as
    /// `actor`, and return at once.
    ///
    /// # Errors
    ///
    /// [`LaunchError::Running`] while a backfill runs; [`LaunchError::Unavailable`] when
    /// none can start now.
    fn launch(&self, actor: &str) -> Result<(), LaunchError>;
}

/// The answer of a started backfill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillStarted {
    /// Always `started`: the backfill runs in the background; its runs appear in the
    /// process registry and the journal as `prov.backfill`.
    pub status: String,
}

/// The backfill route, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![(
        "/admin/history/backfill",
        post(start),
        Action::RunHistoryBackfill,
    )]
}

/// `POST /api/v1/admin/history/backfill`.
async fn start(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Response {
    let actor = principal.actor().to_string();
    let launched = match state.history_backfill() {
        Some(launcher) => launch(launcher, actor.clone()).await,
        None => Err(LaunchError::Unavailable),
    };
    match launched {
        Ok(()) => started(&state, &actor).await,
        Err(err) => refuse(&state, &actor, err).await,
    }
}

/// Run the host's launcher off the async executor; a panicked launcher is unavailable.
async fn launch(
    launcher: std::sync::Arc<dyn BackfillLauncher>,
    actor: String,
) -> Result<(), LaunchError> {
    let joined = tokio::task::spawn_blocking(move || launcher.launch(&actor)).await;
    joined.unwrap_or(Err(LaunchError::Unavailable))
}

/// Journal the started request and answer `200`.
async fn started(state: &ApiState, actor: &str) -> Response {
    let entry = JournalEntry::new(BACKFILL_REQUEST_OPERATION, Outcome::Succeeded).actor(actor);
    journal_entry(state, entry).await;
    let body = BackfillStarted {
        status: "started".into(),
    };
    (StatusCode::OK, Json(body)).into_response()
}

/// Journal the refusal once and answer its typed error.
async fn refuse(state: &ApiState, actor: &str, err: LaunchError) -> Response {
    let (level, code, message) = match err {
        LaunchError::Running => (
            Level::Warn,
            ErrorCode::Conflict,
            "A history backfill is already running.",
        ),
        LaunchError::Unavailable => (
            Level::Error,
            ErrorCode::Unavailable,
            "The history backfill cannot start now. Try again in a moment.",
        ),
    };
    let entry = JournalEntry::new(BACKFILL_REQUEST_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}
