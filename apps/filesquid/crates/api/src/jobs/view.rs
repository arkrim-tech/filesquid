//! [`JobView`]: what the job routes and the `job.update` event show of a job (API-S-037):
//! never its source identity, checkpoint, lease holder or fencing number.

use super::items::JobDestination;
use super::model::{Job, JobControl, JobState};
use crate::journal::format_rfc3339_ms;
use serde::{Deserialize, Serialize};

/// One job as the routes and events show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobView {
    /// The job ID.
    pub id: String,
    /// The kind of work.
    pub kind: String,
    /// The job's operation ID.
    pub operation_id: String,
    /// Where an upload job's files go (API-S-037 **Per-file progress**); absent for a kind
    /// without a destination or on a host that keeps no per-file progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<JobDestination>,
    /// Where it stands.
    pub state: JobState,
    /// Whether it waits under a pause.
    pub paused: bool,
    /// The control asked of it and not yet carried out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<JobControl>,
    /// Units done at the last checkpoint.
    pub done: u64,
    /// Units in all, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// How many times a worker took the job.
    pub attempts: u32,
    /// Why it failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// When it was submitted (RFC 3339, UTC).
    pub created: String,
    /// When it last changed (RFC 3339, UTC).
    pub updated: String,
    /// Once it has completed, the result its kind defines (for `library.rebuild`, the rebuild
    /// report of API-S-025); `null` before then, for a kind without one, and in a
    /// [`JobView::summary`].
    pub result: Option<serde_json::Value>,
}

impl JobView {
    /// The view of `job`. Pure.
    pub fn of(job: &Job) -> Self {
        Self {
            id: job.id.clone(),
            kind: job.kind.clone(),
            operation_id: job.operation_id.clone(),
            destination: None,
            state: job.state,
            paused: job.is_paused(),
            control: job.control,
            done: job.done,
            total: job.total,
            attempts: job.attempts,
            error_code: job.error_code.clone(),
            created: format_rfc3339_ms(job.created_ms),
            updated: format_rfc3339_ms(job.updated_ms),
            result: shown_result(job),
        }
    }
}

impl JobView {
    /// This view with `destination` as where the job's files go. Pure.
    #[must_use]
    pub fn with_destination(mut self, destination: Option<JobDestination>) -> Self {
        self.destination = destination;
        self
    }

    /// The view of `job` without its result (`null`): what the job listing and the
    /// `job.update` event carry, so neither grows with a completed job's report; the result
    /// is answered by `GET /api/v1/jobs/{job}` ([`JobView::of`]). Pure.
    pub fn summary(job: &Job) -> Self {
        let mut view = Self::of(&Job {
            result: None,
            ..job.clone()
        });
        view.result = None;
        view
    }
}

/// The result `job` shows: its stored result once it has completed, else none. A stored
/// result that is not JSON is not shown and is logged. Pure but for the log.
fn shown_result(job: &Job) -> Option<serde_json::Value> {
    let text = job.result.as_deref()?;
    if job.state != JobState::Completed {
        return None;
    }
    serde_json::from_str(text)
        .inspect_err(|err| tracing::warn!(error = %err, "a job's stored result is not JSON"))
        .ok()
}

/// The answer of `GET /api/v1/jobs`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPage {
    /// The jobs, newest first.
    pub jobs: Vec<JobView>,
    /// Whether more jobs matched than were answered.
    pub truncated: bool,
}

/// The answer of a control route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobControlAnswer {
    /// The job after the control.
    pub job: JobView,
    /// Whether the job was handed to its kind's worker to run now (resume only); false
    /// leaves a pending job for the next launch.
    pub scheduled: bool,
}
