//! The schema of one library's catalog file and its versioned migration
//! ([ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md):
//! "Schema migrations are versioned per database").
//!
//! Every library has its own file, so no table carries a library column: the file is the
//! library. Version 1 holds the library row, folders, collections, virtual collections,
//! virtual-collection memberships, files with their hide and delete state, operation
//! receipts, pending intent and the journal outbox.

use super::{failed, CATALOG_FILE_EXTENSION, CATALOG_SCHEMA_VERSION};
use crate::journal_sqlite::create_owner_only;
use filesquid_api::library::{is_uuid_v4, CatalogError};
use filesquid_core::LibraryId;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a statement waits for another connection's lock before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Version 1 of the catalog schema.
///
/// - `library`: exactly one row (`singleton = 1`): the library, its repository, name,
///   revision counter and creation order (Unix milliseconds of its creation).
/// - `virtual_members` references `virtual_collections` and `files` with `ON DELETE CASCADE`,
///   so removing a virtual collection or purging a file removes its memberships. Files do
///   not reference collections: removing a collection keeps its files (they become unfiled).
/// - `receipts`: the answer of every committed operation ID, for replays.
/// - `pending_intents`: intent recorded before canonical work (cleared by its commit).
/// - `journal_outbox`: journal entries committed with their change, until the host journal
///   holds them.
const SCHEMA_V1: &str = "
CREATE TABLE library (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    id TEXT NOT NULL,
    repository_id TEXT NOT NULL,
    name TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    created_order INTEGER NOT NULL
);
CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    order_key INTEGER NOT NULL
);
CREATE TABLE collections (
    id TEXT PRIMARY KEY,
    folder_id TEXT,
    title TEXT NOT NULL,
    sort TEXT NOT NULL,
    cover_file_id TEXT,
    order_key INTEGER NOT NULL
);
CREATE INDEX collections_page ON collections(order_key, id);
CREATE TABLE virtual_collections (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    expression TEXT,
    cover_file_id TEXT,
    order_key INTEGER NOT NULL
);
CREATE INDEX virtual_collections_page ON virtual_collections(order_key, id);
CREATE TABLE files (
    id TEXT PRIMARY KEY,
    home_collection_id TEXT NOT NULL,
    list_order INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
    deleted_at_ms INTEGER,
    purge_after_ms INTEGER,
    CHECK ((deleted_at_ms IS NULL) = (purge_after_ms IS NULL))
);
CREATE INDEX files_page ON files(list_order, id);
CREATE INDEX files_home ON files(home_collection_id, list_order, id);
CREATE TABLE virtual_members (
    virtual_collection_id TEXT NOT NULL
        REFERENCES virtual_collections(id) ON DELETE CASCADE,
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    order_key INTEGER NOT NULL,
    PRIMARY KEY (virtual_collection_id, file_id)
) WITHOUT ROWID;
CREATE INDEX virtual_members_page ON virtual_members(virtual_collection_id, order_key, file_id);
CREATE INDEX virtual_members_file ON virtual_members(file_id);
CREATE TABLE receipts (
    operation_id TEXT PRIMARY KEY,
    revision TEXT NOT NULL,
    created_id TEXT,
    deleted_at_ms INTEGER,
    purge_after_ms INTEGER,
    CHECK ((deleted_at_ms IS NULL) = (purge_after_ms IS NULL))
);
CREATE TABLE pending_intents (
    operation_id TEXT PRIMARY KEY,
    operation TEXT NOT NULL,
    expected_revision TEXT NOT NULL,
    recorded_at_ms INTEGER NOT NULL
);
CREATE TABLE journal_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry TEXT NOT NULL
);
";

/// The catalog file name of `library_id`: `<uuid>.sqlite`, or `None` when the identifier is
/// not a canonical UUID v4 (ADR 021), so no identifier can name a path outside the catalog
/// directory. Pure.
pub(crate) fn file_name(library_id: &LibraryId) -> Option<String> {
    is_uuid_v4(library_id.as_str())
        .then(|| format!("{}.{CATALOG_FILE_EXTENSION}", library_id.as_str()))
}

/// The library a catalog file name belongs to, or `None` for any other file. Pure.
pub(crate) fn library_of(file_name: &str) -> Option<LibraryId> {
    let stem = file_name.strip_suffix(&format!(".{CATALOG_FILE_EXTENSION}"))?;
    is_uuid_v4(stem)
        .then(|| LibraryId::new(stem).ok())
        .flatten()
}

/// The catalog file of `library_id` under `root`. Pure.
///
/// # Errors
/// [`CatalogError::Failed`] when the identifier is not a canonical UUID v4.
pub(crate) fn file_path(root: &Path, library_id: &LibraryId) -> Result<PathBuf, CatalogError> {
    file_name(library_id)
        .map(|name| root.join(name))
        .ok_or_else(|| failed("library identifier is not a UUID v4"))
}

/// Open (creating if needed, owner-only) the catalog file at `path` in WAL mode with foreign
/// keys on, and bring its schema to [`CATALOG_SCHEMA_VERSION`]. Filesystem and database I/O.
///
/// # Errors
/// [`CatalogError::Unavailable`] when the file cannot be created or opened;
/// [`CatalogError::Failed`] when its schema is newer than this host or migration fails.
pub(crate) fn open_catalog_db(path: &Path) -> Result<Connection, CatalogError> {
    create_owner_only(path).map_err(|e| unavailable("create catalog file", &e))?;
    let conn = Connection::open(path).map_err(|e| unavailable("open catalog file", &e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| unavailable("set WAL mode", &e))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| unavailable("set synchronous", &e))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| unavailable("enable foreign keys", &e))?;
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| unavailable("set busy timeout", &e))?;
    migrate(conn)
}

/// Bring the schema of `conn` to [`CATALOG_SCHEMA_VERSION`] in one transaction. Database I/O.
fn migrate(mut conn: Connection) -> Result<Connection, CatalogError> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| failed(format!("read schema version: {e}")))?;
    if version > CATALOG_SCHEMA_VERSION {
        return Err(failed(format!(
            "catalog schema version {version} is newer than this host ({CATALOG_SCHEMA_VERSION})"
        )));
    }
    if version == CATALOG_SCHEMA_VERSION {
        return Ok(conn);
    }
    let tx = conn
        .transaction()
        .map_err(|e| failed(format!("begin migration: {e}")))?;
    tx.execute_batch(SCHEMA_V1)
        .map_err(|e| failed(format!("create schema v1: {e}")))?;
    tx.pragma_update(None, "user_version", CATALOG_SCHEMA_VERSION)
        .map_err(|e| failed(format!("set schema version: {e}")))?;
    tx.commit()
        .map_err(|e| failed(format!("commit migration: {e}")))?;
    Ok(conn)
}

/// An [`CatalogError::Unavailable`] for `step`. The I/O error's kind and message are kept; a
/// path is never added, so the error carries no path to the API.
fn unavailable(step: &str, err: &dyn std::fmt::Display) -> CatalogError {
    CatalogError::Unavailable(format!("{step}: {err}"))
}
