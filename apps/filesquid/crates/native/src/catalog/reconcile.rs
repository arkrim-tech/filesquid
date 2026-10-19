//! Reconciling a library's catalog with its canonical snapshot at open (Task DE-0.3, ADR 005:
//! "crash recovery reconciles pending intent and committed snapshots idempotently").
//!
//! The snapshot head is canonical. A catalog that does not reflect it (a crash after the
//! version was published, a catalog file restored from an older copy, or none at all) is
//! rebuilt from it; the receipts of every published operation are restored from the version
//! messages, so replays keep answering their first outcome; and intents whose operation was
//! published are cleared. An intent with no published version stays pending. The work is one
//! SQLite transaction, journaled as `library.reconcile`.
//!
//! Owner decision (2026-10-19): a command that changed nothing publishes no version and
//! stores no receipt and no journal entry, so there is nothing of it to restore. A retry of
//! that operation ID is planned again; with the revision it carries it is answered
//! `conflict` or the same no-op, never a change.

use super::rows::sql;
use super::{outbox, write};
use filesquid_api::canonical::{LibrarySnapshot, OperationRecord};
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::CatalogError;
use filesquid_core::LibraryId;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Journal operation of a reconcile that changed the catalog.
pub const RECONCILE_OPERATION: &str = "library.reconcile";

/// The snapshot version the catalog reflects, or `None` when it reflects none. Database I/O.
pub(crate) fn snapshot_version(conn: &Connection) -> Result<Option<String>, CatalogError> {
    let row: Option<Option<String>> = conn
        .query_row("SELECT snapshot_version FROM library", [], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok(row.flatten())
}

/// Record that the catalog now reflects snapshot `version`. Database I/O.
pub(crate) fn set_snapshot_version(
    tx: &Transaction<'_>,
    version: &str,
) -> Result<(), CatalogError> {
    tx.execute("UPDATE library SET snapshot_version = ?1", params![version])
        .map(drop)
        .map_err(sql)
}

/// Bring the catalog in `conn` to `snapshot` (see the module documentation) and answer
/// whether anything changed: only a catalog behind the head is rebuilt, has its receipts
/// restored and published intents cleared, and has `replay` (the journal entries the commits
/// it missed wrote into their versions, with their pre-allocated IDs, so the host journal
/// stores each once) pushed to its outbox. Database I/O.
///
/// # Errors
/// [`CatalogError::Failed`] when the snapshot names another library or holds an entity of
/// another library, or an entity cannot be stored; nothing changes.
pub(crate) fn reconcile(
    conn: &mut Connection,
    library_id: &LibraryId,
    snapshot: &LibrarySnapshot,
    replay: &[JournalEntry],
) -> Result<bool, CatalogError> {
    let version = snapshot.version.to_string();
    let behind = snapshot_version(conn)?.as_deref() != Some(version.as_str());
    if snapshot.library.record.library.id() != library_id {
        return Err(super::failed("the snapshot names another library"));
    }
    // A catalog that reflects the head stored every receipt and cleared every intent in the
    // transaction that recorded that head, so only a catalog behind it is touched.
    if !behind {
        return Ok(false);
    }
    let tx = conn.transaction().map_err(sql)?;
    rebuild(&tx, library_id, snapshot)?;
    restore_receipts(&tx, &snapshot.operations)?;
    clear_published(&tx, &snapshot.operations)?;
    for replayed in replay {
        outbox::push(&tx, replayed)?;
    }
    let entry = JournalEntry::new(RECONCILE_OPERATION, Outcome::Succeeded)
        .target(library_id.as_str())
        .target(version);
    outbox::push(&tx, &entry)?;
    tx.commit().map_err(sql)?;
    Ok(true)
}

/// Replace every organisation row with the snapshot's. Database I/O.
fn rebuild(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    snapshot: &LibrarySnapshot,
) -> Result<(), CatalogError> {
    tx.execute_batch(
        "DELETE FROM virtual_members; DELETE FROM files; DELETE FROM collections;
         DELETE FROM virtual_collections; DELETE FROM folders; DELETE FROM library;",
    )
    .map_err(sql)?;
    let head = &snapshot.library;
    write::insert_library(tx, &head.record, head.created_order)?;
    let revision =
        i64::try_from(head.revision).map_err(|_| super::failed("revision out of range"))?;
    tx.execute("UPDATE library SET revision = ?1", params![revision])
        .map_err(sql)?;
    set_snapshot_version(tx, &snapshot.version.to_string())?;
    write::index_folders(tx, library_id, &snapshot.folders)?;
    let files: Vec<_> = snapshot
        .files
        .iter()
        .map(|f| (f.file.clone(), f.order))
        .collect();
    write::index_files(tx, library_id, &files)?;
    for file in &snapshot.files {
        write::set_file_state(tx, file.file.id().as_str(), &file.state)?;
    }
    rebuild_collections(tx, snapshot)
}

/// Store the snapshot's collections, virtual collections and memberships. Database I/O.
fn rebuild_collections(
    tx: &Transaction<'_>,
    snapshot: &LibrarySnapshot,
) -> Result<(), CatalogError> {
    for collection in &snapshot.collections {
        write::same_library(
            &snapshot.library.record.library.id().clone(),
            collection.library_id(),
        )?;
        write::put_collection(tx, collection)?;
    }
    for entry in &snapshot.virtual_collections {
        write::same_library(
            &snapshot.library.record.library.id().clone(),
            entry.collection.library_id(),
        )?;
        write::put_virtual(tx, &entry.collection)?;
        for member in &entry.members {
            write::add_member(tx, member)?;
        }
    }
    Ok(())
}

/// Store the receipt of every published operation the catalog does not hold yet.
/// Database I/O.
fn restore_receipts(
    tx: &Transaction<'_>,
    operations: &[OperationRecord],
) -> Result<(), CatalogError> {
    let mut stmt = tx
        .prepare(
            "INSERT OR IGNORE INTO receipts
             (operation_id, revision, created_id, deleted_at_ms, purge_after_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(sql)?;
    for op in operations {
        stmt.execute(params![
            op.operation_id,
            op.revision,
            op.created_id,
            op.deletion.map(|d| d.deleted_at_ms),
            op.deletion.map(|d| d.purge_after_ms),
        ])
        .map_err(sql)?;
    }
    Ok(())
}

/// Forget the snapshot version the catalog claims to reflect: its library's OCFL root holds
/// no snapshot (it was moved or deleted), so the next commit publishes every entity again
/// instead of a partial delta. Database I/O.
pub(crate) fn forget_snapshot(conn: &Connection) -> Result<bool, CatalogError> {
    let changed = conn
        .execute(
            "UPDATE library SET snapshot_version = NULL WHERE snapshot_version IS NOT NULL",
            [],
        )
        .map_err(sql)?;
    Ok(changed > 0)
}

/// Remove the intents of published operations. Database I/O.
fn clear_published(
    tx: &Transaction<'_>,
    operations: &[OperationRecord],
) -> Result<(), CatalogError> {
    let mut stmt = tx
        .prepare("DELETE FROM pending_intents WHERE operation_id = ?1")
        .map_err(sql)?;
    for op in operations {
        stmt.execute(params![op.operation_id]).map_err(sql)?;
    }
    Ok(())
}
