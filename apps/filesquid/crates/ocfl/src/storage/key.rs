//! Storage keys: the `/`-separated relative paths that name objects inside a
//! storage root. They are the OCFL relative paths on every backend (file
//! paths under a local root, object keys under an S3-compatible prefix).

use std::fmt;

use super::error::StorageError;

/// A validated storage key.
///
/// A key is a non-empty, `/`-separated relative path. It MUST NOT start with
/// `/`, contain an empty, `.` or `..` segment, a backslash or a NUL byte, so a
/// key can never name anything outside the storage root on any backend.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageKey(String);

impl StorageKey {
    /// Validate `raw` and wrap it as a key.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidKey`] naming the rule `raw` breaks.
    pub fn new(raw: &str) -> Result<Self, StorageError> {
        match key_violation(raw) {
            Some(reason) => Err(StorageError::InvalidKey {
                key: raw.to_string(),
                reason,
            }),
            None => Ok(Self(raw.to_string())),
        }
    }

    /// The key as a `/`-separated string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key's path segments, in order.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Return the first key rule `raw` breaks, or `None` when it is valid.
fn key_violation(raw: &str) -> Option<&'static str> {
    if raw.is_empty() {
        return Some("a key must not be empty");
    }
    if raw.starts_with('/') {
        return Some("a key must be relative");
    }
    if raw.contains('\\') {
        return Some("a key must use '/' as its only separator");
    }
    if raw.contains('\0') {
        return Some("a key must not contain NUL");
    }
    raw.split('/').find_map(segment_violation)
}

/// Return the rule a single path segment breaks, or `None`.
fn segment_violation(segment: &str) -> Option<&'static str> {
    match segment {
        "" => Some("a key must not contain an empty segment"),
        "." | ".." => Some("a key must not contain a '.' or '..' segment"),
        _ => None,
    }
}

/// Validate a listing prefix: empty (the whole root), or a valid key that MAY
/// end with one `/`.
///
/// # Errors
///
/// [`StorageError::InvalidKey`] when the prefix could escape the root.
pub(crate) fn validate_prefix(prefix: &str) -> Result<(), StorageError> {
    if prefix.is_empty() {
        return Ok(());
    }
    let trimmed = prefix.strip_suffix('/').unwrap_or(prefix);
    match key_violation(trimmed) {
        Some(reason) => Err(StorageError::InvalidKey {
            key: prefix.to_string(),
            reason,
        }),
        None => Ok(()),
    }
}
