//! The expiry of deleted files as a durable job on the desktop (Task API-F2.15; API-S-019,
//! API-S-033, ADR 041 Decision 11).
//!
//! One run is one `library.expiry` job: it resumes an unfinished one of this installation or
//! submits a new one, then expires every library the catalog has open, one after the other,
//! through `filesquid-api`'s [`LibraryService::expire_library`] with the catalog as the
//! [`ObjectPurger`](filesquid_api::library::ObjectPurger). The job registers in the process registry and journals its state
//! changes through [`Jobs`]; its checkpoint is the count of libraries done and its result
//! holds counts only, never a name or path. A library that fails is counted and retried by
//! the next run; setting `stop` suspends the job between libraries. A library that is not
//! open is not visited: its due files are expired by the first run after it is opened.

use crate::SqliteCatalog;
use filesquid_api::jobs::{Halt, Job, JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::library::{mint_uuid_v4, LibraryService, EXPIRY_JOB_KIND};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// How often the lease helper wakes, in milliseconds.
const RENEW_POLL_MS: u64 = 250;

/// How often the lease is renewed while one library runs, in milliseconds: well within the
/// jobs' lease time to live.
const RENEW_EVERY_MS: u64 = 3_000;

/// The opaque actor of the scheduled expiry.
pub const EXPIRY_ACTOR: &str = "system";

/// What one expiry job run did: counts only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpiryJobReport {
    /// Libraries visited.
    pub libraries: u64,
    /// Files expired.
    pub expired: u64,
    /// Object works finished.
    pub objects_done: u64,
    /// Object works that failed and wait for the next run.
    pub objects_failed: u64,
    /// Libraries whose run failed and wait for the next run.
    pub libraries_failed: u64,
}

/// Run one expiry job over every library `catalog` has open, under `source` (the
/// installation), starting `resumed` when the job routes resumed that job. A job suspended
/// earlier resumes from its checkpoint, the libraries it finished; a job of another source is
/// left and a new one submitted. A pause or cancel asked during a library halts the run before
/// its next library and before it completes. Database, filesystem and journal I/O.
///
/// # Errors
/// The job could not be submitted, started, checkpointed or completed (for example its lease
/// is held by another run); the next run tries again.
pub fn run_expiry_job(
    jobs: &Jobs,
    service: &LibraryService,
    catalog: &SqliteCatalog,
    source: &str,
    resumed: Option<&str>,
    stop: &AtomicBool,
) -> Result<(JobState, ExpiryJobReport), JobError> {
    let mut running = start_job(jobs, source, resumed)?;
    let mut done = done_libraries(running.checkpoint_text());
    let libraries = catalog
        .open_libraries()
        .map_err(|err| JobError::Unavailable(format!("{err:?}")))?;
    let total = u64::try_from(libraries.len()).unwrap_or(u64::MAX);
    let mut report = ExpiryJobReport::default();
    let remaining: Vec<_> = libraries
        .iter()
        .filter(|l| !done.contains(l.as_str()))
        .collect();
    for library in remaining {
        running = match pause_point(running, stop)? {
            Halt::Continued(next) => *next,
            Halt::Halted(state) => return Ok((state, report)),
        };
        let finished = expire_renewing(service, catalog, library, &running, &mut report);
        report.libraries += 1;
        if finished {
            done.insert(library.as_str().to_string());
        }
        let progress = u64::try_from(done.len()).unwrap_or(u64::MAX);
        running.checkpoint(&checkpoint(&done), progress, Some(total))?;
    }
    match pause_point(running, stop)? {
        Halt::Continued(last) => finish(*last, &report).map(|()| (JobState::Completed, report)),
        Halt::Halted(state) => Ok((state, report)),
    }
}

/// Stop the run where it may stop: suspend it when `stop` is set (the host quits), halt it
/// through [`RunningJob::halt_if_requested`] when a pause or cancel was asked (checked again in
/// the store, so a resume that cleared it keeps the run going), and otherwise hand it back.
/// Store I/O when it stops.
fn pause_point(running: RunningJob, stop: &AtomicBool) -> Result<Halt, JobError> {
    if stop.load(Ordering::SeqCst) {
        return running.suspend().map(Halt::Halted);
    }
    if running.requested().is_none() {
        return Ok(Halt::Continued(Box::new(running)));
    }
    running.halt_if_requested()
}

/// Start `resumed`, the job the job routes resumed, when given; otherwise this installation's
/// oldest unfinished expiry job, or a new one when there is none or the unfinished one belongs
/// to another source.
fn start_job(jobs: &Jobs, source: &str, resumed: Option<&str>) -> Result<RunningJob, JobError> {
    let id = match resumed {
        Some(id) => id.to_string(),
        None => oldest_or_new(jobs, source)?.id,
    };
    match jobs.start(&id, source) {
        Err(JobError::SourceChanged) => {
            let fresh = submit(jobs, source)?;
            jobs.start(&fresh.id, source)
        }
        started => started,
    }
}

/// The oldest unfinished expiry job, or a newly submitted one of `source` when there is none.
fn oldest_or_new(jobs: &Jobs, source: &str) -> Result<Job, JobError> {
    match jobs.unfinished_of(EXPIRY_JOB_KIND, 1)?.into_iter().next() {
        Some(job) => Ok(job),
        None => submit(jobs, source),
    }
}

/// The libraries a checkpoint lists as finished; none for a missing or unreadable one (every
/// library is visited again, which only finds nothing due).
fn done_libraries(checkpoint: Option<&str>) -> BTreeSet<String> {
    let parsed = checkpoint.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    parsed
        .as_ref()
        .and_then(|value| value.get("done"))
        .and_then(serde_json::Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The checkpoint of the libraries in `done`.
fn checkpoint(done: &BTreeSet<String>) -> String {
    serde_json::json!({ "done": done }).to_string()
}

/// Expire `library` while a helper thread renews the job's lease every third of its time to
/// live, so a long library is never fenced; answer whether the library finished (a failed
/// one is retried by the next run and not checkpointed as done).
fn expire_renewing(
    service: &LibraryService,
    catalog: &SqliteCatalog,
    library: &filesquid_core::LibraryId,
    running: &RunningJob,
    report: &mut ExpiryJobReport,
) -> bool {
    let working = AtomicBool::new(true);
    let step = Duration::from_millis(RENEW_POLL_MS);
    std::thread::scope(|scope| {
        scope.spawn(|| renew_while(running, &working, step));
        let finished = expire_one(service, catalog, library, report);
        working.store(false, Ordering::SeqCst);
        finished
    })
}

/// Renew `running`'s lease about every [`RENEW_EVERY_MS`] until `working` is cleared. A
/// refused renewal ends the helper; the run finds the fence at its next checkpoint.
fn renew_while(running: &RunningJob, working: &AtomicBool, step: Duration) {
    let mut since = 0;
    while working.load(Ordering::SeqCst) {
        std::thread::sleep(step);
        since += RENEW_POLL_MS;
        if since < RENEW_EVERY_MS {
            continue;
        }
        since = 0;
        if let Err(err) = running.renew() {
            tracing::warn!(
                operation = EXPIRY_JOB_KIND,
                error_code = err.error_code(),
                "the expiry job's lease could not be renewed"
            );
            return;
        }
    }
}

/// Submit a new expiry job of `source`.
fn submit(jobs: &Jobs, source: &str) -> Result<Job, JobError> {
    let operation_id = mint_uuid_v4().map_err(|err| JobError::Unavailable(err.to_string()))?;
    jobs.submit(JobSpec {
        kind: EXPIRY_JOB_KIND.to_string(),
        operation_id,
        source: source.to_string(),
    })
}

/// Expire `library` and add its counts to `report`; answer whether it finished. A failure is
/// counted and logged (the service journals what it refused).
fn expire_one(
    service: &LibraryService,
    catalog: &SqliteCatalog,
    library: &filesquid_core::LibraryId,
    report: &mut ExpiryJobReport,
) -> bool {
    match service.expire_library(EXPIRY_ACTOR, library, catalog) {
        Ok(done) => {
            report.expired += done.expired as u64;
            report.objects_done += done.objects_done as u64;
            report.objects_failed += done.objects_failed as u64;
            true
        }
        Err(err) => {
            report.libraries_failed += 1;
            tracing::warn!(
                operation = EXPIRY_JOB_KIND,
                error_code = err.error_code(),
                "a library's expiry failed; the next run tries again"
            );
            false
        }
    }
}

/// Complete `running` with `report` as its result.
fn finish(running: RunningJob, report: &ExpiryJobReport) -> Result<(), JobError> {
    let result = serde_json::json!({
        "libraries": report.libraries,
        "expired": report.expired,
        "objects_done": report.objects_done,
        "objects_failed": report.objects_failed,
        "libraries_failed": report.libraries_failed,
    });
    running.complete_with_result(&result.to_string())
}
