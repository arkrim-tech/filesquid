//! SQLite findings store in the host's operational database: the native
//! [`FindingsStore`] the hosts inject into `filesquid-api` for OCFL validation (API-S-022).
//!
//! Three tables sit beside the activity journal in `filesquid.sqlite`: the open findings
//! per object, the per-library cursor and the last run per library. Replacing an object's
//! findings is one transaction, so a crash never leaves half of one validation stored.

use filesquid_api::ocfl_validation::{
    ErrorSummary, Finding, FindingChange, FindingsStore, RunOutcome, RunRecord, RunTrigger,
    Severity, StoreError,
};
use filesquid_core::LibraryId;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Schema: findings keyed by object, code and location; one cursor and one last run per
/// library.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ocfl_findings (
    library_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    version TEXT,
    code TEXT NOT NULL,
    severity TEXT NOT NULL,
    location TEXT NOT NULL,
    PRIMARY KEY (library_id, object_id, code, location)
);
CREATE TABLE IF NOT EXISTS ocfl_validation_cursors (
    library_id TEXT PRIMARY KEY,
    cursor TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ocfl_validation_runs (
    library_id TEXT PRIMARY KEY,
    run_trigger TEXT NOT NULL,
    started_ms INTEGER NOT NULL,
    ended_ms INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    checked INTEGER NOT NULL,
    not_checked INTEGER NOT NULL,
    error_code TEXT
);
";

/// Columns of a finding, in [`finding`] order.
const FINDING_COLUMNS: &str = "library_id, object_id, version, code, severity, location";

/// The findings store backed by SQLite.
pub struct SqliteFindings {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteFindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteFindings").finish_non_exhaustive()
    }
}

/// A store failure, with the SQLite detail for stdout: a busy or locked database is
/// transient, anything else is not.
fn sql_err(err: rusqlite::Error) -> StoreError {
    let busy = matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    );
    if busy {
        StoreError::unavailable(err.to_string())
    } else {
        StoreError::failed(err.to_string())
    }
}

impl SqliteFindings {
    /// Open (creating if needed) the operational database at `path` in WAL mode, made
    /// owner-only on Unix and opened without following a symbolic link, and ensure the
    /// findings schema. Filesystem and database I/O.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the file or the schema cannot be opened.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        crate::owner_only::create_owner_only(path)
            .map_err(|err| StoreError::unavailable(format!("{}: {err}", path.display())))?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let conn = Connection::open_with_flags(path, flags).map_err(sql_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(sql_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sql_err)?;
        conn.execute_batch(SCHEMA).map_err(sql_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The connection, locked.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError::failed("findings store lock poisoned"))
    }
}

/// A finding from a row in [`FINDING_COLUMNS`] order.
fn finding(row: &Row<'_>) -> rusqlite::Result<Finding> {
    let library: String = row.get(0)?;
    let severity: String = row.get(4)?;
    Ok(Finding {
        library_id: library_id(&library)?,
        object_id: row.get(1)?,
        version: row.get(2)?,
        code: row.get(3)?,
        severity: if severity == "error" {
            Severity::Error
        } else {
            Severity::Warning
        },
        location: row.get(5)?,
    })
}

/// A stored library ID, or a conversion error for a corrupt row.
fn library_id(text: &str) -> rusqlite::Result<LibraryId> {
    LibraryId::new(text).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

/// The open findings of one object, inside `tx`.
fn object_findings(
    tx: &rusqlite::Transaction<'_>,
    library_id: &LibraryId,
    object_id: &str,
) -> rusqlite::Result<Vec<Finding>> {
    let sql = format!(
        "SELECT {FINDING_COLUMNS} FROM ocfl_findings WHERE library_id = ?1 AND object_id = ?2"
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![library_id.as_str(), object_id], finding)?;
    rows.collect()
}

/// Store `findings` of one object in `tx`, replacing what it had.
fn write_findings(
    tx: &rusqlite::Transaction<'_>,
    library_id: &LibraryId,
    object_id: &str,
    findings: &[Finding],
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM ocfl_findings WHERE library_id = ?1 AND object_id = ?2",
        params![library_id.as_str(), object_id],
    )?;
    let sql = format!(
        "INSERT OR REPLACE INTO ocfl_findings ({FINDING_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    );
    for found in findings {
        tx.execute(
            &sql,
            params![
                library_id.as_str(),
                object_id,
                found.version,
                found.code,
                found.severity.as_str(),
                found.location
            ],
        )?;
    }
    Ok(())
}

/// A run trigger from its stored name.
fn trigger(text: &str) -> RunTrigger {
    match text {
        "scheduled" => RunTrigger::Scheduled,
        _ => RunTrigger::Requested,
    }
}

/// A run outcome from its stored name; an unknown name reads as `failed`.
fn outcome(text: &str) -> RunOutcome {
    match text {
        "completed" => RunOutcome::Completed,
        "capped" => RunOutcome::Capped,
        "cancelled" => RunOutcome::Cancelled,
        _ => RunOutcome::Failed,
    }
}

/// A run record from a `ocfl_validation_runs` row.
fn run(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    let library: String = row.get(0)?;
    let run_trigger: String = row.get(1)?;
    let run_outcome: String = row.get(4)?;
    Ok(RunRecord {
        library_id: library_id(&library)?,
        trigger: trigger(&run_trigger),
        started_ms: row.get(2)?,
        ended_ms: row.get(3)?,
        outcome: outcome(&run_outcome),
        checked: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        not_checked: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
        error_code: row.get(7)?,
    })
}

impl FindingsStore for SqliteFindings {
    fn replace_findings(
        &self,
        library_id: &LibraryId,
        object_id: &str,
        findings: Vec<Finding>,
    ) -> Result<FindingChange, StoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sql_err)?;
        let previous = object_findings(&tx, library_id, object_id).map_err(sql_err)?;
        write_findings(&tx, library_id, object_id, &findings).map_err(sql_err)?;
        tx.commit().map_err(sql_err)?;
        Ok(FindingChange::between(&previous, &findings))
    }

    fn objects_with_findings(&self, library_id: &LibraryId) -> Result<Vec<String>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT object_id FROM ocfl_findings WHERE library_id = ?1 ORDER BY object_id")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map(params![library_id.as_str()], |row| row.get(0))
            .map_err(sql_err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(sql_err)
    }

    fn open_findings(&self, limit: usize) -> Result<Vec<Finding>, StoreError> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT {FINDING_COLUMNS} FROM ocfl_findings \
             ORDER BY library_id, object_id, code, location LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![limit], finding).map_err(sql_err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(sql_err)
    }

    fn error_summary(&self) -> Result<ErrorSummary, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT code FROM ocfl_findings WHERE severity = 'error' ORDER BY code",
            )
            .map_err(sql_err)?;
        let codes = stmt
            .query_map([], |row| row.get(0))
            .map_err(sql_err)?
            .collect::<rusqlite::Result<Vec<String>>>()
            .map_err(sql_err)?;
        let objects: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM (SELECT DISTINCT library_id, object_id FROM ocfl_findings WHERE severity = 'error')",
                [],
                |row| row.get(0),
            )
            .map_err(sql_err)?;
        Ok(ErrorSummary {
            codes,
            objects: usize::try_from(objects).unwrap_or(0),
        })
    }

    fn cursor(&self, library_id: &LibraryId) -> Result<Option<String>, StoreError> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT cursor FROM ocfl_validation_cursors WHERE library_id = ?1",
            params![library_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_err)
    }

    fn set_cursor(&self, library_id: &LibraryId, cursor: Option<&str>) -> Result<(), StoreError> {
        let conn = self.lock()?;
        let changed = match cursor {
            Some(id) => conn.execute(
                "INSERT INTO ocfl_validation_cursors (library_id, cursor) VALUES (?1, ?2) \
                 ON CONFLICT(library_id) DO UPDATE SET cursor = excluded.cursor",
                params![library_id.as_str(), id],
            ),
            None => conn.execute(
                "DELETE FROM ocfl_validation_cursors WHERE library_id = ?1",
                params![library_id.as_str()],
            ),
        };
        changed.map(|_| ()).map_err(sql_err)
    }

    fn record_run(&self, record: &RunRecord) -> Result<(), StoreError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR REPLACE INTO ocfl_validation_runs \
             (library_id, run_trigger, started_ms, ended_ms, outcome, checked, not_checked, error_code) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.library_id.as_str(),
                record.trigger.as_str(),
                record.started_ms,
                record.ended_ms,
                record.outcome.as_str(),
                i64::try_from(record.checked).unwrap_or(i64::MAX),
                i64::try_from(record.not_checked).unwrap_or(i64::MAX),
                record.error_code
            ],
        )
        .map(|_| ())
        .map_err(sql_err)
    }

    fn last_run(&self, library_id: &LibraryId) -> Result<Option<RunRecord>, StoreError> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT library_id, run_trigger, started_ms, ended_ms, outcome, checked, not_checked, error_code \
             FROM ocfl_validation_runs WHERE library_id = ?1",
            params![library_id.as_str()],
            run,
        )
        .optional()
        .map_err(sql_err)
    }
}
