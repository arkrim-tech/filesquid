//! The desktop's library registry across launches (Task DE-1.8; API-S-038, DE-S-018): what
//! [`crate::HostLocations`] remembers of each library it bound (its name, the location it was
//! last opened from and when) and the folder behind that location, in the host's SQLite
//! operational store, so the recent libraries and the library facts survive a restart.
//!
//! A row holds the canonical folder of its location so the next launch can register it again
//! under the same opaque location ID. The folder stays in the operational store (owner-only,
//! DE-S-014) and never crosses the API (DE-S-006). At most [`MAX_REMEMBERED_LIBRARIES`] rows
//! are kept, the least recently opened forgotten first, as the in-memory book does.

use filesquid_api::local_library::MAX_REMEMBERED_LIBRARIES;
use filesquid_core::LibraryId;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Schema: one row per remembered library.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS library_registry (
    library_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location_id TEXT NOT NULL,
    folder BLOB NOT NULL,
    last_opened_ms INTEGER NOT NULL
) WITHOUT ROWID;
";

/// One remembered library, as kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryRow {
    /// The library.
    pub library_id: LibraryId,
    /// Its name.
    pub name: String,
    /// The opaque location it was last opened from.
    pub location_id: String,
    /// The canonical folder of that location.
    pub folder: PathBuf,
    /// When it was last created or opened (Unix milliseconds, UTC).
    pub last_opened_ms: i64,
}

/// The library registry in an SQLite operational store.
pub struct SqliteLibraryRegistry {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteLibraryRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteLibraryRegistry")
            .finish_non_exhaustive()
    }
}

/// A store failure as a message with no path. Pure.
fn failed(err: rusqlite::Error) -> String {
    format!("the library registry could not be read or written ({err})")
}

impl SqliteLibraryRegistry {
    /// Open (creating when missing, owner-only, never through a symbolic link) the registry in
    /// the store at `path`. Opens a database connection.
    ///
    /// # Errors
    /// A message with no path when the store cannot be opened or prepared.
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = crate::owner_only::open_operational_db(path, "the library registry")?;
        conn.execute_batch(SCHEMA).map_err(failed)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The connection, locked.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "the library registry is unavailable".to_string())
    }

    /// Keep `row`, replacing what was kept of its library, and forget every other library not
    /// in `retained` (the libraries the in-memory book still holds, at most
    /// [`MAX_REMEMBERED_LIBRARIES`]), in one transaction, so the store keeps exactly the set the
    /// book keeps, whatever its eviction decided. Writes the store.
    ///
    /// # Errors
    /// A store failure; nothing is kept then.
    pub fn save(&self, row: &RegistryRow, retained: &[LibraryId]) -> Result<(), String> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(failed)?;
        tx.execute(
            "INSERT OR REPLACE INTO library_registry
             (library_id, name, location_id, folder, last_opened_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                row.library_id.as_str(),
                row.name,
                row.location_id,
                folder_bytes(&row.folder),
                row.last_opened_ms
            ],
        )
        .map_err(failed)?;
        let kept: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT library_id FROM library_registry")
                .map_err(failed)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(failed)?;
            rows.collect::<Result<_, _>>().map_err(failed)?
        };
        for library in kept {
            if !retained.iter().any(|id| id.as_str() == library) {
                tx.execute(
                    "DELETE FROM library_registry WHERE library_id = ?1",
                    params![library],
                )
                .map_err(failed)?;
            }
        }
        tx.commit().map_err(failed)
    }

    /// Rename the kept `library_id` to `name`; a library not kept is left alone. Writes the
    /// store.
    ///
    /// # Errors
    /// A store failure.
    pub fn rename(&self, library_id: &LibraryId, name: &str) -> Result<(), String> {
        self.lock()?
            .execute(
                "UPDATE library_registry SET name = ?2 WHERE library_id = ?1",
                params![library_id.as_str(), name],
            )
            .map(|_| ())
            .map_err(failed)
    }

    /// Every kept library, most recently opened first; a row whose library ID or folder
    /// cannot be read is left out. Reads the store.
    ///
    /// # Errors
    /// A store failure.
    pub fn rows(&self) -> Result<Vec<RegistryRow>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT library_id, name, location_id, folder, last_opened_ms
                 FROM library_registry ORDER BY last_opened_ms DESC, library_id LIMIT ?1",
            )
            .map_err(failed)?;
        let rows = stmt
            .query_map(params![MAX_REMEMBERED_LIBRARIES as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(failed)?;
        let mut kept = Vec::new();
        for row in rows {
            let (library, name, location_id, folder, last_opened_ms) = row.map_err(failed)?;
            let (Ok(library_id), Some(folder)) = (LibraryId::new(&library), folder_of(folder))
            else {
                continue;
            };
            kept.push(RegistryRow {
                library_id,
                name,
                location_id,
                folder,
                last_opened_ms,
            });
        }
        Ok(kept)
    }
}

/// `folder` as stored: its bytes on Unix. Pure.
#[cfg(unix)]
fn folder_bytes(folder: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    folder.as_os_str().as_bytes().to_vec()
}

/// The folder stored as `bytes` on Unix. Pure.
#[cfg(unix)]
fn folder_of(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// `folder` as stored on Windows: its UTF-16 code units, little-endian, so a name that is not
/// valid Unicode survives. Pure.
#[cfg(windows)]
fn folder_bytes(folder: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    folder
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// The folder stored as UTF-16 code units on Windows. Pure.
#[cfg(windows)]
fn folder_of(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Some(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

/// `folder` as stored on another platform: its text. Pure.
#[cfg(not(any(unix, windows)))]
fn folder_bytes(folder: &Path) -> Vec<u8> {
    folder.to_string_lossy().into_owned().into_bytes()
}

/// The folder stored as text on another platform. Pure.
#[cfg(not(any(unix, windows)))]
fn folder_of(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}
