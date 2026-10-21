//! The desktop's preference store (Task DE-1.1 host wiring of API-F2.2; API-S-011,
//! SRS-F-062): each user's UI preferences in the host's SQLite operational store
//! (`filesquid.sqlite`), as the JSON body the preference routes check, keyed by the user's
//! opaque actor name. At most [`MAX_PREFERENCE_USERS`] users.

use filesquid_api::preferences::{PreferenceStore, PreferenceStoreError, Preferences};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Most users the store keeps (AGENTS.md section 22).
pub const MAX_PREFERENCE_USERS: i64 = 10_000;

/// Schema: one row per user.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ui_preferences (
    user TEXT PRIMARY KEY,
    preferences TEXT NOT NULL
);
";

/// Each user's preferences in an SQLite operational store.
pub struct SqlitePreferences {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqlitePreferences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlitePreferences").finish_non_exhaustive()
    }
}

/// The flags of the store's connection: never through a symbolic link
/// (`SQLITE_OPEN_NOFOLLOW`), as the other operational stores open the file. Pure.
fn open_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
}

/// A store failure as the port reports it; the detail goes to stdout only.
fn unavailable(err: impl std::fmt::Display) -> PreferenceStoreError {
    PreferenceStoreError::Unavailable(err.to_string())
}

impl SqlitePreferences {
    /// Open the store in the operational store at `path` (the activity journal's file,
    /// created owner-only by the journal). Opens a database connection, held until the store
    /// is dropped.
    ///
    /// # Errors
    ///
    /// The database or its table could not be opened.
    pub fn open(path: &Path) -> Result<Self, PreferenceStoreError> {
        let path = crate::owner_only::resolve_folders(path).map_err(unavailable)?;
        crate::owner_only::create_owner_only(&path).map_err(unavailable)?;
        let conn = Connection::open_with_flags(&path, open_flags()).map_err(unavailable)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(unavailable)?;
        conn.execute_batch(SCHEMA).map_err(unavailable)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The locked connection.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, PreferenceStoreError> {
        self.conn
            .lock()
            .map_err(|_| unavailable("preference store lock poisoned"))
    }
}

impl PreferenceStore for SqlitePreferences {
    fn load(&self, user: &str) -> Result<Option<Preferences>, PreferenceStoreError> {
        let conn = self.lock()?;
        let stored: Option<String> = conn
            .query_row(
                "SELECT preferences FROM ui_preferences WHERE user = ?1",
                params![user],
                |row| row.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        stored
            .map(|text| serde_json::from_str(&text).map_err(unavailable))
            .transpose()
    }

    fn save(&self, user: &str, preferences: &Preferences) -> Result<(), PreferenceStoreError> {
        let text = serde_json::to_string(preferences).map_err(unavailable)?;
        let mut conn = self.lock()?;
        // An immediate transaction takes the write lock first, so the cap check and the
        // upsert are one step even across connections to the same file.
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let changed = tx
            .execute(
                "INSERT INTO ui_preferences (user, preferences)
                 SELECT ?1, ?2
                 WHERE EXISTS (SELECT 1 FROM ui_preferences WHERE user = ?1)
                    OR (SELECT COUNT(*) FROM ui_preferences) < ?3
                 ON CONFLICT(user) DO UPDATE SET preferences = excluded.preferences",
                params![user, text, MAX_PREFERENCE_USERS],
            )
            .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        match changed {
            0 => Err(PreferenceStoreError::Full),
            _ => Ok(()),
        }
    }
}
