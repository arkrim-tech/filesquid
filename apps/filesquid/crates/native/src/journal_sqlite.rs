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
    error_code TEXT,
    operation_id TEXT,
    object_ref TEXT
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

/// Columns added after the first release of the schema, as `(name, type)`.
/// [`migrate`] adds each one a journal created earlier lacks (DE-0.10:
/// the PROV-O linkage of ADR 040 Decision 6; API-F2.10: the client event ID of
/// API-S-030).
const ADDED_COLUMNS: [(&str, &str); 3] = [
    ("operation_id", "TEXT"),
    ("object_ref", "TEXT"),
    ("client_event_id", "TEXT"),
];

/// Indexes on the added columns, created after the migration. The client event ID is
/// unique, so each one is journaled at most once, across restarts (API-S-030); rows without
/// one hold NULL, which a unique index never compares equal.
const LINK_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS activity_journal_operation_id ON activity_journal(operation_id);
CREATE INDEX IF NOT EXISTS activity_journal_object_ref ON activity_journal(object_ref);
CREATE UNIQUE INDEX IF NOT EXISTS activity_journal_client_event_id
    ON activity_journal(client_event_id);
";

/// Insert one row; a row whose id or client event ID is already stored is skipped.
const INSERT: &str = "INSERT INTO activity_journal
    (time_ms, host, process_id, actor, operation, outcome, level, target_ids, duration_ms,
     error_code, operation_id, object_ref, client_event_id, id)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
    ON CONFLICT DO NOTHING";

const COLUMNS: &str = "id, time_ms, host, process_id, actor, operation, outcome, level, \
     target_ids, duration_ms, error_code, operation_id, object_ref, client_event_id";

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
        migrate(&conn)?;
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
        insert(tx, host, entry).map(|(id, _)| id)
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
    crate::owner_only::create_owner_only(path)
        .map_err(|err| JournalError(format!("{}: {err}", path.display())))
}

/// Add every column of [`ADDED_COLUMNS`] that the journal lacks, then the
/// indexes on them. Idempotent: a current journal is left as it is. Adding
/// a nullable column changes no stored row, so the append-only triggers
/// still hold; old rows read the new columns as absent.
fn migrate(conn: &Connection) -> Result<(), JournalError> {
    let present = journal_columns(conn)?;
    for (name, kind) in ADDED_COLUMNS {
        if !present.iter().any(|column| column == name) {
            let sql = format!("ALTER TABLE activity_journal ADD COLUMN {name} {kind}");
            conn.execute_batch(&sql).map_err(sql_err)?;
            tracing::info!(column = name, "migrated the activity journal forward");
        }
    }
    conn.execute_batch(LINK_INDEXES).map_err(sql_err)
}

/// The column names of the journal table.
fn journal_columns(conn: &Connection) -> Result<Vec<String>, JournalError> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info('activity_journal')")
        .map_err(sql_err)?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_err)?;
    Ok(names)
}

/// Insert one entry and return its id and whether it was stored now. A skipped entry
/// answers the id it carries, or the id of the row holding its client event ID.
fn insert(
    conn: &Connection,
    default_host: &str,
    e: &JournalEntry,
) -> Result<(i64, bool), JournalError> {
    let e = &e.clone().redacted();
    let host = if e.host.is_empty() {
        default_host
    } else {
        &e.host
    };
    let targets =
        serde_json::to_string(&e.target_ids).map_err(|err| JournalError(err.to_string()))?;
    let changed = conn
        .execute(
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
                e.error_code,
                e.operation_id,
                e.object_ref,
                e.client_event_id,
                e.id
            ],
        )
        .map_err(sql_err)?;
    if changed > 0 {
        return Ok((e.id.unwrap_or_else(|| conn.last_insert_rowid()), true));
    }
    let held = match (&e.id, &e.client_event_id) {
        (Some(id), _) => *id,
        (None, Some(client)) => client_event_row(conn, client)?,
        (None, None) => return Err(JournalError("journal row was not stored".into())),
    };
    Ok((held, false))
}

/// The id of the row holding the client event ID `client`. Database I/O.
fn client_event_row(conn: &Connection, client: &str) -> Result<i64, JournalError> {
    conn.query_row(
        "SELECT id FROM activity_journal WHERE client_event_id = ?1",
        [client],
        |r| r.get(0),
    )
    .map_err(sql_err)
}

/// Reserve `count` ids above every stored and every earlier reserved id by advancing the
/// table's AUTOINCREMENT counter in one transaction, so later automatic ids skip them.
/// Database I/O.
fn reserve_ids(conn: &mut Connection, count: i64) -> Result<Vec<i64>, JournalError> {
    let tx = conn.transaction().map_err(sql_err)?;
    let stored: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(id), 0) FROM activity_journal",
            [],
            |r| r.get(0),
        )
        .map_err(sql_err)?;
    let sequence: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM sqlite_sequence WHERE name = 'activity_journal'",
            [],
            |r| r.get(0),
        )
        .map_err(sql_err)?;
    let first = stored.max(sequence) + 1;
    let last = first + count - 1;
    tx.execute(
        "DELETE FROM sqlite_sequence WHERE name = 'activity_journal'",
        [],
    )
    .map_err(sql_err)?;
    tx.execute(
        "INSERT INTO sqlite_sequence (name, seq) VALUES ('activity_journal', ?1)",
        rusqlite::params![last],
    )
    .map_err(sql_err)?;
    tx.commit().map_err(sql_err)?;
    Ok((first..=last).collect())
}

/// The condition that one parameter is among an entry's targets (a JSON array of strings).
const TARGET_MATCH: &str = "EXISTS (SELECT 1 FROM json_each(activity_journal.target_ids) \
     WHERE json_each.value = ?)";

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
    link_filters(q, &mut parts, &mut args);
    let sql = if parts.is_empty() {
        "1 = 1".to_string()
    } else {
        parts.join(" AND ")
    };
    (sql, args)
}

/// Add the library, operation ID and object filters of `q` (API-S-031) to `parts` and
/// `args`. An object matches its object reference's object (`<object>@...`) or a target.
fn link_filters(q: &JournalQuery, parts: &mut Vec<String>, args: &mut Vec<Value>) {
    if let Some(l) = &q.library_id {
        parts.push(TARGET_MATCH.to_string());
        args.push(Value::Text(l.clone()));
    }
    if let Some(o) = &q.operation_id {
        parts.push("operation_id = ?".to_string());
        args.push(Value::Text(o.clone()));
    }
    if let Some(o) = &q.object_id {
        parts.push(format!("(substr(object_ref, 1, ?) = ? OR {TARGET_MATCH})"));
        let prefix = format!("{o}@");
        args.push(Value::Integer(prefix.chars().count() as i64));
        args.push(Value::Text(prefix));
        args.push(Value::Text(o.clone()));
    }
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
        operation_id: row.get(11)?,
        object_ref: row.get(12)?,
        client_event_id: row.get(13)?,
    })
}

impl JournalStore for SqliteJournal {
    fn host(&self) -> &str {
        &self.host
    }

    fn allocate_ids(&self, count: usize) -> Result<Vec<i64>, JournalError> {
        let count = filesquid_api::journal::check_allocation(count)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut conn = self.lock()?;
        reserve_ids(&mut conn, count)
    }

    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        let stored = self.append_new(entries)?;
        Ok(stored.into_iter().map(|(id, _)| id).collect())
    }

    fn append_new(&self, entries: &[JournalEntry]) -> Result<Vec<(i64, bool)>, JournalError> {
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

    fn entry(&self, id: i64) -> Result<Option<JournalEntry>, JournalError> {
        use rusqlite::OptionalExtension;
        let conn = self.lock()?;
        let sql = format!("SELECT {COLUMNS} FROM activity_journal WHERE id = ?1");
        conn.query_row(&sql, [id], row_to_entry)
            .optional()
            .map_err(sql_err)
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
