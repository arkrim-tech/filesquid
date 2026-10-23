//! The import route (Task DE-1.2; F-002, API-S-036): `POST /api/v1/imports` (`files.write`)
//! asks the host to let the owner pick files or a folder in the host's own native dialog and to
//! import them into a library as one durable job, and answers the job, or that the owner
//! cancelled. No path crosses the API (API-S-005): the host reads the source and keeps where it
//! is; the answer names opaque IDs only.
//!
//! An import commits, so its body carries a [`CommandEnvelope`] (API-S-002): its operation ID
//! is the idempotency key, and the same operation ID answers the same job instead of starting a
//! second import. An import only adds new files and changes no existing one, so the envelope's
//! scope MUST be `library:<library_id>` and its expected revision is not compared.
//!
//! Every import names its destination (owner decision 2026-10-22, "Must choose", ADR 041
//! Decision 4): `collection_id`, a collection of the library, and optionally `folder`, a
//! collection logical folder path inside it. A request without `collection_id` is refused
//! before any dialog ([`DESTINATION_REQUIRED`]), so nothing is imported to a library's top
//! level and no default collection is chosen; an unknown collection, or one of another
//! library, is `not_found` ([`COLLECTION_NOT_FOUND`]); a folder outside the collection-path
//! rule or inside a reserved folder is `invalid_request` (`invalid_path`). The collection is
//! looked up through the navigation service's library catalog, the one meaning of API-S-019.
//!
//! What the dialog shows and how the job runs is the host's, behind [`ImportStarter`],
//! installed with [`crate::ApiState::install_import_starter`]; a host without one
//! (`filesquid-server`) answers `503 unavailable`. Each request is journaled once as
//! [`IMPORT_START_OPERATION`]: `succeeded` with the job ID, `cancelled`, or `failed` with its
//! error code; never a path or file name.

use crate::auth::{journal_entry, Action, Principal};
use crate::canonical::collection_path::check_folder;
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::library::CatalogError;
use crate::navigation::{NavigationError, NodeRef};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json};
use filesquid_core::{
    CollectionId, CommandEnvelope, CommandOutcome, CommandOutcomeKind, OperationId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Journal operation of an import request.
pub const IMPORT_START_OPERATION: &str = "import.start";

/// Journal error code of a request that names no destination collection.
pub const DESTINATION_REQUIRED: &str = "destination_required";

/// Journal error code of a destination collection that is unknown or of another library.
pub const COLLECTION_NOT_FOUND: &str = "collection_not_found";

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
    /// The destination collection, a collection of the library. Required: a request
    /// without it is refused ([`DESTINATION_REQUIRED`]); it is optional here only so that
    /// refusal can be journaled with its own code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// The destination folder inside the collection, a collection logical path; the
    /// collection's root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Files or a folder.
    pub source: ImportSource,
}

/// What became of an import request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// The import job runs (or ran, for a repeated operation ID); `job_id` names it.
    Started,
    /// The owner closed the dialog without picking anything; no job was made.
    Cancelled,
}

/// The answer of an import request: the command outcome API-S-002 requires, and what
/// became of the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportAnswer {
    /// `pending` for a started import (the job's work is durable and not yet complete; a
    /// repeated operation ID answers it again), `committed` for a cancelled one (nothing
    /// changed, as a library command that changes nothing answers).
    pub outcome: CommandOutcome,
    /// Started or cancelled.
    pub import: ImportOutcome,
    /// The import job; absent when cancelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

impl ImportAnswer {
    /// The answer naming the import job `job_id` of `operation_id`. Pure.
    pub fn started(operation_id: &OperationId, job_id: String) -> Self {
        Self {
            outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Pending),
            import: ImportOutcome::Started,
            job_id: Some(job_id),
        }
    }

    /// The answer of `operation_id` whose dialog the owner closed. Pure.
    pub fn cancelled(operation_id: &OperationId) -> Self {
        Self {
            outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Committed),
            import: ImportOutcome::Cancelled,
            job_id: None,
        }
    }
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

    /// The answer of the job `request`'s operation ID already started, if any, without a
    /// dialog: the route answers a repeated operation with it before checking the destination
    /// again, so a retry of a started import is never refused because its collection changed
    /// since. The default knows of no job.
    ///
    /// # Errors
    /// [`ImportStartError::Unavailable`] when the host cannot tell.
    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        let _ = request;
        Ok(None)
    }
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
    let starter = state.import_starter();
    match replayed(starter.clone(), request.clone()).await {
        Ok(Some(answer)) => return answer_started(&state, &actor, answer).await,
        Ok(None) => {}
        Err(err) => return refuse(&state, &actor, &request, err).await,
    }
    if let Err(refusal) = checked_destination(&state, &principal, &request).await {
        return refuse_destination(&state, &actor, &request, refusal).await;
    }
    let started = match starter {
        Some(starter) => run(starter, actor.clone(), request.clone()).await,
        None => Err(ImportStartError::Unavailable),
    };
    match started {
        Ok(answer) => answer_started(&state, &actor, answer).await,
        Err(err) => refuse(&state, &actor, &request, err).await,
    }
}

/// Whether the request is well formed: the protocol's schema version, a library ID, and an
/// envelope scope naming that library. Pure.
fn scoped(request: &ImportRequest) -> bool {
    request.envelope.schema_version() == filesquid_core::PROTOCOL_SCHEMA_VERSION
        && !request.library_id.is_empty()
        && request.envelope.scope().as_str() == format!("library:{}", request.library_id)
}

/// The answer of the job the request's operation already started, asked of the host's
/// starter off the async executor; none without a starter.
async fn replayed(
    starter: Option<Arc<dyn ImportStarter>>,
    request: ImportRequest,
) -> Result<Option<ImportAnswer>, ImportStartError> {
    let Some(starter) = starter else {
        return Ok(None);
    };
    let joined = tokio::task::spawn_blocking(move || starter.replay(&request)).await;
    joined.unwrap_or(Err(ImportStartError::Unavailable))
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
        .operation_id(answer.outcome.operation_id().as_str());
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
    let message = "The request names unknown, missing or malformed fields, its envelope is not \
                   schema version 1, or its scope is not library:<library_id>.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Why a request's destination was refused before any dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DestinationRefusal {
    /// No `collection_id`.
    Required,
    /// A `folder` outside the collection-path rule or in a reserved folder.
    Folder,
    /// An unknown collection, one of another library, or one the caller may not read.
    NotFound,
    /// The library catalog cannot answer now.
    Unavailable,
    /// The library catalog failed in a way a retry will not fix.
    Internal,
}

impl DestinationRefusal {
    /// The refusal of a catalog failure: `unavailable` only when a retry may succeed. Pure.
    fn of_catalog(err: &CatalogError) -> Self {
        match err {
            CatalogError::Unavailable(_) => Self::Unavailable,
            CatalogError::Conflict | CatalogError::Failed(_) => Self::Internal,
        }
    }

    /// The refusal of a readability check of the collection and its ancestors. Pure.
    fn of_navigation(err: NavigationError) -> Self {
        match err {
            NavigationError::NotFound | NavigationError::Invalid(_) => Self::NotFound,
            NavigationError::Unavailable => Self::Unavailable,
            NavigationError::Internal => Self::Internal,
        }
    }

    /// The journal error code, the answer's status code and its message (which names the
    /// error code and never the request's values).
    fn answer(self) -> (&'static str, ErrorCode, &'static str) {
        match self {
            Self::Required => (
                DESTINATION_REQUIRED,
                ErrorCode::InvalidRequest,
                "Choose the collection to import into (destination_required); nothing is \
                 imported to a library's top level.",
            ),
            Self::Folder => (
                "invalid_path",
                ErrorCode::InvalidRequest,
                "The destination folder is not a folder path inside the collection \
                 (invalid_path).",
            ),
            Self::NotFound => (
                COLLECTION_NOT_FOUND,
                ErrorCode::NotFound,
                "That collection is not in this library (collection_not_found).",
            ),
            Self::Unavailable => (
                "unavailable",
                ErrorCode::Unavailable,
                "This host cannot import now.",
            ),
            Self::Internal => (
                "internal",
                ErrorCode::Internal,
                "This host could not check the destination.",
            ),
        }
    }
}

/// Check the request's destination: a collection is named, the folder (if any) follows the
/// collection-path rule outside the reserved folders, and the collection belongs to the
/// request's library and is readable to `principal`. Reads the library catalog on a
/// blocking thread.
async fn checked_destination(
    state: &ApiState,
    principal: &Principal,
    request: &ImportRequest,
) -> Result<(), DestinationRefusal> {
    let collection = request
        .collection_id
        .clone()
        .ok_or(DestinationRefusal::Required)?;
    if let Some(folder) = &request.folder {
        check_folder(folder).map_err(|_| DestinationRefusal::Folder)?;
    }
    let navigation = state.navigation().ok_or(DestinationRefusal::Unavailable)?;
    let (principal, library) = (principal.clone(), request.library_id.clone());
    let found = tokio::task::spawn_blocking(move || {
        collection_in(&navigation, &principal, &library, &collection)
    })
    .await;
    found.unwrap_or(Err(DestinationRefusal::Unavailable))
}

/// Whether `collection` is a collection of `library` that `principal` may read. Reads the
/// library catalog (may block).
fn collection_in(
    navigation: &crate::navigation::NavigationService,
    principal: &Principal,
    library: &str,
    collection: &str,
) -> Result<(), DestinationRefusal> {
    let id = CollectionId::new(collection).map_err(|_| DestinationRefusal::NotFound)?;
    let found = navigation
        .catalog()
        .collection(&id)
        .map_err(|err| DestinationRefusal::of_catalog(&err))?
        .ok_or(DestinationRefusal::NotFound)?;
    if found.library_id().as_str() != library {
        return Err(DestinationRefusal::NotFound);
    }
    navigation
        .check_readable(principal, &NodeRef::Collection(id))
        .map_err(DestinationRefusal::of_navigation)
}

/// Journal a refused destination once and answer its typed error.
async fn refuse_destination(
    state: &ApiState,
    actor: &str,
    request: &ImportRequest,
    refusal: DestinationRefusal,
) -> Response {
    let (error_code, code, message) = refusal.answer();
    let level = match refusal {
        DestinationRefusal::Unavailable | DestinationRefusal::Internal => Level::Error,
        _ => Level::Warn,
    };
    let entry = JournalEntry::new(IMPORT_START_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .operation_id(request.envelope.operation_id().as_str())
        .error_code(error_code);
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
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
