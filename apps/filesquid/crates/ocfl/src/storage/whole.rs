//! Whole-object helpers for small objects such as sidecars and inventories:
//! write a byte buffer of any size (single put or multipart) and read one
//! object back as a single consistent version.
//!
//! Large content SHOULD be streamed with [`ChunkReader`] and
//! [`MultipartWriter`] instead, since these helpers hold the whole object in
//! memory.

use super::backend::StorageBackend;
use super::error::StorageError;
use super::key::StorageKey;
use super::stream::{ChunkReader, MultipartWriter};
use super::types::{ETag, MAX_CHUNK_BYTES, MAX_PUT_BYTES};

/// Read attempts before an object that keeps changing is reported.
pub const READ_ATTEMPTS: usize = 3;

/// Store `bytes` at `key` with one put, or part by part through multipart
/// upload when larger than [`MAX_PUT_BYTES`]. A failed multipart upload is
/// aborted.
///
/// # Errors
///
/// Any backend error.
pub async fn write_whole<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    bytes: &[u8],
) -> Result<(), StorageError> {
    if bytes.len() as u64 <= MAX_PUT_BYTES {
        return backend.put(key, bytes).await.map(|_| ());
    }
    let mut writer = MultipartWriter::start(backend, key.clone()).await?;
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

/// Read the object at `key` as one consistent version, in chunks of `chunk`
/// bytes.
///
/// Each chunk is a separate ranged read, so a writer that replaces the object
/// between chunks could mix two versions. The bytes read are therefore
/// checked against the entity tag taken before reading, and the read is
/// retried up to [`READ_ATTEMPTS`] times when they differ. The check needs
/// entity tags that are the SHA-256 of the stored bytes, as on the shipped
/// in-memory and filesystem backends.
///
/// # Errors
///
/// [`StorageError::NotFound`] when the object is missing (also when it is
/// deleted during the read), another backend error, or
/// [`StorageError::Backend`] when it changed during every attempt.
pub async fn read_verified<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    chunk: u64,
) -> Result<Vec<u8>, StorageError> {
    for _ in 0..READ_ATTEMPTS {
        let expected = backend.head(key).await?;
        let bytes = read_once(backend, key, chunk, expected.size).await?;
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
            // tag check in `read_verified`, which reads again.
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
        complete_immediately, ByteRange, CompletedPart, ListPage, MemoryBackend, ObjectMeta,
        Precondition, UploadId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        complete_immediately(read_verified(&backend, &key, 3)).expect("ready")
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
    fn an_object_that_keeps_changing_is_reported() {
        let err = read_with_swaps(1_000).expect_err("never stable");
        assert!(matches!(err, StorageError::Backend { .. }), "{err:?}");
    }

    #[test]
    fn a_stable_object_reads_in_one_pass() {
        assert_eq!(read_with_swaps(0).expect("read"), b"AAAAAAAA");
    }

    #[test]
    fn a_missing_object_is_not_found() {
        let backend = MemoryBackend::new();
        let key = StorageKey::new("missing").expect("key");
        let result = complete_immediately(read_verified(&backend, &key, 4)).expect("ready");
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    #[test]
    fn large_buffers_are_written_through_multipart() {
        let backend = MemoryBackend::new();
        let key = StorageKey::new("big").expect("key");
        let data = vec![7u8; MAX_PUT_BYTES as usize + 3];
        complete_immediately(write_whole(&backend, &key, &data))
            .expect("ready")
            .expect("write");
        let back = complete_immediately(read_verified(&backend, &key, MAX_CHUNK_BYTES))
            .expect("ready")
            .expect("read");
        assert_eq!(back, data);
    }
}
