//! Per-file progress of an upload job (API-S-037 **Per-file progress**, owner answer 6 of
//! 2026-10-22, "Yes, report progress per file"): the [`JobItems`] port a host answers its
//! jobs' destinations and files through, the item types the routes show, and
//! [`MemoryJobItems`], the reference port for tests and hosts without a database. A file is
//! named by its display name only (its base name), never a path, and an item's ID is the file
//! ID the host minted when it planned the import.

use super::model::JobError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

/// Most files one items page answers.
pub const MAX_LISTED_ITEMS: usize = 1_000;

/// Where an upload job's files go: a collection of the library and a folder inside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDestination {
    /// The destination collection.
    pub collection_id: String,
    /// The destination folder inside it, a collection logical path; the collection's root
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// Where one file of an upload job stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobItemState {
    /// Waiting to be copied.
    Queued,
    /// Being copied: `bytes_done` of `bytes_total`.
    Copying,
    /// Copied, checked and committed.
    Copied,
    /// Skipped: the library already holds it.
    Skipped,
    /// Failed, with its error code; a retry puts it back to queued.
    Failed,
}

impl JobItemState {
    /// The stored name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Copying => "copying",
            Self::Copied => "copied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }

    /// The state a stored name names, if any.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Queued,
            Self::Copying,
            Self::Copied,
            Self::Skipped,
            Self::Failed,
        ]
        .into_iter()
        .find(|state| state.as_str() == name)
    }
}

/// One file of an upload job, as the items route shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobItem {
    /// The item: the file ID the host minted for it.
    pub item_id: String,
    /// Its display name: the base name only, never a folder path.
    pub name: String,
    /// The destination folder inside the job's collection it goes to; the collection's root
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Where it stands.
    pub state: JobItemState,
    /// Bytes copied so far.
    pub bytes_done: u64,
    /// Its size in bytes.
    pub bytes_total: u64,
    /// Why it failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// One page of a job's files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobItemPage {
    /// The files of this page, in the job's order.
    pub items: Vec<JobItem>,
    /// The cursor of the next page; `null` after the last.
    pub next_cursor: Option<String>,
}

/// The host port of per-file progress: what a host keeps of each upload job's destination
/// and files. Every call is one store read or one atomic write.
pub trait JobItems: Send + Sync {
    /// The destination of job `job_id`, when it is an upload job the host keeps files for.
    ///
    /// # Errors
    /// A store failure.
    fn destination(&self, job_id: &str) -> Result<Option<JobDestination>, JobError>;

    /// At most `limit` (capped at [`MAX_LISTED_ITEMS`]) files of job `job_id` after
    /// `cursor`, in the job's order; an empty page for a job without files.
    ///
    /// # Errors
    /// A store failure.
    fn page(
        &self,
        job_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<JobItemPage, JobError>;

    /// Put the failed file `item` of job `job_id` (every failed file when `None`) back to
    /// queued, atomically, and answer the IDs of the files put back; a file that is not
    /// failed is left as it is.
    ///
    /// # Errors
    /// [`JobError::NotFound`] for an item the job has no file of; a store failure.
    fn retry(&self, job_id: &str, item: Option<&str>) -> Result<Vec<String>, JobError>;
}

/// The files of one job, in order, with their destination.
#[derive(Clone, Debug, Default)]
struct JobFiles {
    destination: Option<JobDestination>,
    items: Vec<JobItem>,
}

/// Per-file progress held in memory: the reference port, for tests and hosts without a
/// database.
#[derive(Debug, Default)]
pub struct MemoryJobItems {
    jobs: Mutex<BTreeMap<String, JobFiles>>,
}

impl MemoryJobItems {
    /// An empty port.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep `items` (in order) as the files of job `job_id`, going to `destination`.
    pub fn put(&self, job_id: &str, destination: Option<JobDestination>, items: Vec<JobItem>) {
        self.lock()
            .insert(job_id.to_string(), JobFiles { destination, items });
    }

    /// Set the state of file `item` of job `job_id` to `state` with `bytes_done` copied and
    /// `error_code`; answer whether the file was found.
    pub fn set(
        &self,
        job_id: &str,
        item: &str,
        state: JobItemState,
        bytes_done: u64,
        error_code: Option<&str>,
    ) -> bool {
        let mut jobs = self.lock();
        let Some(found) = jobs
            .get_mut(job_id)
            .and_then(|files| files.items.iter_mut().find(|i| i.item_id == item))
        else {
            return false;
        };
        found.state = state;
        found.bytes_done = bytes_done;
        found.error_code = error_code.map(str::to_string);
        true
    }

    /// Every file of job `job_id`, in order.
    pub fn all(&self, job_id: &str) -> Vec<JobItem> {
        self.lock()
            .get(job_id)
            .map(|files| files.items.clone())
            .unwrap_or_default()
    }

    /// The jobs, locked; a poisoned lock is recovered (plain data).
    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, JobFiles>> {
        self.jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl JobItems for MemoryJobItems {
    fn destination(&self, job_id: &str) -> Result<Option<JobDestination>, JobError> {
        Ok(self
            .lock()
            .get(job_id)
            .and_then(|files| files.destination.clone()))
    }

    fn page(
        &self,
        job_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<JobItemPage, JobError> {
        let jobs = self.lock();
        let items = jobs.get(job_id).map(|files| files.items.as_slice());
        Ok(page_of(items.unwrap_or(&[]), cursor, limit))
    }

    fn retry(&self, job_id: &str, item: Option<&str>) -> Result<Vec<String>, JobError> {
        let mut jobs = self.lock();
        let files = jobs.get_mut(job_id).map(|files| &mut files.items);
        let files = files.ok_or(JobError::NotFound)?;
        if item.is_some_and(|item| !files.iter().any(|i| i.item_id == item)) {
            return Err(JobError::NotFound);
        }
        Ok(requeue(files, item))
    }
}

/// Put every failed file of `files` (or the one named `item`) back to queued; answer the IDs
/// put back. Pure over the slice.
pub fn requeue(files: &mut [JobItem], item: Option<&str>) -> Vec<String> {
    let mut put_back = Vec::new();
    for file in files.iter_mut() {
        let named = item.is_none_or(|item| file.item_id == item);
        if !named || file.state != JobItemState::Failed {
            continue;
        }
        file.state = JobItemState::Queued;
        file.bytes_done = 0;
        file.error_code = None;
        put_back.push(file.item_id.clone());
    }
    put_back
}

/// At most `limit` (1 to [`MAX_LISTED_ITEMS`]) of `items` after `cursor`, the position of
/// the first item of the page; a cursor that is not a position starts at the first item.
/// Pure.
pub fn page_of(items: &[JobItem], cursor: Option<&str>, limit: usize) -> JobItemPage {
    let start = cursor
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0)
        .min(items.len());
    let limit = limit.clamp(1, MAX_LISTED_ITEMS);
    let end = start.saturating_add(limit).min(items.len());
    let next_cursor = (end < items.len()).then(|| end.to_string());
    JobItemPage {
        items: items[start..end].to_vec(),
        next_cursor,
    }
}
