//! The persistent repair ledger of the desktop (Task DE-1.1; API-S-023, API-S-025): the
//! native [`RepairLedger`] the host injects into the version-inventory repair.
//!
//! It lives in the host's SQLite operational store (`filesquid.sqlite`), next to the activity
//! journal: the owner's consent entry is journaled and recorded in one transaction before any
//! object is repaired, and each object's outcome entry is journaled together with its repair
//! receipt in one transaction, or neither is stored. Entries are redacted by the journal's
//! insert, as every journal row is; receipts hold paths relative to the object root
//! (`v2/inventory.json`), which no answer to a UI carries.
//!
//! The rows written here go straight to the journal table inside the ledger's transaction,
//! so they raise no `journal.entry` event; the journal page reads them like any other row.

use crate::journal_sqlite::SqliteJournal;
use filesquid_api::journal::JournalEntry;
use filesquid_api::local_library::{LedgerError, RecordedRepair, RepairLedger, RepairRecord};
use rusqlite::{params, Connection, ErrorCode, OpenFlags};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Schema: consents and receipts, each naming the journal row stored with it.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS repair_consents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    journal_id INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS repair_receipts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    journal_id INTEGER NOT NULL,
    object_id TEXT NOT NULL,
    version TEXT NOT NULL,
    digest_algorithm TEXT NOT NULL,
    inventory_digest TEXT NOT NULL,
    written TEXT NOT NULL
);
";

/// Most receipts [`SqliteRepairLedger::receipts`] reads at once.
pub const MAX_RECEIPTS_READ: usize = 10_000;

/// The repair ledger in an SQLite operational store.
pub struct SqliteRepairLedger {
    host: String,
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteRepairLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteRepairLedger").finish_non_exhaustive()
    }
}

/// A store failure, as the ledger port reports it. Details go to stdout only.
fn failed(err: impl std::fmt::Display) -> LedgerError {
    LedgerError::Failed(err.to_string())
}

impl SqliteRepairLedger {
    /// Open the ledger in the operational store at `path` (the activity journal's file,
    /// which [`SqliteJournal::open`] creates owner-only), journaling as `host`. Opens a
    /// database connection, held until the ledger is dropped.
    ///
    /// # Errors
    ///
    /// The journal or the ledger tables could not be opened.
    pub fn open(path: &Path, host: impl Into<String>) -> Result<Self, LedgerError> {
        let host = host.into();
        drop(SqliteJournal::open(path, host.clone()).map_err(failed)?);
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let conn = Connection::open_with_flags(path, flags).map_err(classify)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(classify)?;
        conn.execute_batch(SCHEMA).map_err(classify)?;
        Ok(Self {
            host,
            conn: Mutex::new(conn),
        })
    }

    /// How many consents are recorded. Reads the database.
    ///
    /// # Errors
    ///
    /// The database could not be read.
    pub fn consent_count(&self) -> Result<i64, LedgerError> {
        let conn = self.lock()?;
        conn.query_row("SELECT COUNT(*) FROM repair_consents", [], |row| row.get(0))
            .map_err(classify)
    }

    /// The receipts recorded, oldest first, at most [`MAX_RECEIPTS_READ`]. Reads the
    /// database.
    ///
    /// # Errors
    ///
    /// The database could not be read, or a stored receipt is malformed.
    pub fn receipts(&self) -> Result<Vec<RecordedRepair>, LedgerError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT object_id, version, digest_algorithm, inventory_digest, written
                 FROM repair_receipts ORDER BY id LIMIT ?1",
            )
            .map_err(classify)?;
        let rows = stmt
            .query_map(params![MAX_RECEIPTS_READ as i64], receipt_row)
            .map_err(classify)?;
        rows.map(|row| row.map_err(classify).and_then(|r| r))
            .collect()
    }

    /// The locked connection.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, LedgerError> {
        self.conn
            .lock()
            .map_err(|_| LedgerError::Failed("repair ledger lock poisoned".into()))
    }

    /// Journal `entry` and run `then` with its row ID in one transaction. Writes the
    /// database.
    fn with_journal_row(
        &self,
        entry: &JournalEntry,
        then: impl FnOnce(&rusqlite::Transaction<'_>, i64) -> rusqlite::Result<usize>,
    ) -> Result<(), LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(classify)?;
        let journal_id =
            SqliteJournal::record_in_transaction(&tx, &self.host, entry).map_err(failed)?;
        then(&tx, journal_id).map_err(classify)?;
        tx.commit().map_err(classify)
    }
}

/// An SQLite failure as the port reports it: a busy or locked database is `Unavailable` (a
/// retry may succeed), every other failure `Failed`. Pure.
fn classify(err: rusqlite::Error) -> LedgerError {
    let busy = matches!(
        err.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    );
    match busy {
        true => LedgerError::Unavailable(err.to_string()),
        false => LedgerError::Failed(err.to_string()),
    }
}

/// One stored receipt row; a malformed `written` list is reported, never skipped.
fn receipt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<RecordedRepair, LedgerError>> {
    let written: String = row.get(4)?;
    let receipt = RecordedRepair {
        object_id: row.get(0)?,
        version: row.get(1)?,
        digest_algorithm: row.get(2)?,
        inventory_digest: row.get(3)?,
        written: Vec::new(),
    };
    Ok(serde_json::from_str::<Vec<String>>(&written)
        .map(|written| RecordedRepair { written, ..receipt })
        .map_err(failed))
}

impl RepairLedger for SqliteRepairLedger {
    fn record_consent(&self, entry: JournalEntry) -> Result<(), LedgerError> {
        self.with_journal_row(&entry, |tx, journal_id| {
            tx.execute(
                "INSERT INTO repair_consents (journal_id) VALUES (?1)",
                params![journal_id],
            )
        })
    }

    fn record_outcome(&self, record: RepairRecord) -> Result<(), LedgerError> {
        let written = match &record.receipt {
            Some(receipt) => Some(serde_json::to_string(&receipt.written).map_err(failed)?),
            None => None,
        };
        self.with_journal_row(&record.journal, |tx, journal_id| {
            let (Some(receipt), Some(written)) = (&record.receipt, written) else {
                return Ok(0);
            };
            tx.execute(
                "INSERT INTO repair_receipts
                 (journal_id, object_id, version, digest_algorithm, inventory_digest, written)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    journal_id,
                    receipt.object_id,
                    receipt.version,
                    receipt.digest_algorithm,
                    receipt.inventory_digest,
                    written
                ],
            )
        })
    }
}
