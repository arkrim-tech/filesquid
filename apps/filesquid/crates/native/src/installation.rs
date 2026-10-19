//! The installation UUID (DE-S-012; [ADR 040](../../../../../resources/architecture/040-prov-o-history.md)
//! Decision 4; [ADR 021](../../../../../resources/architecture/021-uuid-v4-identifiers.md)).
//!
//! Each FileSquid installation has one UUID v4, generated from the
//! operating system's random source on the first start that finds none. It
//! names the installation agent of PROV-O records (`urn:uuid:<uuid>`) and is
//! the address of every OCFL version `user` the host writes. It is kept in
//! two copies in the data directory, `installation.json` (owner-only) and a
//! row of the operational store (`filesquid.sqlite`), and read at every
//! start. A missing copy is restored from the other; when both are lost a
//! new UUID is generated and the caller journals
//! `host.installation.created`. It is never derived from hardware, account
//! or host names. Errors name no path and no stored value.

use std::fmt;
use std::io::Write;
use std::path::Path;

use filesquid_core::InstallationId;
use rusqlite::{params, Connection, OptionalExtension};

use crate::owner_only::{create_owner_only, open_owner_only};
use crate::JOURNAL_FILE;

/// The file copy of the installation UUID in the data directory.
pub const INSTALLATION_FILE: &str = "installation.json";

/// Journal operation of a newly generated installation UUID.
pub const INSTALLATION_CREATED: &str = "host.installation.created";

/// Largest installation file read, in bytes.
const MAX_FILE_BYTES: u64 = 4096;

/// The operational-store table that holds the store copy.
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS host_installation (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    uuid TEXT NOT NULL
)";

/// Where the installation UUID of this start came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallationSource {
    /// Both copies held it.
    Stored,
    /// Only the operational store held it; the file was written back.
    RestoredFile,
    /// Only the file held it; the operational store was written back.
    RestoredStore,
    /// Neither copy existed: a new UUID was generated and stored in both.
    Created,
}

/// The installation UUID and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    /// The installation UUID.
    pub id: InstallationId,
    /// Where it came from at this start.
    pub source: InstallationSource,
}

/// Why the installation UUID could not be read or stored. Messages name no
/// path and no stored value.
#[derive(Debug)]
pub enum InstallationError {
    /// A copy exists but does not hold a UUID v4.
    Malformed {
        /// Which copy: `file` or `store`.
        copy: &'static str,
    },
    /// The two copies hold different UUIDs.
    Mismatch,
    /// The operating system's random source failed.
    Random,
    /// A copy could not be read or written; `kind` is the I/O error kind.
    Io {
        /// Which copy: `file` or `store`.
        copy: &'static str,
        /// The error kind.
        kind: std::io::ErrorKind,
    },
}

impl fmt::Display for InstallationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { copy } => {
                write!(f, "the installation ID {copy} copy is not a UUID v4")
            }
            Self::Mismatch => f.write_str("the installation ID copies disagree"),
            Self::Random => f.write_str("the random source failed"),
            Self::Io { copy, kind } => write!(f, "the installation ID {copy} copy: {kind}"),
        }
    }
}

impl std::error::Error for InstallationError {}

/// Read the installation UUID of `data_dir`, restoring a missing copy from
/// the other and generating a new one when both are missing. Filesystem
/// and database I/O.
///
/// # Errors
///
/// [`InstallationError`] when a copy is malformed, the copies disagree, a
/// copy cannot be read or written, or the random source fails.
pub fn load_or_create(data_dir: &Path) -> Result<Installation, InstallationError> {
    let file_path = data_dir.join(INSTALLATION_FILE);
    let mut store = open_store(&data_dir.join(JOURNAL_FILE))?;
    let guard = store
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| store_error(&err))?;
    let installation = settle(&file_path, &guard)?;
    guard.commit().map_err(|err| store_error(&err))?;
    Ok(installation)
}

/// Read both copies and restore or create what is missing, inside the store's immediate
/// transaction `store`, which serialises concurrent launches over the same data directory:
/// a second launch waits, then reads the copies the first one wrote. Filesystem and
/// database I/O.
fn settle(file_path: &Path, store: &Connection) -> Result<Installation, InstallationError> {
    let in_file = read_file(file_path)?;
    let in_store = read_store(store)?;
    let (id, source) = match (in_file, in_store) {
        (Some(a), Some(b)) if a == b => (a, InstallationSource::Stored),
        (Some(_), Some(_)) => return Err(InstallationError::Mismatch),
        (Some(id), None) => (id, InstallationSource::RestoredStore),
        (None, Some(id)) => (id, InstallationSource::RestoredFile),
        (None, None) => (mint()?, InstallationSource::Created),
    };
    if matches!(
        source,
        InstallationSource::Created | InstallationSource::RestoredFile
    ) {
        write_file(file_path, &id)?;
    }
    if matches!(
        source,
        InstallationSource::Created | InstallationSource::RestoredStore
    ) {
        write_store(store, &id)?;
    }
    if source != InstallationSource::Stored {
        tracing::info!(source = ?source, "installation ID restored or created");
    }
    Ok(Installation { id, source })
}

/// A new UUID v4 from the operating system's random source.
fn mint() -> Result<InstallationId, InstallationError> {
    let uuid = filesquid_api::library::mint_uuid_v4().map_err(|_| InstallationError::Random)?;
    InstallationId::parse(&uuid).map_err(|_| InstallationError::Random)
}

/// The file copy, if present (file I/O).
fn read_file(path: &Path) -> Result<Option<InstallationId>, InstallationError> {
    let io = |err: std::io::Error| InstallationError::Io {
        copy: "file",
        kind: err.kind(),
    };
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io(err)),
    };
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(InstallationError::Malformed { copy: "file" });
    }
    let text = std::fs::read_to_string(path).map_err(io)?;
    parse_file(&text).map(Some)
}

/// The UUID of an installation file's text.
fn parse_file(text: &str) -> Result<InstallationId, InstallationError> {
    let malformed = || InstallationError::Malformed { copy: "file" };
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| malformed())?;
    let uuid = value
        .get("installation_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(malformed)?;
    InstallationId::parse(uuid).map_err(|_| malformed())
}

/// Write the file copy owner-only, through a temporary file renamed into
/// place (file I/O).
fn write_file(path: &Path, id: &InstallationId) -> Result<(), InstallationError> {
    let io = |err: std::io::Error| InstallationError::Io {
        copy: "file",
        kind: err.kind(),
    };
    let staging = path.with_extension("json.tmp");
    if let Err(err) = std::fs::remove_file(&staging) {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(io(err));
        }
    }
    let text = format!("{{\"installation_id\":\"{}\"}}\n", id.as_str());
    let written = open_owner_only(&staging).and_then(|mut file| {
        file.set_len(0)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    if let Err(err) = written.and_then(|()| std::fs::rename(&staging, path)) {
        if let Err(cleanup) = std::fs::remove_file(&staging) {
            tracing::warn!(error = %cleanup.kind(), "could not remove the installation staging file");
        }
        return Err(io(err));
    }
    Ok(())
}

/// Open the operational store and ensure its installation table (database
/// I/O).
fn open_store(path: &Path) -> Result<Connection, InstallationError> {
    let fail = |err: rusqlite::Error| store_error(&err);
    create_owner_only(path).map_err(|err| InstallationError::Io {
        copy: "store",
        kind: err.kind(),
    })?;
    let conn = Connection::open(path).map_err(fail)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(fail)?;
    conn.execute_batch(SCHEMA).map_err(fail)?;
    Ok(conn)
}

/// The store copy, if present (database I/O).
fn read_store(conn: &Connection) -> Result<Option<InstallationId>, InstallationError> {
    let uuid: Option<String> = conn
        .query_row("SELECT uuid FROM host_installation WHERE id = 1", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|err| store_error(&err))?;
    uuid.map(|uuid| {
        InstallationId::parse(&uuid).map_err(|_| InstallationError::Malformed { copy: "store" })
    })
    .transpose()
}

/// Write the store copy (database I/O).
fn write_store(conn: &Connection, id: &InstallationId) -> Result<(), InstallationError> {
    conn.execute(
        "INSERT OR REPLACE INTO host_installation (id, uuid) VALUES (1, ?1)",
        params![id.as_str()],
    )
    .map_err(|err| store_error(&err))?;
    Ok(())
}

/// A store failure, logged with its detail on stdout only.
fn store_error(err: &rusqlite::Error) -> InstallationError {
    tracing::warn!(error = %err, "installation ID store copy failed");
    InstallationError::Io {
        copy: "store",
        kind: std::io::ErrorKind::Other,
    }
}
