//! The installation UUID (DE-S-012; [ADR 040](../../../../../resources/architecture/040-prov-o-history.md)
//! Decision 4; [ADR 021](../../../../../resources/architecture/021-uuid-v4-identifiers.md)).
//!
//! Each FileSquid installation has one UUID v4, generated from the
//! operating system's random source on the first start that finds none. It
//! names the installation agent of PROV-O records (`urn:uuid:<uuid>`) and is
//! the address of every OCFL version `user` the host writes. It is kept in
//! two copies in the data directory, `installation.json` (owner-only;
//! `{"schema": 1, "installation_id": "<uuid>", "created": "<RFC 3339 UTC>"}`)
//! and a row of the operational store (`filesquid.sqlite`) with the same
//! creation time, and read at every start. A file an earlier build wrote
//! (no `schema`, no `created`) is still read and is rewritten as schema 1. A missing copy is restored from the other; when both are lost a
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

/// The schema of the installation file this build writes.
pub const FILE_SCHEMA: u64 = 1;

/// The operational-store table that holds the store copy.
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS host_installation (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    uuid TEXT NOT NULL,
    created TEXT
)";

/// One copy of the installation UUID: the UUID and, unless an earlier build wrote the copy,
/// when the installation was created (RFC 3339 UTC).
#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredCopy {
    id: InstallationId,
    created: Option<String>,
}

/// The outcome of settling the store copy: the UUID, where it came from, its creation time,
/// and whether the file copy must be (re)written.
struct Settled {
    id: InstallationId,
    source: InstallationSource,
    created: String,
    write_file: bool,
}

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
    let in_file = read_file(&file_path)?;
    let mut store = open_store(&data_dir.join(JOURNAL_FILE))?;
    let Settled {
        id,
        source,
        created,
        write_file: rewrite,
    } = settle_store(&mut store, in_file)?;
    if rewrite {
        rewrite_file(&file_path, &store, (&id, &created, source))?;
    }
    if source != InstallationSource::Stored {
        tracing::info!(source = ?source, "installation ID restored or created");
    }
    Ok(Installation { id, source })
}

/// Write the file copy of `id`, created at `created`. A file that already held the UUID (an
/// earlier build's file being upgraded to schema 1) and cannot be rewritten stays as it is,
/// with a warning (stdout), since it still names the installation; any other failure undoes
/// a store copy this launch created and is answered. File and database I/O.
///
/// # Errors
/// The file copy could not be written where none held the UUID.
fn rewrite_file(
    path: &Path,
    store: &Connection,
    (id, created, source): (&InstallationId, &str, InstallationSource),
) -> Result<(), InstallationError> {
    let Err(err) = write_file(path, id, created) else {
        return Ok(());
    };
    if matches!(
        source,
        InstallationSource::Stored | InstallationSource::RestoredStore
    ) {
        tracing::warn!(error = %err, "the installation file could not be upgraded to schema 1");
        return Ok(());
    }
    forget_created(store, id, source);
    Err(err)
}

/// Remove the store copy this launch just created when its file copy could not be written,
/// so the next launch creates (and journals) the installation again instead of reporting a
/// restore. Only the row naming `id` is removed, so a copy another launch wrote stays. A
/// failure is logged (stdout). Database I/O.
fn forget_created(store: &Connection, id: &InstallationId, source: InstallationSource) {
    if source != InstallationSource::Created {
        return;
    }
    let removed = store.execute(
        "DELETE FROM host_installation WHERE id = 1 AND uuid = ?1",
        params![id.as_str()],
    );
    if let Err(err) = removed {
        tracing::warn!(error = %err, "could not undo the store copy of an unwritten installation");
    }
}

/// Decide the installation ID against the store copy, and write the store copy when it is
/// missing or lacks its creation time, in one short immediate transaction: concurrent
/// launches over the same data directory are serialised there, so the first to commit names
/// the ID and a later one reads it. No file I/O happens inside the transaction, so the
/// journal writer sharing the database is never held up by it. The creation time is the
/// file's, else the store's, else now; the store is rewritten whenever it holds another one,
/// so both copies end with one creation time, and the file is to be written when it is
/// missing or lacks it. Database I/O.
fn settle_store(
    store: &mut Connection,
    in_file: Option<StoredCopy>,
) -> Result<Settled, InstallationError> {
    let tx = store
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| store_error(&err))?;
    let in_store = read_store(&tx)?;
    let ids = (
        in_file.as_ref().map(|c| c.id.clone()),
        in_store.as_ref().map(|c| c.id.clone()),
    );
    let (id, source) = decide(ids.0, ids.1)?;
    let file_created = in_file.and_then(|c| c.created);
    let store_created = in_store.and_then(|c| c.created);
    let write_file = file_created.is_none();
    let created = file_created
        .or(store_created.clone())
        .unwrap_or_else(now_rfc3339);
    if store_created.as_deref() != Some(created.as_str()) {
        write_store(&tx, &id, &created)?;
    }
    tx.commit().map_err(|err| store_error(&err))?;
    Ok(Settled {
        id,
        source,
        created,
        write_file,
    })
}

/// The current time, RFC 3339 UTC with milliseconds. Reads the clock.
fn now_rfc3339() -> String {
    filesquid_api::journal::format_rfc3339_ms(filesquid_api::journal::now_ms())
}

/// The installation ID and where it came from, given both copies: equal copies are stored,
/// a missing copy is restored from the other, two missing copies mint a new ID, and copies
/// that disagree are refused. Pure apart from the random source.
fn decide(
    in_file: Option<InstallationId>,
    in_store: Option<InstallationId>,
) -> Result<(InstallationId, InstallationSource), InstallationError> {
    Ok(match (in_file, in_store) {
        (Some(a), Some(b)) if a == b => (a, InstallationSource::Stored),
        (Some(_), Some(_)) => return Err(InstallationError::Mismatch),
        (Some(id), None) => (id, InstallationSource::RestoredStore),
        (None, Some(id)) => (id, InstallationSource::RestoredFile),
        (None, None) => (mint()?, InstallationSource::Created),
    })
}

/// A new UUID v4 from the operating system's random source.
fn mint() -> Result<InstallationId, InstallationError> {
    let uuid = filesquid_api::library::mint_uuid_v4().map_err(|_| InstallationError::Random)?;
    InstallationId::parse(&uuid).map_err(|_| InstallationError::Random)
}

/// The file copy, if present (file I/O).
fn read_file(path: &Path) -> Result<Option<StoredCopy>, InstallationError> {
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

/// The copy an installation file's text holds: schema 1 with its `created`, or an earlier
/// build's file with neither `schema` nor `created`. Any other schema, a schema 1 file
/// without `created`, an unversioned file with one, or a `created` that is not RFC 3339 UTC
/// is malformed. Pure.
fn parse_file(text: &str) -> Result<StoredCopy, InstallationError> {
    let malformed = || InstallationError::Malformed { copy: "file" };
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| malformed())?;
    let schema = value.get("schema").map(serde_json::Value::as_u64);
    let versioned = matches!(schema, Some(Some(FILE_SCHEMA)));
    let known = versioned || schema.is_none();
    if !known || versioned != value.get("created").is_some() {
        return Err(malformed());
    }
    let uuid = value
        .get("installation_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(malformed)?;
    let id = InstallationId::parse(uuid).map_err(|_| malformed())?;
    let created = match value.get("created") {
        None => None,
        Some(created) => Some(checked_time(created.as_str()).ok_or_else(malformed)?),
    };
    Ok(StoredCopy { id, created })
}

/// `text` when it is an RFC 3339 UTC time. Pure.
fn checked_time(text: Option<&str>) -> Option<String> {
    text.filter(|t| t.ends_with('Z'))
        .filter(|t| filesquid_api::journal::parse_rfc3339_ms(t).is_some())
        .map(str::to_string)
}

/// A staging-file extension unique to this process and call, so concurrent launches never
/// share (and rename or remove) one another's staging file. Pure apart from a counter.
fn staging_extension() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("json.{}-{n}.tmp", std::process::id())
}

/// Write the file copy owner-only, through a temporary file renamed into
/// place (file I/O).
fn write_file(path: &Path, id: &InstallationId, created: &str) -> Result<(), InstallationError> {
    let io = |err: std::io::Error| InstallationError::Io {
        copy: "file",
        kind: err.kind(),
    };
    let staging = path.with_extension(staging_extension());
    if let Err(err) = std::fs::remove_file(&staging) {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(io(err));
        }
    }
    let text = file_text(id, created);
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

/// The schema 1 text of the installation file. Pure.
fn file_text(id: &InstallationId, created: &str) -> String {
    let body = serde_json::json!({
        "schema": FILE_SCHEMA,
        "installation_id": id.as_str(),
        "created": created,
    });
    format!("{body}\n")
}

/// Open the operational store and ensure its installation table, migrating a
/// table an earlier build created forward (database I/O).
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
    migrate_store(&conn)?;
    Ok(conn)
}

/// Add the `created` column to an installation table an earlier build created without it.
/// Idempotent, and safe when concurrent launches migrate at once: an `ALTER` that fails
/// because another launch added the column first is accepted. The old row reads it as absent.
/// Database I/O.
fn migrate_store(conn: &Connection) -> Result<(), InstallationError> {
    if has_created_column(conn)? {
        return Ok(());
    }
    let altered = conn.execute_batch("ALTER TABLE host_installation ADD COLUMN created TEXT");
    match altered {
        Ok(()) => tracing::info!("migrated the installation store copy forward"),
        Err(err) if has_created_column(conn)? => {
            tracing::debug!(error = %err, "another launch migrated the installation store copy");
        }
        Err(err) => return Err(store_error(&err)),
    }
    Ok(())
}

/// Whether the installation table has its `created` column. Database I/O.
fn has_created_column(conn: &Connection) -> Result<bool, InstallationError> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info('host_installation')
         WHERE name = 'created')",
        [],
        |r| r.get(0),
    )
    .map_err(|err| store_error(&err))
}

/// The store copy, if present; a creation time that is not RFC 3339 UTC reads as absent
/// and is written again (database I/O).
fn read_store(conn: &Connection) -> Result<Option<StoredCopy>, InstallationError> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT uuid, created FROM host_installation WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|err| store_error(&err))?;
    row.map(|(uuid, created)| {
        let id = InstallationId::parse(&uuid)
            .map_err(|_| InstallationError::Malformed { copy: "store" })?;
        Ok(StoredCopy {
            id,
            created: checked_time(created.as_deref()),
        })
    })
    .transpose()
}

/// Write the store copy (database I/O).
fn write_store(
    conn: &Connection,
    id: &InstallationId,
    created: &str,
) -> Result<(), InstallationError> {
    conn.execute(
        "INSERT OR REPLACE INTO host_installation (id, uuid, created) VALUES (1, ?1, ?2)",
        params![id.as_str(), created],
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

#[cfg(test)]
mod tests {
    //! The rollback of a created store copy whose file copy could not be written.

    use super::*;

    /// A store in a fresh temporary directory named after `name`.
    fn store(name: &str) -> (Connection, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("fs-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        (open_store(&dir.join(JOURNAL_FILE)).expect("store"), dir)
    }

    /// The UUID of the store copy, if any.
    fn stored_id(conn: &Connection) -> Option<InstallationId> {
        read_store(conn).expect("read").map(|copy| copy.id)
    }

    #[test]
    fn a_store_table_of_an_earlier_build_gains_the_creation_time() {
        let (conn, dir) = store("migrate");
        drop(conn);
        let path = dir.join(JOURNAL_FILE);
        let old = Connection::open(&path).expect("open");
        old.execute_batch(
            "DROP TABLE host_installation;
             CREATE TABLE host_installation (id INTEGER PRIMARY KEY CHECK (id = 1),
                 uuid TEXT NOT NULL);
             INSERT INTO host_installation (id, uuid)
                 VALUES (1, '0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d');",
        )
        .expect("an earlier build's table");
        drop(old);
        let mut conn = open_store(&path).expect("migrated");
        let settled = settle_store(&mut conn, None).expect("settle");
        assert_eq!(settled.source, InstallationSource::RestoredFile);
        assert!(settled.write_file);
        let copy = read_store(&conn).expect("read").expect("a copy");
        assert_eq!(copy.created.as_deref(), Some(settled.created.as_str()));
        drop(conn);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn mixed_file_shapes_are_malformed() {
        let uuid = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
        let schema_without_created = format!(r#"{{"schema":1,"installation_id":"{uuid}"}}"#);
        let created_without_schema =
            format!(r#"{{"installation_id":"{uuid}","created":"2026-10-31T00:00:00.000Z"}}"#);
        for text in [schema_without_created, created_without_schema] {
            assert!(
                matches!(
                    parse_file(&text),
                    Err(InstallationError::Malformed { copy: "file" })
                ),
                "{text}"
            );
        }
    }

    #[test]
    fn copies_with_two_creation_times_settle_on_the_files() {
        let (mut conn, dir) = store("two-times");
        let id = InstallationId::parse("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d").expect("id");
        write_store(&conn, &id, "2026-01-01T00:00:00.000Z").expect("store copy");
        let file = StoredCopy {
            id: id.clone(),
            created: Some("2026-10-31T00:00:00.000Z".to_string()),
        };
        let settled = settle_store(&mut conn, Some(file)).expect("settle");
        assert_eq!(settled.source, InstallationSource::Stored);
        assert_eq!(settled.created, "2026-10-31T00:00:00.000Z");
        let stored = read_store(&conn).expect("read").expect("a copy");
        assert_eq!(stored.created.as_deref(), Some("2026-10-31T00:00:00.000Z"));
        drop(conn);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn a_file_with_a_creation_time_that_is_not_utc_is_malformed() {
        let text = r#"{"schema":1,"installation_id":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","created":"2026-10-31T01:00:00+01:00"}"#;
        assert!(matches!(
            parse_file(text),
            Err(InstallationError::Malformed { copy: "file" })
        ));
    }

    #[test]
    fn a_created_store_copy_is_forgotten_so_the_next_launch_creates_again() {
        let (mut conn, dir) = store("forget");
        let Settled { id, source, .. } = settle_store(&mut conn, None).expect("settle");
        assert_eq!(source, InstallationSource::Created);
        assert_eq!(stored_id(&conn), Some(id.clone()));
        forget_created(&conn, &id, source);
        assert_eq!(stored_id(&conn), None);
        let next = settle_store(&mut conn, None).expect("next launch").source;
        assert_eq!(next, InstallationSource::Created);
        drop(conn);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn a_restored_or_foreign_store_copy_is_kept() {
        let (mut conn, dir) = store("keep");
        let Settled { id, source, .. } = settle_store(&mut conn, None).expect("settle");
        forget_created(&conn, &id, InstallationSource::RestoredFile);
        assert_eq!(stored_id(&conn), Some(id.clone()));
        let other = mint().expect("uuid");
        forget_created(&conn, &other, source);
        assert_eq!(stored_id(&conn), Some(id), "another ID's row stays");
        drop(conn);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
