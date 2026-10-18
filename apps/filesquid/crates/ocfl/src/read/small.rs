//! Keys inside a root, and bounded whole reads of the small OCFL files a
//! reader needs (declarations, inventories, sidecars).

use super::error::ReadError;
use crate::inventory::FormatError;
use crate::storage::{
    is_consistent, read_once, validate_prefix, StorageBackend, StorageError, StorageKey,
    MAX_CHUNK_BYTES, READ_ATTEMPTS,
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

/// Read the whole object at `key` as one consistent version, never holding
/// more than `limit` bytes (an engine bound, no OCFL code).
///
/// Each attempt reads the size first and refuses an object above `limit`
/// before any byte is read; it then reads at most that many bytes and
/// accepts them only when a second `head` shows the object unchanged (the
/// [`read_verified`](crate::storage::read_verified) rules). A writer that
/// replaces the object with a larger one between the two steps therefore
/// costs one bounded read and a retry, never a large buffer. At most
/// [`READ_ATTEMPTS`] attempts are made.
///
/// # Errors
///
/// [`ReadError::Format`] with [`FormatError::TooLarge`] above `limit`,
/// [`ReadError::Storage`] with [`StorageError::NotFound`] when it is
/// missing, [`StorageError::Backend`] when it changed during every attempt,
/// or another backend error.
pub(crate) async fn read_small<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    limit: usize,
    what: &'static str,
) -> Result<Vec<u8>, ReadError> {
    for _ in 0..READ_ATTEMPTS {
        let before = backend.head(key).await?;
        check_size(before.size, limit, what)?;
        let bytes = read_once(backend, key, MAX_CHUNK_BYTES, before.size).await?;
        let after = backend.head(key).await?;
        if is_consistent(backend, &before, &after, &bytes) {
            return Ok(bytes);
        }
    }
    Err(StorageError::Backend {
        reason: format!("{key} changed during each of {READ_ATTEMPTS} read attempts"),
    }
    .into())
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
