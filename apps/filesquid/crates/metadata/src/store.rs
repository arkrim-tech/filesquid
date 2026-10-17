//! Persist `metadata.md` through a `filesquid-ocfl` storage backend.
//!
//! The store runs over any [`StorageBackend`] the host supplies (local
//! filesystem, in-memory, later S3-compatible), so it reaches bytes only
//! through `filesquid-ocfl` ([ADR 025](../../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! [`load`] and [`save`] are the native path-based form used by the CLI: they
//! apply the Git guard and run the same code over a local filesystem backend.

use std::path::Path;

use filesquid_ocfl::storage::{
    complete_immediately, ChunkReader, FsBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::{reject_git_in_ocfl_root, WriteError};

use crate::markdown::{from_markdown, to_markdown};
use crate::record::Record;

/// Canonical logical path inside an OCFL object.
pub const METADATA_MD: &str = "metadata.md";

/// Return the storage key of `metadata.md` under `object_prefix` (an object's
/// key prefix, with or without a trailing `/`; empty for the backend root).
///
/// # Errors
///
/// [`WriteError::InvalidPath`] when the prefix could escape the root.
pub fn metadata_key(object_prefix: &str) -> Result<StorageKey, WriteError> {
    let prefix = object_prefix.strip_suffix('/').unwrap_or(object_prefix);
    let raw = match prefix {
        "" => METADATA_MD.to_string(),
        _ => format!("{prefix}/{METADATA_MD}"),
    };
    StorageKey::new(&raw).map_err(|_| WriteError::InvalidPath)
}

/// Read the record stored at `key`. A missing `metadata.md` is an empty
/// record. The file is streamed in bounded chunks.
///
/// # Errors
///
/// Any backend failure other than a missing object, as a [`WriteError`].
pub async fn load_from<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
) -> Result<Record, WriteError> {
    if !backend.exists(key).await? {
        return Ok(Record::default());
    }
    let mut reader = ChunkReader::new(backend, key.clone(), MAX_CHUNK_BYTES)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = reader.next_chunk().await? {
        bytes.extend_from_slice(&chunk);
    }
    Ok(from_markdown(&String::from_utf8_lossy(&bytes)))
}

/// Write `record` as `metadata.md` at `key`, replacing any earlier record.
///
/// # Errors
///
/// Any backend failure, as a [`WriteError`].
pub async fn save_to<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    record: &Record,
) -> Result<(), WriteError> {
    let bytes = to_markdown(record).into_bytes();
    backend.put(key, &bytes).await?;
    Ok(())
}

/// Open a local filesystem backend on an object directory after the Git
/// guard, with multipart staging in the system temporary directory.
fn object_backend(object_root: &Path) -> Result<FsBackend, WriteError> {
    reject_git_in_ocfl_root(object_root)?;
    let staging = std::env::temp_dir().join("filesquid-ocfl-staging");
    Ok(FsBackend::new(object_root, staging)?)
}

/// Read `metadata.md` from an object directory. Missing file is an empty record.
///
/// # Errors
///
/// [`WriteError::Git`] when the directory holds `.git`, or a backend failure.
pub fn load(object_root: &Path) -> Result<Record, WriteError> {
    let backend = object_backend(object_root)?;
    let key = metadata_key("")?;
    complete_immediately(load_from(&backend, &key))?
}

/// Write `metadata.md` into an object directory (Git-guarded).
///
/// # Errors
///
/// [`WriteError::Git`] when the directory holds `.git`, or a backend failure.
pub fn save(object_root: &Path, record: &Record) -> Result<(), WriteError> {
    let backend = object_backend(object_root)?;
    let key = metadata_key("")?;
    complete_immediately(save_to(&backend, &key, record))?
}
