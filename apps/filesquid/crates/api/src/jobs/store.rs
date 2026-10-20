//! [`JobStore`]: where a host keeps its jobs, and the in-memory reference store.

use super::model::{Job, JobError};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

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
}
