//! [`JobStore`]: where a host keeps its jobs, and the in-memory reference store.

use super::model::{Job, JobError, JobState};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

/// The kinds whose live leases a counted start counts for a job: those that share its class
/// (API-S-037 caps).
pub type SlotKinds<'a> = &'a dyn Fn(&Job) -> Vec<String>;

/// A host's job store. Every method is one atomic step; the rules are the caller's
/// ([`super::rules`]), applied inside [`JobStore::update`].
pub trait JobStore: Send + Sync {
    /// Store a new job.
    ///
    /// # Errors
    /// [`JobError::Conflict`] (`job_exists`) for a known ID, or a store failure.
    fn insert(&self, job: &Job) -> Result<(), JobError>;

    /// The job `id`, if stored.
    ///
    /// # Errors
    /// A store failure.
    fn get(&self, id: &str) -> Result<Option<Job>, JobError>;

    /// Replace job `id` with what `change` makes of it, atomically: no other call sees or
    /// changes the job in between. Nothing is stored when `change` refuses or changes the
    /// job's ID.
    ///
    /// # Errors
    /// [`JobError::NotFound`], the error `change` answers, [`JobError::Failed`] for a
    /// changed ID, or a store failure.
    fn update(
        &self,
        id: &str,
        change: &mut dyn FnMut(Job) -> Result<Job, JobError>,
    ) -> Result<Job, JobError>;

    /// At most `limit` unfinished (pending or running) jobs, oldest first (by submission
    /// time, then ID).
    ///
    /// # Errors
    /// A store failure.
    fn unfinished(&self, limit: usize) -> Result<Vec<Job>, JobError>;

    /// At most `limit` unfinished jobs of `kind`, oldest first: how a host finds its own
    /// job whatever else is waiting.
    ///
    /// # Errors
    /// A store failure.
    fn unfinished_of(&self, kind: &str, limit: usize) -> Result<Vec<Job>, JobError>;

    /// [`JobStore::update`] that also passes `change` how many other jobs hold a slot of
    /// the job's class (API-S-037 caps): the running jobs of the kinds `kinds` names for the
    /// job whose lease has not ended at `now_ms`, counted in the same atomic step, so no
    /// other start, from this service or another one over the same store, takes a slot in
    /// between. A job for which `kinds` names none is passed 0 without a count.
    ///
    /// # Errors
    /// As [`JobStore::update`].
    fn update_counting_leases(
        &self,
        id: &str,
        counted: (SlotKinds<'_>, i64),
        change: &mut dyn FnMut(Job, usize) -> Result<Job, JobError>,
    ) -> Result<Job, JobError>;

    /// At most `filter.limit` jobs that `filter` accepts, finished ones included, newest
    /// first (by submission time, then ID, both descending), and whether more matched. The
    /// default lists the unfinished jobs only (at most [`super::MAX_RECOVERED_JOBS`] read): a
    /// store that keeps finished jobs MUST override it (both stores of FileSquid do).
    ///
    /// # Errors
    /// A store failure.
    fn list(&self, filter: &JobFilter) -> Result<JobList, JobError> {
        let open = self.unfinished(super::MAX_RECOVERED_JOBS)?;
        Ok(JobList::newest(
            open.into_iter().filter(|j| filter.accepts(j)),
            filter.limit,
        ))
    }
}

/// Which jobs [`JobStore::list`] answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFilter {
    /// Only jobs of this kind.
    pub kind: Option<String>,
    /// Only jobs in this state.
    pub state: Option<JobState>,
    /// At most this many jobs (1 to [`MAX_LISTED_JOBS`]; larger values are capped).
    pub limit: usize,
}

/// Most jobs one listing answers.
pub const MAX_LISTED_JOBS: usize = 100;

impl JobFilter {
    /// Whether `job` is one this filter accepts. Pure.
    pub fn accepts(&self, job: &Job) -> bool {
        self.kind.as_deref().is_none_or(|kind| job.kind == kind)
            && self.state.is_none_or(|state| job.state == state)
    }

    /// The limit, between 1 and [`MAX_LISTED_JOBS`]. Pure.
    pub fn effective_limit(&self) -> usize {
        self.limit.clamp(1, MAX_LISTED_JOBS)
    }
}

/// One listing of jobs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JobList {
    /// The jobs, newest first.
    pub jobs: Vec<Job>,
    /// Whether more jobs matched than were answered.
    pub truncated: bool,
}

impl JobList {
    /// The newest `limit` (capped as [`JobFilter::effective_limit`]) of `jobs`. Pure.
    pub fn newest(jobs: impl Iterator<Item = Job>, limit: usize) -> Self {
        let limit = limit.clamp(1, MAX_LISTED_JOBS);
        let mut jobs: Vec<Job> = jobs.collect();
        jobs.sort_by(|a, b| (b.created_ms, &b.id).cmp(&(a.created_ms, &a.id)));
        let truncated = jobs.len() > limit;
        jobs.truncate(limit);
        Self { jobs, truncated }
    }
}

/// Refuse a change that gave the job another ID. Pure.
///
/// # Errors
/// [`JobError::Failed`] when `changed` is not job `id`.
pub fn same_id(id: &str, changed: Job) -> Result<Job, JobError> {
    match changed.id == id {
        true => Ok(changed),
        false => Err(JobError::Failed("a job update changed the job's ID".into())),
    }
}

/// Jobs held in memory: the reference store, for tests and for hosts without a database.
#[derive(Debug, Default)]
pub struct MemoryJobStore {
    jobs: Mutex<BTreeMap<String, Job>>,
}

impl MemoryJobStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// At most `limit` unfinished jobs that `keep` accepts, oldest first. Only references
    /// are sorted; just the jobs answered are cloned.
    fn oldest_open(&self, limit: usize, keep: impl Fn(&Job) -> bool) -> Result<Vec<Job>, JobError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let jobs = self.lock()?;
        let mut open: Vec<&Job> = jobs
            .values()
            .filter(|job| !job.state.is_finished() && keep(job))
            .collect();
        open.sort_by(|a, b| (a.created_ms, &a.id).cmp(&(b.created_ms, &b.id)));
        Ok(open.into_iter().take(limit).cloned().collect())
    }

    /// The jobs, locked.
    fn lock(&self) -> Result<MutexGuard<'_, BTreeMap<String, Job>>, JobError> {
        self.jobs
            .lock()
            .map_err(|_| JobError::Failed("job store lock poisoned".into()))
    }
}

impl JobStore for MemoryJobStore {
    fn insert(&self, job: &Job) -> Result<(), JobError> {
        let mut jobs = self.lock()?;
        if jobs.contains_key(&job.id) {
            return Err(JobError::Conflict("job_exists"));
        }
        jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<Job>, JobError> {
        Ok(self.lock()?.get(id).cloned())
    }

    fn update(
        &self,
        id: &str,
        change: &mut dyn FnMut(Job) -> Result<Job, JobError>,
    ) -> Result<Job, JobError> {
        let mut jobs = self.lock()?;
        let current = jobs.get(id).cloned().ok_or(JobError::NotFound)?;
        let changed = same_id(id, change(current)?)?;
        jobs.insert(id.to_string(), changed.clone());
        Ok(changed)
    }

    fn unfinished(&self, limit: usize) -> Result<Vec<Job>, JobError> {
        self.oldest_open(limit, |_| true)
    }

    fn unfinished_of(&self, kind: &str, limit: usize) -> Result<Vec<Job>, JobError> {
        self.oldest_open(limit, |job| job.kind == kind)
    }

    fn update_counting_leases(
        &self,
        id: &str,
        (kinds, now_ms): (SlotKinds<'_>, i64),
        change: &mut dyn FnMut(Job, usize) -> Result<Job, JobError>,
    ) -> Result<Job, JobError> {
        let mut jobs = self.lock()?;
        let current = jobs.get(id).cloned().ok_or(JobError::NotFound)?;
        let counted = kinds(&current);
        let held = match counted.is_empty() {
            true => 0,
            false => jobs
                .values()
                .filter(|job| job.id != id && counted.contains(&job.kind))
                .filter(|job| super::rules::holds_slot(job, now_ms))
                .count(),
        };
        let changed = same_id(id, change(current, held)?)?;
        jobs.insert(id.to_string(), changed.clone());
        Ok(changed)
    }

    /// Only references are sorted; just the jobs answered are cloned.
    fn list(&self, filter: &JobFilter) -> Result<JobList, JobError> {
        let jobs = self.lock()?;
        let mut matching: Vec<&Job> = jobs.values().filter(|job| filter.accepts(job)).collect();
        matching.sort_by(|a, b| (b.created_ms, &b.id).cmp(&(a.created_ms, &a.id)));
        let limit = filter.effective_limit();
        Ok(JobList {
            truncated: matching.len() > limit,
            jobs: matching.into_iter().take(limit).cloned().collect(),
        })
    }
}
