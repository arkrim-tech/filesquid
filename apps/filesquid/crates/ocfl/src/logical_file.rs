//! Path-based logical-file helpers for native callers (behind the `fs`
//! feature). They keep their original signatures for existing callers such as
//! `filesquid-metadata`, but every byte now goes through [`FsBackend`], and
//! files are written and read in bounded chunks.
//!
//! New code SHOULD take a [`StorageBackend`] chosen by its host instead of a
//! filesystem path (ADR 025).

use std::path::Path;

use crate::error::{OcflError, WriteError};
use crate::storage::{
    complete_immediately, ChunkReader, ETag, FsBackend, MultipartWriter, StorageBackend,
    StorageError, StorageKey, MAX_CHUNK_BYTES, MAX_PUT_BYTES,
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
    complete_immediately(write_bytes(&backend, key, bytes))??;
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
    Ok(complete_immediately(read_bytes(
        &backend,
        key,
        MAX_CHUNK_BYTES,
    ))??)
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

/// Read attempts before a file that keeps changing is reported.
const READ_ATTEMPTS: usize = 3;

/// Read the object at `key` as one consistent version.
///
/// Each chunk is a separate ranged read, so a writer that replaces the file
/// between chunks could mix two versions. The bytes read are therefore
/// checked against the entity tag taken before reading (the SHA-256 of the
/// stored bytes on the shipped backends), and the read is retried a bounded
/// number of times when they differ.
///
/// # Errors
///
/// A backend error, or [`StorageError::Backend`] when the file changed during
/// every attempt.
async fn read_bytes<B: StorageBackend>(
    backend: &B,
    key: StorageKey,
    chunk: u64,
) -> Result<Vec<u8>, StorageError> {
    for _ in 0..READ_ATTEMPTS {
        let expected = backend.head(&key).await?;
        let bytes = read_once(backend, &key, chunk, expected.size).await?;
        if ETag::sha256_of(&bytes) == expected.etag {
            return Ok(bytes);
        }
    }
    Err(StorageError::Backend {
        reason: format!("{key} changed during each of {READ_ATTEMPTS} read attempts"),
    })
}

/// Stream the object at `key` into one buffer in chunks of `chunk` bytes,
/// stopping at the `size` reported before the read.
async fn read_once<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    chunk: u64,
    size: u64,
) -> Result<Vec<u8>, StorageError> {
    let mut out = Vec::new();
    if size == 0 {
        return Ok(out);
    }
    let mut reader = ChunkReader::with_limit(backend, key.clone(), chunk, size)?;
    let max_reads = size / chunk + 2;
    for _ in 0..max_reads {
        match reader.next_chunk().await {
            Ok(Some(bytes)) => out.extend_from_slice(&bytes),
            // A file that grew past `size` is not lost: its bytes fail the
            // tag check in `read_bytes`, which reads again.
            Ok(None) | Err(StorageError::LimitExceeded { .. }) => return Ok(out),
            Err(err) => return Err(err),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        ByteRange, CompletedPart, ListPage, MemoryBackend, ObjectMeta, Precondition, UploadId,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn missing_git_is_ok() {
        let root = PathBuf::from("/tmp/filesquid-ocfl-no-git-placeholder");
        assert_eq!(reject_git_in_ocfl_root(&root), Ok(()));
    }

    /// A backend whose object is replaced by another writer after each of the
    /// first `swaps` ranged reads, alternating between two versions.
    struct Replaced {
        inner: MemoryBackend,
        key: StorageKey,
        versions: [&'static [u8]; 2],
        swaps: AtomicUsize,
    }

    impl Replaced {
        fn new(swaps: usize) -> Self {
            let key = StorageKey::new("f.txt").expect("key");
            let inner = MemoryBackend::new();
            let versions: [&'static [u8]; 2] = [b"AAAAAAAA", b"BBBBBBBB"];
            complete_immediately(inner.put(&key, versions[0]))
                .expect("ready")
                .expect("put");
            let swaps = AtomicUsize::new(swaps);
            Self {
                inner,
                key,
                versions,
                swaps,
            }
        }

        /// Simulate the other writer replacing the object.
        async fn swap(&self) -> Result<(), StorageError> {
            let left = self.swaps.load(Ordering::SeqCst);
            if left == 0 {
                return Ok(());
            }
            self.swaps.store(left - 1, Ordering::SeqCst);
            let next = self.versions[left % 2];
            self.inner.put(&self.key, next).await.map(|_| ())
        }
    }

    impl StorageBackend for Replaced {
        async fn put(&self, k: &StorageKey, b: &[u8]) -> Result<ObjectMeta, StorageError> {
            self.inner.put(k, b).await
        }
        async fn put_if(
            &self,
            k: &StorageKey,
            b: &[u8],
            p: &Precondition,
        ) -> Result<ObjectMeta, StorageError> {
            self.inner.put_if(k, b, p).await
        }
        async fn get_range(&self, k: &StorageKey, r: ByteRange) -> Result<Vec<u8>, StorageError> {
            let bytes = self.inner.get_range(k, r).await?;
            self.swap().await?;
            Ok(bytes)
        }
        async fn head(&self, k: &StorageKey) -> Result<ObjectMeta, StorageError> {
            self.inner.head(k).await
        }
        async fn exists(&self, k: &StorageKey) -> Result<bool, StorageError> {
            self.inner.exists(k).await
        }
        async fn list(
            &self,
            p: &str,
            a: Option<&StorageKey>,
            l: usize,
        ) -> Result<ListPage, StorageError> {
            self.inner.list(p, a, l).await
        }
        async fn delete(&self, k: &StorageKey) -> Result<(), StorageError> {
            self.inner.delete(k).await
        }
        async fn create_multipart(&self, k: &StorageKey) -> Result<UploadId, StorageError> {
            self.inner.create_multipart(k).await
        }
        async fn upload_part(
            &self,
            u: &UploadId,
            n: u32,
            b: &[u8],
        ) -> Result<CompletedPart, StorageError> {
            self.inner.upload_part(u, n, b).await
        }
        async fn complete_multipart(
            &self,
            u: &UploadId,
            p: &[CompletedPart],
            c: Option<&Precondition>,
        ) -> Result<ObjectMeta, StorageError> {
            self.inner.complete_multipart(u, p, c).await
        }
        async fn abort_multipart(&self, u: &UploadId) -> Result<(), StorageError> {
            self.inner.abort_multipart(u).await
        }
    }

    fn read_with_swaps(swaps: usize) -> Result<Vec<u8>, StorageError> {
        let backend = Replaced::new(swaps);
        let key = backend.key.clone();
        complete_immediately(read_bytes(&backend, key, 3)).expect("ready")
    }

    #[test]
    fn a_read_never_mixes_two_versions() {
        let bytes = read_with_swaps(1).expect("retried read");
        assert!(
            bytes == b"AAAAAAAA" || bytes == b"BBBBBBBB",
            "mixed read {:?}",
            String::from_utf8_lossy(&bytes)
        );
    }

    #[test]
    fn a_file_that_keeps_changing_is_reported() {
        let err = read_with_swaps(1_000).expect_err("never stable");
        assert!(matches!(err, StorageError::Backend { .. }), "{err:?}");
    }

    #[test]
    fn a_stable_file_reads_in_one_pass() {
        assert_eq!(read_with_swaps(0).expect("read"), b"AAAAAAAA");
    }
}
