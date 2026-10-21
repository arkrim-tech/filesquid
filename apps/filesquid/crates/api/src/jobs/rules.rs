//! The pure transitions of a job (API-S-033; ADR 007). A store applies each one inside one
//! atomic read-modify-write, so every host enforces the same lease and fencing rules.

use super::model::{Job, JobControl, JobError, JobState, Lease};
use super::MAX_CHECKPOINT_BYTES;

/// Error code of a job whose source changed since it was submitted.
pub const SOURCE_CHANGED: &str = "source_changed";

/// Start `job` for `holder` working on `source`, in one step: a finished job, and one under
/// a live lease (whoever holds it, the same holder included), is refused; a job whose
/// source differs from `source` is failed (`source_changed`) without ever running;
/// otherwise the job is leased until `now_ms + ttl_ms`, its fencing number and attempt
/// count raised.
///
/// # Errors
/// [`JobError::Conflict`] (`job_finished`, `lease_held`, or `job_paused` for a job held by a
/// pause, API-S-036).
pub fn start(
    job: Job,
    (holder, source): (&str, &str),
    now_ms: i64,
    ttl_ms: i64,
) -> Result<Job, JobError> {
    if job.state.is_finished() {
        return Err(JobError::Conflict("job_finished"));
    }
    if lease_live(&job, now_ms) {
        return Err(JobError::Conflict("lease_held"));
    }
    if job.control == Some(JobControl::Pause) {
        return Err(JobError::Conflict("job_paused"));
    }
    match job.source == source {
        true => Ok(acquire(job, holder, now_ms, ttl_ms)),
        false => Ok(fail_source(job, now_ms)),
    }
}

/// Lease `job` to `holder` until `now_ms + ttl_ms`.
fn acquire(mut job: Job, holder: &str, now_ms: i64, ttl_ms: i64) -> Job {
    job.state = JobState::Running;
    job.fence += 1;
    job.attempts = job.attempts.saturating_add(1);
    job.holder = Some(holder.to_string());
    job.lease_expires_ms = Some(now_ms.saturating_add(ttl_ms));
    job.updated_ms = now_ms;
    job
}

/// Fail `job` because its source changed; it never runs and its checkpoint is not reused.
fn fail_source(mut job: Job, now_ms: i64) -> Job {
    job.state = JobState::Failed;
    job.error_code = Some(SOURCE_CHANGED.to_string());
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    job
}

/// The lease a worker holds on `job` after [`start`]. Pure.
///
/// # Errors
/// [`JobError::Fenced`] when `job` is not leased.
pub fn lease_of(job: &Job) -> Result<Lease, JobError> {
    match (&job.holder, job.lease_expires_ms) {
        (Some(holder), Some(expires_ms)) if job.state == JobState::Running => Ok(Lease {
            job_id: job.id.clone(),
            holder: holder.clone(),
            fence: job.fence,
            expires_ms,
        }),
        _ => Err(JobError::Fenced),
    }
}

/// Store `checkpoint` and the progress under `lease` and renew the lease until at least
/// `now_ms + ttl_ms`.
///
/// # Errors
/// [`JobError::Fenced`] when the lease is no longer the job's (checked first, so a stale
/// worker is always fenced); [`JobError::TooLarge`] past [`MAX_CHECKPOINT_BYTES`].
pub fn checkpoint(
    mut job: Job,
    lease: &Lease,
    (checkpoint, done, total): (&str, u64, Option<u64>),
    now_ms: i64,
    ttl_ms: i64,
) -> Result<Job, JobError> {
    check_lease(&job, lease, now_ms)?;
    if checkpoint.len() > MAX_CHECKPOINT_BYTES {
        return Err(JobError::TooLarge);
    }
    job.checkpoint = Some(checkpoint.to_string());
    job.done = done;
    job.total = total;
    job.lease_expires_ms = Some(extended(&job, now_ms, ttl_ms));
    job.updated_ms = now_ms;
    Ok(job)
}

/// Renew `lease` until at least `now_ms + ttl_ms` without changing the checkpoint: what a
/// worker does while one long unit of work runs.
///
/// # Errors
/// [`JobError::Fenced`] when the lease is no longer the job's.
pub fn renew(mut job: Job, lease: &Lease, now_ms: i64, ttl_ms: i64) -> Result<Job, JobError> {
    check_lease(&job, lease, now_ms)?;
    job.lease_expires_ms = Some(extended(&job, now_ms, ttl_ms));
    job.updated_ms = now_ms;
    Ok(job)
}

/// The lease end after a renewal at `now_ms`: never earlier than the stored end, so a
/// delayed renewal or checkpoint cannot shorten a lease another call already extended.
fn extended(job: &Job, now_ms: i64, ttl_ms: i64) -> i64 {
    let renewed = now_ms.saturating_add(ttl_ms);
    job.lease_expires_ms.map_or(renewed, |end| end.max(renewed))
}

/// Give the lease back at quit: the job is pending again with its checkpoint (and a pause
/// asked of it) kept; a job under a cancel request is cancelled instead.
///
/// # Errors
/// [`JobError::Fenced`] when the lease is no longer the job's.
pub fn suspend(job: Job, lease: &Lease, now_ms: i64) -> Result<Job, JobError> {
    check_lease(&job, lease, now_ms)?;
    Ok(release(job, now_ms))
}

/// The job pending again without a lease, or cancelled when a cancel was asked of it.
fn release(mut job: Job, now_ms: i64) -> Job {
    job.state = match job.control {
        Some(JobControl::Cancel) => JobState::Cancelled,
        _ => JobState::Pending,
    };
    if job.state == JobState::Cancelled {
        job.control = None;
    }
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    job
}

/// Finish the job under `lease` as `state`, a finished state (completed, failed or
/// cancelled), with `error_code`.
///
/// # Errors
/// [`JobError::Conflict`] (`state_not_final`) for a state that is not finished;
/// [`JobError::Fenced`] when the lease is no longer the job's.
pub fn finish(
    mut job: Job,
    lease: &Lease,
    (state, error_code): (JobState, Option<&str>),
    now_ms: i64,
) -> Result<Job, JobError> {
    if !state.is_finished() {
        return Err(JobError::Conflict("state_not_final"));
    }
    check_lease(&job, lease, now_ms)?;
    job.state = state;
    job.error_code = error_code.map(str::to_string);
    job.control = None;
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    Ok(job)
}

/// Release the expired lease of a run that crashed: the job is pending again with its
/// checkpoint, or cancelled when a cancel was asked of it. Any other job is answered
/// unchanged. Pure.
pub fn expire(job: Job, now_ms: i64) -> Job {
    if job.state != JobState::Running || lease_live(&job, now_ms) {
        return job;
    }
    release(job, now_ms)
}

/// Pause `job` (API-S-036): a pending job is held; a running job is asked to suspend at its
/// next checkpoint. Pausing a job already paused changes nothing. Pure.
///
/// # Errors
/// [`JobError::Conflict`]: `job_finished`, or `cancel_requested` for a job being cancelled.
pub fn pause(mut job: Job, now_ms: i64) -> Result<Job, JobError> {
    refuse_finished_or_cancelling(&job)?;
    if job.control != Some(JobControl::Pause) {
        job.control = Some(JobControl::Pause);
        job.updated_ms = now_ms;
    }
    Ok(job)
}

/// Resume `job`: clear the pause asked of it, so it may start again. A pending job without a
/// pause (one suspended at quit) is answered unchanged, ready to be handed to its runner.
/// Pure.
///
/// # Errors
/// [`JobError::Conflict`]: `job_finished`, `cancel_requested`, or `job_running` for a running
/// job that was not asked to pause.
pub fn resume(mut job: Job, now_ms: i64) -> Result<Job, JobError> {
    refuse_finished_or_cancelling(&job)?;
    if job.state == JobState::Running && job.control.is_none() {
        return Err(JobError::Conflict("job_running"));
    }
    if job.control.is_some() {
        job.control = None;
        job.updated_ms = now_ms;
    }
    Ok(job)
}

/// Cancel `job`: a pending job is cancelled at once; a running job is asked to stop at its
/// next checkpoint. Cancelling a job already asked to stop changes nothing. Pure.
///
/// # Errors
/// [`JobError::Conflict`] (`job_finished`).
pub fn cancel(mut job: Job, now_ms: i64) -> Result<Job, JobError> {
    if job.state.is_finished() {
        return Err(JobError::Conflict("job_finished"));
    }
    if job.state == JobState::Pending {
        job.state = JobState::Cancelled;
        job.control = None;
        job.updated_ms = now_ms;
        return Ok(job);
    }
    if job.control != Some(JobControl::Cancel) {
        job.control = Some(JobControl::Cancel);
        job.updated_ms = now_ms;
    }
    Ok(job)
}

/// Refuse a control on a finished job or one already asked to stop.
fn refuse_finished_or_cancelling(job: &Job) -> Result<(), JobError> {
    if job.state.is_finished() {
        return Err(JobError::Conflict("job_finished"));
    }
    match job.control == Some(JobControl::Cancel) {
        true => Err(JobError::Conflict("cancel_requested")),
        false => Ok(()),
    }
}

/// Whether `job` runs under a lease that has not ended at `now_ms`: what holds one of its
/// class's slots. Pure.
pub fn holds_slot(job: &Job, now_ms: i64) -> bool {
    lease_live(job, now_ms)
}

/// Whether `job` is running under a lease that has not ended at `now_ms`.
fn lease_live(job: &Job, now_ms: i64) -> bool {
    job.state == JobState::Running && job.lease_expires_ms.is_some_and(|end| end > now_ms)
}

/// Refuse a write under `lease` unless it is this job's current, unexpired lease.
fn check_lease(job: &Job, lease: &Lease, now_ms: i64) -> Result<(), JobError> {
    let current = lease.job_id == job.id
        && job.state == JobState::Running
        && job.fence == lease.fence
        && job.holder.as_deref() == Some(lease.holder.as_str());
    match current && lease_live(job, now_ms) {
        true => Ok(()),
        false => Err(JobError::Fenced),
    }
}
