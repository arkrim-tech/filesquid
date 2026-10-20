//! What each import job imports (Task DE-1.2): the library, the files or folder the owner
//! picked and the operation that asked for it, kept in the host's SQLite operational store
//! next to the job rows. A job a quit suspended resumes from its plan at the next launch, and a
//! request repeating an operation ID finds its job here (API-S-036 idempotency). The picked
//! paths live only here, never in a job row, an answer or the journal (DE-S-006).

use filesquid_metadata::import::ImportMethod;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Schema: one plan per job; one job per operation.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS import_plans (
    job_id TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL UNIQUE,
    library_id TEXT NOT NULL,
    collection_id TEXT,
    sources BLOB NOT NULL,
    method TEXT NOT NULL
);
";

/// Most sources one plan names (the files picked at once).
pub const MAX_PLAN_SOURCES: usize = 10_000;

/// One import job's plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPlan {
    /// The library to import into.
    pub library_id: String,
    /// The collection the files join, when the owner chose one.
    pub collection_id: Option<String>,
    /// The files or the folder the owner picked.
    pub sources: Vec<PathBuf>,
    /// How the files reached FileSquid.
    pub method: ImportMethod,
}

/// The import plans in an SQLite operational store.
pub struct SqliteImportPlans {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteImportPlans {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteImportPlans").finish_non_exhaustive()
    }
}

/// A store failure, as a message with no path.
fn failed(err: rusqlite::Error) -> String {
    format!("the import plans could not be read or written ({err})")
}

/// The method's stored name. Pure.
fn method_name(method: ImportMethod) -> &'static str {
    match method {
        ImportMethod::FilePicker => "file_picker",
        ImportMethod::Folder => "folder",
        ImportMethod::DragAndDrop => "drag_and_drop",
        ImportMethod::OtherApplication => "other_application",
        ImportMethod::ApiUpload => "api_upload",
    }
}

/// The method a stored name names. Pure.
fn method_of(name: &str) -> Option<ImportMethod> {
    [
        ImportMethod::FilePicker,
        ImportMethod::Folder,
        ImportMethod::DragAndDrop,
        ImportMethod::OtherApplication,
        ImportMethod::ApiUpload,
    ]
    .into_iter()
    .find(|method| method_name(*method) == name)
}

/// The sources as one NUL-separated byte string (a path never holds a NUL). Pure.
fn encode_sources(sources: &[PathBuf]) -> Vec<u8> {
    let parts: Vec<&[u8]> = sources
        .iter()
        .map(|p| p.as_os_str().as_encoded_bytes())
        .collect();
    parts.join(&0u8)
}

/// The sources of a NUL-separated byte string. Pure.
fn decode_sources(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .take(MAX_PLAN_SOURCES)
        .map(|part| path_of(part.to_vec()))
        .collect()
}

impl SqliteImportPlans {
    /// Open the plans in the operational store at `path` (owner-only, never through a
    /// symbolic link). Opens a database connection, held until the store is dropped.
    ///
    /// # Errors
    /// The database or its table could not be opened.
    pub fn open(path: &Path) -> Result<Self, String> {
        crate::owner_only::create_owner_only(path)
            .map_err(|err| format!("the import plans could not be opened ({})", err.kind()))?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let conn = Connection::open_with_flags(path, flags).map_err(failed)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(failed)?;
        conn.execute_batch(SCHEMA).map_err(failed)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The locked connection.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "the import plans are unavailable".to_string())
    }

    /// Keep `plan` for `job_id`, asked by `operation_id`. Writes the database.
    ///
    /// # Errors
    /// The plan could not be written (an operation that already has a job included).
    pub fn save(&self, job_id: &str, operation_id: &str, plan: &ImportPlan) -> Result<(), String> {
        if plan.sources.len() > MAX_PLAN_SOURCES {
            return Err("too many sources were picked".to_string());
        }
        self.lock()?
            .execute(
                "INSERT INTO import_plans
                 (job_id, operation_id, library_id, collection_id, sources, method)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    job_id,
                    operation_id,
                    plan.library_id,
                    plan.collection_id,
                    encode_sources(&plan.sources),
                    method_name(plan.method)
                ],
            )
            .map(|_| ())
            .map_err(failed)
    }

    /// The plan of `job_id`, if kept. Reads the database.
    ///
    /// # Errors
    /// The plans could not be read.
    pub fn load(&self, job_id: &str) -> Result<Option<ImportPlan>, String> {
        type Row = (String, Option<String>, Vec<u8>, String);
        let row: Option<Row> = self
            .lock()?
            .query_row(
                "SELECT library_id, collection_id, sources, method FROM import_plans
                 WHERE job_id = ?1",
                params![job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(failed)?;
        Ok(
            row.and_then(|(library_id, collection_id, sources, method)| {
                Some(ImportPlan {
                    library_id,
                    collection_id,
                    sources: decode_sources(&sources),
                    method: method_of(&method)?,
                })
            }),
        )
    }

    /// The job `operation_id` started, if any. Reads the database.
    ///
    /// # Errors
    /// The plans could not be read.
    pub fn job_of_operation(&self, operation_id: &str) -> Result<Option<String>, String> {
        self.lock()?
            .query_row(
                "SELECT job_id FROM import_plans WHERE operation_id = ?1",
                params![operation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(failed)
    }
}

/// Stored path bytes as a path. Pure.
#[cfg(unix)]
fn path_of(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

/// Stored path bytes as a path. Pure.
#[cfg(not(unix))]
fn path_of(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}
