//! Path-based logical-file helpers for native callers (behind the `fs`
//! feature). They keep their original signatures for existing callers such as
//! `filesquid-metadata`, but every byte now goes through [`FsBackend`], and
//! files are written and read in bounded chunks.
//!
//! New code SHOULD take a [`StorageBackend`] chosen by its host instead of a
//! filesystem path (ADR 025).

use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use crate::error::{OcflError, WriteError};
use crate::storage::{
    ChunkReader, FsBackend, MultipartWriter, StorageBackend, StorageError, StorageKey,
    MAX_CHUNK_BYTES, MAX_PUT_BYTES,
};

/// Return an error when `root` contains a `.git` directory.
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
/// [`MAX_PUT_BYTES`] are written part by part through multipart upload.
///
/// # Errors
///
/// [`WriteError::Git`], [`WriteError::InvalidPath`], [`WriteError::Io`] or
/// [`WriteError::Storage`].
pub fn write_logical_file(
    object_root: &Path,
    logical_path: &str,
    bytes: &[u8],
) -> Result<(), WriteError> {
    let (backend, key) = open_object(object_root, logical_path)?;
    drive(write_bytes(&backend, key, bytes))??;
    Ok(())
}

/// Read a logical file from `object_root` after the Git guard. The file is
/// streamed in chunks of at most [`MAX_CHUNK_BYTES`].
///
/// # Errors
///
/// [`WriteError::Io`] with `ErrorKind::NotFound` for a missing file, or as
/// [`write_logical_file`].
pub fn read_logical_file(object_root: &Path, logical_path: &str) -> Result<Vec<u8>, WriteError> {
    let (backend, key) = open_object(object_root, logical_path)?;
    Ok(drive(read_bytes(&backend, key))??)
}

/// Apply the Git guard and path checks, then open a filesystem backend rooted
/// at the object directory with multipart staging in the system temporary
/// directory (outside the object).
fn open_object(
    object_root: &Path,
    logical_path: &str,
) -> Result<(FsBackend, StorageKey), WriteError> {
    reject_git_in_ocfl_root(object_root)?;
    if logical_path.contains("..") || Path::new(logical_path).is_absolute() {
        return Err(WriteError::InvalidPath);
    }
    let key = StorageKey::new(logical_path).map_err(|_| WriteError::InvalidPath)?;
    let staging = std::env::temp_dir().join("filesquid-ocfl-staging");
    Ok((FsBackend::new(object_root, staging)?, key))
}

/// Store `bytes` with one put, or part by part when above [`MAX_PUT_BYTES`].
async fn write_bytes<B: StorageBackend>(
    backend: &B,
    key: StorageKey,
    bytes: &[u8],
) -> Result<(), StorageError> {
    if bytes.len() as u64 <= MAX_PUT_BYTES {
        return backend.put(&key, bytes).await.map(|_| ());
    }
    let mut writer = MultipartWriter::start(backend, key).await?;
    let written = write_parts(&mut writer, bytes).await;
    let finished = match written {
        Ok(()) => writer.finish(None).await.map(|_| ()),
        Err(err) => Err(err),
    };
    if let Err(err) = finished {
        if let Err(abort_err) = writer.abort().await {
            tracing::warn!(error = %abort_err, "could not abort multipart upload");
        }
        return Err(err);
    }
    Ok(())
}

/// Upload `bytes` as parts of at most [`MAX_CHUNK_BYTES`].
async fn write_parts<B: StorageBackend>(
    writer: &mut MultipartWriter<'_, B>,
    bytes: &[u8],
) -> Result<(), StorageError> {
    for part in bytes.chunks(MAX_CHUNK_BYTES as usize) {
        writer.write_part(part).await?;
    }
    Ok(())
}

/// Stream the object at `key` into one buffer, chunk by chunk.
async fn read_bytes<B: StorageBackend>(
    backend: &B,
    key: StorageKey,
) -> Result<Vec<u8>, StorageError> {
    let mut reader = ChunkReader::new(backend, key, MAX_CHUNK_BYTES)?;
    let mut out = Vec::new();
    while let Some(chunk) = reader.next_chunk().await? {
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Drive a filesystem-backend future to completion without an executor.
///
/// [`FsBackend`] futures do their blocking I/O on the first poll and never
/// return `Pending`, so one poll with a no-op waker completes them. A future
/// that does return `Pending` is reported as an error instead of being lost.
fn drive<F: Future>(future: F) -> Result<F::Output, WriteError> {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(WriteError::Storage(StorageError::Backend {
            reason: "the filesystem backend did not complete synchronously".to_string(),
        })),
    }
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
    fn pending_futures_are_reported_not_dropped_silently() {
        let result = drive(std::future::pending::<()>());
        assert!(matches!(result, Err(WriteError::Storage(_))));
    }
}
