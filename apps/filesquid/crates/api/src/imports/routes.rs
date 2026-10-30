//! The handlers of the import routes (API-S-036): each runs only after the router authorised
//! `files.write` (API-S-004), asks the host's [`ImportStarter`] off the async executor, and
//! journals its request once with IDs and codes only.
//!
//! - `POST /api/v1/imports`: the request claims its operation ID (a second request of the
//!   same ID while the first runs is `dialog_open`); a repeated operation ID is answered first
//!   with what it already produced; then the destination is checked before any dialog; then the host plans (or,
//!   without a review, starts). Journaled as `import.plan` (`import.start` when the host
//!   started at once).
//! - `GET /api/v1/imports/{plan_id}/items`: the planned files, paged; a refusal is journaled
//!   as `import.plan_items`.
//! - `POST /api/v1/imports/{plan_id}/start`: the job, once; journaled as `import.start`.
//! - `DELETE /api/v1/imports/{plan_id}`: the plan discarded; journaled as `import.discard`.

use super::{
    ImportAnswer, ImportOutcome, ImportRequest, ImportStartError, ImportStarter, PlanDiscarded,
    PlanItemPage, IMPORT_DISCARD_OPERATION, IMPORT_PLAN_ITEMS_OPERATION, IMPORT_PLAN_OPERATION,
    IMPORT_START_OPERATION, MAX_PLAN_ITEMS,
};
use crate::auth::{journal_entry, Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::home::destination::{self, Destination, DestinationRefusal};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Extension, Json};
use serde::Deserialize;
use std::sync::Arc;

/// The import routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/imports", post(plan), Action::WriteFiles),
        ("/imports/{plan_id}/items", get(items), Action::WriteFiles),
        ("/imports/{plan_id}/start", post(start), Action::WriteFiles),
        ("/imports/{plan_id}", delete(discard), Action::WriteFiles),
    ]
}

/// `POST /api/v1/imports`: plan the import (or start it at once on a host without the
/// review).
async fn plan(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<ImportRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let request = match request.ok().map(|Json(r)| r).filter(scoped) {
        Some(request) => request,
        None => return refuse_invalid(&state, &actor).await,
    };
    let flights = state.import_flights();
    let Some(_claim) = flights.claim(request.envelope.operation_id().as_str()) else {
        return refuse(&state, &actor, Some(&request), ImportStartError::Busy).await;
    };
    let starter = state.import_starter();
    match replayed(starter.clone(), request.clone()).await {
        Ok(Some(answer)) => return answer_planned(&state, &actor, answer).await,
        Ok(None) => {}
        Err(err) => return refuse(&state, &actor, Some(&request), err).await,
    }
    let destination = Destination {
        collection_id: request.collection_id.clone(),
        folder: request.folder.clone(),
    };
    let checked = destination::check(&state, &principal, &request.library_id, &destination).await;
    if let Err(refusal) = checked {
        return refuse_destination(&state, &actor, &request, refusal).await;
    }
    let planned = match starter {
        Some(starter) => run_plan(starter, actor.clone(), request.clone()).await,
        None => Err(ImportStartError::Unavailable),
    };
    match planned {
        Ok(answer) => answer_planned(&state, &actor, answer).await,
        Err(err) => refuse(&state, &actor, Some(&request), err).await,
    }
}

/// The query of `GET /api/v1/imports/{plan_id}/items`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemsQuery {
    /// The cursor of the page to answer, from the previous page.
    cursor: Option<String>,
    /// At most this many files (1 to 1,000, default 1,000).
    limit: Option<usize>,
}

/// `GET /api/v1/imports/{plan_id}/items`: the planned files, paged.
async fn items(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(plan_id): Path<String>,
    query: Result<Query<ItemsQuery>, QueryRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let Ok(Query(query)) = query else {
        return refuse_items_query(&state, &actor).await;
    };
    let limit = query.limit.unwrap_or(MAX_PLAN_ITEMS);
    if !(1..=MAX_PLAN_ITEMS).contains(&limit) {
        return refuse_items_query(&state, &actor).await;
    }
    let Some(starter) = state.import_starter() else {
        return refuse_items(&state, &actor, ImportStartError::Unavailable).await;
    };
    let paged = tokio::task::spawn_blocking(move || {
        starter.plan_items(&plan_id, query.cursor.as_deref(), limit)
    })
    .await
    .unwrap_or(Err(ImportStartError::Unavailable));
    match paged {
        Ok(page) => Json::<PlanItemPage>(super::plan::named_plan_page(page)).into_response(),
        Err(err) => refuse_items(&state, &actor, err).await,
    }
}

/// `POST /api/v1/imports/{plan_id}/start`: the job of the plan, once.
async fn start(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(plan_id): Path<String>,
) -> Response {
    let actor = principal.actor().to_string();
    let Some(starter) = state.import_starter() else {
        return refuse(&state, &actor, None, ImportStartError::Unavailable).await;
    };
    let started = {
        let actor = actor.clone();
        tokio::task::spawn_blocking(move || starter.start_plan(&actor, &plan_id))
            .await
            .unwrap_or(Err(ImportStartError::Unavailable))
    };
    match started {
        Ok(answer) => answer_started(&state, &actor, answer).await,
        Err(err) => refuse(&state, &actor, None, err).await,
    }
}

/// `DELETE /api/v1/imports/{plan_id}`: discard the plan.
async fn discard(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(plan_id): Path<String>,
) -> Response {
    let actor = principal.actor().to_string();
    let Some(starter) = state.import_starter() else {
        return refuse_discard(&state, &actor, ImportStartError::Unavailable).await;
    };
    let discarded = {
        let (actor, plan_id) = (actor.clone(), plan_id.clone());
        tokio::task::spawn_blocking(move || starter.discard(&actor, &plan_id))
            .await
            .unwrap_or(Err(ImportStartError::Unavailable))
    };
    match discarded {
        Ok(()) => {
            let entry = JournalEntry::new(IMPORT_DISCARD_OPERATION, Outcome::Succeeded)
                .actor(&actor)
                .target(&plan_id);
            journal_entry(&state, entry).await;
            Json(PlanDiscarded {
                plan_id,
                discarded: true,
            })
            .into_response()
        }
        Err(err) => refuse_discard(&state, &actor, err).await,
    }
}

/// Whether the request is well formed: the protocol's schema version, a library ID, and an
/// envelope scope naming that library. Pure.
fn scoped(request: &ImportRequest) -> bool {
    request.envelope.schema_version() == filesquid_core::PROTOCOL_SCHEMA_VERSION
        && !request.library_id.is_empty()
        && request.envelope.scope().as_str() == format!("library:{}", request.library_id)
}

/// The answer of what the request's operation already produced, asked of the host's
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

/// Run the host's plan (dialog and all) off the async executor; a panicked starter is
/// unavailable.
async fn run_plan(
    starter: Arc<dyn ImportStarter>,
    actor: String,
    request: ImportRequest,
) -> Result<ImportAnswer, ImportStartError> {
    let joined = tokio::task::spawn_blocking(move || starter.plan(&actor, &request)).await;
    joined.unwrap_or(Err(ImportStartError::Unavailable))
}

/// Journal a plan request's answer once (`import.plan` with the plan ID, `cancelled`, or
/// `import.start` with the job ID when the host started at once) and send it.
async fn answer_planned(state: &ApiState, actor: &str, answer: ImportAnswer) -> Response {
    let entry = match (answer.import, &answer.plan_id, &answer.job_id) {
        (ImportOutcome::Planned, Some(plan), _) => {
            JournalEntry::new(IMPORT_PLAN_OPERATION, Outcome::Succeeded).target(plan)
        }
        (ImportOutcome::Started, _, Some(job)) => {
            JournalEntry::new(IMPORT_START_OPERATION, Outcome::Succeeded).target(job)
        }
        _ => JournalEntry::new(IMPORT_PLAN_OPERATION, Outcome::Cancelled),
    };
    let entry = entry
        .actor(actor)
        .operation_id(answer.outcome.operation_id().as_str());
    journal_entry(state, entry).await;
    Json(answer).into_response()
}

/// Journal a start's answer once (`import.start` with the job ID) and send it.
async fn answer_started(state: &ApiState, actor: &str, answer: ImportAnswer) -> Response {
    let entry = JournalEntry::new(IMPORT_START_OPERATION, Outcome::Succeeded)
        .actor(actor)
        .operation_id(answer.outcome.operation_id().as_str());
    let entry = match &answer.job_id {
        Some(job) => entry.target(job),
        None => entry,
    };
    journal_entry(state, entry).await;
    Json(answer).into_response()
}

/// Journal an unreadable or mis-scoped body once and answer `400 invalid_request`.
async fn refuse_invalid(state: &ApiState, actor: &str) -> Response {
    let entry = JournalEntry::new(IMPORT_PLAN_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .error_code("invalid_request");
    journal_entry(state, entry).await;
    let message = "The request names unknown, missing or malformed fields, its envelope is not \
                   schema version 1, or its scope is not library:<library_id>.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Journal an unreadable items query once and answer `400 invalid_request`.
async fn refuse_items_query(state: &ApiState, actor: &str) -> Response {
    let entry = JournalEntry::new(IMPORT_PLAN_ITEMS_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .error_code("invalid_request");
    journal_entry(state, entry).await;
    let message = "The items query names an unknown field or a limit outside 1 to 1000.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Journal a refused items read once and answer its typed error.
async fn refuse_items(state: &ApiState, actor: &str, err: ImportStartError) -> Response {
    let (level, code, message) = typed(&err);
    let entry = JournalEntry::new(IMPORT_PLAN_ITEMS_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}

/// Journal a refused discard once and answer its typed error.
async fn refuse_discard(state: &ApiState, actor: &str, err: ImportStartError) -> Response {
    let (level, code, message) = typed(&err);
    let entry = JournalEntry::new(IMPORT_DISCARD_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}

/// Journal a refused destination once (`import.plan`) and answer its typed error.
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
    let entry = JournalEntry::new(IMPORT_PLAN_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .operation_id(request.envelope.operation_id().as_str())
        .error_code(error_code);
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}

/// The level, status code and message of a starter refusal; the message names the error
/// code and never a value of the request. Pure.
fn typed(err: &ImportStartError) -> (Level, ErrorCode, &'static str) {
    match err {
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
        ImportStartError::OperationReused => (
            Level::Warn,
            ErrorCode::Conflict,
            "That operation ID already planned or started another import (operation_reused).",
        ),
        ImportStartError::PlanNotFound => (
            Level::Warn,
            ErrorCode::NotFound,
            "No open import plan has this ID: it is unknown, expired, discarded or already \
             started (plan_not_found).",
        ),
        ImportStartError::SourceChanged => (
            Level::Warn,
            ErrorCode::Conflict,
            "The picked files changed since the plan was made (source_changed); review them \
             again.",
        ),
    }
}

/// Journal a plan or start refusal once (`import.plan` for a plan request, `import.start`
/// for a start) and answer its typed error.
async fn refuse(
    state: &ApiState,
    actor: &str,
    request: Option<&ImportRequest>,
    err: ImportStartError,
) -> Response {
    let (level, code, message) = typed(&err);
    let operation = match request {
        Some(_) => IMPORT_PLAN_OPERATION,
        None => IMPORT_START_OPERATION,
    };
    let entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    let entry = match request {
        Some(request) => entry.operation_id(request.envelope.operation_id().as_str()),
        None => entry,
    };
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}
