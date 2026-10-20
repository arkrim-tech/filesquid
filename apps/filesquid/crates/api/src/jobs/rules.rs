//! The pure transitions of a job (API-S-033; ADR 007). A store applies each one inside one
//! atomic read-modify-write, so every host enforces the same lease and fencing rules.

use super::model::{Job, JobError, JobState, Lease};
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
/// [`JobError::Conflict`] (`job_finished` or `lease_held`).
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

/// Give the lease back at quit: the job is pending again with its checkpoint kept.
///
/// # Errors
/// [`JobError::Fenced`] when the lease is no longer the job's.
pub fn suspend(mut job: Job, lease: &Lease, now_ms: i64) -> Result<Job, JobError> {
    check_lease(&job, lease, now_ms)?;
    job.state = JobState::Pending;
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    Ok(job)
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
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    Ok(job)
}

/// Release the expired lease of a run that crashed: the job is pending again with its
/// checkpoint. Any other job is answered unchanged. Pure.
pub fn expire(mut job: Job, now_ms: i64) -> Job {
    if job.state != JobState::Running || lease_live(&job, now_ms) {
        return job;
    }
    job.state = JobState::Pending;
    job.holder = None;
    job.lease_expires_ms = None;
    job.updated_ms = now_ms;
    job
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
