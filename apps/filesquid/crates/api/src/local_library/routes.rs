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
//! **Command envelopes (Task API-F2.19; API-S-002, API-S-019).** The five mutation routes
//! (create, open, copy, rebuild and rename) take `{"envelope": CommandEnvelope, "input": ...}`
//! ([`crate::envelope`]) and answer a `CommandOutcome` for the envelope's operation ID with the
//! route's former answer as `result`: `committed` for create, open, copy and rename, `pending`
//! with the job for a rebuild. The scope is `location:<location_id>` for create and open and
//! `library:<library_id>` for copy, rebuild and rename. The expected revision is compared by
//! the operations under their binding lock (create `0`; open `0`, or the current revision of a
//! library already open there, and only the current one when the open renames it; copy,
//! rebuild and rename the current revision); a stale one, like every other 409 refusal, is
//! HTTP 409 with a `conflict` outcome beside the `ApiError` fields. A repeated operation ID of
//! a command that changed something (a create, a fresh open, an open that renamed, a copy, a
//! rebuild that submitted its job, a rename that changed the name) answers its first answer
//! again with no second change; a command that changed nothing (an open of an open library, a
//! rebuild that answered an unfinished job, a rename to the current name, a refusal) stores
//! no receipt. The repair route keeps its consent body and the location-choices route its
//! purpose: neither is an envelope route.
//!
//! The operations journal their own outcomes and refusals. A handler journals only what it
//! refuses itself, once: a body or query it cannot read, an envelope it refuses, an operation
//! ID that names another command or is still running, a host that has not installed its
//! library commands yet, and (on the facts route) a library ID that is malformed or names a
//! library the caller may not read, journaled as `library.facts` `library_not_found`.

use super::operations::LibraryOperations;
use super::rebuild_job::{submit_rebuild, RebuildJobAnswer, RebuildOutcome, SubmitRefusal};
use super::repair::REPAIR_OPERATION;
use super::service::{
    CANDIDATES_OPERATION, COPY_OPERATION, CREATE_OPERATION, FACTS_OPERATION, OPEN_OPERATION,
    REBUILD_OPERATION, RECENT_OPERATION, RENAME_OPERATION,
};
use super::{
    LibraryRequest, LocalLibraryError, LocationRequest, RenameLibraryRequest, SnapshotCopyRequest,
};
use crate::auth::{journal_entry, Action, Principal};
use crate::envelope::{self, Checked, Command, Settled};
use crate::error::{ApiError, ErrorCode};
use crate::files::receipts::ClaimGuard;
use crate::jobs::routes as job_routes;
use crate::jobs::{JobError, JobView};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::operation::OperationId;
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json};
use filesquid_core::LibraryId;
use serde::Serialize;
use std::future::Future;
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

/// What a command that ran answers: the route's result, whether it is complete, and whether
/// it changed anything (so its answer is stored as a receipt).
struct Done<V> {
    /// What the route answers as `result`.
    result: V,
    /// `committed`, or `pending` for a durable job.
    pending: bool,
    /// True when the command changed something.
    changed: bool,
}

/// The target a library command's receipt is keyed by, and the scope its envelope must name.
trait CommandInput: Serialize {
    /// `location:<id>` for a command that binds a folder, `library:<id>` otherwise.
    fn scope(&self) -> String;
}

impl CommandInput for LocationRequest {
    fn scope(&self) -> String {
        format!("location:{}", self.location_id)
    }
}

impl CommandInput for SnapshotCopyRequest {
    fn scope(&self) -> String {
        format!("library:{}", self.library_id.as_str())
    }
}

impl CommandInput for LibraryRequest {
    fn scope(&self) -> String {
        format!("library:{}", self.library_id.as_str())
    }
}

impl CommandInput for RenameLibraryRequest {
    fn scope(&self) -> String {
        format!("library:{}", self.library_id.as_str())
    }
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

/// The checked command of a JSON mutation route, or the refusal to journal here.
fn checked<T: CommandInput>(
    request: Result<Json<Command<T>>, JsonRejection>,
) -> Result<Checked<T>, LocalLibraryError> {
    let command = body(request).ok_or_else(unreadable)?;
    let scope = command.input.scope();
    envelope::check(command.envelope, command.input, &scope)
        .map_err(|message| LocalLibraryError::invalid("invalid_request", message))
}

/// Run the command of the route journaled as `operation` once: answer its receipt when the
/// same command already ran, refuse an operation ID of another command or one in flight,
/// else `execute` it and store the answer of a change.
async fn run<T, V, F, Fut>(
    state: &ApiState,
    principal: &Principal,
    operation: &'static str,
    command: Result<Checked<T>, LocalLibraryError>,
    execute: F,
) -> Response
where
    T: CommandInput,
    V: Serialize,
    F: FnOnce(Arc<dyn LibraryOperations>, Checked<T>, String) -> Fut,
    Fut: Future<Output = Result<Done<V>, LocalLibraryError>>,
{
    let actor = principal.actor().to_string();
    let read_id = command.as_ref().ok().map(|c| c.operation_id.clone());
    let (operations, command) = match command.and_then(|c| ready(state, Some(c))) {
        Ok(ready) => ready,
        Err(err) => return refuse(state, operation, &actor, read_id.as_ref(), err).await,
    };
    let operation_id = command.operation_id.clone();
    let target = command.input.scope();
    let receipts = state.command_receipts();
    let claimed = envelope::claim(receipts, operation, &target, &command.input, &operation_id);
    let guard = match claimed {
        Ok(guard) => guard,
        Err(settled) => return settle(state, operation, &actor, &operation_id, settled).await,
    };
    match execute(operations, command, actor).await {
        Ok(done) => answer_done(operation, &operation_id, done, guard),
        Err(err) => envelope::refusal(Some(&operation_id), ApiError::from(err)),
    }
}

/// Answer a claim that did not run the command: a replay changes nothing and is not
/// journaled; a refusal is journaled once.
async fn settle(
    state: &ApiState,
    operation: &'static str,
    actor: &str,
    operation_id: &OperationId,
    settled: Settled,
) -> Response {
    let Some((code, message)) = settled.refusal() else {
        let Settled::Replay(answer) = settled else {
            unreachable!("only a replay refuses nothing");
        };
        return Json(answer).into_response();
    };
    let err = match code {
        ErrorCode::InvalidRequest => LocalLibraryError::invalid("invalid_request", message),
        ErrorCode::Conflict => LocalLibraryError::refused("conflict", message),
        _ => LocalLibraryError::Internal,
    };
    refuse(state, operation, actor, Some(operation_id), err).await
}

/// Store the answer of a change as its command's receipt, then answer it.
fn answer_done<V: Serialize>(
    operation: &str,
    operation_id: &OperationId,
    done: Done<V>,
    guard: ClaimGuard,
) -> Response {
    let encoded = match done.pending {
        true => envelope::pending(operation_id, done.result),
        false => envelope::committed(operation_id, done.result),
    };
    let Some(answer) = encoded else {
        return ApiError::internal(operation, "an answer did not encode").into_response();
    };
    if done.changed {
        guard.store(answer.clone());
    }
    Json(answer).into_response()
}

/// `POST /api/v1/libraries`: create a library, answered `committed` with the library.
async fn create(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<LocationRequest>>, JsonRejection>,
) -> Response {
    let command = checked(request);
    run(
        &state,
        &principal,
        CREATE_OPERATION,
        command,
        |ops, c, actor| async move {
            let view = ops.create(actor, c.input, c.expected).await?;
            Ok(Done {
                result: view,
                pending: false,
                changed: true,
            })
        },
    )
    .await
}

/// `POST /api/v1/libraries/openings`: open a library, answered `committed` with the
/// library; a library already open there is answered again and stores no receipt.
async fn open(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<LocationRequest>>, JsonRejection>,
) -> Response {
    let command = checked(request);
    run(
        &state,
        &principal,
        OPEN_OPERATION,
        command,
        |ops, c, actor| async move {
            let opened = ops.open(actor, c.input, c.expected).await?;
            Ok(Done {
                result: opened.view,
                pending: false,
                changed: opened.changed,
            })
        },
    )
    .await
}

/// `POST /api/v1/libraries/snapshot-copies`: copy a completed snapshot, answered
/// `committed` with the copy report.
async fn copy_snapshot(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<SnapshotCopyRequest>>, JsonRejection>,
) -> Response {
    let command = checked(request);
    run(
        &state,
        &principal,
        COPY_OPERATION,
        command,
        |ops, c, actor| async move {
            let report = ops.copy_snapshot(actor, c.input, c.expected).await?;
            Ok(Done {
                result: report,
                pending: false,
                changed: true,
            })
        },
    )
    .await
}

/// `POST /api/v1/libraries/rebuilds`: refuse a library that is not open or read at another
/// revision, then answer the library's unfinished rebuild job or a new one under the
/// envelope's operation ID, handed to the host's runner, as the `pending` outcome (API-S-025
/// **Rebuild as a job**). Only a new job stores a receipt.
async fn rebuild(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<LibraryRequest>>, JsonRejection>,
) -> Response {
    let command = checked(request);
    if let (Ok(c), None) = (&command, state.jobs()) {
        let actor = principal.actor();
        return refuse(
            &state,
            REBUILD_OPERATION,
            actor,
            Some(&c.operation_id),
            no_jobs(),
        )
        .await;
    }
    let jobs_state = state.clone();
    run(
        &state,
        &principal,
        REBUILD_OPERATION,
        command,
        |ops, c, actor| async move {
            ops.rebuild_ready(actor.clone(), c.input.clone(), c.expected)
                .await?;
            let answer = submit(&jobs_state, &actor, &c.input, c.operation_id.as_str()).await?;
            let changed = answer.job.operation_id == c.operation_id.as_str();
            Ok(Done {
                result: answer,
                pending: true,
                changed,
            })
        },
    )
    .await
}

/// Submit the rebuild job of `request.library_id` under `operation_id`, or answer its
/// unfinished job, and hand a new job to the runner. A submission the job store refused
/// without journaling is journaled here once.
async fn submit(
    state: &ApiState,
    actor: &str,
    request: &LibraryRequest,
    operation_id: &str,
) -> Result<RebuildJobAnswer, LocalLibraryError> {
    let jobs = state.jobs().ok_or_else(no_jobs)?;
    let library = request.library_id.clone();
    let id = operation_id.to_string();
    let submitted = job_routes::blocking(move || Ok(submit_rebuild(&jobs, &library, id)))
        .await
        .unwrap_or_else(|error| {
            let journaled = false;
            Err(SubmitRefusal { error, journaled })
        });
    let job = match submitted {
        Ok(job) => job,
        Err(refusal) => {
            let err = job_failure(&refusal.error);
            if !refusal.journaled {
                journal_refusal(state, REBUILD_OPERATION, actor, Some(operation_id), &err).await;
            }
            return Err(err);
        }
    };
    let scheduled = !job.is_paused() && job_routes::schedule(state, &job);
    Ok(RebuildJobAnswer {
        outcome: RebuildOutcome::Pending,
        job_id: job.id.clone(),
        job: JobView::of(&job),
        scheduled,
    })
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
        Err(err) => return refuse(&state, CANDIDATES_OPERATION, &actor, None, err).await,
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
        Err(err) => {
            return refuse(&state, REPAIR_OPERATION, principal.actor(), None, err).await;
        }
    };
    answer(operations.repair(principal, request).await)
}

/// `POST /api/v1/libraries/renames`: rename an open library, answered `committed` with its
/// facts; a rename that changes nothing stores no receipt.
async fn rename(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<RenameLibraryRequest>>, JsonRejection>,
) -> Response {
    let command = checked(request);
    run(
        &state,
        &principal,
        RENAME_OPERATION,
        command,
        |ops, c, actor| async move {
            let (facts, changed) = ops.rename(actor, c.input, c.expected).await?;
            Ok(Done {
                result: facts,
                pending: false,
                changed,
            })
        },
    )
    .await
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
        Err(err) => return refuse(&state, FACTS_OPERATION, &actor, None, err).await,
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
        Err(err) => return refuse(&state, RECENT_OPERATION, &actor, None, err).await,
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

/// Journal a refusal of `operation` made by a handler, once, with its operation ID when the
/// envelope was read.
async fn journal_refusal(
    state: &ApiState,
    operation: &'static str,
    actor: &str,
    operation_id: Option<&str>,
    err: &LocalLibraryError,
) {
    let level = match err {
        LocalLibraryError::Unavailable | LocalLibraryError::Internal => Level::Error,
        _ => Level::Warn,
    };
    let mut entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    if let Some(id) = operation_id {
        entry = entry.operation_id(id);
    }
    journal_entry(state, entry).await;
}

/// Journal a refusal of `operation` made by a handler, once, and answer the typed error: a
/// `conflict` of a command whose envelope was read as HTTP 409 with a `conflict` outcome.
async fn refuse(
    state: &ApiState,
    operation: &'static str,
    actor: &str,
    operation_id: Option<&OperationId>,
    err: LocalLibraryError,
) -> Response {
    journal_refusal(
        state,
        operation,
        actor,
        operation_id.map(OperationId::as_str),
        &err,
    )
    .await;
    envelope::refusal(operation_id, ApiError::from(err))
}
