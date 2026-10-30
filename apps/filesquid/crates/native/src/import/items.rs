//! Per-file progress of the desktop's import jobs (Task DE-1.8; API-S-037 **Per-file
//! progress**, owner answer 6 of 2026-10-22, verbatim: "Yes, report progress per file"): the
//! [`JobItems`] port of `filesquid-api` over the host's SQLite operational store, beside the
//! import plans.
//!
//! Each file of a job's manifest is one item, in the manifest's order (its index), named by
//! the file ID minted for it, its base name (never a folder path, DE-S-006) and the folder of
//! the destination collection it goes to. The import job sets each item's state as it runs:
//! `queued` when the manifest is kept, `copying` with the bytes read so far, then `copied`,
//! `skipped` (the library already holds it) or `failed` with its error code. A retry puts a
//! failed item back to `queued` in one statement; the job then imports it alone. Entries of the
//! manifest that could not be listed have no file and no item; they are journaled with their
//! code.
//!
//! The same store keeps the SHA-256 of every file an import committed, per library, as the
//! API answered it from the commit, so a plan can tell which picked files the library already
//! holds (API-S-036 **Review**) and the job skips them.

use filesquid_api::jobs::{
    page_of_items as page_of, JobDestination, JobError, JobItem, JobItemPage, JobItemState,
    JobItems,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Schema: one row per file of an import job's manifest.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS import_items (
    job_id TEXT NOT NULL,
    idx INTEGER NOT NULL,
    file_id TEXT NOT NULL,
    name TEXT NOT NULL,
    folder TEXT,
    state TEXT NOT NULL,
    bytes_done INTEGER NOT NULL DEFAULT 0,
    bytes_total INTEGER NOT NULL DEFAULT 0,
    error_code TEXT,
    PRIMARY KEY (job_id, idx)
) WITHOUT ROWID;
CREATE UNIQUE INDEX IF NOT EXISTS import_items_file ON import_items(job_id, file_id);
CREATE TABLE IF NOT EXISTS import_digests (
    library_id TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    file_id TEXT NOT NULL,
    PRIMARY KEY (library_id, sha256, file_id)
) WITHOUT ROWID;
";

/// Most items one job keeps (its manifest's bound).
pub const MAX_JOB_ITEMS: usize = super::job::MAX_IMPORT_FILES;

/// One file of a manifest, as its item is first kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewItem {
    /// The index of its manifest entry.
    pub index: usize,
    /// The file ID minted for it.
    pub file_id: String,
    /// Its base name.
    pub name: String,
    /// The destination folder inside the collection; the collection's root when `None`.
    pub folder: Option<String>,
    /// Its size in bytes when listed.
    pub bytes_total: u64,
}

/// The per-file progress of import jobs in an SQLite operational store.
pub struct SqliteJobItems {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteJobItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteJobItems").finish_non_exhaustive()
    }
}

/// A store failure as a job error with no path. Pure.
fn store_error(err: rusqlite::Error) -> JobError {
    JobError::Failed(format!(
        "the import items could not be read or written ({err})"
    ))
}

impl SqliteJobItems {
    /// Open (creating when missing) the items in the store at `path`, the store the import
    /// plans live in ([`super::plans::SqliteImportPlans`], opened first, holds each job's
    /// destination). Opens a database connection.
    ///
    /// # Errors
    /// A message with no path when the store cannot be opened or prepared.
    pub fn open(path: &Path) -> Result<Self, String> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
        let conn = Connection::open_with_flags(path, flags)
            .map_err(|err| format!("the import items could not be opened ({err})"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .and_then(|()| conn.execute_batch(SCHEMA))
            .map_err(|err| format!("the import items could not be prepared ({err})"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The connection, locked.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, JobError> {
        self.conn
            .lock()
            .map_err(|_| JobError::Unavailable("the import items are unavailable".into()))
    }

    /// Keep `items` of job `job_id` as `queued`, in one transaction; items kept before are
    /// left as they are, so keeping a manifest again changes nothing. Writes the store.
    ///
    /// # Errors
    /// A store failure; nothing is kept then.
    pub fn keep(&self, job_id: &str, items: &[NewItem]) -> Result<(), JobError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(store_error)?;
        for item in items.iter().take(MAX_JOB_ITEMS) {
            tx.execute(
                "INSERT OR IGNORE INTO import_items
                 (job_id, idx, file_id, name, folder, state, bytes_total)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
                params![
                    job_id,
                    item.index as i64,
                    item.file_id,
                    item.name,
                    item.folder,
                    i64::try_from(item.bytes_total).unwrap_or(i64::MAX)
                ],
            )
            .map_err(store_error)?;
        }
        tx.commit().map_err(store_error)
    }

    /// Set the item of manifest entry `index` of job `job_id` to `state`, with `bytes_done`
    /// and, for a failure, its `error_code`. Writes the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn set(
        &self,
        job_id: &str,
        index: usize,
        state: JobItemState,
        bytes_done: u64,
        error_code: Option<&str>,
    ) -> Result<(), JobError> {
        self.lock()?
            .execute(
                "UPDATE import_items SET state = ?3, bytes_done = ?4, error_code = ?5
                 WHERE job_id = ?1 AND idx = ?2",
                params![
                    job_id,
                    index as i64,
                    state.as_str(),
                    i64::try_from(bytes_done).unwrap_or(i64::MAX),
                    error_code
                ],
            )
            .map(|_| ())
            .map_err(store_error)
    }

    /// Record that `bytes` of the file `file_id` of job `job_id` are copied, while it is
    /// `copying` (at most its size). Writes the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn progress(&self, job_id: &str, file_id: &str, bytes: u64) -> Result<(), JobError> {
        self.lock()?
            .execute(
                "UPDATE import_items SET bytes_done = MIN(?3, bytes_total)
                 WHERE job_id = ?1 AND file_id = ?2 AND state = 'copying'",
                params![job_id, file_id, i64::try_from(bytes).unwrap_or(i64::MAX)],
            )
            .map(|_| ())
            .map_err(store_error)
    }

    /// The manifest indexes of job `job_id`'s items that wait to be imported (`queued`, or
    /// `copying` when a run stopped in the middle of the file), in order. Reads the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn waiting(&self, job_id: &str) -> Result<Vec<usize>, JobError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT idx FROM import_items
                 WHERE job_id = ?1 AND state IN ('queued', 'copying') ORDER BY idx LIMIT ?2",
            )
            .map_err(store_error)?;
        let rows = stmt
            .query_map(params![job_id, MAX_JOB_ITEMS as i64], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(store_error)?;
        rows.map(|row| {
            row.map_err(store_error)
                .map(|idx| usize::try_from(idx).unwrap_or(usize::MAX))
        })
        .collect()
    }

    /// Whether any item of job `job_id` failed. Reads the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn has_failed(&self, job_id: &str) -> Result<bool, JobError> {
        self.lock()?
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM import_items WHERE job_id = ?1 AND state = 'failed')",
                params![job_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(store_error)
    }

    /// Every item of job `job_id`, in order, at most [`MAX_JOB_ITEMS`]. Reads the store.
    fn all(&self, job_id: &str) -> Result<Vec<JobItem>, JobError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT file_id, name, folder, state, bytes_done, bytes_total, error_code
                 FROM import_items WHERE job_id = ?1 ORDER BY idx LIMIT ?2",
            )
            .map_err(store_error)?;
        let rows = stmt
            .query_map(params![job_id, MAX_JOB_ITEMS as i64], item_of)
            .map_err(store_error)?;
        rows.map(|row| row.map_err(store_error)).collect()
    }
}

/// The item of one row. Pure over the row.
fn item_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobItem> {
    let state: String = row.get(3)?;
    Ok(JobItem {
        item_id: row.get(0)?,
        name: row.get(1)?,
        folder: row.get(2)?,
        state: JobItemState::parse(&state).unwrap_or(JobItemState::Failed),
        bytes_done: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
        bytes_total: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        error_code: row.get(6)?,
    })
}

impl JobItems for SqliteJobItems {
    fn destination(&self, job_id: &str) -> Result<Option<JobDestination>, JobError> {
        let found = self
            .lock()?
            .query_row(
                "SELECT collection_id, folder FROM import_plans WHERE job_id = ?1",
                params![job_id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(store_error)?;
        Ok(found.and_then(|(collection, folder)| {
            collection.map(|collection_id| JobDestination {
                collection_id,
                folder,
            })
        }))
    }

    fn page(
        &self,
        job_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<JobItemPage, JobError> {
        Ok(page_of(&self.all(job_id)?, cursor, limit))
    }

    fn retry(&self, job_id: &str, item: Option<&str>) -> Result<Vec<String>, JobError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(store_error)?;
        if let Some(item) = item {
            let known = tx
                .query_row(
                    "SELECT 1 FROM import_items WHERE job_id = ?1 AND file_id = ?2",
                    params![job_id, item],
                    |_| Ok(()),
                )
                .optional()
                .map_err(store_error)?;
            known.ok_or(JobError::NotFound)?;
        }
        let put_back = requeue(&tx, job_id, item)?;
        tx.commit().map_err(store_error)?;
        Ok(put_back)
    }
}

/// Put the failed item `item` (every failed item when `None`) of job `job_id` back to queued
/// and answer their file IDs. Writes the store.
fn requeue(
    tx: &rusqlite::Transaction<'_>,
    job_id: &str,
    item: Option<&str>,
) -> Result<Vec<String>, JobError> {
    let mut stmt = tx
        .prepare(
            "UPDATE import_items SET state = 'queued', bytes_done = 0, error_code = NULL
             WHERE job_id = ?1 AND state = 'failed' AND (?2 IS NULL OR file_id = ?2)
             RETURNING file_id",
        )
        .map_err(store_error)?;
    let rows = stmt
        .query_map(params![job_id, item], |row| row.get::<_, String>(0))
        .map_err(store_error)?;
    let mut put_back: Vec<String> = rows
        .map(|row| row.map_err(store_error))
        .collect::<Result<_, _>>()?;
    put_back.sort();
    Ok(put_back)
}

impl SqliteJobItems {
    /// Set the item of manifest entry `index` of job `job_id` to `copied`, its `size` bytes
    /// all done (the size it was committed with, which a file missing when the manifest was
    /// kept did not have then). Writes the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn copied(&self, job_id: &str, index: usize, size: u64) -> Result<(), JobError> {
        let size = i64::try_from(size).unwrap_or(i64::MAX);
        self.lock()?
            .execute(
                "UPDATE import_items SET state = 'copied', bytes_done = ?3, bytes_total = ?3,
                 error_code = NULL WHERE job_id = ?1 AND idx = ?2",
                params![job_id, index as i64, size],
            )
            .map(|_| ())
            .map_err(store_error)
    }
}

/// The items of the files of `manifest`, each named by its base name and placed in the folder
/// of its collection logical path, with its size as listed now (0 when it cannot be read).
/// Reads each file's metadata.
pub fn items_of_manifest(manifest: &[super::plans::ManifestEntry]) -> Vec<NewItem> {
    manifest
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match entry {
            super::plans::ManifestEntry::File {
                path,
                file_id,
                logical_path,
                ..
            } => Some(NewItem {
                index,
                file_id: file_id.clone(),
                name: base_name(path),
                folder: logical_path.as_deref().and_then(folder_of),
                bytes_total: std::fs::metadata(path).map_or(0, |meta| meta.len()),
            }),
            super::plans::ManifestEntry::Skipped { .. } => None,
        })
        .collect()
}

/// The base name of `path`, lossily as text. Pure.
fn base_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The folder of a collection logical path (all but its last segment), `None` at the
/// collection's root. Pure.
fn folder_of(logical_path: &str) -> Option<String> {
    logical_path
        .rsplit_once('/')
        .map(|(folder, _)| folder.to_string())
}

/// Open the import plans and their items in the operational store at `path`, the plans first
/// so the items can read each job's destination. Opens two database connections.
///
/// # Errors
/// A message with no path when either cannot be opened.
pub fn open_import_stores(
    path: &Path,
) -> Result<
    (
        std::sync::Arc<super::plans::SqliteImportPlans>,
        std::sync::Arc<SqliteJobItems>,
    ),
    String,
> {
    let plans = super::plans::SqliteImportPlans::open(path)?;
    let items = SqliteJobItems::open(path)?;
    Ok((std::sync::Arc::new(plans), std::sync::Arc::new(items)))
}

impl SqliteJobItems {
    /// Remember that the import of `file_id` committed content of digest `sha256` into
    /// `library_id`. Writes the store; remembering it again changes nothing.
    ///
    /// # Errors
    /// A store failure.
    pub fn record_digest(
        &self,
        library_id: &str,
        sha256: &str,
        file_id: &str,
    ) -> Result<(), JobError> {
        self.lock()?
            .execute(
                "INSERT OR IGNORE INTO import_digests (library_id, sha256, file_id)
                 VALUES (?1, ?2, ?3)",
                params![library_id, sha256, file_id],
            )
            .map(|_| ())
            .map_err(store_error)
    }

    /// Whether an import committed content of digest `sha256` into `library_id`. Reads the
    /// store.
    ///
    /// # Errors
    /// A store failure.
    pub fn holds_digest(&self, library_id: &str, sha256: &str) -> Result<bool, JobError> {
        self.lock()?
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM import_digests
                 WHERE library_id = ?1 AND sha256 = ?2)",
                params![library_id, sha256],
                |row| row.get::<_, bool>(0),
            )
            .map_err(store_error)
    }

    /// Whether the item of manifest entry `index` of job `job_id` is `skipped` (the library
    /// already held it when the import was planned). Reads the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn is_skipped(&self, job_id: &str, index: usize) -> Result<bool, JobError> {
        self.lock()?
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM import_items
                 WHERE job_id = ?1 AND idx = ?2 AND state = 'skipped')",
                params![job_id, index as i64],
                |row| row.get::<_, bool>(0),
            )
            .map_err(store_error)
    }
}
