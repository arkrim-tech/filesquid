//! The library and repair routes (Task API-F1.12; API-S-025): one handler per route, each
//! reading its request, calling the installed [`LibraryOperations`] and answering JSON or
//! the typed error. Every handler runs only after the router's guard has authorised its
//! action and put the [`Principal`] in the request extensions (API-S-004):
//!
//! - `POST /api/v1/libraries` (`library.manage`): create a library in an empty location.
//! - `POST /api/v1/libraries/openings` (`library.manage`): open the library in a location.
//! - `POST /api/v1/libraries/snapshot-copies` (`library.manage`): copy a completed snapshot.
//! - `POST /api/v1/libraries/rebuilds` (`library.manage`): rebuild from files as a durable
//!   `library.rebuild` job (Task API-F2.17), answered `pending` with the job.
//! - `GET /api/v1/libraries/repair-candidates` (`library.read`): the W010 candidates.
//! - `POST /api/v1/libraries/repairs` (`library.repair`): the consented repair; the policy
//!   grants it to the desktop owner only.
//! - `POST /api/v1/libraries/renames` (`library.manage`): rename a library (API-S-038).
//! - `GET /api/v1/libraries/{library}/facts` (`library.read`): a library's name and free
//!   bytes, for any caller who may read it; an unreadable library is `not_found`.
//! - `GET /api/v1/libraries/recent` (`library.read`): the libraries this host opened that
//!   the caller may read, at most 20, newest first.
//!
//! Readability is the navigation access the host installed (API-S-021), which must admit the
//! host's repository and the library, else this release's host access.
//!
//! The operations journal their own outcomes and refusals. A handler journals only what it
//! refuses itself, once: a body or query it cannot read, a host that has not installed its
//! library commands yet, and (on the facts route) a library ID that is malformed or names a
//! library the caller may not read, journaled as `library.facts` `library_not_found`.

use super::operations::LibraryOperations;
use super::rebuild_job::{submit_rebuild, RebuildJobAnswer, RebuildOutcome, SubmitRefusal};
use super::repair::REPAIR_OPERATION;
use super::service::{
    CANDIDATES_OPERATION, COPY_OPERATION, CREATE_OPERATION, FACTS_OPERATION, OPEN_OPERATION,
    REBUILD_OPERATION, RECENT_OPERATION, RENAME_OPERATION,
};
use super::LocalLibraryError;
use crate::auth::{journal_entry, Action, Principal};
use crate::error::ApiError;
use crate::jobs::routes as job_routes;
use crate::jobs::{JobError, JobView};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json};
use filesquid_core::LibraryId;
use serde::Serialize;
use std::sync::Arc;

/// The library and repair routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/libraries", post(create), Action::ManageLibraries),
        ("/libraries/openings", post(open), Action::ManageLibraries),
        (
            "/libraries/snapshot-copies",
            post(copy_snapshot),
            Action::ManageLibraries,
        ),
        (
            "/libraries/rebuilds",
            post(rebuild),
            Action::ManageLibraries,
        ),
        (
            "/libraries/repair-candidates",
            get(repair_candidates),
            Action::ReadLibraries,
        ),
        ("/libraries/repairs", post(repair), Action::RepairLibraries),
        ("/libraries/renames", post(rename), Action::ManageLibraries),
        (
            "/libraries/{library}/facts",
            get(facts),
            Action::ReadLibraries,
        ),
        ("/libraries/recent", get(recent), Action::ReadLibraries),
    ]
}

/// The refusal for a host that has not installed its library commands.
fn not_installed() -> LocalLibraryError {
    tracing::info!(target: super::LOG_TARGET, "no library commands are installed");
    LocalLibraryError::Unavailable
}

/// The refusal for a body or query that cannot be read.
fn unreadable() -> LocalLibraryError {
    LocalLibraryError::invalid(
        "invalid_request",
        "The request names unknown, missing or malformed fields.",
    )
}

/// The installed operations and the request, or the refusal to journal here.
fn ready<T>(
    state: &ApiState,
    request: Option<T>,
) -> Result<(Arc<dyn LibraryOperations>, T), LocalLibraryError> {
    let operations = state.libraries().ok_or_else(not_installed)?;
    let request = request.ok_or_else(unreadable)?;
    Ok((operations, request))
}

/// The JSON body, or `None` for a missing, extra or malformed field.
fn body<T>(body: Result<Json<T>, JsonRejection>) -> Option<T> {
    body.ok().map(|Json(b)| b)
}

/// The query, or `None` for a missing, extra or malformed parameter.
fn query<T>(query: Result<Query<T>, QueryRejection>) -> Option<T> {
    query.ok().map(|Query(q)| q)
}

/// `POST /api/v1/libraries`.
async fn create(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::LocationRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let (operations, request) = match ready(&state, body(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, CREATE_OPERATION, &actor, err).await,
    };
    answer(operations.create(actor, request).await)
}

/// `POST /api/v1/libraries/openings`.
async fn open(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::LocationRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let (operations, request) = match ready(&state, body(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, OPEN_OPERATION, &actor, err).await,
    };
    answer(operations.open(actor, request).await)
}

/// `POST /api/v1/libraries/snapshot-copies`.
async fn copy_snapshot(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::SnapshotCopyRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let (operations, request) = match ready(&state, body(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, COPY_OPERATION, &actor, err).await,
    };
    answer(operations.copy_snapshot(actor, request).await)
}

/// `POST /api/v1/libraries/rebuilds`: refuse a library that is not open, then answer the
/// library's unfinished rebuild job or a new one, handed to the host's runner (API-S-025
/// **Rebuild as a job**).
async fn rebuild(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::LibraryRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let checked = ready(&state, body(request))
        .and_then(|(ops, request)| Ok((ops, request, state.jobs().ok_or_else(no_jobs)?)));
    let (operations, request, jobs) = match checked {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, REBUILD_OPERATION, &actor, err).await,
    };
    if let Err(err) = operations
        .rebuild_ready(actor.clone(), request.clone())
        .await
    {
        return ApiError::from(err).into_response();
    }
    let library = request.library_id;
    let submitted = job_routes::blocking(move || Ok(submit_rebuild(&jobs, &library)))
        .await
        .unwrap_or_else(|error| {
            let journaled = false;
            Err(SubmitRefusal { error, journaled })
        });
    let job = match submitted {
        Ok(job) => job,
        Err(refusal) if refusal.journaled => {
            return ApiError::from(job_failure(&refusal.error)).into_response();
        }
        Err(refusal) => {
            let err = job_failure(&refusal.error);
            return refuse(&state, REBUILD_OPERATION, &actor, err).await;
        }
    };
    let scheduled = !job.is_paused() && job_routes::schedule(&state, &job);
    Json(RebuildJobAnswer {
        outcome: RebuildOutcome::Pending,
        job_id: job.id.clone(),
        job: JobView::of(&job),
        scheduled,
    })
    .into_response()
}

/// The refusal for a host that runs no durable jobs yet.
fn no_jobs() -> LocalLibraryError {
    tracing::info!(target: super::LOG_TARGET, "no job service is installed");
    LocalLibraryError::Unavailable
}

/// The library error of a job store failure: retryable `unavailable`, else `internal`. Pure
/// but for the log.
fn job_failure(err: &JobError) -> LocalLibraryError {
    tracing::info!(target: super::LOG_TARGET, error = %err, "a rebuild job was not submitted");
    match err {
        JobError::Unavailable(_) => LocalLibraryError::Unavailable,
        _ => LocalLibraryError::Internal,
    }
}

/// `GET /api/v1/libraries/repair-candidates`.
async fn repair_candidates(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<super::LibraryRequest>, QueryRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let (operations, request) = match ready(&state, query(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, CANDIDATES_OPERATION, &actor, err).await,
    };
    answer(operations.repair_candidates(actor, request).await)
}

/// `POST /api/v1/libraries/repairs`.
async fn repair(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::RepairRequest>, JsonRejection>,
) -> Response {
    let (operations, request) = match ready(&state, body(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, REPAIR_OPERATION, principal.actor(), err).await,
    };
    answer(operations.repair(principal, request).await)
}

/// `POST /api/v1/libraries/renames`.
async fn rename(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<super::RenameLibraryRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let (operations, request) = match ready(&state, body(request)) {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, RENAME_OPERATION, &actor, err).await,
    };
    answer(operations.rename(actor, request).await)
}

/// `GET /api/v1/libraries/{library}/facts`: a library the caller may not read is refused
/// exactly as an unknown one.
async fn facts(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(library): Path<String>,
) -> Response {
    let actor = principal.actor().to_string();
    let library = LibraryId::new(library).ok();
    let readable = library
        .as_ref()
        .is_some_and(|id| may_read(&state, &principal, id));
    let checked = ready(&state, library.filter(|_| readable)).map_err(|err| match err {
        LocalLibraryError::Unavailable => err,
        _ => LocalLibraryError::not_found("library_not_found", "That library is not known here."),
    });
    let (operations, library) = match checked {
        Ok(ready) => ready,
        Err(err) => return refuse(&state, FACTS_OPERATION, &actor, err).await,
    };
    answer(operations.facts(actor, library).await)
}

/// `GET /api/v1/libraries/recent`.
async fn recent(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Response {
    let actor = principal.actor().to_string();
    let operations = match ready(&state, Some(())) {
        Ok((operations, ())) => operations,
        Err(err) => return refuse(&state, RECENT_OPERATION, &actor, err).await,
    };
    let reader = state.clone();
    let readable: super::Readable = Box::new(move |library| may_read(&reader, &principal, library));
    answer(operations.recent(actor, readable).await)
}

/// Whether `principal` may read `library`: by the installed navigation access, which must
/// admit both the host's repository and the library (as navigation resolves a library under
/// its repository), else by this release's host access.
fn may_read(state: &ApiState, principal: &Principal, library: &LibraryId) -> bool {
    use crate::navigation::NodeRef;
    let node = NodeRef::Library(library.clone());
    match state.navigation() {
        Some(navigation) => {
            let repository = NodeRef::Repository(navigation.repository_id().clone());
            navigation.may_read(principal, &repository) && navigation.may_read(principal, &node)
        }
        None => {
            use crate::navigation::NavigationAccess;
            crate::navigation::HostAccess.may_read(principal, &node)
        }
    }
}

/// The JSON answer, or the typed error of a refusal the operation already journaled.
fn answer<T: Serialize>(outcome: Result<T, LocalLibraryError>) -> Response {
    match outcome {
        Ok(view) => Json(view).into_response(),
        Err(err) => ApiError::from(err).into_response(),
    }
}

/// Journal a refusal of `operation` made by a handler, once, and answer the typed error.
async fn refuse(
    state: &ApiState,
    operation: &'static str,
    actor: &str,
    err: LocalLibraryError,
) -> Response {
    let level = match err {
        LocalLibraryError::Unavailable | LocalLibraryError::Internal => Level::Error,
        _ => Level::Warn,
    };
    let entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::from(err).into_response()
}
