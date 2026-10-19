//! Where the history routes read records from (API-S-029; ADR 040 Decision 7).
//!
//! The host injects a [`HistorySource`] into [`crate::ApiState`]. The routes ask it for a
//! [`HistoryIndex`] of every record and for the stored JSON-LD of one version's record. The
//! index is a cache that is always rebuilt from the records, never a second source of
//! truth.
//!
//! - [`MemoryHistory`] holds records in memory: for tests and for hosts that already hold
//!   parsed records.
//! - [`OcflHistory`] reads the records of every object of an OCFL storage root through
//!   `filesquid-prov`, bounded by [`MAX_HISTORY_OBJECTS`] and [`MAX_HISTORY_RECORDS`]. It
//!   drives the engine's futures with `complete_immediately`, so it serves backends whose
//!   futures never wait (the filesystem and in-memory backends); the routes call it off the
//!   async executor.

use std::sync::Mutex;

use filesquid_core::{
    HistoryEntry, JournalLink, OperationId, OperationKind, OperationMessage, Rfc3339Utc, VersionRef,
};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{complete_immediately, StorageBackend};
use filesquid_ocfl::VersionNum;
use filesquid_prov::ocfl::{list_records, read_record};
use filesquid_prov::{
    to_json_ld, Activity, Derivation, HistoryIndex, HistoryQuery, IndexError, PageRequest, Record,
    MAX_PAGE_SIZE,
};

/// Most objects one [`OcflHistory`] index reads.
pub const MAX_HISTORY_OBJECTS: usize = 100_000;

/// Most records one index holds.
pub const MAX_HISTORY_RECORDS: usize = 100_000;

/// `tracing` target of this module's stdout-only diagnostics.
const LOG_TARGET: &str = "filesquid::history";

/// Why the history could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryError {
    /// The storage holding the records cannot be reached now; a retry may succeed.
    Unavailable,
    /// A stored record or index entry is unusable.
    Corrupt,
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => f.write_str("the history storage is unavailable"),
            Self::Corrupt => f.write_str("a history record is unusable"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// The records the history routes read, injected by the host.
pub trait HistorySource: Send + Sync + 'static {
    /// An index of every record the source holds (at most [`MAX_HISTORY_RECORDS`]), rebuilt
    /// from the records.
    ///
    /// # Errors
    ///
    /// [`HistoryError`] when the records cannot be read.
    fn index(&self) -> Result<HistoryIndex, HistoryError>;

    /// The JSON-LD of the record of `version` of the object `object_id`, or `None` when that
    /// version has no record.
    ///
    /// # Errors
    ///
    /// [`HistoryError`] when the record cannot be read.
    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError>;
}

/// The history entry the API answers for `activity` (`HistoryEntry`, API-S-029). A
/// reconstructed activity has no recorded start time.
///
/// # Errors
///
/// [`HistoryError::Corrupt`] when the activity's identifiers or times do not make an entry.
pub fn entry_of(activity: &Activity) -> Result<HistoryEntry, HistoryError> {
    let corrupt = |_| HistoryError::Corrupt;
    let operation_id = OperationId::new(activity.operation()).map_err(|_| HistoryError::Corrupt)?;
    let object = activity.object_version();
    let version = VersionRef::new(object.object_id(), object.version()).map_err(corrupt)?;
    let reconstructed = activity.derivation() == Some(Derivation::Reconstructed);
    let started_at = match reconstructed {
        true => None,
        false => Some(utc(activity.started_at().as_str())?),
    };
    let journal = JournalLink::new(operation_id.clone(), activity.journal_entries().to_vec())
        .map_err(corrupt)?;
    Ok(HistoryEntry {
        kind: OperationKind::parse(activity.kind()).unwrap_or(OperationKind::Unknown),
        operation_id,
        version,
        agents: activity.associated_with().to_vec(),
        started_at,
        ended_at: utc(activity.ended_at().as_str())?,
        journal,
        label: label_of(activity.message()),
        reconstructed,
    })
}

/// A record time as [`Rfc3339Utc`].
fn utc(text: &str) -> Result<Rfc3339Utc, HistoryError> {
    Rfc3339Utc::parse(text).map_err(|_| HistoryError::Corrupt)
}

/// The version label a `filesquid.operation.v1` message carries, if any.
fn label_of(message: Option<&str>) -> Option<String> {
    let parsed = OperationMessage::parse(message?).ok().flatten()?;
    parsed.label().map(str::to_string)
}

/// The record of `version` of `object_id` in `index`, found by walking the object's pages
/// (at most `MAX_HISTORY_RECORDS / MAX_PAGE_SIZE + 1` of them).
pub(crate) fn find_record<'a>(
    index: &'a HistoryIndex,
    object_id: &str,
    version: &str,
) -> Option<&'a Record> {
    let query = HistoryQuery::Object(object_id.to_string());
    let mut after = None;
    for _ in 0..=MAX_HISTORY_RECORDS / MAX_PAGE_SIZE {
        let request = PageRequest::new(MAX_PAGE_SIZE, after).ok()?;
        let page = index.query(&query, &request);
        let hit = page
            .items()
            .iter()
            .find(|activity| activity.object_version().version() == version);
        if let Some(activity) = hit {
            return index.record(activity.id());
        }
        after = Some(page.next()?.clone());
    }
    None
}

/// Records held in memory.
#[derive(Debug, Default)]
pub struct MemoryHistory {
    records: Mutex<Vec<Record>>,
}

impl MemoryHistory {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `record`, refusing one that conflicts with a held record of the same activity.
    ///
    /// # Errors
    ///
    /// [`IndexError::ConflictingRecord`] for a conflicting record, or when the history
    /// already holds [`MAX_HISTORY_RECORDS`].
    pub fn insert(&self, record: Record) -> Result<(), IndexError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| IndexError::ConflictingRecord)?;
        if records.len() >= MAX_HISTORY_RECORDS {
            return Err(IndexError::ConflictingRecord);
        }
        let mut check = HistoryIndex::new();
        for held in records
            .iter()
            .filter(|held| held.activity().id() == record.activity().id())
        {
            check.insert(held.clone())?;
        }
        check.insert(record.clone())?;
        records.push(record);
        Ok(())
    }

    /// A copy of the held records.
    fn snapshot(&self) -> Result<Vec<Record>, HistoryError> {
        let records = self.records.lock().map_err(|_| HistoryError::Unavailable)?;
        Ok(records.clone())
    }
}

impl HistorySource for MemoryHistory {
    fn index(&self) -> Result<HistoryIndex, HistoryError> {
        let mut index = HistoryIndex::new();
        for record in self.snapshot()? {
            index.insert(record).map_err(|_| HistoryError::Corrupt)?;
        }
        Ok(index)
    }

    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError> {
        let index = self.index()?;
        let found = find_record(&index, object_id, version);
        Ok(found.map(|record| to_json_ld(record).into_bytes()))
    }
}

/// The records of every object of an OCFL storage root.
#[derive(Debug)]
pub struct OcflHistory<B> {
    backend: B,
}

impl<B: StorageBackend + Send + Sync + 'static> OcflHistory<B> {
    /// The history of the storage root on `backend`, whose futures must never wait.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Read every record of the storage root into an index (storage I/O).
    async fn read_index(&self) -> Result<HistoryIndex, HistoryError> {
        let storage = StorageRoot::open(&self.backend)
            .await
            .map_err(unavailable)?;
        let mut index = HistoryIndex::new();
        let mut budget = MAX_HISTORY_RECORDS;
        for root in storage.objects().iter().take(MAX_HISTORY_OBJECTS) {
            let Some(object) = open_object(&storage, root).await else {
                continue;
            };
            budget = read_object_records(&object, &mut index, budget).await;
            if budget == 0 {
                tracing::warn!(target: LOG_TARGET, "history index stopped at its record cap");
                break;
            }
        }
        Ok(index)
    }

    /// Read the stored record of `version` of `object_id` (storage I/O).
    async fn read_one(
        &self,
        object_id: &str,
        version: &str,
    ) -> Result<Option<Vec<u8>>, HistoryError> {
        let Ok(version) = VersionNum::parse(version) else {
            return Ok(None);
        };
        let storage = StorageRoot::open(&self.backend)
            .await
            .map_err(unavailable)?;
        for root in storage.objects().iter().take(MAX_HISTORY_OBJECTS) {
            let Some(object) = open_object(&storage, root).await else {
                continue;
            };
            if object.id() != object_id || !object.inventory().versions.contains_key(&version) {
                continue;
            }
            let record = read_record(&object, &version).await.map_err(|err| {
                tracing::warn!(target: LOG_TARGET, error = %err, "history record unreadable");
                HistoryError::Corrupt
            })?;
            return Ok(record.map(|record| to_json_ld(&record).into_bytes()));
        }
        Ok(None)
    }
}

impl<B: StorageBackend + Send + Sync + 'static> HistorySource for OcflHistory<B> {
    fn index(&self) -> Result<HistoryIndex, HistoryError> {
        complete_immediately(self.read_index()).map_err(unavailable)?
    }

    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError> {
        complete_immediately(self.read_one(object_id, version)).map_err(unavailable)?
    }
}

/// Open the object at `root`, or log and skip it when it cannot be read.
async fn open_object<'a, B: StorageBackend>(
    storage: &StorageRoot<'a, B>,
    root: &str,
) -> Option<Object<'a, B>> {
    match storage.open_object(root).await {
        Ok(object) => Some(object),
        Err(err) => {
            tracing::warn!(target: LOG_TARGET, error = %err, "history skipped an unreadable object");
            None
        }
    }
}

/// Add the records of `object` to `index`, at most `budget` of them; return what is left of
/// the budget. Unreadable or conflicting records are logged and skipped.
async fn read_object_records<B: StorageBackend>(
    object: &Object<'_, B>,
    index: &mut HistoryIndex,
    budget: usize,
) -> usize {
    let versions = list_records(object);
    let mut left = budget;
    for version in versions.iter().take(budget) {
        left -= 1;
        let record = match read_record(object, version).await {
            Ok(Some(record)) => record,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(target: LOG_TARGET, error = %err, "history skipped a record");
                continue;
            }
        };
        if let Err(err) = index.insert(record) {
            tracing::warn!(target: LOG_TARGET, error = %err, "history skipped a conflicting record");
        }
    }
    left
}

/// Any storage failure as [`HistoryError::Unavailable`], logged on stdout.
fn unavailable<E: std::fmt::Display>(err: E) -> HistoryError {
    tracing::warn!(target: LOG_TARGET, error = %err, "history storage unavailable");
    HistoryError::Unavailable
}
