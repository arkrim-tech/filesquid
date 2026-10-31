//! The installation's Recently deleted settings in the desktop's operational store
//! (`filesquid.sqlite`; Task API-F2.15, API-S-019, ADR 041 Decisions 10 and 11): the default
//! retention period and the expiry setting, one row, read by every library and changed only
//! by the admin (the route checks it). A change is stored, then its journal entry is
//! appended to the host's journal before the change is answered, so no change is ever
//! acknowledged unjournaled; the entry carries no value.

use filesquid_api::journal::{JournalEntry, JournalStore};
use filesquid_api::library::{CatalogError, ExpiryMode, InstallationSettings, RetentionSettings};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// The settings table, created when missing.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS recently_deleted_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    default_retention_days INTEGER NOT NULL CHECK (default_retention_days BETWEEN 1 AND 3650),
    expiry TEXT NOT NULL CHECK (expiry IN ('keep_version_history', 'delete_all_version_history'))
);
";

/// [`RetentionSettings`] over the operational store.
pub struct SqliteRetentionSettings {
    /// The store's connection.
    conn: Mutex<Connection>,
    /// The host journal each change is appended to.
    journal: Arc<dyn JournalStore>,
}

impl std::fmt::Debug for SqliteRetentionSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteRetentionSettings")
            .finish_non_exhaustive()
    }
}

/// A store failure with `err`'s text (never a path: SQLite errors name none).
fn unavailable(err: impl std::fmt::Display) -> CatalogError {
    CatalogError::Unavailable(format!("settings store: {err}"))
}

impl SqliteRetentionSettings {
    /// Open the operational store at `path` (owner-only, created when missing) and its
    /// settings table; changes are journaled in `journal`. Filesystem and database I/O.
    ///
    /// # Errors
    /// The file or table cannot be opened.
    pub fn open(path: &Path, journal: Arc<dyn JournalStore>) -> Result<Self, CatalogError> {
        let path = crate::owner_only::resolve_folders(path).map_err(unavailable)?;
        crate::owner_only::create_owner_only(&path).map_err(unavailable)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let conn = Connection::open_with_flags(&path, flags).map_err(unavailable)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(unavailable)?;
        conn.execute_batch(SCHEMA).map_err(unavailable)?;
        Ok(Self {
            conn: Mutex::new(conn),
            journal,
        })
    }

    /// The connection, or a store failure for a poisoned lock.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, CatalogError> {
        self.conn.lock().map_err(|_| unavailable("lock poisoned"))
    }
}

impl RetentionSettings for SqliteRetentionSettings {
    fn read(&self) -> Result<InstallationSettings, CatalogError> {
        let conn = self.lock()?;
        let row: Option<(u32, String)> = conn
            .query_row(
                "SELECT default_retention_days, expiry FROM recently_deleted_settings",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?;
        Ok(match row {
            None => InstallationSettings::default(),
            Some((days, expiry)) => InstallationSettings {
                default_retention_days: days,
                expiry: match expiry.as_str() {
                    "delete_all_version_history" => ExpiryMode::DeleteAllVersionHistory,
                    _ => ExpiryMode::KeepVersionHistory,
                },
            },
        })
    }

    fn store(
        &self,
        settings: &InstallationSettings,
        journal: JournalEntry,
    ) -> Result<(), CatalogError> {
        if !settings.is_valid() {
            return Err(CatalogError::Failed("retention period out of range".into()));
        }
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO recently_deleted_settings (singleton, default_retention_days, expiry)
             VALUES (1, ?1, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
                 default_retention_days = excluded.default_retention_days,
                 expiry = excluded.expiry",
            params![settings.default_retention_days, settings.expiry.as_str()],
        )
        .map_err(unavailable)?;
        self.journal
            .append(&[journal.redacted()])
            .map(drop)
            .map_err(|_| CatalogError::Unavailable("the journal refused the change".into()))
    }
}
