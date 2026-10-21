//! The job record, its lease and its errors.

use serde::{Deserialize, Serialize};

/// Where a job stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Waiting to run, or suspended with its checkpoint: resumable.
    Pending,
    /// Leased by a worker.
    Running,
    /// Finished; nothing is left to do.
    Completed,
    /// Finished with an error code.
    Failed,
    /// Finished because it was cancelled.
    Cancelled,
}

impl JobState {
    /// The stored name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// The state a stored name names, if any.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Running,
            Self::Completed,
            Self::Failed,
            Self::Cancelled,
        ]
        .into_iter()
        .find(|state| state.as_str() == name)
    }

    /// Whether the job is finished for good.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// What a job is when it is submitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSpec {
    /// The kind of work (`prov.backfill`, later `import`, `upload`...); its process kind.
    pub kind: String,
    /// The idempotent operation ID every step and journal entry of the job carries.
    pub operation_id: String,
    /// The opaque identity of what the job works on, compared again on every resume. It is
    /// stored, never journaled.
    pub source: String,
}

/// One stored job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// The job ID (UUID v4).
    pub id: String,
    /// The kind of work.
    pub kind: String,
    /// The job's operation ID.
    pub operation_id: String,
    /// The opaque source identity.
    pub source: String,
    /// Where it stands.
    pub state: JobState,
    /// The last checkpoint (at most [`super::MAX_CHECKPOINT_BYTES`]), kept across a quit.
    pub checkpoint: Option<String>,
    /// Units done at the last checkpoint.
    pub done: u64,
    /// Units in all, when known.
    pub total: Option<u64>,
    /// How many times a worker took the lease.
    pub attempts: u32,
    /// The fencing number of the current or last lease; each new lease raises it.
    pub fence: u64,
    /// The worker that holds the lease, while running.
    pub holder: Option<String>,
    /// When the lease ends (Unix ms, UTC), while running.
    pub lease_expires_ms: Option<i64>,
    /// When the job was submitted (Unix ms, UTC).
    pub created_ms: i64,
    /// When the job last changed (Unix ms, UTC).
    pub updated_ms: i64,
    /// Why a failed job failed.
    pub error_code: Option<String>,
    /// The control asked of the job and not yet carried out (API-S-037): a pending job under
    /// [`JobControl::Pause`] is paused; a running job's worker sees the request at its next
    /// checkpoint or renewal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<JobControl>,
}

impl Job {
    /// Whether the job waits under a pause: pending, held until it is resumed.
    pub fn is_paused(&self) -> bool {
        self.state == JobState::Pending && self.control == Some(JobControl::Pause)
    }
}

/// A control asked of a job through the job routes (API-S-037).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobControl {
    /// Hold the job: a pending job does not start; a running job suspends at its next
    /// checkpoint and stays held.
    Pause,
    /// Stop the job: a running job stops at its next checkpoint and is cancelled.
    Cancel,
}

impl JobControl {
    /// Lowercase wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Cancel => "cancel",
        }
    }

    /// Parse a lowercase wire name.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Pause, Self::Cancel]
            .into_iter()
            .find(|control| control.as_str() == name)
    }
}

/// An exclusive lease on a job: the proof a worker shows for every write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lease {
    /// The leased job.
    pub job_id: String,
    /// The worker.
    pub holder: String,
    /// The fencing number; a write whose number is not the job's current one is refused.
    pub fence: u64,
    /// When the lease ends unless renewed by a checkpoint (Unix ms, UTC).
    pub expires_ms: i64,
}

/// Why a job call was refused or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobError {
    /// No such job.
    NotFound,
    /// The job cannot be leased now: `lease_held` or `job_finished`.
    Conflict(&'static str),
    /// The lease is no longer the job's: it expired or another worker took it over.
    Fenced,
    /// The checkpoint is larger than [`super::MAX_CHECKPOINT_BYTES`].
    TooLarge,
    /// The source changed since the job was submitted; the job failed and nothing of it is
    /// reused.
    SourceChanged,
    /// The store cannot be reached now; a retry may succeed.
    Unavailable(String),
    /// The store failed.
    Failed(String),
}

impl JobError {
    /// The journal and API error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Conflict(code) => code,
            Self::Fenced => "fenced",
            Self::TooLarge => "too_large",
            Self::SourceChanged => "source_changed",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "internal",
        }
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(detail) | Self::Failed(detail) => {
                write!(f, "job store {}: {detail}", self.error_code())
            }
            _ => write!(f, "job refused: {}", self.error_code()),
        }
    }
}

impl std::error::Error for JobError {}
