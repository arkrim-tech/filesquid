//! Keys inside a root, and bounded whole reads of the small OCFL files a
//! reader needs (declarations, inventories, sidecars).

use super::error::ReadError;
use crate::inventory::FormatError;
use crate::storage::{
    read_verified, validate_prefix, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES,
};

/// A root directory as a key prefix without a trailing `/` (empty for the
/// backend's own root).
///
/// # Errors
///
/// [`ReadError::Storage`] with [`StorageError::InvalidKey`] when `root`
/// could escape the backend root.
pub(crate) fn root_dir(root: &str) -> Result<String, ReadError> {
    validate_prefix(root)?;
    Ok(root.strip_suffix('/').unwrap_or(root).to_string())
}

/// The relative path `rel` inside the directory `dir` (empty for the root).
pub(crate) fn child(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        return rel.to_string();
    }
    format!("{dir}/{rel}")
}

/// The storage key of `rel` inside `dir`.
///
/// # Errors
///
/// [`ReadError::Storage`] with [`StorageError::InvalidKey`] for a path no
/// key can name.
pub(crate) fn key_in(dir: &str, rel: &str) -> Result<StorageKey, ReadError> {
    Ok(StorageKey::new(&child(dir, rel))?)
}

/// Read the whole object at `key`, refusing it before any byte is read when
/// it is larger than `limit` (an engine bound, no OCFL code). The read is one
/// consistent version of the object ([`read_verified`]).
///
/// # Errors
///
/// [`ReadError::Format`] with [`FormatError::TooLarge`] above `limit`,
/// [`ReadError::Storage`] with [`StorageError::NotFound`] when it is
/// missing, or another backend error.
pub(crate) async fn read_small<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    limit: usize,
    what: &'static str,
) -> Result<Vec<u8>, ReadError> {
    let size = backend.head(key).await?.size;
    check_size(size, limit, what)?;
    let bytes = read_verified(backend, key, MAX_CHUNK_BYTES).await?;
    // The object may have grown between the size check and the read.
    check_size(bytes.len() as u64, limit, what)?;
    Ok(bytes)
}

/// Refuse a size above `limit`.
fn check_size(size: u64, limit: usize, what: &'static str) -> Result<(), ReadError> {
    if size <= limit as u64 {
        return Ok(());
    }
    Err(FormatError::TooLarge {
        what,
        size: usize::try_from(size).unwrap_or(usize::MAX),
        limit,
    }
    .into())
}

/// Turn a missing object into the OCFL rule its absence breaks; pass every
/// other error through.
pub(crate) fn missing_as(err: ReadError, code: &'static str, at: &str, reason: &str) -> ReadError {
    match err {
        ReadError::Storage(StorageError::NotFound { .. }) => ReadError::invalid(code, at, reason),
        other => other,
    }
}

/// Locate a refusal of the file at `key` by its full key, so the error names
/// the object as well as the file. Engine bounds pass through unchanged.
pub(crate) fn at_key(err: FormatError, key: &StorageKey) -> ReadError {
    match err {
        FormatError::Invalid { code, reason, .. } => ReadError::invalid(code, key.as_str(), reason),
        other => other.into(),
    }
}
