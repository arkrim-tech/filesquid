//! The rebuild from files as a durable job (Task API-F2.17; API-S-025 **Rebuild as a job**,
//! owner answer 9 of 2026-10-22, verbatim: "Yes, rebuild should be a durable job";
//! API-S-033, API-S-037).
//!
//! `POST /api/v1/libraries/rebuilds` submits one job of kind [`REBUILD_JOB_KIND`] per library
//! ([`submit_rebuild`]; a library whose rebuild is unfinished answers that job) and hands it
//! to the runner the host installed for the kind. [`RebuildRunner`] is that runner: it leases
//! the job, reads the library's objects through the host's [`LibraryOperations`] in
//! object-root order and, every batch of objects, stores in one atomic step a checkpoint (the
//! root of the last object read, a cursor of a few bytes) and the report so far as the job's
//! stored result. A pause or cancel asked through the job routes is seen at the next
//! checkpoint (or lease renewal), where the run stops: a paused job stays pending with its
//! checkpoint, a cancelled one ends as cancelled. A resumed run, and a run after a crash
//! whose lease recovery released, continue after the cursor, so no checkpointed object is
//! read again. A finished run completes the job with the whole report as its result, which
//! the job view carries once the job has completed.
//!
//! The job's source identity is `library:<library-id>`; it is stored, never journaled or
//! answered, like the checkpoint and its object root (API-S-005, API-S-033). A rebuild writes
//! nothing to the library's files, so a cancel changes none of them.

use super::error::LocalLibraryError;
use super::locations::LibraryRequest;
use super::operations::LibraryOperations;
use super::rebuild::{RebuildEnd, RebuildEntry, RebuildProgress, RebuildReport, RebuildStep};
use crate::jobs::{
    Halt, Job, JobError, JobRunner, JobSpec, JobState, JobView, Jobs, RunningJob,
    MAX_RECOVERED_JOBS,
};
use crate::library::mint_uuid_v4;
use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// The job kind of a rebuild from files.
pub const REBUILD_JOB_KIND: &str = "library.rebuild";

/// Objects read between two checkpoints by default.
pub const REBUILD_BATCH: usize = 64;

/// Most passes one run makes (AGENTS.md §22): a pass ends where the run stopped for a request
/// that a resume then cleared before the run gave its lease back, and the run goes on.
const MAX_RUN_PASSES: usize = 1_000;

/// Error code of a job whose source identity names no library.
const INVALID_SOURCE: &str = "invalid_source";

/// Serialises [`submit_rebuild`], so two requests cannot both find no unfinished job of one
/// library and submit two.
static SUBMITTING: Mutex<()> = Mutex::new(());

/// The answer of `POST /api/v1/libraries/rebuilds`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildJobAnswer {
    /// Always `pending`: the rebuild's work is durable and not yet complete.
    pub outcome: RebuildOutcome,
    /// The job's ID.
    pub job_id: String,
    /// The job as the job routes show it.
    pub job: JobView,
    /// Whether the job was handed to the host's runner to run now; false leaves it pending
    /// (paused, already running, or on a host without a runner for the kind).
    pub scheduled: bool,
}

/// What became of a rebuild request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildOutcome {
    /// The rebuild is a durable job that has not finished.
    Pending,
}

/// The source identity of the rebuild job of `library`. Pure.
pub fn rebuild_source(library: &LibraryId) -> String {
    format!("library:{}", library.as_str())
}

/// The library the source identity of `job` names, if any. Pure.
fn library_of(job: &Job) -> Option<LibraryId> {
    let id = job.source.strip_prefix("library:")?;
    LibraryId::new(id).ok()
}

/// The unfinished rebuild job of `library`, or a new pending one (store I/O; `job.created` is
/// journaled for a new job).
///
/// # Errors
/// A store failure, or [`JobError::Unavailable`] when no ID can be minted.
pub fn submit_rebuild(jobs: &Jobs, library: &LibraryId) -> Result<Job, JobError> {
    let _one = SUBMITTING.lock().unwrap_or_else(PoisonError::into_inner);
    let source = rebuild_source(library);
    let open = jobs.unfinished_of(REBUILD_JOB_KIND, MAX_RECOVERED_JOBS)?;
    if let Some(job) = open.into_iter().find(|job| job.source == source) {
        return Ok(job);
    }
    let operation_id = mint_uuid_v4().map_err(|err| JobError::Unavailable(err.to_string()))?;
    jobs.submit(JobSpec {
        kind: REBUILD_JOB_KIND.to_string(),
        operation_id,
        source,
    })
}

/// A rebuild job's checkpoint: the root of the last object read. Never answered or
/// journaled.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RebuildCheckpoint {
    after: String,
}

/// The host's runner of rebuild jobs: install it with
/// [`crate::ApiState::install_job_runner`] for [`REBUILD_JOB_KIND`].
#[derive(Clone)]
pub struct RebuildRunner {
    jobs: Jobs,
    libraries: Arc<dyn LibraryOperations>,
    actor: String,
    batch: usize,
    renewal: Option<Duration>,
    runtime: Option<tokio::runtime::Handle>,
}

impl std::fmt::Debug for RebuildRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RebuildRunner")
            .field("batch", &self.batch)
            .finish_non_exhaustive()
    }
}

impl RebuildRunner {
    /// A runner of rebuild jobs through `jobs`, reading libraries through `libraries` as
    /// `actor` (the host's owner), checkpointing every [`REBUILD_BATCH`] objects.
    pub fn new(
        jobs: Jobs,
        libraries: Arc<dyn LibraryOperations>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            jobs,
            libraries,
            actor: actor.into(),
            batch: REBUILD_BATCH,
            renewal: None,
            runtime: None,
        }
    }

    /// Run resumed jobs on `runtime`, the host's long-lived runtime (default: the runtime of
    /// the call that hands the job over).
    #[must_use]
    pub fn with_runtime(mut self, runtime: tokio::runtime::Handle) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Checkpoint every `batch` objects (at least one).
    #[must_use]
    pub fn with_batch(mut self, batch: usize) -> Self {
        self.batch = batch.max(1);
        self
    }

    /// Renew the lease every `every` (at least a millisecond) while objects are read
    /// (default: a third of the lease).
    #[must_use]
    pub fn with_renewal(mut self, every: Duration) -> Self {
        self.renewal = Some(every.max(Duration::from_millis(1)));
        self
    }

    /// How often the lease is renewed while objects are read.
    fn renewal(&self) -> Duration {
        let third = u64::try_from(self.jobs.lease_ttl_ms() / 3).unwrap_or(1);
        self.renewal
            .unwrap_or_else(|| Duration::from_millis(third.max(1)))
    }

    /// Run rebuild job `job_id` from its checkpoint until it completes, fails, or stops for a
    /// pause or cancel; answer the state it left the job in (store, storage and journal I/O).
    ///
    /// # Errors
    /// The refusals of [`Jobs::start`] (for example `job_paused`, `job_finished`,
    /// `lease_held`), [`JobError::Fenced`] when the run lost its lease, or a store failure.
    pub async fn run(&self, job_id: &str) -> Result<JobState, JobError> {
        let job = self.jobs.job(job_id)?;
        let run = self.jobs.start(job_id, &job.source)?;
        let Some(library) = library_of(&job) else {
            run.fail(INVALID_SOURCE)?;
            return Ok(JobState::Failed);
        };
        let mut tracker = Tracker::resume(run.job(), self.batch);
        self.drive(run, &library, &mut tracker).await
    }

    /// Run passes until the job ends or stops for a request; at the pass bound, give the
    /// lease back with the checkpoint kept.
    async fn drive(
        &self,
        mut run: RunningJob,
        library: &LibraryId,
        tracker: &mut Tracker,
    ) -> Result<JobState, JobError> {
        for _ in 0..MAX_RUN_PASSES {
            let (returned, end) = self.pass(run, library, tracker).await?;
            if let Some(err) = tracker.refused.take() {
                return Err(err);
            }
            run = match end {
                Ok(RebuildEnd::Finished { total, journal }) => {
                    return tracker.complete(returned, total, journal);
                }
                Ok(RebuildEnd::Stopped) => match returned.halt_if_requested()? {
                    Halt::Halted(state) => return Ok(state),
                    Halt::Continued(continued) => *continued,
                },
                Err(err) => {
                    returned.fail(err.error_code())?;
                    return Ok(JobState::Failed);
                }
            };
        }
        tracing::warn!(
            error_code = "pass_limit",
            "a rebuild run reached its pass bound"
        );
        run.suspend()
    }

    /// One pass: read the library's objects after the cursor while a ticker renews the lease;
    /// answer the run back with how the rebuild ended.
    ///
    /// # Errors
    /// [`JobError::Failed`] when the run is still shared after its ticker stopped.
    async fn pass(
        &self,
        run: RunningJob,
        library: &LibraryId,
        tracker: &mut Tracker,
    ) -> Result<(RunningJob, Result<RebuildEnd, LocalLibraryError>), JobError> {
        let shared = Arc::new(Mutex::new(run));
        let mut ticker = Renewal::start(shared.clone(), self.renewal());
        let request = LibraryRequest {
            library_id: library.clone(),
        };
        let after = tracker.after.clone();
        let mut progress = JobProgress {
            run: &shared,
            tracker,
        };
        let end = self
            .libraries
            .rebuild_from(self.actor.clone(), request, after, &mut progress)
            .await;
        ticker.stop().await;
        let run = Arc::into_inner(shared)
            .ok_or_else(|| JobError::Failed("a rebuild run is still shared".into()))?;
        Ok((
            run.into_inner().unwrap_or_else(PoisonError::into_inner),
            end,
        ))
    }

    /// Run job `job_id` and log, to stdout only, why it did not run (the job service has
    /// journaled every refusal and failure).
    async fn run_logged(&self, job_id: &str) {
        if let Err(err) = self.run(job_id).await {
            tracing::warn!(error_code = err.error_code(), "a rebuild job did not run");
        }
    }
}

impl JobRunner for RebuildRunner {
    /// Spawn the run on the runner's runtime, else the current one, and return at once.
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        let runtime = match &self.runtime {
            Some(runtime) => runtime.clone(),
            None => tokio::runtime::Handle::try_current()
                .map_err(|err| JobError::Unavailable(err.to_string()))?,
        };
        let (runner, id) = (self.clone(), job.id.clone());
        runtime.spawn(async move { runner.run_logged(&id).await });
        Ok(())
    }
}

/// Lock the shared run.
fn lock(run: &Mutex<RunningJob>) -> MutexGuard<'_, RunningJob> {
    run.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The task that renews a pass's lease, aborted when the pass ends or is dropped (a crash
/// drops it, and its lease then ends).
struct Renewal(tokio::task::JoinHandle<()>);

impl Renewal {
    /// Renew `run`'s lease every `every` until a renewal is refused or the task is aborted.
    fn start(run: Arc<Mutex<RunningJob>>, every: Duration) -> Self {
        Self(tokio::spawn(async move {
            let mut ticks = tokio::time::interval(every);
            ticks.tick().await;
            loop {
                ticks.tick().await;
                if lock(&run).renew().is_err() {
                    return;
                }
            }
        }))
    }

    /// Abort the task and wait for it to end, so it no longer holds the run.
    async fn stop(&mut self) {
        self.0.abort();
        if let Err(err) = (&mut self.0).await {
            if err.is_panic() {
                tracing::error!(
                    error_code = "panicked",
                    "a rebuild's lease renewal panicked"
                );
            }
        }
    }
}

impl Drop for Renewal {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// What a run has read: the cursor, the report so far and the progress.
struct Tracker {
    after: Option<String>,
    report: RebuildReport,
    done: u64,
    total: Option<u64>,
    unsaved: usize,
    batch: usize,
    /// Why the last checkpoint was refused, which ends the run.
    refused: Option<JobError>,
}

impl Tracker {
    /// The state `job` was checkpointed in: its cursor and the stored report so far, or the
    /// start when it has no checkpoint or its stored state cannot be read. Pure but for logs.
    fn resume(job: &Job, batch: usize) -> Self {
        let mut tracker = Self {
            after: None,
            report: RebuildReport {
                objects: Vec::new(),
                unreadable: Vec::new(),
                journal: None,
            },
            done: job.done,
            total: job.total,
            unsaved: 0,
            batch,
            refused: None,
        };
        let (Some(checkpoint), Some(result)) = (&job.checkpoint, &job.result) else {
            tracker.done = 0;
            return tracker;
        };
        let cursor = serde_json::from_str::<RebuildCheckpoint>(checkpoint);
        let report = serde_json::from_str::<RebuildReport>(result);
        match (cursor, report) {
            (Ok(cursor), Ok(report)) => {
                tracker.after = Some(cursor.after);
                tracker.report = report;
            }
            _ => {
                tracing::warn!(
                    error_code = "checkpoint_unreadable",
                    "a rebuild starts over"
                );
                tracker.done = 0;
            }
        }
        tracker
    }

    /// Record `step`.
    fn add(&mut self, step: RebuildStep<'_>) {
        match step.entry {
            RebuildEntry::Read(object) => self.report.objects.push(object),
            RebuildEntry::Unreadable(item) => self.report.unreadable.push(item),
        }
        self.after = Some(step.root.to_string());
        self.done = step.done;
        self.total = Some(step.total);
        self.unsaved += 1;
    }

    /// Store the cursor and the report so far under `run`'s lease, in one step (store I/O).
    ///
    /// # Errors
    /// The refusals of [`RunningJob::checkpoint_with_result`].
    fn save(&mut self, run: &mut RunningJob) -> Result<(), JobError> {
        let checkpoint = RebuildCheckpoint {
            after: self.after.clone().unwrap_or_default(),
        };
        let checkpoint = encode(&checkpoint)?;
        let report = encode(&self.report)?;
        run.checkpoint_with_result(&checkpoint, &report, self.done, self.total)?;
        self.unsaved = 0;
        Ok(())
    }

    /// Complete `run` with the whole report, after a last checkpoint of `total` of `total`
    /// objects (store I/O).
    fn complete(
        &mut self,
        mut run: RunningJob,
        total: u64,
        journal: Option<super::JournalRebuild>,
    ) -> Result<JobState, JobError> {
        self.report.journal = journal;
        (self.done, self.total) = (total, Some(total));
        self.save(&mut run)?;
        run.complete_with_result(&encode(&self.report)?)?;
        Ok(JobState::Completed)
    }
}

/// `value` as JSON text.
///
/// # Errors
/// [`JobError::Failed`] when it cannot be encoded.
fn encode<T: Serialize>(value: &T) -> Result<String, JobError> {
    serde_json::to_string(value).map_err(|err| JobError::Failed(err.to_string()))
}

/// The progress of one pass: records each object, checkpoints every batch, and stops the
/// rebuild at a checkpoint once a pause or cancel is asked of the job (or a checkpoint is
/// refused).
struct JobProgress<'a> {
    run: &'a Mutex<RunningJob>,
    tracker: &'a mut Tracker,
}

impl RebuildProgress for JobProgress<'_> {
    fn object(&mut self, step: RebuildStep<'_>) -> ControlFlow<()> {
        self.tracker.add(step);
        let mut run = lock(self.run);
        if run.requested().is_none() && self.tracker.unsaved < self.tracker.batch {
            return ControlFlow::Continue(());
        }
        if let Err(err) = self.tracker.save(&mut run) {
            self.tracker.refused = Some(err);
            return ControlFlow::Break(());
        }
        match run.requested() {
            Some(_) => ControlFlow::Break(()),
            None => ControlFlow::Continue(()),
        }
    }
}
