//! The job routes (Task API-F1.6; API-S-037): one handler per route, each calling the
//! installed [`Jobs`] service off the async runtime and answering JSON or the typed error.
//! Every handler runs only after the router's guard has authorised its action (API-S-004):
//!
//! - `GET /api/v1/jobs` (`jobs.read`): at most 100 jobs, newest first, by `kind` and `state`.
//! - `GET /api/v1/jobs/{job}` (`jobs.read`): one job.
//! - `POST /api/v1/jobs/{job}/pause`, `/resume`, `/cancel` (`jobs.manage`): one control each;
//!   a resumed pending job is handed to the host's [`super::JobRunner`] for its kind.
//! - `GET /api/v1/jobs/{job}/items` (`jobs.read`): an upload job's files, paged (API-S-037
//!   **Per-file progress**).
//! - `POST /api/v1/jobs/{job}/items/{item}/retry` and `POST /api/v1/jobs/{job}/retry-failed`
//!   (`jobs.manage`): failed files back to queued, and a pending job handed to its runner.
//!
//! The service journals each control and each refusal it makes; a handler journals only what
//! it refuses itself (a query it cannot read).

use super::items::MAX_LISTED_ITEMS;
use super::model::{Job, JobError, JobState};
use super::store::JobFilter;
use super::view::{JobControlAnswer, JobPage};
use super::{Jobs, JOB_REFUSED, MAX_LISTED_JOBS};
use crate::auth::Action;
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

/// The job routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/jobs", get(list), Action::ReadJobs),
        ("/jobs/{job}", get(one), Action::ReadJobs),
        ("/jobs/{job}/pause", post(pause), Action::ManageJobs),
        ("/jobs/{job}/resume", post(resume), Action::ManageJobs),
        ("/jobs/{job}/cancel", post(cancel), Action::ManageJobs),
        ("/jobs/{job}/items", get(items), Action::ReadJobs),
        (
            "/jobs/{job}/items/{item}/retry",
            post(retry_item),
            Action::ManageJobs,
        ),
        (
            "/jobs/{job}/retry-failed",
            post(retry_failed),
            Action::ManageJobs,
        ),
    ]
}

/// The query of `GET /api/v1/jobs`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobsQuery {
    /// Only jobs of this kind.
    kind: Option<String>,
    /// Only jobs in this state (`pending`, `running`, `completed`, `failed`, `cancelled`).
    state: Option<String>,
    /// At most this many jobs (1 to 1000, default 100; a limit above 100 answers 100).
    limit: Option<usize>,
}

/// `GET /api/v1/jobs`: the jobs the query accepts.
async fn list(
    State(state): State<ApiState>,
    query: Result<Query<JobsQuery>, QueryRejection>,
) -> Response {
    let jobs = match installed(&state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    let filter = match query.map_err(|_| ()).and_then(|Query(q)| filter_of(q)) {
        Ok(filter) => filter,
        Err(()) => return invalid(&jobs, FILTER_MESSAGE).into_response(),
    };
    let listed = blocking(move || {
        let list = jobs.list(&filter)?;
        Ok(JobPage {
            jobs: list.jobs.iter().map(|job| jobs.summary(job)).collect(),
            truncated: list.truncated,
        })
    })
    .await;
    match listed {
        Ok(page) => Json(page).into_response(),
        Err(err) => api_error(&err).into_response(),
    }
}

/// The query of `GET /api/v1/jobs/{job}/items`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemsQuery {
    /// The cursor of the page to answer, from the previous page.
    cursor: Option<String>,
    /// At most this many files (1 to 1000, default 1000).
    limit: Option<usize>,
}

/// `GET /api/v1/jobs/{job}/items`: an upload job's files, paged.
async fn items(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    query: Result<Query<ItemsQuery>, QueryRejection>,
) -> Response {
    let jobs = match installed(&state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    let Ok(Query(query)) = query else {
        return invalid(&jobs, ITEMS_MESSAGE).into_response();
    };
    let limit = query.limit.unwrap_or(MAX_LISTED_ITEMS);
    if !(1..=MAX_LISTED_ITEMS).contains(&limit) {
        return invalid(&jobs, ITEMS_MESSAGE).into_response();
    }
    let paged = blocking(move || jobs.item_page(&id, query.cursor.as_deref(), limit)).await;
    match paged {
        Ok(page) => Json(page).into_response(),
        Err(err) => api_error(&err).into_response(),
    }
}

/// `POST /api/v1/jobs/{job}/items/{item}/retry`: one failed file back to queued.
async fn retry_item(
    State(state): State<ApiState>,
    Path((id, item)): Path<(String, String)>,
) -> Response {
    retried(&state, id, Some(item)).await
}

/// `POST /api/v1/jobs/{job}/retry-failed`: every failed file back to queued.
async fn retry_failed(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    retried(&state, id, None).await
}

/// Retry `item` (every failed file when `None`) of job `id`, then hand a pending job whose
/// files were put back to its runner; answer the job view. A retry that put nothing back
/// hands nothing to the runner.
async fn retried(state: &ApiState, id: String, item: Option<String>) -> Response {
    let jobs = match installed(state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    let retrying = jobs.clone();
    let (job, put_back) = match blocking(move || retrying.retry(&id, item.as_deref())).await {
        Ok(retried) => retried,
        Err(err) => return api_error(&err).into_response(),
    };
    let scheduled = !put_back.is_empty() && schedule(state, &job);
    answered(jobs, job, scheduled).await
}

/// `GET /api/v1/jobs/{job}`: one job.
async fn one(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let jobs = match installed(&state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    match blocking(move || jobs.job(&id).map(|job| jobs.view(&job))).await {
        Ok(view) => Json(view).into_response(),
        Err(err) => api_error(&err).into_response(),
    }
}

/// `POST /api/v1/jobs/{job}/pause`.
async fn pause(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    controlled(&state, move |jobs| jobs.pause(&id)).await
}

/// `POST /api/v1/jobs/{job}/cancel`.
async fn cancel(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    controlled(&state, move |jobs| jobs.cancel(&id)).await
}

/// `POST /api/v1/jobs/{job}/resume`: resume the job, then hand a pending one to its runner.
async fn resume(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let jobs = match installed(&state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    let resuming = jobs.clone();
    let job = match blocking(move || resuming.resume(&id)).await {
        Ok(job) => job,
        Err(err) => return api_error(&err).into_response(),
    };
    let scheduled = schedule(&state, &job);
    answered(jobs, job, scheduled).await
}

/// Hand the pending `job` to the runner of its kind; false when it has none or it refused.
pub(crate) fn schedule(state: &ApiState, job: &Job) -> bool {
    if job.state != JobState::Pending {
        return false;
    }
    let Some(runner) = state.job_runner(&job.kind) else {
        return false;
    };
    match runner.resume(job) {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(error = %err, "a job runner did not take a resumed job");
            false
        }
    }
}

/// Run the control `act` on the installed service and answer the job.
async fn controlled(
    state: &ApiState,
    act: impl FnOnce(&Jobs) -> Result<Job, JobError> + Send + 'static,
) -> Response {
    let jobs = match installed(state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    let acting = jobs.clone();
    match blocking(move || act(&acting)).await {
        Ok(job) => answered(jobs, job, false).await,
        Err(err) => api_error(&err).into_response(),
    }
}

/// Answer the control's `job` with its view, whose destination is read from the host's
/// per-file progress port on a blocking thread, and whether it was handed to its runner.
async fn answered(jobs: Arc<Jobs>, job: Job, scheduled: bool) -> Response {
    let view = blocking(move || Ok(jobs.view(&job))).await;
    match view {
        Ok(job) => Json(JobControlAnswer { job, scheduled }).into_response(),
        Err(err) => api_error(&err).into_response(),
    }
}

/// The installed job service, or the retryable refusal of a host that has none.
fn installed(state: &ApiState) -> Result<Arc<Jobs>, ApiError> {
    state.jobs().ok_or_else(|| {
        let message = "This host runs no durable jobs yet; try again shortly.";
        ApiError::new(ErrorCode::Unavailable, message)
    })
}

/// Longest `kind` a filter may name, as the contract's `kind` parameter.
const MAX_KIND_CHARS: usize = 128;

/// Largest `limit` the contract's `PageSize` accepts; a limit above [`MAX_LISTED_JOBS`]
/// answers that many.
const MAX_PAGE_SIZE: usize = 1000;

/// The filter of `query`, or `Err` for an unknown state, a `kind` outside 1 to 128
/// characters or a `limit` outside 1 to 1000. Pure.
fn filter_of(query: JobsQuery) -> Result<JobFilter, ()> {
    let state = match query.state.as_deref() {
        Some(name) => Some(JobState::parse(name).ok_or(())?),
        None => None,
    };
    let kind_ok = query
        .kind
        .as_deref()
        .is_none_or(|kind| (1..=MAX_KIND_CHARS).contains(&kind.chars().count()));
    let limit = query.limit.unwrap_or(MAX_LISTED_JOBS);
    match kind_ok && (1..=MAX_PAGE_SIZE).contains(&limit) {
        true => Ok(JobFilter {
            kind: query.kind,
            state,
            limit,
        }),
        false => Err(()),
    }
}

/// The refusal message of a malformed job filter.
const FILTER_MESSAGE: &str = "The job filter names an unknown field, state or limit.";

/// The refusal message of a malformed items query.
const ITEMS_MESSAGE: &str = "The items query names an unknown field or a limit outside 1 to 1000.";

/// Journal a query a route cannot read, once, and answer its refusal with `message`.
fn invalid(jobs: &Jobs, message: &'static str) -> ApiError {
    let entry = JournalEntry::new(JOB_REFUSED, Outcome::Failed)
        .level(Level::Warn)
        .error_code("invalid_request");
    jobs.journal_entry(entry);
    ApiError::new(ErrorCode::InvalidRequest, message)
}

/// The typed error of a job refusal or failure. Pure.
fn api_error(err: &JobError) -> ApiError {
    let (code, message) = match err {
        JobError::NotFound => (ErrorCode::NotFound, "No job or item has this ID."),
        JobError::Conflict(rule) => (ErrorCode::Conflict, conflict_message(rule)),
        JobError::Unavailable(_) => (
            ErrorCode::Unavailable,
            "The job store is not available right now; try again.",
        ),
        JobError::Fenced | JobError::TooLarge | JobError::SourceChanged => (
            ErrorCode::Conflict,
            "The job changed while this request was answered; read it again.",
        ),
        JobError::Failed(_) => (
            ErrorCode::Internal,
            "FileSquid could not complete the request.",
        ),
    };
    ApiError::new(code, message)
}

/// The message of the conflict `rule`. Pure.
fn conflict_message(rule: &str) -> &'static str {
    match rule {
        "job_finished" => "The job has finished; it can no longer be paused, resumed or cancelled.",
        "cancel_requested" => "The job is being cancelled; it can no longer be paused or resumed.",
        "job_running" => "The job is running and was not paused; there is nothing to resume.",
        "job_paused" => "The job is paused; resume it before retrying its files.",
        _ => "The job cannot take this control now.",
    }
}

/// Run the store call `work` on the blocking pool, so a host's SQLite store never blocks the
/// async runtime.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, JobError> + Send + 'static,
) -> Result<T, JobError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| JobError::Failed(err.to_string()))?
}
