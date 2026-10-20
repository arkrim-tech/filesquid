//! SQLite job store in the host's operational database: the native [`JobStore`] the hosts
//! inject into `filesquid-api`'s durable jobs (API-S-033, DE-S-014; ADR 007).
//!
//! One `jobs` table sits beside the activity journal in `filesquid.sqlite`. Each call is one
//! transaction (`BEGIN IMMEDIATE` for a read-modify-write), so a lease, a checkpoint or a
//! job's end is stored whole or not at all, and no second writer sees the job in between.
//! The rules are the API's ([`filesquid_api::jobs::rules`]); this module only stores.

use filesquid_api::jobs::{Job, JobError, JobState, JobStore};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Schema: one row per job, with an index for the launch-time list of unfinished jobs.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    source TEXT NOT NULL,
    state TEXT NOT NULL,
    checkpoint TEXT,
    done INTEGER NOT NULL,
    total INTEGER,
    attempts INTEGER NOT NULL,
    fence INTEGER NOT NULL,
    holder TEXT,
    lease_expires_ms INTEGER,
    created_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL,
    error_code TEXT
);
CREATE INDEX IF NOT EXISTS jobs_unfinished ON jobs (state, created_ms, id);
";

/// Columns of a job, in [`job`] and [`write`] order.
const COLUMNS: &str = "id, kind, operation_id, source, state, checkpoint, done, total, \
    attempts, fence, holder, lease_expires_ms, created_ms, updated_ms, error_code";

/// The job store backed by SQLite.
pub struct SqliteJobs {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteJobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteJobs").finish_non_exhaustive()
    }
}

/// A store failure: a busy or locked database is transient, anything else is not. The
/// SQLite detail never names a path.
fn sql_err(err: rusqlite::Error) -> JobError {
    let busy = matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    );
    match busy {
        true => JobError::Unavailable(err.to_string()),
        false => JobError::Failed(err.to_string()),
    }
}

impl SqliteJobs {
    /// Open (creating if needed) the operational database at `path` in WAL mode, made
    /// owner-only on Unix and opened without following a symbolic link, and ensure the job
    /// schema. Filesystem and database I/O.
    ///
    /// # Errors
    /// [`JobError::Unavailable`] when the file cannot be created; a store failure when the
    /// database or the schema cannot be opened. The error never names the path.
    pub fn open(path: &Path) -> Result<Self, JobError> {
        crate::owner_only::create_owner_only(path)
            .map_err(|err| JobError::Unavailable(format!("job store file: {}", err.kind())))?;
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
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, JobError> {
        self.conn
            .lock()
            .map_err(|_| JobError::Failed("job store lock poisoned".into()))
    }
}

impl JobStore for SqliteJobs {
    fn insert(&self, job: &Job) -> Result<(), JobError> {
        let conn = self.lock()?;
        let exists = read(&conn, &job.id)?.is_some();
        if exists {
            return Err(JobError::Conflict("job_exists"));
        }
        write(&conn, job)
    }

    fn get(&self, id: &str) -> Result<Option<Job>, JobError> {
        read(&*self.lock()?, id)
    }

    fn update(
        &self,
        id: &str,
        change: &mut dyn FnMut(Job) -> Result<Job, JobError>,
    ) -> Result<Job, JobError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_err)?;
        let current = read(&tx, id)?.ok_or(JobError::NotFound)?;
        let changed = change(current)?;
        write(&tx, &changed)?;
        tx.commit().map_err(sql_err)?;
        Ok(changed)
    }

    fn unfinished(&self, limit: usize) -> Result<Vec<Job>, JobError> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT {COLUMNS} FROM jobs WHERE state IN ('pending', 'running') \
             ORDER BY created_ms, id LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![limit], job).map_err(sql_err)?;
        rows.collect::<Result<Vec<Job>, _>>().map_err(sql_err)
    }
}

/// The job `id`, if stored. Database I/O.
fn read(conn: &Connection, id: &str) -> Result<Option<Job>, JobError> {
    let sql = format!("SELECT {COLUMNS} FROM jobs WHERE id = ?1");
    conn.query_row(&sql, params![id], job)
        .optional()
        .map_err(sql_err)
}

/// Insert or replace `job`. Database I/O.
fn write(conn: &Connection, job: &Job) -> Result<(), JobError> {
    let sql = format!(
        "INSERT OR REPLACE INTO jobs ({COLUMNS}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
    );
    conn.execute(
        &sql,
        params![
            job.id,
            job.kind,
            job.operation_id,
            job.source,
            job.state.as_str(),
            job.checkpoint,
            to_i64(job.done),
            job.total.map(to_i64),
            job.attempts,
            to_i64(job.fence),
            job.holder,
            job.lease_expires_ms,
            job.created_ms,
            job.updated_ms,
            job.error_code,
        ],
    )
    .map(drop)
    .map_err(sql_err)
}

/// A count as SQLite's signed integer, saturating. Pure.
fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A job from a row in [`COLUMNS`] order; an unknown state is a conversion failure.
fn job(row: &Row<'_>) -> rusqlite::Result<Job> {
    let state: String = row.get(4)?;
    let state = JobState::parse(&state).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, "state".into())
    })?;
    let unsigned = |value: i64| u64::try_from(value).unwrap_or_default();
    Ok(Job {
        id: row.get(0)?,
        kind: row.get(1)?,
        operation_id: row.get(2)?,
        source: row.get(3)?,
        state,
        checkpoint: row.get(5)?,
        done: unsigned(row.get(6)?),
        total: row.get::<_, Option<i64>>(7)?.map(unsigned),
        attempts: row.get(8)?,
        fence: unsigned(row.get(9)?),
        holder: row.get(10)?,
        lease_expires_ms: row.get(11)?,
        created_ms: row.get(12)?,
        updated_ms: row.get(13)?,
        error_code: row.get(14)?,
    })
}
