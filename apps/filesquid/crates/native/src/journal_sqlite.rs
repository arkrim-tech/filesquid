//! SQLite journal store (WAL mode) in the host's operational database: the native
//! [`JournalStore`] the hosts inject into `filesquid-api` (SRS-F-061 Journal store).

use filesquid_api::journal::{
    retention_cutoff, retention_entry, JournalEntry, JournalError, JournalPage, JournalQuery,
    JournalStore, Level, Outcome, RetentionPolicy,
};
use rusqlite::{params_from_iter, types::Value, Connection, Transaction};
use std::path::Path;
use std::sync::Mutex;

/// Schema: append-only table guarded by triggers; deletes only through the retention gate.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS activity_journal (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    time_ms INTEGER NOT NULL,
    host TEXT NOT NULL,
    process_id TEXT,
    actor TEXT NOT NULL,
    operation TEXT NOT NULL,
    outcome TEXT NOT NULL,
    level TEXT NOT NULL,
    target_ids TEXT NOT NULL,
    duration_ms INTEGER,
    error_code TEXT
);
CREATE INDEX IF NOT EXISTS activity_journal_time ON activity_journal(time_ms);
CREATE INDEX IF NOT EXISTS activity_journal_operation ON activity_journal(operation);
CREATE TABLE IF NOT EXISTS activity_journal_gate (id INTEGER PRIMARY KEY CHECK (id = 1), open INTEGER NOT NULL);
INSERT OR IGNORE INTO activity_journal_gate (id, open) VALUES (1, 0);
CREATE TRIGGER IF NOT EXISTS activity_journal_no_update BEFORE UPDATE ON activity_journal
BEGIN SELECT RAISE(ABORT, 'activity_journal is append-only'); END;
CREATE TRIGGER IF NOT EXISTS activity_journal_no_delete BEFORE DELETE ON activity_journal
WHEN (SELECT open FROM activity_journal_gate WHERE id = 1) = 0
BEGIN SELECT RAISE(ABORT, 'activity_journal is append-only; use retention'); END;
";

const INSERT: &str = "INSERT INTO activity_journal
    (time_ms, host, process_id, actor, operation, outcome, level, target_ids, duration_ms, error_code)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

const COLUMNS: &str =
    "id, time_ms, host, process_id, actor, operation, outcome, level, target_ids, duration_ms, error_code";

/// Journal store backed by an SQLite file. Created with its schema on first open.
pub struct SqliteJournal {
    host: String,
    conn: Mutex<Connection>,
}

fn sql_err(err: rusqlite::Error) -> JournalError {
    JournalError(err.to_string())
}

impl SqliteJournal {
    /// Open (creating if needed) the database at `path` in WAL mode and ensure the schema.
    /// On Unix the file is made owner-only (mode 0600) first; SQLite gives its `-wal` and
    /// `-shm` files the same mode, so other local users cannot read the journal directly.
    pub fn open(path: &Path, host: impl Into<String>) -> Result<Self, JournalError> {
        restrict_to_owner(path)?;
        let conn = Connection::open(path).map_err(sql_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(sql_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(sql_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sql_err)?;
        conn.execute_batch(SCHEMA).map_err(sql_err)?;
        Ok(Self {
            host: host.into(),
            conn: Mutex::new(conn),
        })
    }

    /// Journal a domain operation inside the caller's transaction, so the entry commits or
    /// rolls back together with the change it describes (SRS-F-061, T-055).
    pub fn record_in_transaction(
        tx: &Transaction<'_>,
        host: &str,
        entry: &JournalEntry,
    ) -> Result<i64, JournalError> {
        insert(tx, host, entry)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, JournalError> {
        self.conn
            .lock()
            .map_err(|_| JournalError("sqlite journal lock poisoned".into()))
    }
}

/// Create `path` if missing and set it to mode 0600 (Unix only; elsewhere the data
/// directory's access control applies). Filesystem I/O.
fn restrict_to_owner(path: &Path) -> Result<(), JournalError> {
    create_owner_only(path).map_err(|err| JournalError(format!("{}: {err}", path.display())))
}

/// Create the file at `path` if missing and set it to mode 0600 on Unix, so other local users
/// cannot read it; elsewhere the data directory's access control applies. Shared by the
/// journal and the library catalogs. Filesystem I/O.
pub(crate) fn create_owner_only(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Insert one entry and return its id.
fn insert(conn: &Connection, default_host: &str, e: &JournalEntry) -> Result<i64, JournalError> {
    let e = &e.clone().redacted();
    let host = if e.host.is_empty() {
        default_host
    } else {
        &e.host
    };
    let targets =
        serde_json::to_string(&e.target_ids).map_err(|err| JournalError(err.to_string()))?;
    conn.execute(
        INSERT,
        rusqlite::params![
            e.time_ms,
            host,
            e.process_id,
            e.actor,
            e.operation,
            e.outcome.as_str(),
            e.level.as_str(),
            targets,
            e.duration_ms.map(|d| d as i64),
            e.error_code
        ],
    )
    .map_err(sql_err)?;
    Ok(conn.last_insert_rowid())
}

/// WHERE clause and parameters for the filters of `q` (cursor and limit excluded).
fn where_clause(q: &JournalQuery) -> (String, Vec<Value>) {
    let mut parts = Vec::new();
    let mut args = Vec::new();
    let mut push = |sql: &str, v: Value| {
        parts.push(sql.to_string());
        args.push(v);
    };
    if let Some(t) = q.from_ms {
        push("time_ms >= ?", Value::Integer(t));
    }
    if let Some(t) = q.to_ms {
        push("time_ms <= ?", Value::Integer(t));
    }
    if let Some(a) = &q.actor {
        push("actor = ?", Value::Text(a.clone()));
    }
    if let Some(o) = &q.operation {
        push("operation = ?", Value::Text(o.clone()));
    }
    if let Some(o) = q.outcome {
        push("outcome = ?", Value::Text(o.as_str().into()));
    }
    if let Some(l) = q.level {
        push("level = ?", Value::Text(l.as_str().into()));
    }
    if let Some(p) = &q.process_id {
        push("process_id = ?", Value::Text(p.clone()));
    }
    let sql = if parts.is_empty() {
        "1 = 1".to_string()
    } else {
        parts.join(" AND ")
    };
    (sql, args)
}

/// Map one result row to an entry.
fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalEntry> {
    let outcome: String = row.get(6)?;
    let level: String = row.get(7)?;
    let targets: String = row.get(8)?;
    let duration: Option<i64> = row.get(9)?;
    Ok(JournalEntry {
        id: Some(row.get(0)?),
        time_ms: row.get(1)?,
        host: row.get(2)?,
        process_id: row.get(3)?,
        actor: row.get(4)?,
        operation: row.get(5)?,
        outcome: Outcome::parse(&outcome).unwrap_or(Outcome::Info),
        level: Level::parse(&level).unwrap_or(Level::Info),
        target_ids: serde_json::from_str(&targets).unwrap_or_default(),
        duration_ms: duration.map(|d| d.max(0) as u64),
        error_code: row.get(10)?,
    })
}

impl JournalStore for SqliteJournal {
    fn host(&self) -> &str {
        &self.host
    }

    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sql_err)?;
        let ids = entries
            .iter()
            .map(|e| insert(&tx, &self.host, e))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().map_err(sql_err)?;
        Ok(ids)
    }

    fn query(&self, q: &JournalQuery) -> Result<JournalPage, JournalError> {
        let conn = self.lock()?;
        let (filter, mut args) = where_clause(q);
        let limit = q.effective_limit();
        let (cursor_sql, order) = if q.descending {
            ("id < ?", "DESC")
        } else {
            ("id > ?", "ASC")
        };
        let cursor = q
            .after_id
            .unwrap_or(if q.descending { i64::MAX } else { 0 });
        args.push(Value::Integer(cursor));
        args.push(Value::Integer((limit + 1) as i64));
        let sql = format!("SELECT {COLUMNS} FROM activity_journal WHERE {filter} AND {cursor_sql} ORDER BY id {order} LIMIT ?");
        let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
        let rows = stmt
            .query_map(params_from_iter(args), row_to_entry)
            .map_err(sql_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_err)?;
        Ok(JournalPage::from_overfetch(rows, limit))
    }

    fn count(&self, q: &JournalQuery) -> Result<u64, JournalError> {
        let conn = self.lock()?;
        let (filter, args) = where_clause(q);
        let sql = format!("SELECT COUNT(*) FROM activity_journal WHERE {filter}");
        conn.query_row(&sql, params_from_iter(args), |r| r.get::<_, i64>(0))
            .map(|n| n.max(0) as u64)
            .map_err(sql_err)
    }

    fn apply_retention(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, JournalError> {
        let Some(cutoff) = retention_cutoff(policy, now_ms) else {
            return Ok(0);
        };
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(sql_err)?;
        tx.execute("UPDATE activity_journal_gate SET open = 1 WHERE id = 1", [])
            .map_err(sql_err)?;
        let deleted = tx
            .execute("DELETE FROM activity_journal WHERE time_ms < ?1", [cutoff])
            .map_err(sql_err)?;
        tx.execute("UPDATE activity_journal_gate SET open = 0 WHERE id = 1", [])
            .map_err(sql_err)?;
        insert(&tx, &self.host, &retention_entry(deleted as u64, now_ms))?;
        tx.commit().map_err(sql_err)?;
        Ok(deleted as u64)
    }

    fn ping(&self) -> Result<(), JournalError> {
        self.lock()?
            .query_row("SELECT 1", [], |_| Ok(()))
            .map_err(sql_err)
    }
}
