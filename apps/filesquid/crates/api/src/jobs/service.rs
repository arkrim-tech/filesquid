//! [`Jobs`]: the service a host runs durable jobs through, and [`RunningJob`], one leased
//! run. Every state change and every refusal is journaled through the host's bounded
//! journal queue with the job ID, the operation ID (absent only for an unknown job ID) and
//! an error code only; a running job is a process in the registry.

use super::model::{Job, JobControl, JobError, JobSpec, JobState, Lease};
use super::rules::SOURCE_CHANGED;
use super::slots::{JobClass, JobLimits, Slots};
use super::store::{JobFilter, JobList, JobStore};
use super::view::JobView;
use super::{rules, JOB_LEASE_TTL_MS, MAX_RECOVERED_JOBS};
use super::{
    JOB_CANCELLED, JOB_CANCEL_REQUESTED, JOB_COMPLETED, JOB_CREATED, JOB_FAILED, JOB_PAUSED,
    JOB_RECOVERED, JOB_REFUSED, JOB_RESUMED, JOB_STARTED, JOB_SUSPENDED,
};
use crate::events::{topic, ApiEvent, EventBus};
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
    events: Option<EventBus>,
    slots: Slots,
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
            events: None,
            slots: Slots::default(),
        }
    }

    /// Publish one `job.update` event on `events` for every stored change of a job
    /// (API-S-037).
    #[must_use]
    pub fn with_events(mut self, events: EventBus) -> Self {
        self.events = Some(events);
        self
    }

    /// Hold jobs of `kind` to the cap of `class` ([`Self::with_limits`]).
    #[must_use]
    pub fn with_class(mut self, kind: &str, class: JobClass) -> Self {
        self.slots.classify(kind, class);
        self
    }

    /// Use `limits` as the caps of the classes (default [`JobLimits::default`]).
    #[must_use]
    pub fn with_limits(mut self, limits: JobLimits) -> Self {
        self.slots.limit(limits);
        self
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
        self.publish(&job);
        Ok(job)
    }

    /// The job `id` (store I/O).
    ///
    /// # Errors
    /// [`JobError::NotFound`] or a store failure.
    pub fn job(&self, id: &str) -> Result<Job, JobError> {
        self.store.get(id)?.ok_or(JobError::NotFound)
    }

    /// The jobs `filter` accepts, newest first, at most its limit (store I/O).
    ///
    /// # Errors
    /// A store failure.
    pub fn list(&self, filter: &JobFilter) -> Result<JobList, JobError> {
        self.store.list(filter)
    }

    /// Pause job `id`: a pending job is held, a running job is asked to suspend at its next
    /// checkpoint; journaled as `job.paused` when it changed (store I/O).
    ///
    /// # Errors
    /// [`JobError::NotFound`], [`JobError::Conflict`] (`job_finished`, `cancel_requested`) or
    /// a store failure; each is journaled.
    pub fn pause(&self, id: &str) -> Result<Job, JobError> {
        let (job, changed) = self.control(id, rules::pause)?;
        if changed {
            self.journal_job(&job, JOB_PAUSED, Outcome::Info, None);
        }
        Ok(job)
    }

    /// Resume job `id`: clear the pause asked of it so it may start again; journaled as
    /// `job.resumed` (store I/O). Handing it to its worker is the caller's.
    ///
    /// # Errors
    /// [`JobError::NotFound`], [`JobError::Conflict`] (`job_finished`, `cancel_requested`,
    /// `job_running`) or a store failure; each is journaled.
    pub fn resume(&self, id: &str) -> Result<Job, JobError> {
        let (job, _) = self.control(id, rules::resume)?;
        self.journal_job(&job, JOB_RESUMED, Outcome::Resumed, None);
        Ok(job)
    }

    /// Cancel job `id`: a pending job is cancelled at once (`job.cancelled`), a running job
    /// is asked to stop at its next checkpoint (`job.cancel_requested`) (store I/O).
    ///
    /// # Errors
    /// [`JobError::NotFound`], [`JobError::Conflict`] (`job_finished`) or a store failure;
    /// each is journaled.
    pub fn cancel(&self, id: &str) -> Result<Job, JobError> {
        let (job, changed) = self.control(id, rules::cancel)?;
        match (changed, job.state) {
            (false, _) => {}
            (true, JobState::Cancelled) => {
                self.journal_job(&job, JOB_CANCELLED, Outcome::Cancelled, None);
            }
            (true, _) => self.journal_job(&job, JOB_CANCEL_REQUESTED, Outcome::Info, None),
        }
        Ok(job)
    }

    /// Apply the control `rule` to job `id` atomically and publish the job when it changed;
    /// answer the job and whether it changed. A refusal is journaled.
    fn control(
        &self,
        id: &str,
        rule: fn(Job, i64) -> Result<Job, JobError>,
    ) -> Result<(Job, bool), JobError> {
        let now = (self.clock)();
        let (mut operation_id, mut changed) = (None, false);
        let stored = self
            .store
            .update(id, &mut |job| {
                operation_id = Some(job.operation_id.clone());
                let next = rule(job.clone(), now)?;
                changed = next != job;
                Ok(next)
            })
            .inspect_err(|err| self.journal_refusal(id, operation_id.as_deref(), err))?;
        if changed {
            self.publish(&stored);
        }
        Ok((stored, changed))
    }

    /// Journal `entry`, timed by the service's clock: a refusal a job route makes itself.
    pub(crate) fn journal_entry(&self, entry: JournalEntry) {
        self.journal.send(entry.at_ms((self.clock)()));
    }

    /// Publish `job` as one `job.update` event, when the service has an event bus.
    fn publish(&self, job: &Job) {
        let Some(events) = &self.events else {
            return;
        };
        match serde_json::to_value(JobView::of(job)) {
            Ok(data) => {
                events.publish(ApiEvent::new(topic::JOB_UPDATE, data));
            }
            Err(err) => tracing::warn!(error = %err, "a job view could not be encoded"),
        }
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
        if released && stored.state == JobState::Cancelled {
            self.journal_job(&stored, JOB_CANCELLED, Outcome::Cancelled, None);
        } else if released {
            self.journal_job(&stored, JOB_RECOVERED, Outcome::Resumed, None);
        }
        if released {
            self.publish(&stored);
        }
        Ok(stored)
    }

    /// Start job `id` for `source` in one atomic step: a job whose source changed is failed
    /// (`source_changed`, nothing of its checkpoint reused); otherwise it is leased, its run
    /// registered and `job.started` journaled (`resumed` when it has a checkpoint) (store
    /// I/O).
    ///
    /// # Errors
    /// [`JobError::NotFound`], [`JobError::Conflict`] (`slots_full` when its class is at its
    /// cap), [`JobError::SourceChanged`] or a store failure; each is journaled.
    pub fn start(&self, id: &str, source: &str) -> Result<RunningJob, JobError> {
        let mut operation_id = None;
        let started = self
            .lease_job(id, source, &mut operation_id)
            .inspect_err(|err| self.journal_refusal(id, operation_id.as_deref(), err))?;
        self.publish(&started);
        if started.state == JobState::Failed {
            self.journal_job(&started, JOB_FAILED, Outcome::Failed, Some(SOURCE_CHANGED));
            return Err(JobError::SourceChanged);
        }
        let lease = rules::lease_of(&started)?;
        Ok(self.run(started, lease))
    }

    /// Apply [`rules::start`] to job `id` in one store update, refused `slots_full` when it
    /// would take a lease while its class is at its cap, the class's live leases counted in
    /// the same atomic step; `operation_id` is set as soon as the job is read, for the
    /// refusal's journal entry (store I/O).
    fn lease_job(
        &self,
        id: &str,
        source: &str,
        operation_id: &mut Option<String>,
    ) -> Result<Job, JobError> {
        let now = (self.clock)();
        let (holder, ttl, slots) = (self.holder.as_str(), self.ttl_ms, &self.slots);
        let kinds = |job: &Job| slots.kinds_sharing(&job.kind);
        self.store
            .update_counting_leases(id, (&kinds, now), &mut |job, held| {
                *operation_id = Some(job.operation_id.clone());
                let cap = slots.class_of(&job.kind).map(|(_, cap)| cap);
                let next = rules::start(job, (holder, source), now, ttl)?;
                let full = cap.is_some_and(|cap| held >= cap);
                match full && next.state == JobState::Running {
                    true => Err(JobError::Conflict("slots_full")),
                    false => Ok(next),
                }
            })
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
            requested: Mutex::new(job.control),
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
        control: None,
    }
}

/// How [`RunningJob::halt_if_requested`] left a run.
pub enum Halt {
    /// The run gave its lease back; the job is now in this state (pending or cancelled).
    Halted(JobState),
    /// Nothing is asked of the job any more (it was resumed); the run keeps its lease.
    Continued(Box<RunningJob>),
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
    /// The control asked of the job, as the last checkpoint or renewal read it.
    requested: Mutex<Option<JobControl>>,
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

    /// The control asked of the job (a pause or a cancel through the job routes) as the
    /// last checkpoint or renewal read it: a worker that sees one suspends
    /// ([`Self::suspend`]), which keeps a paused job pending and cancels a job asked to stop
    /// (API-S-037).
    pub fn requested(&self) -> Option<JobControl> {
        *self
            .requested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Keep the renewed lease and the control the store answered.
    fn keep(&self, stored: &Job) -> Result<(), JobError> {
        let lease = rules::lease_of(stored)?;
        *self.lease.lock().unwrap_or_else(PoisonError::into_inner) = lease;
        *self
            .requested
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = stored.control;
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
        self.jobs.publish(&stored);
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

    /// Give the lease back at quit or at a pause, keeping the checkpoint for the next run, and
    /// journal `job.suspended`; a job asked to stop is cancelled instead (`job.cancelled`).
    /// Answers the state stored: pending, or cancelled (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn suspend(self) -> Result<JobState, JobError> {
        let now = (self.jobs.clock)();
        let lease = self.lease();
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| rules::suspend(job, &lease, now))
            .inspect_err(|err| self.refused(err))?;
        self.released(&stored);
        Ok(stored.state)
    }

    /// Suspend the run if a pause or cancel is still asked of it, checked in the same store
    /// update, so a resume that cleared the pause keeps the run going (API-S-037). A worker
    /// calls it where it would stop for [`requested`](Self::requested) (store I/O).
    ///
    /// # Errors
    /// [`JobError::Fenced`] or a store failure, journaled.
    pub fn halt_if_requested(self) -> Result<Halt, JobError> {
        let now = (self.jobs.clock)();
        let lease = self.lease();
        let stored = self
            .jobs
            .store
            .update(&self.job.id, &mut |job| rules::halt(job, &lease, now))
            .inspect_err(|err| self.refused(err))?;
        if stored.state == JobState::Running {
            self.keep(&stored)?;
            return Ok(Halt::Continued(Box::new(self)));
        }
        self.released(&stored);
        Ok(Halt::Halted(stored.state))
    }

    /// Journal and announce the run's end at `stored` (pending or cancelled), and end its
    /// process.
    fn released(&self, stored: &Job) {
        let entry = match stored.state {
            JobState::Cancelled => self.jobs.entry(stored, JOB_CANCELLED, Outcome::Cancelled),
            _ => self.jobs.entry(stored, JOB_SUSPENDED, Outcome::Info),
        };
        self.jobs.journal.send(entry.process(self.handle.id()));
        self.jobs.publish(stored);
        self.handle.cancel();
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
        self.jobs.publish(&stored);
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
