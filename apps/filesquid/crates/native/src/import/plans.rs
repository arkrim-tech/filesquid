//! What each import job imports (Task DE-1.2): the library and the folder or file the owner
//! chose, kept in the host's SQLite operational store next to the job rows, so a job a quit
//! suspended can resume at the next launch. The chosen path lives only here, never in a job
//! row, an answer or the journal (DE-S-006).

use filesquid_metadata::import::ImportMethod;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Schema: one plan per job.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS import_plans (
    job_id TEXT PRIMARY KEY,
    library_id TEXT NOT NULL,
    root BLOB NOT NULL,
    method TEXT NOT NULL
);
";

/// One import job's plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPlan {
    /// The library to import into.
    pub library_id: String,
    /// The folder or file the owner chose.
    pub root: PathBuf,
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

    /// Keep `plan` for `job_id`. Writes the database.
    ///
    /// # Errors
    /// The plan could not be written.
    pub fn save(&self, job_id: &str, plan: &ImportPlan) -> Result<(), String> {
        let root = plan.root.as_os_str().as_encoded_bytes();
        self.lock()?
            .execute(
                "INSERT OR REPLACE INTO import_plans (job_id, library_id, root, method)
                 VALUES (?1, ?2, ?3, ?4)",
                params![job_id, plan.library_id, root, method_name(plan.method)],
            )
            .map(|_| ())
            .map_err(failed)
    }

    /// The plan of `job_id`, if kept. Reads the database.
    ///
    /// # Errors
    /// The plans could not be read.
    pub fn load(&self, job_id: &str) -> Result<Option<ImportPlan>, String> {
        let row: Option<(String, Vec<u8>, String)> = self
            .lock()?
            .query_row(
                "SELECT library_id, root, method FROM import_plans WHERE job_id = ?1",
                params![job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(failed)?;
        Ok(row.and_then(|(library_id, root, method)| {
            Some(ImportPlan {
                library_id,
                root: root_path(root),
                method: method_of(&method)?,
            })
        }))
    }

    /// Forget the plan of a finished job. Writes the database.
    ///
    /// # Errors
    /// The plan could not be removed.
    pub fn remove(&self, job_id: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "DELETE FROM import_plans WHERE job_id = ?1",
                params![job_id],
            )
            .map(|_| ())
            .map_err(failed)
    }
}

/// A stored root as a path. Pure.
#[cfg(unix)]
fn root_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

/// A stored root as a path (stored from UTF-16-free encoded bytes). Pure.
#[cfg(not(unix))]
fn root_path(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}
