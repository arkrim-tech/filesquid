//! The import worker of the desktop (Task DE-1.2; NFR-003, DE-S-014, DE-S-015): one thread
//! that runs import jobs one at a time, in the order they were queued, each from its plan.
//! The queue holds at most [`MAX_QUEUED_IMPORTS`] jobs; a full queue refuses another. Setting
//! the stop flag (quit) suspends the running job with its checkpoint and ends the thread; the
//! next launch queues every unfinished import job again ([`ImportWorker::resume_unfinished`]).

use super::host::Build;
use super::job::{run_import_job, ImportContext, IMPORT_KIND};
use super::plans::SqliteImportPlans;
use crate::HostFiles;
use filesquid_api::jobs::Jobs;
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Most import jobs waiting at once.
pub const MAX_QUEUED_IMPORTS: usize = 64;

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
}

/// The import worker: its queue and its thread.
pub struct ImportWorker {
    sender: SyncSender<String>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for ImportWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportWorker").finish_non_exhaustive()
    }
}

impl ImportWorker {
    /// Start the worker thread over `services`. Starts a thread.
    ///
    /// # Errors
    /// No thread could be started.
    pub fn spawn(services: ImportServices) -> Result<Self, String> {
        let (sender, receiver) = sync_channel(MAX_QUEUED_IMPORTS);
        let thread = std::thread::Builder::new()
            .name("filesquid-import".into())
            .spawn(move || worker_loop(&services, &receiver))
            .map_err(|err| format!("the import worker could not be started: {err}"))?;
        Ok(Self {
            sender,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Queue the import job `job_id`.
    ///
    /// # Errors
    /// `queue_full` when [`MAX_QUEUED_IMPORTS`] jobs wait; `unavailable` when the worker ended.
    pub fn enqueue(&self, job_id: &str) -> Result<(), &'static str> {
        match self.sender.try_send(job_id.to_string()) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("queue_full"),
            Err(TrySendError::Disconnected(_)) => Err("unavailable"),
        }
    }

    /// Queue every unfinished import job of `jobs` (a launch after a quit or a crash), at most
    /// [`MAX_QUEUED_IMPORTS`]. Answers how many were queued. Reads the job store.
    pub fn resume_unfinished(&self, jobs: &Jobs) -> usize {
        let unfinished = jobs
            .unfinished_of(IMPORT_KIND, MAX_QUEUED_IMPORTS)
            .unwrap_or_default();
        unfinished
            .iter()
            .filter(|job| self.enqueue(&job.id).is_ok())
            .count()
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

/// Run queued jobs until the stop flag is set or the queue closes.
fn worker_loop(services: &ImportServices, receiver: &Receiver<String>) {
    let runtime = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime,
        Err(_) => {
            journal_failure(services, None, "runtime_unavailable");
            return;
        }
    };
    while !services.stop.load(Ordering::SeqCst) {
        match receiver.recv_timeout(POLL) {
            Ok(job_id) => run_one(services, &runtime, &job_id),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Run the job `job_id` from its plan; a job without a plan, or one that cannot run, is
/// journaled with its code only.
fn run_one(services: &ImportServices, runtime: &tokio::runtime::Runtime, job_id: &str) {
    let plan = match services.plans.load(job_id) {
        Ok(Some(plan)) => plan,
        Ok(None) => return journal_failure(services, Some(job_id), "plan_missing"),
        Err(_) => return journal_failure(services, Some(job_id), "unavailable"),
    };
    let context = ImportContext {
        jobs: &services.jobs,
        files: &services.files,
        journal: &services.journal,
        build: &services.build,
        actor: &services.actor,
        staging_dir: &services.staging_dir,
        stop: &services.stop,
        retry_unit: services.retry_unit,
    };
    if let Err(err) = runtime.block_on(run_import_job(&context, job_id, &plan)) {
        journal_failure(services, Some(job_id), err.error_code());
    }
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
