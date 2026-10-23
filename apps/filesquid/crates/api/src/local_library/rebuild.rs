//! Rebuild from files (F-001, F-042): what every object of a storage root holds, read from
//! the OCFL files alone, so a host can rebuild its catalog without its operational database
//! or a running service.
//!
//! Each object is opened at its committed head and validated in full. Validator errors, such
//! as a digest mismatch left by an external edit, are reported per object and never adopted
//! or repaired; an object whose head has no version inventory (W010) is flagged as a repair
//! candidate, not repaired. An object that cannot be opened, resolved or validated is
//! reported by its error code only, and the rebuild goes on. Nothing is written to the
//! storage root; the journal half (`journal_rebuild`) writes only to the journal store.
//!
//! A rebuild may start after a cursor, the root of the last object a durable job checkpointed
//! (`rebuild_job`, API-S-025 **Rebuild as a job**), so a resumed job reads no completed object
//! again; its [`RebuildProgress`] is told about each object and may stop it.

use super::candidates::head_files;
use super::error::{is_outage, LocalLibraryError};
use super::journal_rebuild::JournalRebuild;
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::{validate_object, ReadError};
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

/// One object rebuilt from its files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuiltObject {
    /// The object identifier.
    pub object_id: String,
    /// The committed head version.
    pub head: String,
    /// Logical files in the head version.
    pub files: usize,
    /// The object's digest algorithm (`sha256` or `sha512`).
    pub digest_algorithm: String,
    /// Distinct OCFL validation error codes (`E###`), sorted; empty for a valid object.
    pub error_codes: Vec<String>,
    /// Whether the head lacks its version inventory or its sidecar (W010, or an interrupted
    /// repair): a candidate for the consented repair.
    pub missing_version_inventory: bool,
}

/// An object that could not be opened. Its root is not reported (API-S-005).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadableObject {
    /// The OCFL validation code when an OCFL rule is broken (`E###`), else `unreadable`.
    pub error_code: String,
}

/// Everything a rebuild found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildReport {
    /// Objects read, in object-root order.
    pub objects: Vec<RebuiltObject>,
    /// Objects that could not be opened.
    pub unreadable: Vec<UnreadableObject>,
    /// What the journal rebuild restored, when the host installed a journal restore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<JournalRebuild>,
}

/// What a rebuild found of one object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebuildEntry {
    /// The object was read.
    Read(RebuiltObject),
    /// The object could not be opened or read.
    Unreadable(UnreadableObject),
}

/// One object a rebuild went through, as its [`RebuildProgress`] is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebuildStep<'a> {
    /// The object's root in the storage root: the cursor a resumed rebuild starts after. It
    /// is a storage key, so it MUST NOT be answered or journaled (API-S-005).
    pub root: &'a str,
    /// What was found.
    pub entry: RebuildEntry,
    /// Objects gone through so far, in object-root order, those before the cursor included.
    pub done: u64,
    /// Objects in the storage root.
    pub total: u64,
}

/// Where a rebuild is told about each object it goes through, and may stop it.
pub trait RebuildProgress {
    /// `step` was gone through. [`ControlFlow::Break`] stops the rebuild before the next
    /// object.
    fn object(&mut self, step: RebuildStep<'_>) -> ControlFlow<()>;
}

/// How a rebuild from a cursor ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebuildEnd {
    /// Every object after the cursor was gone through.
    Finished {
        /// Objects in the storage root.
        total: u64,
        /// What the journal rebuild restored, when the host installed a journal restore.
        journal: Option<JournalRebuild>,
    },
    /// The progress stopped it before its last object.
    Stopped,
}

/// The progress of a one-shot rebuild: every object collected into one report.
struct Collect(RebuildReport);

impl RebuildProgress for Collect {
    fn object(&mut self, step: RebuildStep<'_>) -> ControlFlow<()> {
        match step.entry {
            RebuildEntry::Read(object) => self.0.objects.push(object),
            RebuildEntry::Unreadable(item) => self.0.unreadable.push(item),
        }
        ControlFlow::Continue(())
    }
}

/// Read every object of `backend`'s storage root from its files. Writes nothing.
///
/// # Errors
///
/// The open errors of the storage root; `unavailable` for a storage failure.
pub(super) async fn rebuild<B: StorageBackend>(
    backend: &B,
) -> Result<RebuildReport, LocalLibraryError> {
    let mut collect = Collect(RebuildReport {
        objects: Vec::new(),
        unreadable: Vec::new(),
        journal: None,
    });
    rebuild_from(backend, None, &mut collect).await?;
    Ok(collect.0)
}

/// Read, in object-root order, every object of `backend`'s storage root whose root sorts
/// after `after` (all of them without a cursor), telling `progress` about each one; an object
/// at or before the cursor is not opened. Writes nothing. Answers the number of objects in
/// the storage root, or `None` when `progress` stopped it.
///
/// # Errors
///
/// The open errors of the storage root; `unavailable` for a storage failure.
pub(super) async fn rebuild_from<B: StorageBackend>(
    backend: &B,
    after: Option<&str>,
    progress: &mut (dyn RebuildProgress + Send),
) -> Result<Option<u64>, LocalLibraryError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let roots = storage.objects();
    let total = roots.len() as u64;
    let start = after.map_or(0, |after| {
        roots.partition_point(|root| root.as_str() <= after)
    });
    for (index, root) in roots.iter().enumerate().skip(start) {
        let entry = read_object(backend, &storage, root).await?;
        let done = index as u64 + 1;
        let step = RebuildStep {
            root,
            entry,
            done,
            total,
        };
        if progress.object(step).is_break() {
            return Ok(None);
        }
    }
    Ok(Some(total))
}

/// What one object holds, or why it is unreadable.
///
/// # Errors
///
/// `unavailable` (or `internal`) for a storage outage, which stops the rebuild.
async fn read_object<B: StorageBackend>(
    backend: &B,
    storage: &StorageRoot<'_, B>,
    root: &str,
) -> Result<RebuildEntry, LocalLibraryError> {
    let read = match storage.open_object(root).await {
        Ok(object) => rebuilt(backend, &object).await,
        Err(err) => Err(err),
    };
    match read {
        Ok(object) => Ok(RebuildEntry::Read(object)),
        Err(err) => unreadable(&err).map(RebuildEntry::Unreadable),
    }
}

/// The rebuilt view of one opened object.
///
/// # Errors
///
/// A [`ReadError`] when its head state cannot be resolved or it cannot be validated (for
/// example an engine bound); the rebuild records it for this object and goes on.
async fn rebuilt<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
) -> Result<RebuiltObject, ReadError> {
    let files = object.state(object.head())?.len();
    let report = validate_object(backend, object.root()).await?;
    let head = head_files(backend, object).await?;
    Ok(RebuiltObject {
        object_id: object.id().to_string(),
        head: object.head().to_string(),
        files,
        digest_algorithm: object.inventory().digest_algorithm.name().to_string(),
        error_codes: report.error_codes().into_iter().map(String::from).collect(),
        missing_version_inventory: !head.complete(),
    })
}

/// The report entry of an object that could not be opened or read; a storage outage stops
/// the rebuild instead.
fn unreadable(err: &ReadError) -> Result<UnreadableObject, LocalLibraryError> {
    if let ReadError::Storage(storage) = err {
        if is_outage(storage) {
            return Err(LocalLibraryError::from_storage(storage));
        }
    }
    tracing::info!(target: super::LOG_TARGET, error = %err, "object unreadable during rebuild");
    Ok(UnreadableObject {
        error_code: err.code().unwrap_or("unreadable").to_string(),
    })
}
