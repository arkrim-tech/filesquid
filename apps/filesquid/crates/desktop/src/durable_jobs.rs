//! Durable jobs on the desktop (Task DE-0.4; DE-S-014, API-S-033; NFR-003, ADR 007).
//!
//! The host keeps its jobs in the operational store (`<data dir>/filesquid.sqlite`,
//! [`SqliteJobs`]) and runs them through `filesquid-api`'s [`Jobs`] service, which owns the
//! lease, fencing, checkpoint and source rules and journals every state change. At launch
//! the host reconciles its jobs ([`recover_jobs`]): a bounded read of the job rows, never a
//! rescan of the media they cover. At quit (the last window closing included) a running job
//! is suspended: its lease is given back and its checkpoint kept for the next launch, the
//! default of ADR 007 until a keep-running setting exists.
//!
//! The history backfill (API-S-029) is the first durable job ([`run_backfill_job`]): its
//! checkpoint lists the libraries it completed, so a resumed run skips them, and its source
//! identity is the installation, so a job written by another installation is never resumed.

use filesquid_api::history::backfill::{Backfill, BACKFILL_OPERATION};
use filesquid_api::jobs::{Job, JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::library::mint_uuid_v4;
use filesquid_core::{InstallationId, LibraryId};
use filesquid_native::{HostMonitor, LibraryBackfill, SqliteCatalog, SqliteJobs, JOURNAL_FILE};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Step of the bounded wait for a lease a crashed run left behind.
const LEASE_WAIT_STEP: Duration = Duration::from_millis(100);

/// Open the job store in `data_dir` and the job service of this launch over `host`'s
/// journal queue and process registry, leasing as a holder named for this launch only.
/// Filesystem and database I/O.
///
/// # Errors
/// The store could not be opened or no holder ID could be minted; the message names no
/// path.
pub fn open_jobs(data_dir: &Path, host: &HostMonitor) -> Result<Jobs, String> {
    let store = SqliteJobs::open(&data_dir.join(JOURNAL_FILE))
        .map_err(|err| format!("the job store could not be opened ({})", err.error_code()))?;
    let launch = mint_uuid_v4().map_err(|err| err.to_string())?;
    Ok(Jobs::new(
        Arc::new(store),
        host.monitor().journal(),
        host.monitor().registry().clone(),
        format!("filesquid-desktop-{launch}"),
    ))
}

/// The launch-time reconciliation: release the leases of runs a crash left and answer the
/// unfinished jobs (bounded by the API). A store failure is logged as an error (the journal
/// layer journals it) and answers no job, so the launch goes on. Database I/O.
pub fn recover_jobs(jobs: &Jobs) -> Vec<Job> {
    jobs.recover().unwrap_or_else(|err| {
        tracing::error!(
            operation = "job.recover",
            error_code = err.error_code(),
            "the durable jobs could not be recovered"
        );
        Vec::new()
    })
}

/// The source identity of a backfill job: the installation whose records it writes. Pure.
pub fn backfill_source(installation: &InstallationId) -> String {
    format!("installation:{}", installation.as_str())
}

/// What a backfill job stores between runs: the libraries it completed.
#[derive(Debug, Default, Serialize, Deserialize)]
struct BackfillCheckpoint {
    done: Vec<String>,
}

/// The checkpoint text of a backfill that completed the libraries `done`. Pure.
pub fn backfill_checkpoint(done: &[LibraryId]) -> String {
    let checkpoint = BackfillCheckpoint {
        done: done.iter().map(|id| id.as_str().to_string()).collect(),
    };
    serde_json::to_string(&checkpoint).unwrap_or_else(|_| "{}".to_string())
}

/// The libraries a checkpoint says are complete. An unreadable checkpoint is logged and
/// counts as none, so every library is visited again (each skips the versions it recorded).
fn done_libraries(checkpoint: Option<&str>) -> Vec<String> {
    let Some(text) = checkpoint else {
        return Vec::new();
    };
    match serde_json::from_str::<BackfillCheckpoint>(text) {
        Ok(checkpoint) => checkpoint.done,
        Err(_) => {
            tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = "checkpoint_unreadable",
                "a backfill checkpoint was unreadable; every library is visited again"
            );
            Vec::new()
        }
    }
}

/// What one backfill job run did.
#[derive(Debug)]
pub struct BackfillJobRun {
    /// The job.
    pub job_id: String,
    /// Where the job stands now: completed, or pending (suspended at quit, or after a
    /// library that could not be backfilled, to resume at the next launch).
    pub state: JobState,
    /// The libraries this run visited, in order.
    pub ran: Vec<LibraryBackfill>,
}

/// Run the history backfill as a durable job: resume the unfinished backfill job among
/// `recovered` (or submit one), revalidate its `source`, then backfill every library its
/// checkpoint does not list, checkpointing after each. When `stop` is set (quit) the job is
/// suspended with its checkpoint; a library that could not be backfilled leaves the job
/// suspended too, so the next launch retries it. Filesystem, database and journal I/O.
///
/// # Errors
/// The job could not be submitted, started or written; each refusal is journaled.
pub fn run_backfill_job(
    jobs: &Jobs,
    recovered: &[Job],
    catalog: &SqliteCatalog,
    backfill: &Backfill,
    source: &str,
    stop: &AtomicBool,
) -> Result<BackfillJobRun, JobError> {
    let mut running = start_backfill_job(jobs, recovered, source, stop)?;
    let job_id = running.job().id.clone();
    let (ran, progress) = backfill_remaining(&mut running, catalog, backfill, stop)?;
    let complete = !stop.load(Ordering::SeqCst) && ran.iter().all(library_finished);
    let state = match complete {
        true => running.complete().map(|()| JobState::Completed)?,
        false => checkpoint_and_suspend(running, &progress)?,
    };
    Ok(BackfillJobRun { job_id, state, ran })
}

/// What a backfill job has done so far: the libraries it completed and how many it serves.
struct Progress {
    done: Vec<LibraryId>,
    total: u64,
}

/// Write the checkpoint of `progress` and suspend the job with it (checkpoint on quit).
fn checkpoint_and_suspend(
    mut running: RunningJob,
    progress: &Progress,
) -> Result<JobState, JobError> {
    let text = backfill_checkpoint(&progress.done);
    running.checkpoint(&text, progress.done.len() as u64, Some(progress.total))?;
    running.suspend().map(|()| JobState::Pending)
}

/// Start the unfinished backfill job among `recovered`, or a new one; a job whose source
/// changed is failed by the service and replaced by a new one.
fn start_backfill_job(
    jobs: &Jobs,
    recovered: &[Job],
    source: &str,
    stop: &AtomicBool,
) -> Result<RunningJob, JobError> {
    let found = recovered.iter().find(|job| job.kind == BACKFILL_OPERATION);
    let job = match found {
        Some(job) => job.clone(),
        None => submit_backfill(jobs, source)?,
    };
    match start_when_free(jobs, &job, source, stop) {
        Err(JobError::SourceChanged) => {
            let fresh = submit_backfill(jobs, source)?;
            start_when_free(jobs, &fresh, source, stop)
        }
        started => started,
    }
}

/// Submit a new backfill job for `source` under a fresh operation ID.
fn submit_backfill(jobs: &Jobs, source: &str) -> Result<Job, JobError> {
    let operation_id = mint_uuid_v4().map_err(|err| JobError::Unavailable(err.to_string()))?;
    jobs.submit(JobSpec {
        kind: BACKFILL_OPERATION.into(),
        operation_id,
        source: source.to_string(),
    })
}

/// Start `job`, first waiting out (bounded by the lease length, and by `stop`) a lease that
/// a crashed run left, then trying once more should the lease still be held.
fn start_when_free(
    jobs: &Jobs,
    job: &Job,
    source: &str,
    stop: &AtomicBool,
) -> Result<RunningJob, JobError> {
    let now = filesquid_api::journal::now_ms();
    let left = job
        .lease_expires_ms
        .map_or(0, |end| end.saturating_sub(now));
    wait_unless_stopped(left.clamp(0, jobs.lease_ttl_ms()), stop);
    match jobs.start(&job.id, source) {
        Err(JobError::Conflict("lease_held")) => {
            wait_unless_stopped(jobs.lease_ttl_ms(), stop);
            jobs.start(&job.id, source)
        }
        started => started,
    }
}

/// Sleep up to `ms` milliseconds in [`LEASE_WAIT_STEP`]s, returning early once `stop` is set.
fn wait_unless_stopped(ms: i64, stop: &AtomicBool) {
    let step = LEASE_WAIT_STEP.as_millis() as i64;
    let steps = (ms.max(0) + step - 1) / step;
    for _ in 0..steps {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(LEASE_WAIT_STEP);
    }
}

/// Backfill every library the job's checkpoint does not list, until `stop` is set,
/// checkpointing after each library that finished; answer the libraries visited and the
/// progress reached.
fn backfill_remaining(
    running: &mut RunningJob,
    catalog: &SqliteCatalog,
    backfill: &Backfill,
    stop: &AtomicBool,
) -> Result<(Vec<LibraryBackfill>, Progress), JobError> {
    let mut done = done_libraries(running.checkpoint_text());
    let libraries = catalog
        .library_ids()
        .map_err(|_| JobError::Unavailable("the libraries could not be listed".into()))?;
    let total = libraries.len() as u64;
    let mut ran = Vec::new();
    for library in libraries {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if done.iter().any(|id| id == library.as_str()) {
            continue;
        }
        let result = catalog.backfill_library(library.clone(), backfill, stop);
        if library_finished(&result) {
            done.push(library.as_str().to_string());
            let ids = done_ids(&done);
            running.checkpoint(&backfill_checkpoint(&ids), ids.len() as u64, Some(total))?;
        }
        ran.push(result);
    }
    let progress = Progress {
        done: done_ids(&done),
        total,
    };
    Ok((ran, progress))
}

/// Whether a library's backfill ran to its end: no run failed to start and none was
/// cancelled.
fn library_finished(result: &LibraryBackfill) -> bool {
    result.error.is_none() && !result.reports.iter().any(|report| report.cancelled)
}

/// The library IDs among `done` (an ID that no longer parses is dropped).
fn done_ids(done: &[String]) -> Vec<LibraryId> {
    done.iter()
        .filter_map(|id| LibraryId::new(id.as_str()).ok())
        .collect()
}
