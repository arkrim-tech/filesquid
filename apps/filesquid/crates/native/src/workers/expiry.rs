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
use filesquid_api::jobs::{Job, JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::library::{mint_uuid_v4, LibraryService, EXPIRY_JOB_KIND};
use std::sync::atomic::{AtomicBool, Ordering};

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
/// installation). Database, filesystem and journal I/O.
///
/// # Errors
/// The job could not be submitted, started, checkpointed or completed (for example its lease
/// is held by another run); the next run tries again.
pub fn run_expiry_job(
    jobs: &Jobs,
    service: &LibraryService,
    catalog: &SqliteCatalog,
    source: &str,
    stop: &AtomicBool,
) -> Result<(JobState, ExpiryJobReport), JobError> {
    let job = match jobs.unfinished_of(EXPIRY_JOB_KIND, 1)?.into_iter().next() {
        Some(job) => job,
        None => submit(jobs, source)?,
    };
    let mut running = jobs.start(&job.id, source)?;
    let libraries = catalog
        .open_libraries()
        .map_err(|err| JobError::Unavailable(format!("{err:?}")))?;
    let total = u64::try_from(libraries.len()).unwrap_or(u64::MAX);
    let mut report = ExpiryJobReport::default();
    for library in &libraries {
        if stop.load(Ordering::SeqCst) || running.requested().is_some() {
            return running.suspend().map(|state| (state, report));
        }
        expire_one(service, catalog, library, &mut report);
        report.libraries += 1;
        running.checkpoint(&report.libraries.to_string(), report.libraries, Some(total))?;
    }
    finish(running, &report).map(|()| (JobState::Completed, report))
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

/// Expire `library` and add its counts to `report`; a failure is counted and logged (the
/// service journals what it refused).
fn expire_one(
    service: &LibraryService,
    catalog: &SqliteCatalog,
    library: &filesquid_core::LibraryId,
    report: &mut ExpiryJobReport,
) {
    match service.expire_library(EXPIRY_ACTOR, library, catalog) {
        Ok(done) => {
            report.expired += done.expired as u64;
            report.objects_done += done.objects_done as u64;
            report.objects_failed += done.objects_failed as u64;
        }
        Err(err) => {
            report.libraries_failed += 1;
            tracing::warn!(
                operation = EXPIRY_JOB_KIND,
                error_code = err.error_code(),
                "a library's expiry failed; the next run tries again"
            );
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
