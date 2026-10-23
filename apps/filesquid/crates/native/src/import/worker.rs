//! The import worker of the desktop (Task DE-1.2; NFR-003, DE-S-014, DE-S-015): one thread
//! that runs import jobs one at a time, each from its plan: first the jobs the import route
//! queued, in the order queued (at most [`MAX_QUEUED_IMPORTS`] waiting; a full queue refuses
//! another), and, whenever none is queued, the unfinished jobs a quit or a crash left, paged
//! from the plans [`RECOVERY_PAGE`] at a time until every one was tried once this launch.
//! Setting the stop flag (quit) suspends the running job with its checkpoint and ends the
//! thread; the next launch tries every unfinished job again.

use super::host::Build;
use super::job::{run_import_job, ImportContext};
use super::plans::SqliteImportPlans;
use crate::HostFiles;
use filesquid_api::jobs::{JobError, Jobs};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Most import jobs waiting at once.
pub const MAX_QUEUED_IMPORTS: usize = 64;

/// Unfinished jobs read from the plans at a time.
pub const RECOVERY_PAGE: usize = 64;

/// Failed reads of the plans after which this launch stops looking for unfinished jobs.
const MAX_RECOVERY_FAILURES: u32 = 5;

/// Most job IDs remembered as run this launch, so recovery does not run a queued job again.
const MAX_RUN_IDS: usize = 100_000;

/// Journal operation of an import job that could not run.
pub const IMPORT_JOB_OPERATION: &str = "import.job";

/// How often the idle worker checks the stop flag.
const POLL: Duration = Duration::from_millis(100);

/// What the worker runs jobs with.
pub struct ImportServices {
    /// The job service.
    pub jobs: Jobs,
    /// The file service the libraries are open in.
    pub files: HostFiles,
    /// The library index each imported file is registered in.
    pub index: Arc<dyn super::index::ImportIndex>,
    /// Each job's plan.
    pub plans: Arc<SqliteImportPlans>,
    /// The host's journal.
    pub journal: JournalSender,
    /// This FileSquid build.
    pub build: Build,
    /// Who imports.
    pub actor: String,
    /// The folder whose volume holds the destination's staging.
    pub staging_dir: PathBuf,
    /// Set at quit.
    pub stop: Arc<AtomicBool>,
    /// The first retry's wait.
    pub retry_unit: Duration,
    /// Whether to run the unfinished jobs a quit or a crash left: true once the launch
    /// released the leases of crashed runs, so none is refused as still leased.
    pub recover: bool,
}

/// The import worker: its queue and its thread.
pub struct ImportWorker {
    sender: SyncSender<Queued>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for ImportWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportWorker").finish_non_exhaustive()
    }
}

/// A job the import route queued: its ID and its source identity, so a job whose plan is
/// missing can still be failed.
struct Queued {
    id: String,
    source: String,
}

impl ImportWorker {
    /// Start the worker thread over `services`, with its own current-thread runtime. Starts a
    /// thread; the thread looks for unfinished jobs as soon as it is idle.
    ///
    /// # Errors
    /// No runtime or no thread could be started; no job is queued then.
    pub fn spawn(services: ImportServices) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|err| format!("the import worker's runtime could not be built: {err}"))?;
        let (sender, receiver) = sync_channel(MAX_QUEUED_IMPORTS);
        let thread = std::thread::Builder::new()
            .name("filesquid-import".into())
            .spawn(move || worker_loop(&services, &runtime, &receiver))
            .map_err(|err| format!("the import worker could not be started: {err}"))?;
        Ok(Self {
            sender,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Queue the import job `job_id`, whose source identity is `source`.
    ///
    /// # Errors
    /// `queue_full` when [`MAX_QUEUED_IMPORTS`] jobs wait; `unavailable` when the worker ended.
    pub fn enqueue(&self, job_id: &str, source: &str) -> Result<(), &'static str> {
        let queued = Queued {
            id: job_id.to_string(),
            source: source.to_string(),
        };
        match self.sender.try_send(queued) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("queue_full"),
            Err(TrySendError::Disconnected(_)) => Err("unavailable"),
        }
    }

    /// Wait, at most `limit`, for the thread to end after the stop flag was set. A thread
    /// still running then is logged and left to end with the process.
    pub fn wait(&self, limit: Duration) {
        let handle = self.thread.lock().ok().and_then(|mut slot| slot.take());
        let Some(handle) = handle else {
            return;
        };
        let deadline = Instant::now() + limit;
        while !handle.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !handle.is_finished() || handle.join().is_err() {
            tracing::warn!(
                operation = IMPORT_JOB_OPERATION,
                error_code = "stop_timeout",
                "the import worker did not stop in time"
            );
        }
    }
}

/// The recovery of one launch: where the paging stands, how often it failed, and the jobs
/// already run.
#[derive(Default)]
struct Recovery {
    after: Option<String>,
    done: bool,
    failures: u32,
    ran: HashSet<String>,
}

impl Recovery {
    /// Remember that `job_id` ran this launch (up to [`MAX_RUN_IDS`]).
    fn note_ran(&mut self, job_id: &str) {
        if self.ran.len() < MAX_RUN_IDS {
            self.ran.insert(job_id.to_string());
        }
    }
}

/// Run queued jobs, and unfinished ones whenever none is queued, until the stop flag is set
/// or the queue closes.
fn worker_loop(
    services: &ImportServices,
    runtime: &tokio::runtime::Runtime,
    receiver: &Receiver<Queued>,
) {
    let mut recovery = Recovery {
        done: !services.recover,
        ..Recovery::default()
    };
    while !services.stop.load(Ordering::SeqCst) {
        match receiver.recv_timeout(POLL) {
            Ok(queued) => {
                recovery.note_ran(&queued.id);
                run_one(services, runtime, &queued.id, Some(&queued.source));
            }
            Err(RecvTimeoutError::Timeout) => recover_page(services, runtime, &mut recovery),
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Run the next page of unfinished jobs not run yet this launch; a failed read of the plans
/// is journaled, and after [`MAX_RECOVERY_FAILURES`] this launch stops looking.
fn recover_page(
    services: &ImportServices,
    runtime: &tokio::runtime::Runtime,
    recovery: &mut Recovery,
) {
    if recovery.done {
        return;
    }
    let page = match services
        .plans
        .unfinished_after(recovery.after.as_deref(), RECOVERY_PAGE)
    {
        Ok(page) => page,
        Err(_) => {
            recovery.failures += 1;
            recovery.done = recovery.failures >= MAX_RECOVERY_FAILURES;
            return journal_failure(services, None, "recovery_unavailable");
        }
    };
    recovery.done = page.is_empty();
    recovery.after = page.last().cloned().or(recovery.after.take());
    for job_id in page {
        if services.stop.load(Ordering::SeqCst) {
            return;
        }
        if !recovery.ran.contains(&job_id) {
            recovery.note_ran(&job_id);
            run_one(services, runtime, &job_id, None);
        }
    }
}

/// Run the job `job_id` from its plan; a job without a plan is failed (when its `source`
/// is known) and journaled, and a job that cannot run is journaled with its code only.
fn run_one(
    services: &ImportServices,
    runtime: &tokio::runtime::Runtime,
    job_id: &str,
    source: Option<&str>,
) {
    let plan = match services.plans.load(job_id) {
        Ok(Some(plan)) => plan,
        Ok(None) => return fail_without_plan(services, job_id, source),
        Err(_) => return journal_failure(services, Some(job_id), "unavailable"),
    };
    let context = ImportContext {
        jobs: &services.jobs,
        files: &services.files,
        index: services.index.as_ref(),
        plans: &services.plans,
        journal: &services.journal,
        build: &services.build,
        actor: &services.actor,
        staging_dir: &services.staging_dir,
        stop: &services.stop,
        retry_unit: services.retry_unit,
    };
    let Err(err) = runtime.block_on(run_import_job(&context, job_id, &plan)) else {
        return;
    };
    journal_failure(services, Some(job_id), err.error_code());
    let finished = matches!(
        err,
        JobError::NotFound | JobError::SourceChanged | JobError::Conflict("job_finished")
    );
    if finished && services.plans.finish(job_id).is_err() {
        journal_failure(services, Some(job_id), "unavailable");
    }
}

/// Fail the job `job_id`, whose plan is missing, so it is not tried at every launch: start it
/// against its `source` and fail it with `plan_missing`; journal the failure either way.
fn fail_without_plan(services: &ImportServices, job_id: &str, source: Option<&str>) {
    let failed = source
        .ok_or(JobError::NotFound)
        .and_then(|source| services.jobs.start(job_id, source))
        .and_then(|running| running.fail("plan_missing"));
    if failed.is_err() {
        tracing::warn!(
            operation = IMPORT_JOB_OPERATION,
            job_id,
            error_code = "plan_missing",
            "an import job without a plan could not be failed"
        );
    }
    journal_failure(services, Some(job_id), "plan_missing");
}

/// Journal an import job that could not run.
fn journal_failure(services: &ImportServices, job_id: Option<&str>, code: &str) {
    let mut entry = JournalEntry::new(IMPORT_JOB_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(services.actor.as_str())
        .error_code(code);
    if let Some(job_id) = job_id {
        entry = entry.target(job_id);
    }
    services.journal.send(entry);
}
