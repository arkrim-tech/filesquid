//! Journal rebuild from files (Task CL-1.9; API-S-032, ADR 033 Decision 7): the second half
//! of the rebuild from files, run when the host installs a [`JournalRestore`].
//!
//! With the journal store lost, the library's files still hold its journal in two places:
//! every version a commit recorded through `CommitRecorder` carries its operation's entries
//! as `provenance/<version>.journal.jsonl`, and the library's journal object holds every
//! sealed segment. The rebuild reads both, keeps one entry per journal ID (a sealed entry
//! first, as the store held it), and appends them in ID order into the store the host
//! injects, under their own IDs. The store stores each ID once, so a second rebuild adds
//! nothing; an ID the store already holds for another entry is reported as `conflicting`
//! and the held entry is kept, never overwritten.
//!
//! A segment, object or line that cannot be read or parsed (or a line with no ID) is
//! counted as `unreadable` and the rebuild goes on; a storage or store outage stops it as
//! `unavailable`. What is lost with the store is bounded: events after the last seal that
//! changed nothing (one sealing interval while seals succeed).

use super::error::LocalLibraryError;
use super::LOG_TARGET;
use crate::canonical::{read_object_journal, CanonicalError, ProfileTransform};
use crate::journal::seal::{last_sealed_at, read_sealed_entries, SealError, JOURNAL_OBJECT_ROOT};
use crate::journal::{JournalEntry, JournalError, JournalStore};
use filesquid_ocfl::read::StorageRoot;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::ReadError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Entries appended to the store per call.
pub const RESTORE_BATCH: usize = 1_000;

/// Most distinct entries one rebuild restores (AGENTS.md §22).
pub const MAX_RESTORED_ENTRIES: usize = 10_000_000;

/// Where a host's rebuild restores the journal: the fresh journal store, and the profile
/// transform the library's files were written with.
#[derive(Clone)]
pub struct JournalRestore {
    /// The store the entries are appended to, under their own IDs.
    pub store: Arc<dyn JournalStore>,
    /// Opens the per-commit journal files and the sealed segments.
    pub transform: Arc<dyn ProfileTransform>,
}

/// What the journal rebuild restored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalRebuild {
    /// Entries the store did not hold and now does.
    pub restored: u64,
    /// Entries the store already held under the same ID.
    pub already_present: u64,
    /// Entries whose ID (or client event ID) the store holds for another entry; the held
    /// entry is kept.
    pub conflicting: u64,
    /// Segments, objects, files or lines that could not be read, and lines with no ID.
    pub unreadable: u64,
    /// When the library's last seal was committed (RFC 3339), or `None` when it was never
    /// sealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sealed_at: Option<String>,
}

/// The entries found in the files, one per journal ID.
#[derive(Default)]
struct Found {
    entries: BTreeMap<i64, JournalEntry>,
    unreadable: u64,
}

impl Found {
    /// Keep `entry` unless its ID is already kept; an entry without an ID is unreadable.
    ///
    /// # Errors
    /// `journal_too_large` past [`MAX_RESTORED_ENTRIES`].
    fn add(&mut self, entry: JournalEntry) -> Result<(), LocalLibraryError> {
        let Some(id) = entry.id else {
            self.unreadable += 1;
            return Ok(());
        };
        if self.entries.len() >= MAX_RESTORED_ENTRIES && !self.entries.contains_key(&id) {
            return Err(LocalLibraryError::refused(
                "journal_too_large",
                "The library's journal holds more entries than one rebuild restores.",
            ));
        }
        self.entries.entry(id).or_insert(entry);
        Ok(())
    }

    /// Keep the entry each line of a per-commit journal file holds.
    fn add_lines(&mut self, lines: &[String]) -> Result<(), LocalLibraryError> {
        for line in lines {
            match serde_json::from_str::<JournalEntry>(line) {
                Ok(entry) => self.add(entry)?,
                Err(_) => self.unreadable += 1,
            }
        }
        Ok(())
    }
}

/// Restore the journal of the library in `backend` into `restore`'s store. Storage and
/// journal-store I/O; needs a Tokio runtime.
///
/// # Errors
/// The open errors of the storage root, `journal_too_large`, or `unavailable`.
pub(super) async fn rebuild_journal<B: StorageBackend>(
    backend: &B,
    restore: &JournalRestore,
) -> Result<JournalRebuild, LocalLibraryError> {
    let transform = restore.transform.as_ref();
    let mut found = Found::default();
    collect_sealed(backend, transform, &mut found).await?;
    collect_committed(backend, transform, &mut found).await?;
    let last_sealed = read_last_seal(backend, &mut found).await?;
    let mut report = append_entries(restore.store.clone(), found.entries).await?;
    report.unreadable = found.unreadable;
    report.last_sealed_at = last_sealed;
    Ok(report)
}

/// Keep every entry of the library's sealed segments. Storage I/O.
async fn collect_sealed<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
    found: &mut Found,
) -> Result<(), LocalLibraryError> {
    match read_sealed_entries(backend, transform).await {
        Ok(entries) => entries.into_iter().try_for_each(|e| found.add(e)),
        Err(err) => seal_unreadable(&err, found),
    }
}

/// When the last seal ran, or `None` (counted unreadable) when the object cannot say.
/// Storage I/O.
async fn read_last_seal<B: StorageBackend>(
    backend: &B,
    found: &mut Found,
) -> Result<Option<String>, LocalLibraryError> {
    match last_sealed_at(backend).await {
        Ok(at) => Ok(at),
        Err(err) => seal_unreadable(&err, found).map(|()| None),
    }
}

/// Keep the per-commit entries of every object's versions but the journal object's.
/// Storage I/O.
async fn collect_committed<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
    found: &mut Found,
) -> Result<(), LocalLibraryError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let roots = storage
        .objects()
        .iter()
        .filter(|r| *r != JOURNAL_OBJECT_ROOT);
    for root in roots {
        let object = match storage.open_object(root).await {
            Ok(object) => object,
            Err(err) => {
                object_unreadable(&err, found)?;
                continue;
            }
        };
        match read_object_journal(backend, &object, transform, None).await {
            Ok(lines) => found.add_lines(&lines)?,
            Err(err) => file_unreadable(&err, found)?,
        }
    }
    Ok(())
}

/// Count a seal read failure unreadable; an outage stops the rebuild.
fn seal_unreadable(err: &SealError, found: &mut Found) -> Result<(), LocalLibraryError> {
    tracing::warn!(target: LOG_TARGET, error = %err, "journal segments unreadable during rebuild");
    match err {
        SealError::Unavailable(_) => Err(LocalLibraryError::Unavailable),
        SealError::Invalid(_) => {
            found.unreadable += 1;
            Ok(())
        }
    }
}

/// Count an object that cannot be opened unreadable; an outage stops the rebuild.
fn object_unreadable(err: &ReadError, found: &mut Found) -> Result<(), LocalLibraryError> {
    if let ReadError::Storage(storage) = err {
        if super::error::is_outage(storage) {
            return Err(LocalLibraryError::from_storage(storage));
        }
    }
    tracing::warn!(target: LOG_TARGET, error = %err, "object unreadable during journal rebuild");
    found.unreadable += 1;
    Ok(())
}

/// Count a per-commit journal file that cannot be read unreadable; an outage stops the
/// rebuild.
fn file_unreadable(err: &CanonicalError, found: &mut Found) -> Result<(), LocalLibraryError> {
    tracing::warn!(target: LOG_TARGET, error = ?err, "commit journal unreadable during rebuild");
    match err {
        CanonicalError::Unavailable(_) => Err(LocalLibraryError::Unavailable),
        _ => {
            found.unreadable += 1;
            Ok(())
        }
    }
}

/// Append `entries`, in ID order, to `store` off the async runtime. Journal-store I/O.
async fn append_entries(
    store: Arc<dyn JournalStore>,
    entries: BTreeMap<i64, JournalEntry>,
) -> Result<JournalRebuild, LocalLibraryError> {
    let entries: Vec<JournalEntry> = entries.into_values().collect();
    tokio::task::spawn_blocking(move || append_batches(store.as_ref(), &entries))
        .await
        .map_err(|err| {
            tracing::warn!(target: LOG_TARGET, error = %err, "journal restore task failed");
            LocalLibraryError::Internal
        })?
}

/// Append `entries` in batches and tally what each one did. Journal-store I/O.
fn append_batches(
    store: &dyn JournalStore,
    entries: &[JournalEntry],
) -> Result<JournalRebuild, LocalLibraryError> {
    let mut report = JournalRebuild::default();
    for batch in entries.chunks(RESTORE_BATCH) {
        let stored = store.append_new(batch).map_err(store_unavailable)?;
        for (entry, (_, new)) in batch.iter().zip(stored) {
            if new {
                report.restored += 1;
                continue;
            }
            tally_held(store, entry, &mut report)?;
        }
    }
    Ok(report)
}

/// Count an entry the store did not store: already present when the store holds the same
/// entry under its ID, else conflicting. Journal-store I/O.
fn tally_held(
    store: &dyn JournalStore,
    entry: &JournalEntry,
    report: &mut JournalRebuild,
) -> Result<(), LocalLibraryError> {
    let held = match entry.id {
        Some(id) => store.entry(id).map_err(store_unavailable)?,
        None => None,
    };
    match held.is_some_and(|held| same_entry(&held, entry)) {
        true => report.already_present += 1,
        false => report.conflicting += 1,
    }
    Ok(())
}

/// Whether two entries under one ID record the same event. Pure.
fn same_entry(a: &JournalEntry, b: &JournalEntry) -> bool {
    a.operation == b.operation && a.outcome == b.outcome && a.time_ms == b.time_ms
}

/// A journal-store failure as `unavailable`, logged on stdout.
fn store_unavailable(err: JournalError) -> LocalLibraryError {
    tracing::warn!(target: LOG_TARGET, error = %err, "journal store failed during rebuild");
    LocalLibraryError::Unavailable
}
