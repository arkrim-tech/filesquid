//! The pure transitions of a job (API-S-033; ADR 007). A store applies each one inside one
//! atomic read-modify-write, so every host enforces the same lease and fencing rules.

use super::model::{Job, JobError, JobState, Lease};
use super::MAX_CHECKPOINT_BYTES;

/// Lease `job` to `holder` until `now_ms + ttl_ms`, raising its fencing number and attempt
/// count. A finished job, and one whose unexpired lease another worker holds, is refused.
///
/// # Errors
/// [`JobError::Conflict`] (`job_finished` or `lease_held`).
pub fn acquire(mut job: Job, holder: &str, now_ms: i64, ttl_ms: i64) -> Result<Job, JobError> {
    if job.state.is_finished() {
        return Err(JobError::Conflict("job_finished"));
    }
    if lease_live(&job, now_ms) && job.holder.as_deref() != Some(holder) {
        return Err(JobError::Conflict("lease_held"));
    }
    job.state = JobState::Running;
    job.fence += 1;
    job.attempts = job.attempts.saturating_add(1);
    job.holder = Some(holder.to_string());
    job.lease_expires_ms = Some(now_ms.saturating_add(ttl_ms));
    job.updated_ms = now_ms;
    Ok(job)
}

/// The lease a worker holds on `job` after [`acquire`]. Pure.
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

/// Store `checkpoint` and the progress under `lease` and renew the lease until
/// `now_ms + ttl_ms`.
///
/// # Errors
/// [`JobError::TooLarge`] past [`MAX_CHECKPOINT_BYTES`]; [`JobError::Fenced`] when the
/// lease is no longer the job's.
pub fn checkpoint(
    mut job: Job,
    lease: &Lease,
    (checkpoint, done, total): (&str, u64, Option<u64>),
    now_ms: i64,
    ttl_ms: i64,
) -> Result<Job, JobError> {
    if checkpoint.len() > MAX_CHECKPOINT_BYTES {
        return Err(JobError::TooLarge);
    }
    check_lease(&job, lease, now_ms)?;
    job.checkpoint = Some(checkpoint.to_string());
    job.done = done;
    job.total = total;
    job.lease_expires_ms = Some(now_ms.saturating_add(ttl_ms));
    job.updated_ms = now_ms;
    Ok(job)
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

/// Finish the job under `lease` as `state` (completed, failed or cancelled) with
/// `error_code`.
///
/// # Errors
/// [`JobError::Fenced`] when the lease is no longer the job's.
pub fn finish(
    mut job: Job,
    lease: &Lease,
    (state, error_code): (JobState, Option<&str>),
    now_ms: i64,
) -> Result<Job, JobError> {
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

/// Refuse a write under `lease` unless it is the job's current, unexpired lease.
fn check_lease(job: &Job, lease: &Lease, now_ms: i64) -> Result<(), JobError> {
    let current = job.state == JobState::Running
        && job.fence == lease.fence
        && job.holder.as_deref() == Some(lease.holder.as_str());
    match current && lease_live(job, now_ms) {
        true => Ok(()),
        false => Err(JobError::Fenced),
    }
}
