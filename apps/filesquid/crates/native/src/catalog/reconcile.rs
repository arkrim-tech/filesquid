//! Reconciling a library's catalog with its canonical snapshot at open (Task DE-0.3, ADR 005:
//! "crash recovery reconciles pending intent and committed snapshots idempotently").
//!
//! The snapshot head is canonical. A catalog that does not reflect it (a crash after the
//! version was published, a catalog file restored from an older copy, or none at all) is
//! rebuilt from it; the receipts of every published operation are restored from the version
//! messages, so replays keep answering their first outcome; and intents whose operation was
//! published are cleared. An intent with no published version stays pending. The work is one
//! SQLite transaction, journaled as `library.reconcile`.

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
/// whether anything changed. Database I/O.
///
/// # Errors
/// [`CatalogError::Failed`] when a snapshot entity cannot be stored; nothing changes.
pub(crate) fn reconcile(
    conn: &mut Connection,
    library_id: &LibraryId,
    snapshot: &LibrarySnapshot,
) -> Result<bool, CatalogError> {
    let version = snapshot.version.to_string();
    let behind = snapshot_version(conn)?.as_deref() != Some(version.as_str());
    let tx = conn.transaction().map_err(sql)?;
    if behind {
        rebuild(&tx, library_id, snapshot)?;
    }
    restore_receipts(&tx, &snapshot.operations)?;
    let cleared = clear_published(&tx, &snapshot.operations)?;
    let changed = behind || cleared > 0;
    if changed {
        let entry = JournalEntry::new(RECONCILE_OPERATION, Outcome::Succeeded)
            .target(library_id.as_str())
            .target(version);
        outbox::push(&tx, &entry)?;
    }
    tx.commit().map_err(sql)?;
    Ok(changed)
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
        write::put_collection(tx, collection)?;
    }
    for entry in &snapshot.virtual_collections {
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

/// Remove the intents of published operations; answer how many. Database I/O.
fn clear_published(
    tx: &Transaction<'_>,
    operations: &[OperationRecord],
) -> Result<usize, CatalogError> {
    let mut stmt = tx
        .prepare("DELETE FROM pending_intents WHERE operation_id = ?1")
        .map_err(sql)?;
    let mut cleared = 0;
    for op in operations {
        cleared += stmt.execute(params![op.operation_id]).map_err(sql)?;
    }
    Ok(cleared)
}
