//! Path-based logical-file helpers for native callers (behind the `fs`
//! feature). They keep their original signatures for existing callers, but
//! every byte goes through [`FsBackend`], files are written and read in
//! bounded chunks, and a read returns one consistent version of the file.
//!
//! New code SHOULD take a [`StorageBackend`](crate::storage::StorageBackend)
//! chosen by its host instead of a filesystem path (ADR 025).

use std::path::Path;

use crate::error::{OcflError, WriteError};
use crate::storage::{
    complete_immediately, read_verified, write_whole, FsBackend, StorageKey, MAX_CHUNK_BYTES,
};

/// Return an error when `root` contains a `.git` directory.
///
/// This checks the filesystem directly; it does not go through a storage
/// backend.
///
/// # Errors
///
/// [`OcflError::GitDirectoryForbidden`] when `root/.git` exists.
pub fn reject_git_in_ocfl_root(root: &Path) -> Result<(), OcflError> {
    if root.join(".git").exists() {
        return Err(OcflError::GitDirectoryForbidden);
    }
    Ok(())
}

/// Write `bytes` to `logical_path` under `object_root` after the Git guard.
///
/// Inventory/`vN` publication remains later OCFL work. Callers use this to
/// persist a sidecar such as `metadata.md`. Files above
/// [`MAX_PUT_BYTES`](crate::storage::MAX_PUT_BYTES) are written part by part
/// through multipart upload.
///
/// # Errors
///
/// [`WriteError::InvalidPath`] (checked before any I/O), [`WriteError::Git`],
/// [`WriteError::Io`] or [`WriteError::Storage`].
pub fn write_logical_file(
    object_root: &Path,
    logical_path: &str,
    bytes: &[u8],
) -> Result<(), WriteError> {
    let (backend, key) = open_object(object_root, logical_path)?;
    complete_immediately(write_whole(&backend, &key, bytes))??;
    Ok(())
}

/// Read a logical file from `object_root` after the Git guard. The file is
/// streamed in chunks of at most [`MAX_CHUNK_BYTES`] and checked against its
/// SHA-256 tag, so a file replaced during the read is read again rather than
/// returned as a mix of two versions.
///
/// # Errors
///
/// [`WriteError::Io`] with `ErrorKind::NotFound` for a missing file, or as
/// [`write_logical_file`].
pub fn read_logical_file(object_root: &Path, logical_path: &str) -> Result<Vec<u8>, WriteError> {
    let (backend, key) = open_object(object_root, logical_path)?;
    Ok(complete_immediately(read_verified(
        &backend,
        &key,
        MAX_CHUNK_BYTES,
    ))??)
}

/// Validate the logical path first (no I/O), then apply the Git guard and
/// open a filesystem backend rooted at the object directory with its default
/// staging directory (see [`FsBackend::with_default_staging`]).
fn open_object(
    object_root: &Path,
    logical_path: &str,
) -> Result<(FsBackend, StorageKey), WriteError> {
    let key = logical_key(logical_path)?;
    reject_git_in_ocfl_root(object_root)?;
    Ok((FsBackend::with_default_staging(object_root)?, key))
}

/// Parse a relative logical path. `StorageKey` refuses empty, absolute, `.`
/// and `..` segments; a name that merely contains `..`, such as `foo..bar`,
/// is allowed.
fn logical_key(logical_path: &str) -> Result<StorageKey, WriteError> {
    if Path::new(logical_path).is_absolute() {
        return Err(WriteError::InvalidPath);
    }
    StorageKey::new(logical_path).map_err(|_| WriteError::InvalidPath)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_git_is_ok() {
        let root = PathBuf::from("/tmp/filesquid-ocfl-no-git-placeholder");
        assert_eq!(reject_git_in_ocfl_root(&root), Ok(()));
    }

    #[test]
    fn dots_inside_a_name_are_allowed() {
        assert_eq!(logical_key("foo..bar").expect("key").as_str(), "foo..bar");
        assert!(logical_key("a/../b").is_err());
        assert!(logical_key("/abs").is_err());
    }
}
