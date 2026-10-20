//! Where the history routes read records from (API-S-029; ADR 040 Decision 7).
//!
//! The host injects a [`HistorySource`] into [`crate::ApiState`]. The routes ask it for a
//! [`HistorySnapshot`] (a [`HistoryIndex`] of every record, flagged when a cap cut it short)
//! and for the stored JSON-LD of one version's record. The index is a cache that is always
//! rebuilt from the records, never a second source of truth.
//!
//! - [`MemoryHistory`] holds records in memory: for tests and for hosts that already hold
//!   parsed records.
//! - [`OcflHistory`] reads the records of every object of an OCFL storage root through
//!   `filesquid-prov`, bounded by [`MAX_HISTORY_OBJECTS`] and [`MAX_HISTORY_RECORDS`]. It
//!   drives the engine's futures with `complete_immediately`, so it serves backends whose
//!   futures never wait (the filesystem and in-memory backends); the routes call it off the
//!   async executor.

use std::sync::{Arc, Mutex, PoisonError};

use filesquid_core::{
    HistoryEntry, JournalLink, OperationId, OperationKind, OperationMessage, Rfc3339Utc, VersionRef,
};
use filesquid_ocfl::read::{Object, ReadError, StorageRoot};
use filesquid_ocfl::storage::{complete_immediately, StorageBackend};
use filesquid_ocfl::VersionNum;
use filesquid_prov::ocfl::{list_records, read_record, RecordError};
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

/// An index of the records a source holds, and whether a cap cut it short.
#[derive(Debug)]
pub struct HistorySnapshot {
    /// The indexed records.
    pub index: HistoryIndex,
    /// True when the source holds records the index left out, because it reached
    /// [`MAX_HISTORY_RECORDS`] or [`MAX_HISTORY_OBJECTS`] or an object could not be opened:
    /// answers built from it may be partial.
    pub truncated: bool,
}

impl HistorySnapshot {
    /// A snapshot of `index` that holds every record of its source.
    pub fn complete(index: HistoryIndex) -> Self {
        Self {
            index,
            truncated: false,
        }
    }
}

/// The records the history routes read, injected by the host.
pub trait HistorySource: Send + Sync + 'static {
    /// An index of every record the source holds (at most [`MAX_HISTORY_RECORDS`]), rebuilt
    /// from the records, flagged when a cap cut it short.
    ///
    /// # Errors
    ///
    /// [`HistoryError`] when the records cannot be read.
    fn index(&self) -> Result<HistorySnapshot, HistoryError>;

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
    /// A record identical to a held one is accepted and not held twice.
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
        if records.contains(&record) {
            return Ok(());
        }
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

    /// How many records the history holds.
    pub fn len(&self) -> usize {
        self.records
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether the history holds no record.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A copy of the held records.
    fn snapshot(&self) -> Result<Vec<Record>, HistoryError> {
        let records = self.records.lock().map_err(|_| HistoryError::Unavailable)?;
        Ok(records.clone())
    }
}

impl HistorySource for MemoryHistory {
    /// Every held record; [`MemoryHistory::insert`] refuses records past the cap, so the
    /// snapshot is never truncated.
    fn index(&self) -> Result<HistorySnapshot, HistoryError> {
        let mut index = HistoryIndex::new();
        for record in self.snapshot()? {
            index.insert(record).map_err(|_| HistoryError::Corrupt)?;
        }
        Ok(HistorySnapshot::complete(index))
    }

    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError> {
        let snapshot = self.index()?;
        let found = find_record(&snapshot.index, object_id, version);
        Ok(found.map(|record| to_json_ld(record).into_bytes()))
    }
}

/// The records of every object of an OCFL storage root.
#[derive(Debug)]
pub struct OcflHistory<B> {
    backend: B,
    max_records: usize,
}

impl<B: StorageBackend + Send + Sync + 'static> OcflHistory<B> {
    /// The history of the storage root on `backend`, whose futures must never wait.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            max_records: MAX_HISTORY_RECORDS,
        }
    }

    /// Index at most `max` records (1 to [`MAX_HISTORY_RECORDS`]); past it the snapshot is
    /// flagged truncated.
    #[must_use]
    pub fn with_max_records(mut self, max: usize) -> Self {
        self.max_records = max.clamp(1, MAX_HISTORY_RECORDS);
        self
    }

    /// Read every record of the storage root into an index (storage I/O), flagged truncated
    /// when the record or object cap left records out, or an object could not be opened.
    async fn read_index(&self) -> Result<HistorySnapshot, HistoryError> {
        let storage = StorageRoot::open(&self.backend)
            .await
            .map_err(unavailable)?;
        let mut reading = Reading::new(self.max_records);
        reading.root(&storage).await;
        Ok(reading.finish())
    }
}

impl<B: StorageBackend + Send + Sync + 'static> HistorySource for OcflHistory<B> {
    fn index(&self) -> Result<HistorySnapshot, HistoryError> {
        complete_immediately(self.read_index()).map_err(unavailable)?
    }

    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError> {
        let mut objects = MAX_HISTORY_OBJECTS;
        let found = read_one(&self.backend, object_id, version, &mut objects);
        complete_immediately(found).map_err(unavailable)?
    }
}

/// One storage root a [`HistoryRoots`] lists: its backend, or why it could not be opened.
pub type RootResult<B> = Result<Arc<B>, HistoryError>;

/// Most storage roots one [`OcflRootsHistory`] reads.
pub const MAX_HISTORY_ROOTS: usize = 256;

/// The OCFL storage roots a host serves, listed again for every read, because a host opens
/// and closes libraries while it runs. The host implements it over its own storage.
pub trait HistoryRoots: Send + Sync + 'static {
    /// The backend of one storage root; its futures must never wait.
    type Backend: StorageBackend + Send + Sync + 'static;

    /// The storage roots served now, each opened or the reason it could not be: a root that
    /// failed is left out and the answers flagged incomplete, while the others are still
    /// read. Past [`MAX_HISTORY_ROOTS`] the rest are left out the same way.
    ///
    /// # Errors
    ///
    /// [`HistoryError`] when the host cannot list its storage roots now.
    fn roots(&self) -> Result<Vec<RootResult<Self::Backend>>, HistoryError>;
}

/// The records of every object of every storage root a host serves, as one history: what a
/// host that serves several libraries installs. One record cap and one object cap hold
/// across all roots; a root that cannot be opened is logged and left out, and the answers
/// say so (the index is flagged truncated, a lookup that found nothing is unavailable).
#[derive(Debug)]
pub struct OcflRootsHistory<R> {
    roots: R,
    max_records: usize,
}

impl<R: HistoryRoots> OcflRootsHistory<R> {
    /// The history of the storage roots `roots` lists.
    pub fn new(roots: R) -> Self {
        Self {
            roots,
            max_records: MAX_HISTORY_RECORDS,
        }
    }

    /// Index at most `max` records across all roots (1 to [`MAX_HISTORY_RECORDS`]); past it
    /// the snapshot is flagged truncated.
    #[must_use]
    pub fn with_max_records(mut self, max: usize) -> Self {
        self.max_records = max.clamp(1, MAX_HISTORY_RECORDS);
        self
    }

    /// Read the records of every listed root into one index (storage I/O).
    async fn read_index(&self) -> Result<HistorySnapshot, HistoryError> {
        let roots = self.roots.roots()?;
        let mut reading = Reading::new(self.max_records);
        reading.skipped = roots.len() > MAX_HISTORY_ROOTS;
        for listed in roots.iter().take(MAX_HISTORY_ROOTS) {
            let Ok(backend) = listed else {
                reading.skipped = true;
                continue;
            };
            match StorageRoot::open(backend.as_ref()).await {
                Ok(storage) => reading.root(&storage).await,
                Err(err) => {
                    tracing::warn!(target: LOG_TARGET, error = %err, "history skipped a storage root");
                    reading.skipped = true;
                }
            }
            if reading.snapshot.truncated {
                break;
            }
        }
        Ok(reading.finish())
    }

    /// Find the record of `version` of `object_id` in the listed roots (storage I/O),
    /// opening at most [`MAX_HISTORY_OBJECTS`] objects across all of them. When no root
    /// holds it but one could not be read, or the object cap left objects unopened, it may be
    /// there: unavailable, not `None`.
    async fn read_any(
        &self,
        object_id: &str,
        version: &str,
    ) -> Result<Option<Vec<u8>>, HistoryError> {
        let roots = self.roots.roots()?;
        let mut skipped = roots.len() > MAX_HISTORY_ROOTS;
        let mut objects = MAX_HISTORY_OBJECTS;
        for listed in roots.iter().take(MAX_HISTORY_ROOTS) {
            let Ok(backend) = listed else {
                skipped = true;
                continue;
            };
            match read_one(backend.as_ref(), object_id, version, &mut objects).await {
                Ok(Some(found)) => return Ok(Some(found)),
                Ok(None) => {}
                Err(HistoryError::Unavailable) => skipped = true,
                Err(err) => return Err(err),
            }
        }
        match skipped {
            true => Err(HistoryError::Unavailable),
            false => Ok(None),
        }
    }
}

impl<R: HistoryRoots> HistorySource for OcflRootsHistory<R> {
    fn index(&self) -> Result<HistorySnapshot, HistoryError> {
        complete_immediately(self.read_index()).map_err(unavailable)?
    }

    fn record(&self, object_id: &str, version: &str) -> Result<Option<Vec<u8>>, HistoryError> {
        complete_immediately(self.read_any(object_id, version)).map_err(unavailable)?
    }
}

/// One index being read from one or more storage roots: the snapshot, the records and
/// objects it may still take, and whether anything was left out.
struct Reading {
    snapshot: HistorySnapshot,
    budget: usize,
    objects: usize,
    skipped: bool,
}

impl Reading {
    /// An empty index that takes at most `max_records` records and [`MAX_HISTORY_OBJECTS`]
    /// objects.
    fn new(max_records: usize) -> Self {
        Self {
            snapshot: HistorySnapshot::complete(HistoryIndex::new()),
            budget: max_records,
            objects: MAX_HISTORY_OBJECTS,
            skipped: false,
        }
    }

    /// Add the records of every object of `storage` (storage I/O), until a cap is reached.
    async fn root<B: StorageBackend>(&mut self, storage: &StorageRoot<'_, B>) {
        let roots = storage.objects();
        self.skipped |= roots.len() > self.objects;
        let take = roots.len().min(self.objects);
        self.objects -= take;
        for root in roots.iter().take(take) {
            let Some(object) = open_object(storage, root).await else {
                self.skipped = true;
                continue;
            };
            self.budget = read_object_records(&object, &mut self.snapshot, self.budget).await;
            if self.snapshot.truncated {
                return;
            }
        }
    }

    /// The snapshot, flagged truncated when anything was left out (logged on stdout).
    fn finish(mut self) -> HistorySnapshot {
        self.snapshot.truncated |= self.skipped;
        if self.snapshot.truncated {
            tracing::warn!(
                target: LOG_TARGET,
                "history index is incomplete: a cap was reached or an object was unreadable"
            );
        }
        self.snapshot
    }
}

/// Read the stored record of `version` of `object_id` from the storage root on `backend`
/// (storage I/O), opening at most `objects` objects and taking those it visits off that
/// shared budget. When no object matched but some could not be opened (or lie past the
/// budget), the target may be among them, so the answer is [`HistoryError::Unavailable`],
/// not `None`.
async fn read_one<B: StorageBackend>(
    backend: &B,
    object_id: &str,
    version: &str,
    objects: &mut usize,
) -> Result<Option<Vec<u8>>, HistoryError> {
    let Ok(version) = VersionNum::parse(version) else {
        return Ok(None);
    };
    let storage = StorageRoot::open(backend).await.map_err(unavailable)?;
    let roots = storage.objects();
    let mut skipped = roots.len() > *objects;
    let take = roots.len().min(*objects);
    *objects -= take;
    for root in roots.iter().take(take) {
        let Some(object) = open_object(&storage, root).await else {
            skipped = true;
            continue;
        };
        if object.id() != object_id || !object.inventory().versions.contains_key(&version) {
            continue;
        }
        let record = read_record(&object, &version)
            .await
            .map_err(record_failure)?;
        return subject_checked(record, object_id, &version);
    }
    match skipped {
        true => Err(unavailable(
            "an object that may hold the record is unreadable",
        )),
        false => Ok(None),
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

/// Add the records of `object` to `snapshot`, at most `budget` of them; return what is left
/// of the budget and flag the snapshot truncated when records were left out. Unreadable,
/// misplaced or conflicting records are logged and skipped.
async fn read_object_records<B: StorageBackend>(
    object: &Object<'_, B>,
    snapshot: &mut HistorySnapshot,
    budget: usize,
) -> usize {
    let versions = list_records(object);
    snapshot.truncated |= versions.len() > budget;
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
        if !is_record_of(&record, object.id(), version) {
            tracing::warn!(target: LOG_TARGET, "history skipped a record of another version");
            continue;
        }
        if let Err(err) = snapshot.index.insert(record) {
            tracing::warn!(target: LOG_TARGET, error = %err, "history skipped a conflicting record");
        }
    }
    left
}

/// Whether `record` is the record of `version` of `object_id`, the subject its stored path
/// names.
fn is_record_of(record: &Record, object_id: &str, version: &VersionNum) -> bool {
    let subject = record.activity().object_version();
    subject.object_id() == object_id && subject.version() == version.to_string()
}

/// The JSON-LD of `record` when it is the record of `version` of `object_id`.
///
/// # Errors
///
/// [`HistoryError::Corrupt`] when the stored record describes another object or version.
fn subject_checked(
    record: Option<Record>,
    object_id: &str,
    version: &VersionNum,
) -> Result<Option<Vec<u8>>, HistoryError> {
    let Some(record) = record else {
        return Ok(None);
    };
    if !is_record_of(&record, object_id, version) {
        tracing::warn!(target: LOG_TARGET, "a history record is stored under another version");
        return Err(HistoryError::Corrupt);
    }
    Ok(Some(to_json_ld(&record).into_bytes()))
}

/// A failed record read as a [`HistoryError`], logged on stdout: a storage failure is
/// [`HistoryError::Unavailable`] (a retry may succeed); a record that breaks the OCFL format
/// or the profile is [`HistoryError::Corrupt`].
fn record_failure(err: RecordError) -> HistoryError {
    if matches!(err, RecordError::Read(ReadError::Storage(_))) {
        return unavailable(err);
    }
    tracing::warn!(target: LOG_TARGET, error = %err, "history record unreadable");
    HistoryError::Corrupt
}

/// Any storage failure as [`HistoryError::Unavailable`], logged on stdout.
fn unavailable<E: std::fmt::Display>(err: E) -> HistoryError {
    tracing::warn!(target: LOG_TARGET, error = %err, "history storage unavailable");
    HistoryError::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use filesquid_ocfl::storage::StorageError;

    #[test]
    fn a_storage_failure_is_unavailable_and_a_bad_record_is_corrupt() {
        let storage = StorageError::NotFound { key: "k".into() };
        let read = RecordError::Read(ReadError::Storage(storage));
        assert_eq!(record_failure(read), HistoryError::Unavailable);
        assert_eq!(record_failure(RecordError::TooLarge), HistoryError::Corrupt);
    }
}
