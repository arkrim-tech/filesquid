//! The import route (Task DE-1.2; F-002, API-S-036): `POST /api/v1/imports` (`files.write`)
//! asks the host to let the owner pick files or a folder in the host's own native dialog and to
//! import them into a library as one durable job, and answers the job, or that the owner
//! cancelled. No path crosses the API (API-S-005): the host reads the source and keeps where it
//! is; the answer names opaque IDs only.
//!
//! An import commits, so its body carries a [`CommandEnvelope`] (API-S-002): its operation ID
//! is the idempotency key, and the same operation ID answers the same job instead of starting a
//! second import. An import only adds new files and changes no existing one, so the envelope's
//! scope MUST be `library:<library_id>` and its expected revision is not compared. Filing the
//! files into a collection is not supported yet: a request naming a `collection_id` is
//! refused before any dialog ([`COLLECTION_NOT_SUPPORTED`]).
//!
//! What the dialog shows and how the job runs is the host's, behind [`ImportStarter`],
//! installed with [`crate::ApiState::install_import_starter`]; a host without one
//! (`filesquid-server`) answers `503 unavailable`. Each request is journaled once as
//! [`IMPORT_START_OPERATION`]: `succeeded` with the job ID, `cancelled`, or `failed` with its
//! error code; never a path or file name.

use crate::auth::{journal_entry, Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json};
use filesquid_core::CommandEnvelope;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Journal operation of an import request.
pub const IMPORT_START_OPERATION: &str = "import.start";

/// Journal error code of a request naming a `collection_id`: imports do not file into a
/// collection yet, so the request is refused rather than its files left unfiled.
pub const COLLECTION_NOT_SUPPORTED: &str = "collection_not_supported";

/// What the owner picks in the host's dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    /// One or more files.
    Files,
    /// One folder, with every file below it.
    Folder,
}

/// Body of `POST /api/v1/imports`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    /// The command envelope: the operation ID is the idempotency key; the scope is
    /// `library:<library_id>`.
    pub envelope: CommandEnvelope,
    /// The open library to import into.
    pub library_id: String,
    /// Reserved for filing the files into a collection; not supported yet, so a request
    /// that carries it is refused ([`COLLECTION_NOT_SUPPORTED`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// Files or a folder.
    pub source: ImportSource,
}

/// How an import request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// The import job runs (or ran, for a repeated operation ID); `job_id` names it.
    Started,
    /// The owner closed the dialog without picking anything; no job was made.
    Cancelled,
}

/// The answer of an import request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportAnswer {
    /// The request's operation ID.
    pub operation_id: String,
    /// Started or cancelled.
    pub outcome: ImportOutcome,
    /// The import job; absent when cancelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

/// Why a host did not start an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportStartError {
    /// The library is not open on this host.
    LibraryNotFound,
    /// A dialog is already open.
    Busy,
    /// The host cannot show a dialog or run a job now.
    Unavailable,
}

impl ImportStartError {
    /// The journal error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            ImportStartError::LibraryNotFound => "library_not_found",
            ImportStartError::Busy => "dialog_open",
            ImportStartError::Unavailable => "unavailable",
        }
    }
}

/// The host's import dialog and job runner.
pub trait ImportStarter: Send + Sync {
    /// Answer the job of `request`'s operation ID when one exists; otherwise let the owner
    /// pick the source, submit one import job for it and start it in the background, as
    /// `actor`. Blocks until the dialog closes.
    ///
    /// # Errors
    /// An [`ImportStartError`]; no job is made.
    fn start(&self, actor: &str, request: &ImportRequest)
        -> Result<ImportAnswer, ImportStartError>;
}

/// The import route, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![("/imports", post(start), Action::WriteFiles)]
}

/// `POST /api/v1/imports`.
async fn start(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<ImportRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let request = match request.ok().map(|Json(r)| r).filter(scoped) {
        Some(request) => request,
        None => return refuse_invalid(&state, &actor).await,
    };
    if request.collection_id.is_some() {
        return refuse_collection(&state, &actor, &request).await;
    }
    let started = match state.import_starter() {
        Some(starter) => run(starter, actor.clone(), request.clone()).await,
        None => Err(ImportStartError::Unavailable),
    };
    match started {
        Ok(answer) => answer_started(&state, &actor, answer).await,
        Err(err) => refuse(&state, &actor, &request, err).await,
    }
}

/// Whether the envelope's scope names the request's library. Pure.
fn scoped(request: &ImportRequest) -> bool {
    request.envelope.scope().as_str() == format!("library:{}", request.library_id)
}

/// Run the host's starter off the async executor; a panicked starter is unavailable.
async fn run(
    starter: Arc<dyn ImportStarter>,
    actor: String,
    request: ImportRequest,
) -> Result<ImportAnswer, ImportStartError> {
    let joined = tokio::task::spawn_blocking(move || starter.start(&actor, &request)).await;
    joined.unwrap_or(Err(ImportStartError::Unavailable))
}

/// Journal the answer and send it.
async fn answer_started(state: &ApiState, actor: &str, answer: ImportAnswer) -> Response {
    let entry = match &answer.job_id {
        Some(job) => JournalEntry::new(IMPORT_START_OPERATION, Outcome::Succeeded).target(job),
        None => JournalEntry::new(IMPORT_START_OPERATION, Outcome::Cancelled),
    };
    let entry = entry
        .actor(actor)
        .operation_id(answer.operation_id.as_str());
    journal_entry(state, entry).await;
    Json(answer).into_response()
}

/// Journal an unreadable or mis-scoped body once and answer `400 invalid_request`.
async fn refuse_invalid(state: &ApiState, actor: &str) -> Response {
    let entry = JournalEntry::new(IMPORT_START_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .error_code("invalid_request");
    journal_entry(state, entry).await;
    let message = "The request names unknown, missing or malformed fields, or its envelope's \
                   scope is not library:<library_id>.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Journal a request naming a collection once and answer `400 invalid_request`.
async fn refuse_collection(state: &ApiState, actor: &str, request: &ImportRequest) -> Response {
    let entry = JournalEntry::new(IMPORT_START_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .operation_id(request.envelope.operation_id().as_str())
        .error_code(COLLECTION_NOT_SUPPORTED);
    journal_entry(state, entry).await;
    let message = "Imports cannot be filed into a collection yet (collection_not_supported); \
                   send the request without collection_id.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Journal the refusal once and answer its typed error.
async fn refuse(
    state: &ApiState,
    actor: &str,
    request: &ImportRequest,
    err: ImportStartError,
) -> Response {
    let (level, code, message) = match err {
        ImportStartError::LibraryNotFound => (
            Level::Warn,
            ErrorCode::NotFound,
            "That library is not open on this host.",
        ),
        ImportStartError::Busy => (
            Level::Warn,
            ErrorCode::Conflict,
            "A dialog is already open.",
        ),
        ImportStartError::Unavailable => (
            Level::Error,
            ErrorCode::Unavailable,
            "This host cannot import now.",
        ),
    };
    let entry = JournalEntry::new(IMPORT_START_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .operation_id(request.envelope.operation_id().as_str())
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}
