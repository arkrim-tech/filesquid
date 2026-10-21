//! The job routes (Task API-F1.6; API-S-036): one handler per route, each calling the
//! installed [`Jobs`] service off the async runtime and answering JSON or the typed error.
//! Every handler runs only after the router's guard has authorised its action (API-S-004):
//!
//! - `GET /api/v1/jobs` (`jobs.read`): at most 100 jobs, newest first, by `kind` and `state`.
//! - `GET /api/v1/jobs/{job}` (`jobs.read`): one job.
//! - `POST /api/v1/jobs/{job}/pause`, `/resume`, `/cancel` (`jobs.manage`): one control each;
//!   a resumed pending job is handed to the host's [`super::JobRunner`] for its kind.
//!
//! The service journals each control and each refusal it makes; a handler journals only what
//! it refuses itself (a query it cannot read).

use super::model::{Job, JobError, JobState};
use super::store::JobFilter;
use super::view::{JobControlAnswer, JobPage, JobView};
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
    /// At most this many jobs (1 to 100, default 100).
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
        Err(()) => return invalid(&jobs).into_response(),
    };
    let listed = blocking(move || jobs.list(&filter)).await;
    match listed {
        Ok(list) => Json(JobPage {
            jobs: list.jobs.iter().map(JobView::of).collect(),
            truncated: list.truncated,
        })
        .into_response(),
        Err(err) => api_error(&err).into_response(),
    }
}

/// `GET /api/v1/jobs/{job}`: one job.
async fn one(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let jobs = match installed(&state) {
        Ok(jobs) => jobs,
        Err(err) => return err.into_response(),
    };
    match blocking(move || jobs.job(&id)).await {
        Ok(job) => Json(JobView::of(&job)).into_response(),
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
    let job = match blocking(move || jobs.resume(&id)).await {
        Ok(job) => job,
        Err(err) => return api_error(&err).into_response(),
    };
    let scheduled = schedule(&state, &job);
    Json(JobControlAnswer {
        job: JobView::of(&job),
        scheduled,
    })
    .into_response()
}

/// Hand the pending `job` to the runner of its kind; false when it has none or it refused.
fn schedule(state: &ApiState, job: &Job) -> bool {
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
    match blocking(move || act(&jobs)).await {
        Ok(job) => Json(JobControlAnswer {
            job: JobView::of(&job),
            scheduled: false,
        })
        .into_response(),
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

/// The filter of `query`, or `Err` for an unknown state. Pure.
fn filter_of(query: JobsQuery) -> Result<JobFilter, ()> {
    let state = match query.state.as_deref() {
        Some(name) => Some(JobState::parse(name).ok_or(())?),
        None => None,
    };
    Ok(JobFilter {
        kind: query.kind,
        state,
        limit: query.limit.unwrap_or(MAX_LISTED_JOBS),
    })
}

/// Journal a query the list route cannot read, once, and answer its refusal.
fn invalid(jobs: &Jobs) -> ApiError {
    let entry = JournalEntry::new(JOB_REFUSED, Outcome::Failed)
        .level(Level::Warn)
        .error_code("invalid_request");
    jobs.journal_entry(entry);
    let message = "The job filter names an unknown field, state or limit.";
    ApiError::new(ErrorCode::InvalidRequest, message)
}

/// The typed error of a job refusal or failure. Pure.
fn api_error(err: &JobError) -> ApiError {
    let (code, message) = match err {
        JobError::NotFound => (ErrorCode::NotFound, "No job has this ID."),
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
        _ => "The job cannot take this control now.",
    }
}

/// Run the store call `work` on the blocking pool, so a host's SQLite store never blocks the
/// async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, JobError> + Send + 'static,
) -> Result<T, JobError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| JobError::Failed(err.to_string()))?
}
