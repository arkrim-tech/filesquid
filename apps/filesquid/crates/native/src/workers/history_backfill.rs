//! Durable jobs on the desktop and the history backfill job worker (Task DE-0.4, moved here
//! from the desktop host by Task DE-0.8; DE-S-014, DE-S-015, API-S-033; NFR-003, ADR 007).
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

use crate::{HostMonitor, LibraryBackfill, SqliteCatalog, SqliteJobs, JOURNAL_FILE};
use filesquid_api::history::backfill::BackfillReport;
use filesquid_api::history::backfill::{Backfill, BACKFILL_OPERATION};
use filesquid_api::jobs::{Job, JobClass, JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::library::mint_uuid_v4;
use filesquid_core::{InstallationId, LibraryId};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Step of the bounded wait for a lease a crashed run left behind.
const LEASE_WAIT_STEP: Duration = Duration::from_millis(100);

/// Open the job store in `data_dir` and the job service of this launch over `host`'s
/// journal queue and process registry, leasing as a holder named for this launch only, with
/// `import` jobs classed as transfers, so at most API-S-037's default of 4 run at once (Task
/// DE-1.9, DE-S-020). Filesystem and database I/O.
///
/// # Errors
/// The store could not be opened or no holder ID could be minted; the message names no
/// path.
pub fn open_jobs(data_dir: &Path, host: &HostMonitor) -> Result<Jobs, String> {
    let store = SqliteJobs::open(&data_dir.join(JOURNAL_FILE))
        .map_err(|err| format!("the job store could not be opened ({})", err.error_code()))?;
    let launch = mint_uuid_v4().map_err(|err| err.to_string())?;
    let jobs = Jobs::new(
        Arc::new(store),
        host.monitor().journal(),
        host.monitor().registry().clone(),
        format!("filesquid-desktop-{launch}"),
    );
    Ok(jobs.with_class(crate::import::job::IMPORT_KIND, JobClass::Transfer))
}

/// The launch-time reconciliation: release the leases of runs a crash left and answer the
/// unfinished jobs (bounded by the API). Database I/O.
///
/// # Errors
/// A store failure; it is logged as an error (the journal layer journals it) and the host
/// schedules no job this launch rather than submit one that may already exist.
pub fn recover_jobs(jobs: &Jobs) -> Result<Vec<Job>, JobError> {
    jobs.recover().inspect_err(|err| {
        tracing::error!(
            operation = "job.recover",
            error_code = err.error_code(),
            "the durable jobs could not be recovered; no job is scheduled this launch"
        );
    })
}

/// The source identity of a backfill job: the installation whose records it writes. Pure.
pub fn backfill_source(installation: &InstallationId) -> String {
    format!("installation:{}", installation.as_str())
}

/// The checkpoint text of a backfill that completed the libraries `done`:
/// `{"done": ["<library-id>", ...]}`. Pure.
pub fn backfill_checkpoint(done: &[LibraryId]) -> String {
    let ids: Vec<&str> = done.iter().map(LibraryId::as_str).collect();
    serde_json::json!({ "done": ids }).to_string()
}

/// The library IDs a checkpoint's JSON lists under `done`, if it is a checkpoint. Pure.
fn checkpoint_done(value: &serde_json::Value) -> Option<Vec<String>> {
    let done = value.get("done")?.as_array()?;
    done.iter()
        .map(|id| id.as_str().map(str::to_string))
        .collect()
}

/// The libraries a checkpoint says are complete. An unreadable checkpoint is logged and
/// counts as none, so every library is visited again (each skips the versions it recorded).
fn done_libraries(checkpoint: Option<&str>) -> Vec<String> {
    let Some(text) = checkpoint else {
        return Vec::new();
    };
    let parsed = serde_json::from_str::<serde_json::Value>(text).ok();
    match parsed.as_ref().and_then(checkpoint_done) {
        Some(done) => done,
        None => {
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
    /// Where the job stands now: completed; pending (suspended at quit, at a pause request,
    /// or after a library that could not be backfilled to its end, to resume at the next
    /// launch or when resumed); or cancelled (a cancel request, API-S-037).
    pub state: JobState,
    /// The libraries this run visited, in order.
    pub ran: Vec<LibraryBackfill>,
}

/// Run the history backfill as a durable job: resume the unfinished backfill job (or submit
/// one), revalidate its `source`, then backfill every library its checkpoint does not list,
/// renewing the lease while each library runs and checkpointing after each that finished.
/// When `stop` is set (quit), or the job routes ask the job to pause or cancel, the job
/// writes its checkpoint and is suspended (a cancel request ends it cancelled); a library that
/// did not finish, or a failure while the run still holds its lease, suspends it too, so the
/// next launch resumes it. Filesystem, database and journal I/O.
///
/// # Errors
/// The job could not be submitted, started or written, or its lease was lost; each refusal
/// is journaled.
pub fn run_backfill_job(
    jobs: &Jobs,
    catalog: &SqliteCatalog,
    backfill: &Backfill,
    source: &str,
    stop: &AtomicBool,
) -> Result<BackfillJobRun, JobError> {
    let mut running = start_backfill_job(jobs, source, stop)?;
    let job_id = running.job().id.clone();
    let ttl = jobs.lease_ttl_ms();
    let (ran, progress) = match backfill_remaining(&mut running, ttl, catalog, backfill, stop) {
        Ok(done) => done,
        Err(err) => return Err(suspend_after(running, err)),
    };
    let complete = !halted(stop, &running) && ran.iter().all(library_finished);
    let state = match complete {
        true => running.complete().map(|()| JobState::Completed)?,
        false => checkpoint_and_suspend(running, &progress)?,
    };
    Ok(BackfillJobRun { job_id, state, ran })
}

/// Whether the run must stop: the host is quitting (`stop`), or the job routes asked the job
/// to pause or cancel (seen at the last checkpoint or renewal, API-S-037). The run then
/// checkpoints and suspends, which keeps a paused job pending and cancels the other.
fn halted(stop: &AtomicBool, running: &RunningJob) -> bool {
    stop.load(Ordering::SeqCst) || running.requested().is_some()
}

/// Suspend a run that failed with `err` while it may still hold its lease (a lost lease
/// cannot be suspended), and answer `err`.
fn suspend_after(running: RunningJob, err: JobError) -> JobError {
    if err != JobError::Fenced {
        if let Err(suspend) = running.suspend() {
            tracing::warn!(
                operation = BACKFILL_OPERATION,
                error_code = suspend.error_code(),
                "a failed backfill run could not be suspended; its lease expires instead"
            );
        }
    }
    err
}

/// What a backfill job has done so far: the libraries it completed and how many it serves.
struct Progress {
    done: Vec<LibraryId>,
    total: u64,
}

/// Write the checkpoint of `progress` and suspend the job with it (checkpoint on quit, or
/// on a pause or cancel request), answering the state the suspension stored: pending, or
/// cancelled under a cancel request.
fn checkpoint_and_suspend(
    mut running: RunningJob,
    progress: &Progress,
) -> Result<JobState, JobError> {
    let text = backfill_checkpoint(&progress.done);
    running.checkpoint(&text, progress.done.len() as u64, Some(progress.total))?;
    running.suspend()
}

/// Start the unfinished backfill job, found by kind whatever else waits, or a new one; a job
/// whose source changed is failed by the service and replaced by a new one.
fn start_backfill_job(
    jobs: &Jobs,
    source: &str,
    stop: &AtomicBool,
) -> Result<RunningJob, JobError> {
    let found = jobs
        .unfinished_of(BACKFILL_OPERATION, 1)?
        .into_iter()
        .next();
    let job = match found {
        Some(job) => job,
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

/// Sleep `ms` milliseconds in [`LEASE_WAIT_STEP`]s, the last one only as long as what is
/// left, returning early once `stop` is set.
fn wait_unless_stopped(ms: i64, stop: &AtomicBool) {
    let total = Duration::from_millis(u64::try_from(ms.max(0)).unwrap_or(0));
    let deadline = Instant::now() + total;
    let steps = total.as_millis().div_ceil(LEASE_WAIT_STEP.as_millis());
    for _ in 0..steps {
        let left = deadline.saturating_duration_since(Instant::now());
        if stop.load(Ordering::SeqCst) || left.is_zero() {
            return;
        }
        std::thread::sleep(LEASE_WAIT_STEP.min(left));
    }
}

/// Backfill every library the job's checkpoint does not list, until `stop` is set or a
/// pause or cancel is asked of the job,
/// checkpointing after each library that finished; answer the libraries visited and the
/// progress reached.
///
/// # Errors
/// The libraries could not be listed, a checkpoint could not be written, or the lease was
/// lost while a library ran.
fn backfill_remaining(
    running: &mut RunningJob,
    ttl_ms: i64,
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
        if halted(stop, running) {
            break;
        }
        if done.iter().any(|id| id == library.as_str()) {
            continue;
        }
        let result = backfill_leased(running, ttl_ms, (catalog, backfill), library.clone(), stop)?;
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

/// Backfill `library` while a second thread renews the job's lease every third of its
/// length, so a long library never outlives the lease; the library's run is cancelled when
/// `stop` is set, a pause or cancel is asked of the job, or the lease is lost.
///
/// # Errors
/// The lease could not be renewed (the library's run was cancelled).
fn backfill_leased(
    running: &RunningJob,
    ttl_ms: i64,
    (catalog, backfill): (&SqliteCatalog, &Backfill),
    library: LibraryId,
    stop: &AtomicBool,
) -> Result<LibraryBackfill, JobError> {
    let cancel = AtomicBool::new(stop.load(Ordering::SeqCst));
    let finished = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let renewer =
            scope.spawn(|| renew_while_running(running, ttl_ms, stop, &cancel, &finished));
        let result = catalog.backfill_library(library, backfill, &cancel);
        finished.store(true, Ordering::SeqCst);
        let renewed = renewer
            .join()
            .unwrap_or_else(|_| Err(JobError::Failed("the lease renewer panicked".into())));
        renewed.map(|()| result)
    })
}

/// Renew `running`'s lease every third of `ttl_ms` until `finished` is set (the library's
/// run ended), passing `stop` and a pause or cancel request on to `cancel` and setting
/// `cancel` when the lease is lost.
///
/// # Errors
/// The lease could not be renewed.
fn renew_while_running(
    running: &RunningJob,
    ttl_ms: i64,
    stop: &AtomicBool,
    cancel: &AtomicBool,
    finished: &AtomicBool,
) -> Result<(), JobError> {
    let every = Duration::from_millis(u64::try_from((ttl_ms / 3).max(1)).unwrap_or(1));
    let mut renewed_at = Instant::now();
    while !finished.load(Ordering::SeqCst) {
        if halted(stop, running) {
            cancel.store(true, Ordering::SeqCst);
        }
        if renewed_at.elapsed() >= every {
            running
                .renew()
                .inspect_err(|_| cancel.store(true, Ordering::SeqCst))?;
            renewed_at = Instant::now();
        }
        std::thread::sleep(LEASE_WAIT_STEP.min(every));
    }
    Ok(())
}

/// Whether a library's backfill ran to its end: no run failed to start, and the last run
/// was neither cancelled nor capped and failed on no object. Anything else leaves the
/// library out of the checkpoint, so the next run visits it again.
fn library_finished(result: &LibraryBackfill) -> bool {
    let last = result.reports.last();
    result.error.is_none() && last.is_some_and(report_complete)
}

/// Whether one run covered everything it was given.
fn report_complete(report: &BackfillReport) -> bool {
    !report.cancelled && !report.reached_cap && report.failures.is_empty()
}

/// The library IDs among `done` (an ID that no longer parses is dropped).
fn done_ids(done: &[String]) -> Vec<LibraryId> {
    done.iter()
        .filter_map(|id| LibraryId::new(id.as_str()).ok())
        .collect()
}
