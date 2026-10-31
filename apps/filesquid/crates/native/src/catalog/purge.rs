//! The object work of expired files on the desktop (Task API-F2.15; API-S-019, ADR 041
//! Decision 11): [`SqliteCatalog`] is the host's [`ObjectPurger`].
//!
//! An expiry commit removes each due file from the index and the snapshot and records its
//! object work in the library's `pending_purges` table in the same transaction, together
//! with the purge's journal entry. The work is then done here, one file at a time, in steps
//! that each leave something a later run completes:
//!
//! 1. **Payload** (`keep_version_history` only): what the tombstone keeps of the original
//!    object (its version facts and PROV-O files, [`read_former`]) is written to the
//!    catalog's work directory, outside the storage root, before anything is deleted. A
//!    payload already there is used as it is.
//! 2. **Delete**: every stored file of the original object is deleted
//!    ([`delete_object`]); a deletion stopped part-way is finished by the next run.
//! 3. **Tombstone** (`keep_version_history` only): the object is created again with the same
//!    identifier at the same path, holding only the tombstone record and the former PROV-O
//!    files, as version 1 of kind `tombstone` with its own PROV-O record. A partial creation
//!    is not a tombstone, so the next run deletes it and creates it again from the payload.
//! 4. **Done**: the payload and the pending row are removed and the tombstone's journal
//!    entries go through the library's outbox.
//!
//! So a crash at any step leaves the row, which the next run (at launch or on schedule)
//! completes, and never a half-deleted object that nobody finishes. A `keep_version_history`
//! purge whose original object is gone without a payload cannot write a tombstone: it is
//! reported (`tombstone_payload_missing`, journaled `failed`) and kept for the owner to see.

use super::canonical::{drive, CanonicalRoot, HOLDER};
use super::history::{plan_commit, History};
use super::rows::sql;
use super::{failed, outbox, SqliteCatalog};
use filesquid_api::canonical::tombstone::{
    activity_id, delete_object, is_tombstone, read_former, stage_tombstone, tombstone_record,
    ExpiryFacts, Former, TOMBSTONE_OPERATION,
};
use filesquid_api::canonical::{file_object_id, file_object_root, parse_operation_message};
use filesquid_api::canonical::{CanonicalError, OperationRecord, SNAPSHOT_ROOT};
use filesquid_api::journal::{format_rfc3339_ms, now_ms, JournalEntry, Outcome};
use filesquid_api::library::{CatalogError, ExpiryMode, ObjectPurger, PendingPurge};
use filesquid_core::{FileId, LibraryId};
use filesquid_ocfl::change::{ChangeModel, StagingArea};
use filesquid_ocfl::read::{version_history, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageKey};
use rusqlite::{params, Connection};

/// Most snapshot versions read, newest first, to find the version a purge committed.
const MAX_PURGE_VERSION_SCAN: usize = 1_000;

/// Where a purge can be stopped as a crash would stop it, for crash-recovery tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurgeStep {
    /// The tombstone's payload is saved and nothing is deleted yet.
    PayloadSaved,
    /// The original object is deleted and the tombstone is not created yet.
    OriginalDeleted,
    /// The tombstone is created and the pending row is not removed yet.
    TombstoneCreated,
}

impl SqliteCatalog {
    /// The libraries this catalog has open (attached), in identifier order: the ones an
    /// expiry run can reach. Reads memory only.
    ///
    /// # Errors
    /// A poisoned lock.
    pub fn open_libraries(&self) -> Result<Vec<LibraryId>, CatalogError> {
        Ok(self
            .lock()?
            .values()
            .map(|library| library.id.clone())
            .collect())
    }

    /// Stop every object work at `step` as a crash would, or at none (crash-recovery tests;
    /// a host never sets it).
    pub fn fail_purges_at(&self, step: Option<PurgeStep>) {
        if let Ok(mut stop) = self.purge_stop.lock() {
            *stop = step;
        }
    }

    /// Do `purge`'s object work in `library_id`, stopping at `stop` as a crash would (tests),
    /// and forget it once done. Database, filesystem and journal I/O.
    ///
    /// # Errors
    /// A store failure, `tombstone_payload_missing`, or the stop; the row stays for the next
    /// run.
    pub fn purge_object_until(
        &self,
        library_id: &LibraryId,
        purge: &PendingPurge,
        actor: &str,
        stop: Option<PurgeStep>,
    ) -> Result<(), CatalogError> {
        let history = self.history();
        let mut libraries = self.lock()?;
        let library = libraries
            .get_mut(library_id.as_str())
            .ok_or_else(|| failed("unknown library"))?;
        let work = ObjectWork {
            root: &library.canonical,
            purge,
            actor,
            history: history.as_ref(),
            stop,
        };
        let entries = work.run();
        match entries {
            Ok(entries) => finish(&mut library.conn, purge, &entries),
            Err(err) => {
                count_attempt(&library.conn, &purge.file_id);
                Err(err)
            }
        }?;
        self.forward_logged(&library.conn);
        Ok(())
    }
}

impl ObjectPurger for SqliteCatalog {
    fn pending_purges(
        &self,
        library_id: &LibraryId,
        limit: usize,
    ) -> Result<Vec<PendingPurge>, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| failed("unknown library"))?;
        pending(&library.conn, limit)
    }

    fn purge_object(
        &self,
        library_id: &LibraryId,
        purge: &PendingPurge,
        actor: &str,
    ) -> Result<(), CatalogError> {
        let stop = self.purge_stop.lock().ok().and_then(|stop| *stop);
        self.purge_object_until(library_id, purge, actor, stop)
    }
}

/// The recorded object works of a library's catalog, oldest first, at most `limit`.
/// Database read.
fn pending(conn: &Connection, limit: usize) -> Result<Vec<PendingPurge>, CatalogError> {
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = conn
        .prepare(
            "SELECT file_id, operation_id, mode FROM pending_purges
             ORDER BY recorded_at_ms, file_id LIMIT ?1",
        )
        .map_err(sql)?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(sql)?;
    rows.map(|row| {
        let (file, operation_id, mode) = row.map_err(sql)?;
        let file_id = FileId::new(file).map_err(|_| failed("empty identifier in the catalog"))?;
        let mode = match mode.as_str() {
            "delete_all_version_history" => ExpiryMode::DeleteAllVersionHistory,
            _ => ExpiryMode::KeepVersionHistory,
        };
        Ok(PendingPurge {
            file_id,
            operation_id,
            mode,
        })
    })
    .collect()
}

/// Remove the pending row of `purge` and store `entries` in the outbox, in one transaction.
/// Database I/O.
fn finish(
    conn: &mut Connection,
    purge: &PendingPurge,
    entries: &[JournalEntry],
) -> Result<(), CatalogError> {
    let tx = conn.transaction().map_err(sql)?;
    tx.execute(
        "DELETE FROM pending_purges WHERE file_id = ?1",
        params![purge.file_id.as_str()],
    )
    .map_err(sql)?;
    for entry in entries {
        outbox::push(&tx, entry)?;
    }
    tx.commit().map_err(sql)
}

/// Count one more failed attempt of the work of `file_id`; a failure to count is logged only.
fn count_attempt(conn: &Connection, file_id: &FileId) {
    let counted = conn.execute(
        "UPDATE pending_purges SET attempts = attempts + 1 WHERE file_id = ?1",
        params![file_id.as_str()],
    );
    if let Err(err) = counted {
        tracing::info!(target: super::LOG_TARGET, error = %err, "a purge attempt was not counted");
    }
}

/// One file's object work in one library.
struct ObjectWork<'a> {
    /// The library's storage root and work directory.
    root: &'a CanonicalRoot,
    /// The work.
    purge: &'a PendingPurge,
    /// Who expired it.
    actor: &'a str,
    /// The host's PROV-O history context, when it records history.
    history: Option<&'a History<'a>>,
    /// Where to stop as a crash would (tests).
    stop: Option<PurgeStep>,
}

impl ObjectWork<'_> {
    /// Run every remaining step; answer the tombstone's journal entries (none for a delete).
    fn run(&self) -> Result<Vec<JournalEntry>, CatalogError> {
        let object_root = file_object_root(&self.purge.file_id).map_err(CatalogError::from)?;
        let backend = self.root.backend();
        if self.purge.mode == ExpiryMode::DeleteAllVersionHistory {
            drive(delete_object(backend.as_ref(), &object_root))?.map_err(CatalogError::from)?;
            return Ok(Vec::new());
        }
        if drive(is_tombstone(backend.as_ref(), &object_root))? {
            self.forget_payload()?;
            return Ok(Vec::new());
        }
        let former = self.saved_or_read(&object_root)?;
        self.stop_at(PurgeStep::PayloadSaved)?;
        drive(delete_object(backend.as_ref(), &object_root))?.map_err(CatalogError::from)?;
        self.stop_at(PurgeStep::OriginalDeleted)?;
        let entries = self.create_tombstone(&object_root, &former)?;
        self.stop_at(PurgeStep::TombstoneCreated)?;
        self.forget_payload()?;
        Ok(entries)
    }

    /// The error of the test stop `step`, when this run stops there.
    fn stop_at(&self, step: PurgeStep) -> Result<(), CatalogError> {
        match self.stop == Some(step) {
            true => Err(CatalogError::Unavailable(format!("stopped at {step:?}"))),
            false => Ok(()),
        }
    }

    /// The payload saved by an earlier run, else the original object's former history, saved
    /// before anything is deleted. Filesystem I/O.
    fn saved_or_read(&self, object_root: &str) -> Result<Former, CatalogError> {
        if let Some(saved) = self.saved_payload()? {
            return Ok(saved);
        }
        let backend = self.root.backend();
        let former = drive(read_former(backend.as_ref(), object_root))?
            .map_err(CatalogError::from)?
            .ok_or_else(|| failed("tombstone_payload_missing: the original object is gone"))?;
        let bytes = serde_json::to_vec(&former).map_err(|e| failed(e.to_string()))?;
        drive(self.root.staging().put(&self.payload_key()?, &bytes))?
            .map_err(|_| CatalogError::Unavailable("the tombstone payload".into()))?;
        Ok(former)
    }

    /// The payload a run saved, if any. Filesystem I/O.
    fn saved_payload(&self) -> Result<Option<Former>, CatalogError> {
        let staging = self.root.staging();
        let key = self.payload_key()?;
        let present = drive(staging.exists(&key))?
            .map_err(|_| CatalogError::Unavailable("the tombstone payload".into()))?;
        if !present {
            return Ok(None);
        }
        let chunk = filesquid_ocfl::storage::MAX_CHUNK_BYTES;
        let bytes = drive(filesquid_ocfl::storage::read_verified(staging, &key, chunk))?
            .map_err(|_| CatalogError::Unavailable("the tombstone payload".into()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| failed("a tombstone payload is unreadable"))
    }

    /// Remove the payload; removing a missing one succeeds. Filesystem I/O.
    fn forget_payload(&self) -> Result<(), CatalogError> {
        drive(self.root.staging().delete(&self.payload_key()?))?
            .map_err(|_| CatalogError::Unavailable("the tombstone payload".into()))
    }

    /// The payload's key in the work directory.
    fn payload_key(&self) -> Result<StorageKey, CatalogError> {
        let name = format!("tombstones/{}.json", self.purge.file_id.as_str());
        StorageKey::new(&name).map_err(|_| failed("a file ID is not a storage key"))
    }

    /// Create the tombstone of `former` at `object_root` as version 1 of kind `tombstone`,
    /// with its PROV-O record; answer its journal entries. A partial object left by an
    /// earlier run is deleted first. Filesystem and journal-store I/O.
    fn create_tombstone(
        &self,
        object_root: &str,
        former: &Former,
    ) -> Result<Vec<JournalEntry>, CatalogError> {
        let backend = self.root.backend();
        drive(delete_object(backend.as_ref(), object_root))?.map_err(CatalogError::from)?;
        let object_id = file_object_id(&self.purge.file_id);
        let time_ms = now_ms();
        let facts = ExpiryFacts {
            operation_id: self.purge.operation_id.clone(),
            actor: self.actor.to_string(),
            time: format_rfc3339_ms(time_ms),
            purge_activity: self.purge_activity()?,
        };
        let record = tombstone_record(&object_id, former, &facts);
        let entry = JournalEntry::new(TOMBSTONE_OPERATION, Outcome::Succeeded)
            .actor(self.actor)
            .operation_id(self.purge.operation_id.as_str())
            .target(self.purge.file_id.as_str())
            .at_ms(time_ms);
        let message = OperationRecord {
            operation_id: self.purge.operation_id.clone(),
            kind: Some("tombstone".to_string()),
            revision: "1".to_string(),
            created_id: None,
            deletion: None,
        };
        let plan = plan_commit(self.history, &message, &entry);
        let area = StagingArea::new(self.root.staging(), "tombstone")
            .map_err(|e| CatalogError::from(CanonicalError::from(e)))?;
        let model = ChangeModel::new(
            backend.as_ref(),
            self.root.leases(),
            HOLDER,
            object_root,
            object_id,
            area,
        );
        drive(stage_tombstone(&model, &record, former))?.map_err(CatalogError::from)?;
        let receipt = drive(model.commit_with(plan.meta(), plan.bound()))?
            .map_err(|e| CatalogError::from(CanonicalError::from(e)))?;
        Ok(plan.published(&receipt))
    }

    /// The PROV-O activity of the purge: that of the snapshot version its operation
    /// committed, found newest first within [`MAX_PURGE_VERSION_SCAN`] versions, else that of
    /// the operation on the snapshot object with no version known. Filesystem I/O.
    fn purge_activity(&self) -> Result<String, CatalogError> {
        let backend = self.root.backend();
        let snapshot_id = self.root.snapshot_object_id().to_string();
        let operation = self.purge.operation_id.as_str();
        let version = drive(purge_version(backend.as_ref(), operation))?;
        let version = version.unwrap_or_else(|| "unknown".to_string());
        Ok(activity_id(operation, &snapshot_id, &version))
    }
}

/// The snapshot version whose operation message names `operation`, if any is found.
async fn purge_version<B: StorageBackend>(backend: &B, operation: &str) -> Option<String> {
    let root = StorageRoot::open(backend).await.ok()?;
    let object = root.open_object(SNAPSHOT_ROOT).await.ok()?;
    version_history(object.inventory())
        .into_iter()
        .rev()
        .take(MAX_PURGE_VERSION_SCAN)
        .find(|info| {
            info.message
                .as_deref()
                .and_then(|m| parse_operation_message(m).ok().flatten())
                .is_some_and(|record| record.operation_id == operation)
        })
        .map(|info| info.version.to_string())
}
