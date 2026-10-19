//! Pending intent: the first step of the commit sequence of
//! [ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md)
//! ("record intent (operation ID and expected revision), make canonical content and OCFL
//! snapshots durable, then update indexes"). The intent coordinator of DE-0.3 records an
//! intent before canonical work; the commit of the same operation ID removes it in its own
//! transaction, so an intent still stored after a crash names work to reconcile.

use super::failed;
use super::rows::sql;
use filesquid_api::library::CatalogError;
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::LibraryId;
use rusqlite::{params, Connection, Transaction};

/// One recorded intent. Every field is an opaque identifier, a dotted operation name or a
/// time; it holds no title, path or content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingIntent {
    /// The library whose catalog file holds the intent.
    pub library_id: LibraryId,
    /// Idempotency key of the operation; its commit clears the intent.
    pub operation_id: OperationId,
    /// Dotted operation name, for example `library.collection.create`.
    pub operation: String,
    /// The library revision the operation was planned against.
    pub expected_revision: ExpectedRevision,
    /// When the intent was recorded, in Unix milliseconds (UTC).
    pub recorded_at_ms: i64,
}

/// Store `intent`, replacing an earlier intent of the same operation ID. Database I/O.
pub(crate) fn record(conn: &Connection, intent: &PendingIntent) -> Result<(), CatalogError> {
    conn.execute(
        "INSERT INTO pending_intents (operation_id, operation, expected_revision, recorded_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(operation_id) DO UPDATE SET operation = excluded.operation,
             expected_revision = excluded.expected_revision,
             recorded_at_ms = excluded.recorded_at_ms",
        params![
            intent.operation_id.as_str(),
            intent.operation,
            intent.expected_revision.as_str(),
            intent.recorded_at_ms,
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Remove the intent of `operation_id`, if any, inside a commit's transaction. Database I/O.
pub(crate) fn clear(tx: &Transaction<'_>, operation_id: &str) -> Result<(), CatalogError> {
    tx.execute(
        "DELETE FROM pending_intents WHERE operation_id = ?1",
        params![operation_id],
    )
    .map(drop)
    .map_err(sql)
}

/// Remove the intent of `operation_id`, if any, outside a transaction: the commit it
/// announced failed before anything was published. Database I/O.
pub(crate) fn discard(conn: &Connection, operation_id: &str) -> Result<(), CatalogError> {
    conn.execute(
        "DELETE FROM pending_intents WHERE operation_id = ?1",
        params![operation_id],
    )
    .map(drop)
    .map_err(sql)
}

/// The oldest `limit` intents of `library_id`, by recorded time then operation ID.
/// Database I/O.
pub(crate) fn list(
    conn: &Connection,
    library_id: &LibraryId,
    limit: usize,
) -> Result<Vec<PendingIntent>, CatalogError> {
    let mut stmt = conn
        .prepare(
            "SELECT operation_id, operation, expected_revision, recorded_at_ms
             FROM pending_intents ORDER BY recorded_at_ms, operation_id LIMIT ?1",
        )
        .map_err(sql)?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
            ))
        })
        .map_err(sql)?;
    rows.map(|row| {
        let (op, operation, revision, recorded_at_ms) = row.map_err(sql)?;
        Ok(PendingIntent {
            library_id: library_id.clone(),
            operation_id: OperationId::new(op).map_err(|_| failed("empty intent operation ID"))?,
            operation,
            expected_revision: ExpectedRevision::new(revision)
                .map_err(|_| failed("empty intent revision"))?,
            recorded_at_ms,
        })
    })
    .collect()
}
