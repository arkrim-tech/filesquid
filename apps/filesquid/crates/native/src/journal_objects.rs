//! The desktop's journal objects (Task DE-0.11; DE-S-013, API-S-032,
//! [ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)
//! Decision 7): where the API's sealing service seals the activity journal on the desktop, and
//! the refill of a lost operational store from what it sealed.
//!
//! - **Installation.** Host events are sealed into the installation's journal object, in an
//!   OCFL storage root of its own at `<data dir>/journal.ocfl/` ([`INSTALLATION_JOURNAL_DIR`]),
//!   created on the first launch that needs it. Its fenced leases and staging area live beside
//!   it in the data directory (`journal-work/`), never inside the root.
//! - **Libraries.** A library's events are sealed into that library's journal object, in the
//!   library's own storage root (the folder the owner chose), through the library's shared
//!   leases, staging in the library's folder under the data directory.
//! - **Refill.** When the operational store (`filesquid.sqlite`) was lost, the installation's
//!   sealed segments are appended back through the SQLite journal store under their own
//!   journal IDs; the store keeps each ID once, so a second refill adds nothing. A library's
//!   own journal comes back with that library's rebuild from files (CL-1.9).
//!
//! Errors name no path.

use crate::files::{backend_at, library_dir_name, real_dir, FILES_DIR};
use crate::locations::HostRoot;
use crate::HostLeases;
use crate::SqliteJournal;
use filesquid_api::canonical::{create_storage_root, OwnerReadable, ProfileTransform};
use filesquid_api::journal::seal::{
    visit_sealed_segments, JournalObject, JournalRoot, OcflJournalObject, SealError,
    JOURNAL_OBJECT_ROOT, SEGMENT_DIR,
};
use filesquid_api::journal::{JournalEntry, JournalStore};
use filesquid_core::{InstallationId, LibraryId};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{FsBackend, StorageBackend};
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::path::Path;
use std::sync::Arc;

/// The installation's journal storage root, under the data directory.
pub const INSTALLATION_JOURNAL_DIR: &str = "journal.ocfl";

/// The installation journal's leases, staging and multipart parts, under the data directory.
const WORK_DIR: &str = "journal-work";

/// The reservation holder name of the desktop's seal runs.
pub const SEAL_HOLDER: &str = "desktop.journal.seal";

/// Entries appended to the store per call during a refill.
const REFILL_BATCH: usize = 1_000;

/// What a refill of the operational store restored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refill {
    /// Entries the store did not hold and now does.
    pub restored: u64,
    /// Entries the store already held under their ID.
    pub already_present: u64,
    /// Segments that could not be read, and sealed entries without a journal ID (the rest is
    /// restored).
    pub unreadable: u64,
}

/// Open the installation's journal object at `<data_dir>/journal.ocfl/`, creating the
/// storage root on first use, sealed as `installation`. Filesystem I/O.
///
/// # Errors
/// A message naming what could not be prepared; it never names a path.
pub async fn open_installation_journal(
    data_dir: &Path,
    installation: &InstallationId,
) -> Result<Arc<dyn JournalObject>, String> {
    let backend = installation_backend(data_dir)?;
    ensure_storage_root(&backend).await?;
    let work = data_dir.join(WORK_DIR);
    real_dir(&work, "the journal's work folder")?;
    let leases = host_leases(&work)?;
    let staging = backend_at(&work, "staging", "the journal's staging area")?;
    let root = journal_root(Arc::new(backend), Arc::new(leases), staging);
    let object = OcflJournalObject::new(root, installation.as_str(), installation);
    Ok(Arc::new(object))
}

/// The journal object of `library_id` in its storage root `root`, through the root's shared
/// leases, staging under the library's folder in `data_dir`, sealed as `installation`.
/// Creates directories.
///
/// # Errors
/// A message naming what could not be prepared; it never names a path.
pub fn library_journal(
    root: &HostRoot,
    library_id: &LibraryId,
    data_dir: &Path,
    installation: &InstallationId,
) -> Result<Arc<dyn JournalObject>, String> {
    let files_dir = data_dir.join(FILES_DIR);
    let dir = files_dir.join(library_dir_name(library_id));
    real_dir(&files_dir, "the files folder")?;
    real_dir(&dir, "the library's folder")?;
    let staging = backend_at(&dir, "journal-staging", "the journal's staging area")?;
    let journal = journal_root(root.backend.clone(), root.reservations.clone(), staging);
    let object = OcflJournalObject::new(journal, library_id.as_str(), installation);
    Ok(Arc::new(object))
}

/// Append every entry the installation's sealed segments hold to `store`, under its own
/// journal ID, segment by segment; an entry the store already holds is counted, not stored
/// again, and a segment that cannot be read is counted and skipped. Then reserve the ids
/// through the highest id any seal read ([`SqliteJournal::reserve_through`]): the
/// installation's seals read every entry, a library's included, so no new entry takes the id
/// of an entry a library's rebuild will restore. Run it before anything else writes to a new
/// store. Restores nothing, and creates nothing, when the data directory has no installation
/// journal. Filesystem and database I/O.
///
/// # Errors
/// The journal object cannot be opened, the store refuses an append, or the ids cannot be
/// reserved; never names a path.
pub async fn restore_installation_journal(
    data_dir: &Path,
    store: &SqliteJournal,
) -> Result<Refill, String> {
    if !data_dir.join(INSTALLATION_JOURNAL_DIR).is_dir() {
        return Ok(Refill::default());
    }
    let backend = installation_backend(data_dir)?;
    let mut refill = Refill::default();
    let mut failure = None;
    let mut keep = |segment: Result<Vec<JournalEntry>, SealError>| {
        let appended = segment
            .map_err(|err| tracing::warn!(error = err.code(), "a sealed segment is unreadable"))
            .map(|entries| append_segment(store, &entries, &mut refill));
        match appended {
            Err(()) => refill.unreadable += 1,
            Ok(Err(err)) => failure = Some(err),
            Ok(Ok(())) => {}
        }
        failure.is_none()
    };
    visit_sealed_segments(&backend, &OwnerReadable, &mut keep)
        .await
        .map_err(|err| {
            format!(
                "the installation journal could not be read ({})",
                err.code()
            )
        })?;
    if let Some(err) = failure {
        return Err(err);
    }
    let through = sealed_through(&backend).await?;
    store
        .reserve_through(through)
        .map_err(|_| "the journal store could not reserve the sealed ids".to_string())?;
    Ok(refill)
}

/// The highest journal ID the installation's seals read: the largest `through` of the
/// segment paths at the head of its journal object (`segments/<first>-<through>.jsonl`, two
/// 20-digit IDs); 0 without the object. Storage I/O.
///
/// # Errors
/// The object cannot be opened.
async fn sealed_through(backend: &FsBackend) -> Result<i64, String> {
    let unreadable = || "the installation journal could not be read".to_string();
    let page = backend
        .list(&format!("{JOURNAL_OBJECT_ROOT}/"), None, 1)
        .await
        .map_err(|_| unreadable())?;
    if page.keys.is_empty() {
        return Ok(0);
    }
    let object = Object::open(backend, JOURNAL_OBJECT_ROOT)
        .await
        .map_err(|_| unreadable())?;
    let state = object.state(object.head()).map_err(|_| unreadable())?;
    Ok(state.paths().filter_map(segment_through).max().unwrap_or(0))
}

/// The `through` ID of a segment path, only when the path is exactly the canonical form
/// `segments/<first>-<through>.jsonl`: two 20-digit IDs with `first <= through`. Pure.
fn segment_through(path: &str) -> Option<i64> {
    let name = path.strip_prefix(&format!("{SEGMENT_DIR}/"))?;
    let (first, through) = name.strip_suffix(".jsonl")?.split_once('-')?;
    let (first, through) = (canonical_id(first)?, canonical_id(through)?);
    (first <= through).then_some(through)
}

/// A journal ID written as exactly 20 ASCII digits. Pure.
fn canonical_id(text: &str) -> Option<i64> {
    let canonical = text.len() == 20 && text.bytes().all(|b| b.is_ascii_digit());
    canonical.then(|| text.parse().ok()).flatten()
}

/// Append one segment's `entries` to `store` in batches, counting what was stored now and
/// what was held already. An entry without a journal ID is counted as unreadable and never
/// appended, since the store would give it a new ID. Database I/O.
///
/// # Errors
/// The store refused an append.
fn append_segment(
    store: &dyn JournalStore,
    entries: &[JournalEntry],
    refill: &mut Refill,
) -> Result<(), String> {
    let (with_id, without): (Vec<JournalEntry>, Vec<JournalEntry>) = entries
        .iter()
        .cloned()
        .partition(|entry| entry.id.is_some());
    refill.unreadable += without.len() as u64;
    for batch in with_id.chunks(REFILL_BATCH) {
        let stored = store
            .append_new(batch)
            .map_err(|_| "the journal store refused the refill".to_string())?;
        let fresh = stored.iter().filter(|(_, new)| *new).count() as u64;
        refill.restored += fresh;
        refill.already_present += stored.len() as u64 - fresh;
    }
    Ok(())
}

/// The marker of a refill that has not finished, in the data directory. It is written before
/// the refill recreates the store and removed only when the refill succeeded, so a refill that
/// failed (leaving a new, incomplete `filesquid.sqlite` behind) runs again at the next launch.
pub const REFILL_PENDING_FILE: &str = "journal-refill.pending";

/// The journal ID a store's counter is moved to when its refill failed: 2^40, far above any ID
/// a sealed journal holds, and still exact as a JavaScript number. Entries the launch writes
/// after a failed refill then never take an ID the retry has still to restore.
pub const FAILED_REFILL_FLOOR: i64 = 1 << 40;

/// When `store_lost` (the launch found no `filesquid.sqlite`), or an earlier refill did not
/// finish ([`REFILL_PENDING_FILE`]), and `data_dir` holds the installation's sealed journal:
/// mark the refill pending, recreate (or open) the operational store, refill it
/// ([`restore_installation_journal`]) and clear the mark once that succeeded. When it fails,
/// the store's ID counter moves to [`FAILED_REFILL_FLOOR`], so the launch, which goes on with
/// that store, writes no entry under an ID still to be restored. A second run over a partly
/// refilled store adds only what is missing. `None` when there was nothing to refill.
/// Run it before anything else opens the store. Filesystem and database I/O.
pub async fn refill_lost_store(
    data_dir: &Path,
    store_lost: bool,
) -> Option<Result<Refill, String>> {
    let marker = data_dir.join(REFILL_PENDING_FILE);
    let pending = store_lost || marker.is_file();
    if !pending || !data_dir.join(INSTALLATION_JOURNAL_DIR).is_dir() {
        return None;
    }
    if std::fs::write(&marker, b"").is_err() {
        return Some(Err("the refill could not be marked as pending".to_string()));
    }
    let refilled = refill_store(data_dir).await;
    match &refilled {
        Ok(_) => clear_marker(&marker),
        Err(_) => reserve_after_failure(data_dir),
    }
    Some(refilled)
}

/// Remove the pending-refill marker of a finished refill; a failure is logged (stdout), and
/// the next launch then refills again, which adds nothing. Filesystem I/O.
fn clear_marker(marker: &Path) {
    if let Err(err) = std::fs::remove_file(marker) {
        tracing::warn!(error = %err.kind(), "the finished refill's marker could not be removed");
    }
}

/// After a failed refill, move the store's ID counter to [`FAILED_REFILL_FLOOR`], so what the
/// launch journals until the retry never takes a sealed ID. A store that cannot be opened is
/// logged (stdout): the host's own open fails the same way. Database I/O.
fn reserve_after_failure(data_dir: &Path) {
    let reserved = SqliteJournal::open(&data_dir.join(crate::JOURNAL_FILE), "filesquid-desktop")
        .and_then(|store| store.reserve_through(FAILED_REFILL_FLOOR));
    if let Err(err) = reserved {
        tracing::error!(error = %err.0, "a failed refill could not reserve the sealed IDs");
    }
}

/// Recreate (or open) the operational store of `data_dir` and refill it from the sealed
/// journal. Filesystem and database I/O.
///
/// # Errors
/// The store cannot be opened, or the refill failed.
async fn refill_store(data_dir: &Path) -> Result<Refill, String> {
    let store = SqliteJournal::open(&data_dir.join(crate::JOURNAL_FILE), "filesquid-desktop")
        .map_err(|_| "the operational store could not be recreated".to_string())?;
    restore_installation_journal(data_dir, &store).await
}

/// The installation journal's storage root backend. Creates its directories.
fn installation_backend(data_dir: &Path) -> Result<FsBackend, String> {
    let root = data_dir.join(INSTALLATION_JOURNAL_DIR);
    real_dir(&root, "the installation journal")?;
    let parts = data_dir.join(WORK_DIR).join("root-parts");
    real_dir(&parts, "the installation journal")?;
    FsBackend::new(root, parts).map_err(|_| "the installation journal could not be opened".into())
}

/// Make `backend` an OCFL storage root when it is still empty; a non-empty one must pass the
/// storage-root checks (`StorageRoot::open`: hierarchy, OCFL 1.1 declaration, no Git
/// metadata) before it is used. Storage I/O.
async fn ensure_storage_root(backend: &FsBackend) -> Result<(), String> {
    let page = backend
        .list("", None, 1)
        .await
        .map_err(|_| "the installation journal could not be read".to_string())?;
    if !page.keys.is_empty() {
        return StorageRoot::open(backend)
            .await
            .map(drop)
            .map_err(|_| "the installation journal is not a valid storage root".to_string());
    }
    create_storage_root(backend)
        .await
        .map_err(|_| "the installation journal could not be created".to_string())
}

/// The fenced leases of the installation journal under `work`. Creates directories.
fn host_leases(work: &Path) -> Result<HostLeases, String> {
    let lease_root = backend_at(work, "leases", "the journal's lease store")?;
    LeaseStore::new(lease_root, "leases", SystemClock, LeaseConfig::default())
        .map_err(|_| "the journal's lease store could not be prepared".to_string())
}

/// A journal root over `backend`, sealed through `reservations`, staging in `staging`.
fn journal_root(
    backend: Arc<FsBackend>,
    reservations: Arc<HostLeases>,
    staging: FsBackend,
) -> JournalRoot<FsBackend, HostLeases, FsBackend> {
    let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
    JournalRoot {
        backend,
        reservations,
        staging: Arc::new(staging),
        transform,
        holder: SEAL_HOLDER.to_string(),
    }
}

#[cfg(test)]
mod tests {
    //! The segment-path parser that bounds the refill's id reservation.

    use super::segment_through;

    #[test]
    fn only_canonical_ordered_segment_paths_count() {
        let path = |first: &str, through: &str| format!("segments/{first}-{through}.jsonl");
        let id = |n: i64| format!("{n:020}");
        assert_eq!(segment_through(&path(&id(1), &id(7))), Some(7));
        assert_eq!(segment_through(&path(&id(7), &id(7))), Some(7));
        assert_eq!(segment_through(&path(&id(8), &id(7))), None, "reversed");
        assert_eq!(
            segment_through(&path("1", &id(7))),
            None,
            "first not canonical"
        );
        assert_eq!(
            segment_through(&path(&id(1), "7")),
            None,
            "through not canonical"
        );
        assert_eq!(
            segment_through("other/00000000000000000001-00000000000000000007.jsonl"),
            None
        );
    }
}
