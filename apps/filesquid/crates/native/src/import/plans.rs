//! What each import job imports (Task DE-1.2): the library, the files or folder the owner
//! picked and the operation that asked for it, kept in the host's SQLite operational store
//! next to the job rows. A job a quit suspended resumes from its plan at the next launch, and a
//! request repeating an operation ID finds its job here (API-S-036 idempotency). The picked
//! paths live only here, never in a job row, an answer or the journal (DE-S-006).
//!
//! The first run of a job lists its sources once and keeps that listing as the job's
//! manifest ([`ManifestEntry`]): every file with the identifier minted for its new file
//! object, and every entry that could not be listed with its error code. A resumed run
//! continues the same manifest at its checkpoint, so a folder that changed in between can
//! neither shift the files a checkpoint skips nor end the job early, and a file committed just
//! before a crash is answered, not imported twice.

use filesquid_metadata::import::ImportMethod;
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Schema: one plan per job and one job per operation; the plan's sources; the job's
/// manifest; and whether the job finished, so a launch pages through the unfinished ones.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS import_plans (
    job_id TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL UNIQUE,
    library_id TEXT NOT NULL,
    method TEXT NOT NULL,
    finished INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1)),
    collection_id TEXT,
    folder TEXT
);
CREATE INDEX IF NOT EXISTS import_plans_unfinished ON import_plans(finished, job_id);
CREATE TABLE IF NOT EXISTS import_sources (
    job_id TEXT NOT NULL REFERENCES import_plans(job_id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    path BLOB NOT NULL,
    PRIMARY KEY (job_id, idx)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS import_manifest (
    job_id TEXT NOT NULL REFERENCES import_plans(job_id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    source_idx INTEGER NOT NULL,
    path BLOB,
    file_id TEXT,
    skipped TEXT,
    logical_path TEXT,
    PRIMARY KEY (job_id, idx),
    CHECK ((path IS NOT NULL AND file_id IS NOT NULL) <> (skipped IS NOT NULL))
) WITHOUT ROWID;
";

/// Columns added after a store was first created, by table, each added at open when missing
/// (Task DE-1.7: the destination and each file's logical path), so a plan kept by an earlier
/// build is still read; such a plan names no destination.
const ADDED_COLUMNS: [(&str, &str, &str); 3] = [
    ("import_plans", "collection_id", "TEXT"),
    ("import_plans", "folder", "TEXT"),
    ("import_manifest", "logical_path", "TEXT"),
];

/// Most sources one plan names (the files picked at once).
pub const MAX_PLAN_SOURCES: usize = 10_000;

/// Most entries one manifest holds (the import's file cap, [`super::job::MAX_IMPORT_FILES`],
/// plus the entries that could not be listed are bounded by it too).
pub const MAX_MANIFEST_ENTRIES: usize = super::job::MAX_IMPORT_FILES;

/// One import job's plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPlan {
    /// The library to import into.
    pub library_id: String,
    /// The destination collection (ADR 041 "Must choose"); `None` only in a plan an earlier
    /// build kept, whose job then fails as `destination_required`.
    pub collection_id: Option<String>,
    /// The destination folder inside the collection, a collection logical path; the
    /// collection's root when `None`.
    pub folder: Option<String>,
    /// The files or the folder the owner picked.
    pub sources: Vec<PathBuf>,
    /// How the files reached FileSquid.
    pub method: ImportMethod,
}

/// One entry of a job's manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestEntry {
    /// A file to import as the new file object `file_id`.
    File {
        /// The file.
        path: PathBuf,
        /// The index of the source (in the plan) it was listed from.
        source: usize,
        /// The identifier minted for its file object (UUID v4).
        file_id: String,
        /// Its collection logical path: its path beneath its picked source, under the
        /// source's (numbered) name, beneath the destination folder; `None` only in a
        /// manifest an earlier build kept.
        logical_path: Option<String>,
    },
    /// An entry of a source that could not be listed, with its error code.
    Skipped {
        /// The index of the source it belongs to.
        source: usize,
        /// Why it was skipped.
        code: String,
    },
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
    /// The database or its tables could not be opened.
    pub fn open(path: &Path) -> Result<Self, String> {
        let unopened =
            |err: std::io::Error| format!("the import plans could not be opened ({})", err.kind());
        let path = crate::owner_only::resolve_folders(path).map_err(unopened)?;
        crate::owner_only::create_owner_only(&path).map_err(unopened)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let conn = Connection::open_with_flags(&path, flags).map_err(failed)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(failed)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(failed)?;
        conn.execute_batch(SCHEMA).map_err(failed)?;
        add_missing_columns(&conn)?;
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

    /// Keep `plan` for `job_id`, asked by `operation_id`, with its sources, in one
    /// transaction. Writes the database.
    ///
    /// # Errors
    /// The plan could not be written (an operation that already has a job included);
    /// nothing is stored then.
    pub fn save(&self, job_id: &str, operation_id: &str, plan: &ImportPlan) -> Result<(), String> {
        if plan.sources.len() > MAX_PLAN_SOURCES {
            return Err("too many sources were picked".to_string());
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        tx.execute(
            "INSERT INTO import_plans
             (job_id, operation_id, library_id, method, collection_id, folder)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job_id,
                operation_id,
                plan.library_id,
                method_name(plan.method),
                plan.collection_id,
                plan.folder
            ],
        )
        .map_err(failed)?;
        insert_sources(&tx, job_id, &plan.sources)?;
        tx.commit().map_err(failed)
    }

    /// The plan of `job_id`, if kept. Reads the database.
    ///
    /// # Errors
    /// The plans could not be read.
    pub fn load(&self, job_id: &str) -> Result<Option<ImportPlan>, String> {
        let conn = self.lock()?;
        let row: Option<PlanRow> = conn
            .query_row(
                "SELECT library_id, method, collection_id, folder FROM import_plans
                 WHERE job_id = ?1",
                params![job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(failed)?;
        let Some((library_id, method, collection_id, folder)) = row else {
            return Ok(None);
        };
        let sources = load_sources(&conn, job_id)?;
        Ok(method_of(&method).map(|method| ImportPlan {
            library_id,
            collection_id,
            folder,
            sources,
            method,
        }))
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

    /// Keep `entries` as the manifest of `job_id`, in one transaction, unless it already has
    /// one; answer the manifest kept. The transaction takes the write lock before it reads,
    /// so a write by another connection in between (the journal's, in the same file) waits
    /// for the busy timeout instead of failing. Writes the database.
    ///
    /// # Errors
    /// More than [`MAX_MANIFEST_ENTRIES`] entries, or the manifest could not be written.
    pub fn save_manifest(
        &self,
        job_id: &str,
        entries: &[ManifestEntry],
    ) -> Result<Vec<ManifestEntry>, String> {
        if entries.len() > MAX_MANIFEST_ENTRIES {
            return Err("the manifest is too large".to_string());
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        let kept = manifest_rows(&tx, job_id)?;
        if !kept.is_empty() {
            return Ok(kept);
        }
        insert_manifest(&tx, job_id, entries)?;
        tx.commit().map_err(failed)?;
        Ok(entries.to_vec())
    }

    /// The manifest of `job_id`; empty when none was kept yet. Reads the database.
    ///
    /// # Errors
    /// The manifest could not be read.
    pub fn manifest(&self, job_id: &str) -> Result<Vec<ManifestEntry>, String> {
        let conn = self.lock()?;
        manifest_rows(&conn, job_id)
    }

    /// Mark the job `job_id` finished (completed or failed), so later launches no longer
    /// queue it. Writes the database.
    ///
    /// # Errors
    /// The plans could not be written.
    pub fn finish(&self, job_id: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "UPDATE import_plans SET finished = 1 WHERE job_id = ?1",
                params![job_id],
            )
            .map(drop)
            .map_err(failed)
    }

    /// At most `limit` unfinished import jobs whose IDs come after `after` (all when
    /// `None`), in ID order: how a launch pages through every job a quit left. Reads the
    /// database.
    ///
    /// # Errors
    /// The plans could not be read.
    pub fn unfinished_after(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT job_id FROM import_plans WHERE finished = 0 AND job_id > ?1
                 ORDER BY job_id LIMIT ?2",
            )
            .map_err(failed)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![after.unwrap_or(""), limit], |row| row.get(0))
            .map_err(failed)?;
        rows.collect::<Result<Vec<String>, _>>().map_err(failed)
    }
}

/// One stored plan row: library, method, destination collection and folder.
type PlanRow = (String, String, Option<String>, Option<String>);

/// Add each of [`ADDED_COLUMNS`] its table lacks. Database I/O.
fn add_missing_columns(conn: &Connection) -> Result<(), String> {
    for (table, column, kind) in ADDED_COLUMNS {
        let present: bool = conn
            .query_row(
                &format!(
                    "SELECT EXISTS (SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)"
                ),
                params![column],
                |row| row.get(0),
            )
            .map_err(failed)?;
        if !present {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind};"))
                .map_err(failed)?;
        }
    }
    Ok(())
}

/// Insert the sources of `job_id` in order. Database I/O.
fn insert_sources(tx: &Transaction<'_>, job_id: &str, sources: &[PathBuf]) -> Result<(), String> {
    let mut stmt = tx
        .prepare("INSERT INTO import_sources (job_id, idx, path) VALUES (?1, ?2, ?3)")
        .map_err(failed)?;
    for (index, source) in sources.iter().enumerate() {
        stmt.execute(params![job_id, index as i64, path_bytes(source)])
            .map_err(failed)?;
    }
    Ok(())
}

/// The sources of `job_id` in order, at most [`MAX_PLAN_SOURCES`]. Database I/O.
fn load_sources(conn: &Connection, job_id: &str) -> Result<Vec<PathBuf>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM import_sources WHERE job_id = ?1 ORDER BY idx LIMIT ?2")
        .map_err(failed)?;
    let rows = stmt
        .query_map(params![job_id, MAX_PLAN_SOURCES as i64], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(failed)?;
    let bytes = rows.collect::<Result<Vec<_>, _>>().map_err(failed)?;
    bytes.into_iter().map(path_of).collect()
}

/// Insert the manifest of `job_id` in order. Database I/O.
fn insert_manifest(
    tx: &Transaction<'_>,
    job_id: &str,
    entries: &[ManifestEntry],
) -> Result<(), String> {
    let mut stmt = tx
        .prepare(
            "INSERT INTO import_manifest
             (job_id, idx, source_idx, path, file_id, skipped, logical_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .map_err(failed)?;
    for (index, entry) in entries.iter().enumerate() {
        let (source, path, file_id, skipped, logical) = match entry {
            ManifestEntry::File {
                path,
                source,
                file_id,
                logical_path,
            } => (
                *source,
                Some(path_bytes(path)),
                Some(file_id.as_str()),
                None,
                logical_path.as_deref(),
            ),
            ManifestEntry::Skipped { source, code } => {
                (*source, None, None, Some(code.as_str()), None)
            }
        };
        stmt.execute(params![
            job_id,
            index as i64,
            source as i64,
            path,
            file_id,
            skipped,
            logical
        ])
        .map_err(failed)?;
    }
    Ok(())
}

/// One stored manifest row: source index, path, file ID, skip code and logical path.
type ManifestRow = (
    i64,
    Option<Vec<u8>>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// The manifest of `job_id` in order, at most [`MAX_MANIFEST_ENTRIES`]. Database I/O.
fn manifest_rows(conn: &Connection, job_id: &str) -> Result<Vec<ManifestEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT source_idx, path, file_id, skipped, logical_path FROM import_manifest
             WHERE job_id = ?1 ORDER BY idx LIMIT ?2",
        )
        .map_err(failed)?;
    let rows = stmt
        .query_map(params![job_id, MAX_MANIFEST_ENTRIES as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .map_err(failed)?;
    let rows: Vec<ManifestRow> = rows.collect::<Result<_, _>>().map_err(failed)?;
    rows.into_iter().map(manifest_entry).collect()
}

/// The manifest entry of one stored row.
fn manifest_entry(row: ManifestRow) -> Result<ManifestEntry, String> {
    let source = usize::try_from(row.0).map_err(|_| "a manifest row is malformed".to_string())?;
    match row {
        (_, Some(path), Some(file_id), None, logical_path) => Ok(ManifestEntry::File {
            path: path_of(path)?,
            source,
            file_id,
            logical_path,
        }),
        (_, None, None, Some(code), None) => Ok(ManifestEntry::Skipped { source, code }),
        _ => Err("a manifest row is malformed".to_string()),
    }
}

/// A path's stored bytes: its own bytes on Unix. Pure.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// Stored path bytes as a path. Pure.
#[cfg(unix)]
fn path_of(bytes: Vec<u8>) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// A path's stored bytes: its UTF-16 code units, little-endian, so every Windows path
/// (unpaired surrogates included) round-trips exactly. Pure.
#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Stored path bytes as a path. Pure.
#[cfg(windows)]
fn path_of(bytes: Vec<u8>) -> Result<PathBuf, String> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        return Err("a stored path is malformed".to_string());
    }
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}
