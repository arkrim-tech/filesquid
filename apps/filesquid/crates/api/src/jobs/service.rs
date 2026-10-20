//! [`Jobs`]: the service a host runs durable jobs through, and [`RunningJob`], one leased
//! run. Every state change and every refusal is journaled through the host's bounded
//! journal queue with the job ID, the operation ID (absent only for an unknown job ID) and
//! an error code only; a running job is a process in the registry.

use super::model::{Job, JobError, JobSpec, JobState, Lease};
use super::rules::SOURCE_CHANGED;
use super::store::JobStore;
use super::{rules, JOB_LEASE_TTL_MS, MAX_RECOVERED_JOBS};
use super::{
    JOB_CANCELLED, JOB_COMPLETED, JOB_CREATED, JOB_FAILED, JOB_RECOVERED, JOB_REFUSED, JOB_STARTED,
    JOB_SUSPENDED,
};
use crate::journal::{now_ms, JournalEntry, Level, Outcome};
use crate::journal_writer::JournalSender;
use crate::library::mint_uuid_v4;
use crate::registry::{Clock, ProcessHandle, ProcessRegistry, ProcessSpec};
use std::sync::{Arc, Mutex, PoisonError};

/// Heartbeat timeout of a running job's process: a run that neither checkpoints, renews nor
/// finishes for this long is stalled.
const JOB_HEARTBEAT_TIMEOUT_MS: i64 = 60_000;

/// The job service of one host launch.
#[derive(Clone)]
pub struct Jobs {
    store: Arc<dyn JobStore>,
    journal: JournalSender,
    registry: ProcessRegistry,
    holder: String,
    clock: Clock,
    ttl_ms: i64,
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("ttl_ms", &self.ttl_ms)
            .finish_non_exhaustive()
    }
}

impl Jobs {
    /// The service over `store`, journaling through `journal`, registering runs in
    /// `registry` and leasing jobs as `holder` (one per host launch).
    pub fn new(
        store: Arc<dyn JobStore>,
        journal: JournalSender,
        registry: ProcessRegistry,
        holder: impl Into<String>,
    ) -> Self {
        Self {
            store,
            journal,
            registry,
            holder: holder.into(),
            clock: Arc::new(now_ms),
            ttl_ms: JOB_LEASE_TTL_MS,
        }
    }

    /// Use `clock` (Unix ms, UTC) for leases and journal times.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Lease jobs for `ttl_ms` (at least one millisecond) at a time.
    #[must_use]
    pub fn with_lease_ttl(mut self, ttl_ms: i64) -> Self {
        self.ttl_ms = ttl_ms.max(1);
        self
    }

    /// How long a lease lasts unless renewed.
    pub fn lease_ttl_ms(&self) -> i64 {
        self.ttl_ms
    }

    /// Store a new pending job for `spec` and journal `job.created`; a job the store
    /// refuses is journaled as `job.refused` (store I/O).
    ///
    /// # Errors
    /// [`JobError::Unavailable`] when no ID can be minted, or a store failure.
    pub fn submit(&self, spec: JobSpec) -> Result<Job, JobError> {
        let id = mint_uuid_v4().map_err(|err| JobError::Unavailable(err.to_string()))?;
        let job = new_job(id, spec, (self.clock)());
        self.store
            .insert(&job)
            .inspect_err(|err| self.journal_refusal(&job.id, Some(&job.operation_id), err))?;
        self.journal_job(&job, JOB_CREATED, Outcome::Succeeded, None);
        Ok(job)
    }

    /// The unfinished jobs of `kind`, at most `limit`, oldest first: how a host finds its own
    /// job even when more than [`MAX_RECOVERED_JOBS`] others wait (store I/O).
    ///
    /// # Errors
    /// A store failure.
    pub fn unfinished_of(&self, kind: &str, limit: usize) -> Result<Vec<Job>, JobError> {
        self.store.unfinished_of(kind, limit)
    }

    /// The bounded launch-time reconciliation: release the expired lease of every run a
    /// crash left (journaled as `job.recovered` when this call released it) and answer at
    /// most [`MAX_RECOVERED_JOBS`] unfinished jobs, oldest first. Reads jobs only, never the
    /// media they cover (store I/O).
    ///
    /// # Errors
    /// A store failure.
    pub fn recover(&self) -> Result<Vec<Job>, JobError> {
        let now = (self.clock)();
        let open = self.store.unfinished(MAX_RECOVERED_JOBS)?;
        open.into_iter()
            .map(|job| self.release_expired(job, now))
            .collect()
    }

    /// The job `job` with a crashed run's expired lease released, journaled only when this
    /// update made that change.
    fn release_expired(&self, job: Job, now: i64) -> Result<Job, JobError> {
        if rules::expire(job.clone(), now) == job {
            return Ok(job);
        }
        let mut released = false;
        let stored = self.store.update(&job.id, &mut |current| {
            let expired = rules::expire(current.clone(), now);
            released = expired != current;
            Ok(expired)
        })?;
        if released {
            self.journal_job(&stored, JOB_RECOVERED, Outcome::Resumed, None);
        }
        Ok(stored)
    }

    /// Start job `id` for `source` in one atomic step: a job whose source changed is failed
    /// (`source_changed`, nothing of its checkpoint reused); otherwise it is leased, its run
    /// registered and `job.started` journaled (`resumed` when it has a checkpoint) (store
    /// I/O).
    ///
    /// # Errors
    /// [`JobError::NotFound`], [`JobError::Conflict`], [`JobError::SourceChanged`] or a
    /// store failure; each is journaled.
    pub fn start(&self, id: &str, source: &str) -> Result<RunningJob, JobError> {
        let now = (self.clock)();
        let (holder, ttl) = (self.holder.as_str(), self.ttl_ms);
        let mut operation_id = None;
        let updated = self.store.update(id, &mut |job| {
            operation_id = Some(job.operation_id.clone());
            rules::start(job, (holder, source), now, ttl)
        });
        let started =
            updated.inspect_err(|err| self.journal_refusal(id, operation_id.as_deref(), err))?;
        if started.state == JobState::Failed {
            self.journal_job(&started, JOB_FAILED, Outcome::Failed, Some(SOURCE_CHANGED));
            return Err(JobError::SourceChanged);
        }
        let lease = rules::lease_of(&started)?;
        Ok(self.run(started, lease))
    }

    /// Register the leased `job` as a process and journal its start.
    fn run(&self, job: Job, lease: Lease) -> RunningJob {
        let handle = self.registry.register(ProcessSpec::new(
            job.kind.as_str(),
            JOB_HEARTBEAT_TIMEOUT_MS,
        ));
        if let Some(total) = job.total {
            handle.progress(job.done, total);
        }
        let outcome = match job.checkpoint {
            Some(_) => Outcome::Resumed,
            None => Outcome::Started,
        };
        let entry = self.entry(&job, JOB_STARTED, outcome).process(handle.id());
        self.journal.send(entry);
        RunningJob {
            jobs: self.clone(),
            job,
            lease: Mutex::new(lease),
            handle,
        }
    }

    /// The journal entry of `operation` for `job`: its ID and operation ID, at now.
    fn entry(&self, job: &Job, operation: &str, outcome: Outcome) -> JournalEntry {
        JournalEntry::new(operation, outcome)
            .target(job.id.as_str())
            .operation_id(job.operation_id.as_str())
            .at_ms((self.clock)())
    }

    /// Journal `operation` for `job`, an error-level entry with `error_code` when it failed.
    fn journal_job(&self, job: &Job, operation: &str, outcome: Outcome, code: Option<&str>) {
        let entry = self.entry(job, operation, outcome);
        self.journal.send(match code {
            Some(code) => entry.error_code(code).level(Level::Error),
            None => entry,
        });
    }

    /// Journal a refused call on job `id` with its operation ID, when known, and error code.
    fn journal_refusal(&self, id: &str, operation_id: Option<&str>, err: &JobError) {
        let entry = JournalEntry::new(JOB_REFUSED, Outcome::Failed)
            .level(Level::Warn)
            .target(id)
            .error_code(err.error_code())
            .at_ms((self.clock)());
        self.journal.send(match operation_id {
            Some(op) => entry.operation_id(op),
            None => entry,
        });
    }
}

/// A new pending job `id` for `spec`, submitted at `now`. Pure.
fn new_job(id: String, spec: JobSpec, now: i64) -> Job {
    Job {
        id,
        kind: spec.kind,
        operation_id: spec.operation_id,
        source: spec.source,
        state: JobState::Pending,
        checkpoint: None,
        done: 0,
        total: None,
        attempts: 0,
        fence: 0,
        holder: None,
        lease_expires_ms: None,
        created_ms: now,
        updated_ms: now,
        error_code: None,
    }
}

/// One leased run of a job. End it through [`complete`](Self::complete),
/// [`fail`](Self::fail), [`cancel`](Self::cancel) or [`suspend`](Self::suspend); a run
/// dropped otherwise (a crash) keeps its stored lease until it expires and
/// [`Jobs::recover`] or the next start releases it. [`renew`](Self::renew) takes `&self`, so
/// another thread can keep the lease alive while one long unit of work runs.
pub struct RunningJob {
    jobs: Jobs,
    job: Job,
    lease: Mutex<Lease>,
    handle: ProcessHandle,
}

impl std::fmt::Debug for RunningJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningJob")
            .field("job_id", &self.job.id)
            .field("fence", &self.fence())
            .finish_non_exhaustive()
    }
}

impl RunningJob {
    /// The job as it was when the run started.
    pub fn job(&self) -> &Job {
        &self.job
    }

    /// The checkpoint the run resumes from, if any.
    pub fn checkpoint_text(&self) -> Option<&str> {
        self.job.checkpoint.as_deref()
    }

    /// The fencing number of this run's lease.
    pub fn fence(&self) -> u64 {
        self.lease().fence
    }

    /// The registry's process ID of this run.
    pub fn process_id(&self) -> &str {
        self.handle.id()
    }

    /// The current lease.
    fn lease(&self) -> Lease {
        self.lease
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Keep the renewed lease the store answered.
    fn keep(&self, stored: &Job) -> Result<(), JobError> {
        let lease = rules::lease_of(stored)?;
        *self.lease.lock().unwrap_or_else(PoisonError::into_inner) = lease;
        Ok(())
    }

    /// Store `checkpoint` with `done` of `total` units, renew the lease and report the
    /// progress to the registry (store I/O). A fenced write fails the run's process.
    ///
    /// # Errors
    /// [`JobError::TooLarge`] (the old checkpoint is kept), [`JobError::Fenced`] or a store
    /// failure; each refusal is journaled.
    pub fn checkpoint(
        &mut self,
        checkpoint: &str,
        done: u64,
        total: Option<u64>,
    ) -> Result<(), JobError> {
        let now = (self.jobs.clock)();
        let (lease, ttl) = (self.lease(), self.jobs.ttl_ms);
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| {
                rules::checkpoint(job, &lease, (checkpoint, done, total), now, ttl)
            })
            .inspect_err(|err| self.refused(err))?;
        self.keep(&stored)?;
        match total {
            Some(total) => self.handle.progress(done, total),
            None => self.handle.heartbeat(),
        }
        Ok(())
    }

    /// Renew the lease without changing the checkpoint, and heartbeat the run's process
    /// (store I/O). Callable from another thread while one long unit of work runs.
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure; each refusal is journaled.
    pub fn renew(&self) -> Result<(), JobError> {
        let now = (self.jobs.clock)();
        let (lease, ttl) = (self.lease(), self.jobs.ttl_ms);
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| rules::renew(job, &lease, now, ttl))
            .inspect_err(|err| self.refused(err))?;
        self.keep(&stored)?;
        self.handle.heartbeat();
        Ok(())
    }

    /// Finish the job as completed (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn complete(self) -> Result<(), JobError> {
        self.finish(JobState::Completed, None)?;
        self.handle.complete();
        Ok(())
    }

    /// Finish the job as failed with `error_code` (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn fail(self, error_code: &str) -> Result<(), JobError> {
        self.finish(JobState::Failed, Some(error_code))?;
        self.handle.fail(error_code);
        Ok(())
    }

    /// Finish the job as cancelled (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn cancel(self) -> Result<(), JobError> {
        self.finish(JobState::Cancelled, None)?;
        self.handle.cancel();
        Ok(())
    }

    /// Give the lease back at quit, keeping the checkpoint for the next launch, and journal
    /// `job.suspended` (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn suspend(self) -> Result<(), JobError> {
        let now = (self.jobs.clock)();
        let lease = self.lease();
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| rules::suspend(job, &lease, now))
            .inspect_err(|err| self.refused(err))?;
        let entry = self.jobs.entry(&stored, JOB_SUSPENDED, Outcome::Info);
        self.jobs.journal.send(entry.process(self.handle.id()));
        self.handle.cancel();
        Ok(())
    }

    /// Store the job's end as `state` with `error_code` and journal it (an error-level
    /// entry for a failure).
    fn finish(&self, state: JobState, error_code: Option<&str>) -> Result<(), JobError> {
        let now = (self.jobs.clock)();
        let lease = self.lease();
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| {
                rules::finish(job, &lease, (state, error_code), now)
            })
            .inspect_err(|err| self.refused(err))?;
        let (operation, outcome) = match state {
            JobState::Completed => (JOB_COMPLETED, Outcome::Succeeded),
            JobState::Cancelled => (JOB_CANCELLED, Outcome::Cancelled),
            _ => (JOB_FAILED, Outcome::Failed),
        };
        let entry = self
            .jobs
            .entry(&stored, operation, outcome)
            .process(self.handle.id());
        self.jobs.journal.send(match error_code {
            Some(code) => entry.error_code(code).level(Level::Error),
            None => entry,
        });
        Ok(())
    }

    /// Journal a refused write and fail the run's process when it was fenced.
    fn refused(&self, err: &JobError) {
        self.jobs
            .journal_refusal(&self.job.id, Some(&self.job.operation_id), err);
        if *err == JobError::Fenced {
            self.handle.fail(err.error_code());
        }
    }
}
