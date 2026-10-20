//! The journal outbox of one library's catalog file.
//!
//! A commit's journal entry is written to `journal_outbox` in the commit's own transaction,
//! so the change and its record land together (logging-enforce.md §2). After the commit the
//! catalog forwards the outbox to the host's activity journal store and deletes what the
//! store accepted. Entries the store refused stay and are forwarded by the next commit or at
//! the next open, so none is dropped. A crash between the store's append and the delete can
//! forward an entry twice; it is never lost.

use super::failed;
use super::rows::sql;
use filesquid_api::journal::{JournalEntry, JournalStore};
use filesquid_api::library::CatalogError;
use rusqlite::{params, Connection, Transaction};

/// Entries forwarded per store append.
pub(crate) const FORWARD_BATCH: i64 = 256;

/// Batches forwarded per call before the rest waits for the next call (AGENTS.md §22).
pub(crate) const MAX_FORWARD_BATCHES: usize = 64;

/// Write `entry`, redacted, to the outbox inside `tx`. Database I/O.
pub(crate) fn push(tx: &Transaction<'_>, entry: &JournalEntry) -> Result<(), CatalogError> {
    let text = serde_json::to_string(&entry.clone().redacted())
        .map_err(|e| failed(format!("encode journal entry: {e}")))?;
    tx.execute(
        "INSERT INTO journal_outbox (entry) VALUES (?1)",
        params![text],
    )
    .map_err(sql)?;
    Ok(())
}

/// Remove the waiting entries of `operation` for `operation_id` inside `tx`: the intent
/// entry of a commit that changed nothing. Database I/O.
pub(crate) fn withdraw(
    tx: &Transaction<'_>,
    operation: &str,
    operation_id: &str,
) -> Result<(), CatalogError> {
    tx.execute(
        "DELETE FROM journal_outbox WHERE json_extract(entry, '$.operation') = ?1
         AND json_extract(entry, '$.operation_id') = ?2",
        params![operation, operation_id],
    )
    .map(drop)
    .map_err(sql)
}

/// Number of entries waiting in the outbox. Database I/O.
pub(crate) fn len(conn: &Connection) -> Result<u64, CatalogError> {
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM journal_outbox", [], |r| r.get(0))
        .map_err(sql)?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// The oldest waiting entries (at most [`FORWARD_BATCH`]) with their outbox ids. Database I/O.
fn oldest(conn: &Connection) -> Result<Vec<(i64, JournalEntry)>, CatalogError> {
    let mut stmt = conn
        .prepare("SELECT id, entry FROM journal_outbox ORDER BY id LIMIT ?1")
        .map_err(sql)?;
    let rows = stmt
        .query_map(params![FORWARD_BATCH], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(sql)?;
    rows.map(|row| {
        let (id, text) = row.map_err(sql)?;
        let entry = serde_json::from_str(&text)
            .map_err(|e| failed(format!("decode journal outbox entry: {e}")))?;
        Ok((id, entry))
    })
    .collect()
}

/// Forward waiting entries to `store`, oldest first, deleting each batch the store accepted.
/// Answers how many entries were forwarded. Stops after [`MAX_FORWARD_BATCHES`] batches.
/// Database and journal-store I/O.
///
/// # Errors
/// [`CatalogError::Unavailable`] when the store refuses a batch (it stays in the outbox);
/// [`CatalogError::Failed`] when the outbox cannot be read or cleared.
pub(crate) fn forward(conn: &Connection, store: &dyn JournalStore) -> Result<usize, CatalogError> {
    let mut forwarded = 0;
    for _ in 0..MAX_FORWARD_BATCHES {
        let batch = oldest(conn)?;
        let Some(last_id) = batch.last().map(|(id, _)| *id) else {
            break;
        };
        let entries: Vec<JournalEntry> = batch.into_iter().map(|(_, e)| e).collect();
        store
            .append(&entries)
            .map_err(|e| CatalogError::Unavailable(format!("host journal: {}", e.0)))?;
        conn.execute(
            "DELETE FROM journal_outbox WHERE id <= ?1",
            params![last_id],
        )
        .map_err(sql)?;
        forwarded += entries.len();
    }
    Ok(forwarded)
}
