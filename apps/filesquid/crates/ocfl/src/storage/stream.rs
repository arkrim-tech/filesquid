//! Bounded streaming on top of any [`StorageBackend`]: read an object in
//! chunks and write a large object part by part, so no caller needs a whole
//! large file in memory (ADR 025 Decision 2).

use std::fmt;

use super::backend::StorageBackend;
use super::error::StorageError;
use super::key::StorageKey;
use super::types::{
    ByteRange, CompletedPart, ObjectMeta, Precondition, UploadId, MAX_OBJECT_BYTES, MAX_PARTS,
};

/// Reads one object as a sequence of chunks of at most `chunk_bytes` each.
pub struct ChunkReader<'a, B: StorageBackend> {
    backend: &'a B,
    key: StorageKey,
    chunk: u64,
    limit: u64,
    offset: u64,
    done: bool,
}

impl<'a, B: StorageBackend> ChunkReader<'a, B> {
    /// Prepare to stream `key` in chunks of `chunk_bytes` (1 to
    /// [`MAX_CHUNK_BYTES`](super::MAX_CHUNK_BYTES)). No I/O happens yet.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidArgument`] for zero,
    /// [`StorageError::LimitExceeded`] above the chunk bound.
    pub fn new(backend: &'a B, key: StorageKey, chunk_bytes: u64) -> Result<Self, StorageError> {
        Self::with_limit(backend, key, chunk_bytes, MAX_OBJECT_BYTES)
    }

    /// Like [`new`](Self::new), but stop at `limit_bytes` (at most
    /// [`MAX_OBJECT_BYTES`]) instead of [`MAX_OBJECT_BYTES`]: an object of
    /// exactly `limit_bytes` streams normally, and any byte past it is an
    /// error. Callers that know an object's expected size pass it here.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new), and [`StorageError::InvalidArgument`] for a
    /// limit of zero or above [`MAX_OBJECT_BYTES`].
    pub fn with_limit(
        backend: &'a B,
        key: StorageKey,
        chunk_bytes: u64,
        limit_bytes: u64,
    ) -> Result<Self, StorageError> {
        ByteRange::new(0, chunk_bytes)?;
        if limit_bytes == 0 || limit_bytes > MAX_OBJECT_BYTES {
            return Err(StorageError::InvalidArgument {
                reason: format!("stream limit {limit_bytes} is outside 1..={MAX_OBJECT_BYTES}"),
            });
        }
        Ok(Self {
            backend,
            key,
            chunk: chunk_bytes,
            limit: limit_bytes,
            offset: 0,
            done: false,
        })
    }

    /// Read the next chunk; `None` once the whole object has been returned.
    ///
    /// The loop a caller runs over this is bounded: every call advances the
    /// offset or ends the stream, and the offset never passes the limit. No
    /// byte past the limit is ever returned.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] for a missing object, a backend error, or
    /// [`StorageError::LimitExceeded`] when the object holds data past the
    /// limit (an object of exactly the limit ends normally).
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, StorageError> {
        if self.done {
            return Ok(None);
        }
        if self.offset >= self.limit {
            return self.check_end().await;
        }
        let want = self.chunk.min(self.limit - self.offset);
        let range = ByteRange::new(self.offset, want)?;
        let bytes = self.backend.get_range(&self.key, range).await?;
        let read = bytes.len() as u64;
        if read > want {
            return Err(StorageError::Backend {
                reason: format!("backend returned {read} bytes for a {want} byte range"),
            });
        }
        self.offset += read;
        self.done = read < want;
        Ok((read > 0).then_some(bytes))
    }

    /// At the limit: end the stream if the object ends here, otherwise report
    /// the overflow without returning its bytes.
    async fn check_end(&mut self) -> Result<Option<Vec<u8>>, StorageError> {
        let probe = ByteRange::new(self.offset, 1)?;
        if self.backend.get_range(&self.key, probe).await?.is_empty() {
            self.done = true;
            return Ok(None);
        }
        Err(StorageError::LimitExceeded {
            what: "streamed object size",
            limit: self.limit,
        })
    }
}

/// Writes one object part by part through multipart upload.
///
/// Call [`finish`](Self::finish) or [`abort`](Self::abort) when done: an
/// async backend cannot clean up on drop, so a dropped writer leaves its
/// upload open until the backend expires it.
pub struct MultipartWriter<'a, B: StorageBackend> {
    backend: &'a B,
    upload: UploadId,
    parts: Vec<CompletedPart>,
}

impl<'a, B: StorageBackend> MultipartWriter<'a, B> {
    /// Open a multipart upload that will publish at `key`.
    ///
    /// # Errors
    ///
    /// A backend error.
    pub async fn start(backend: &'a B, key: StorageKey) -> Result<Self, StorageError> {
        let upload = backend.create_multipart(&key).await?;
        Ok(Self {
            backend,
            upload,
            parts: Vec::new(),
        })
    }

    /// The open upload's identifier.
    pub fn upload_id(&self) -> &UploadId {
        &self.upload
    }

    /// Upload `bytes` as the next part.
    ///
    /// # Errors
    ///
    /// [`StorageError::LimitExceeded`] past [`MAX_PARTS`] parts or above the
    /// part size bound, or a backend error.
    pub async fn write_part(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        let next = u32::try_from(self.parts.len() + 1).unwrap_or(u32::MAX);
        if next > MAX_PARTS {
            return Err(StorageError::LimitExceeded {
                what: "multipart part count",
                limit: u64::from(MAX_PARTS),
            });
        }
        let part = self.backend.upload_part(&self.upload, next, bytes).await?;
        self.parts.push(part);
        Ok(())
    }

    /// Publish the uploaded parts as one object, optionally only if
    /// `precondition` holds.
    ///
    /// # Errors
    ///
    /// As [`StorageBackend::complete_multipart`]. The upload stays open on
    /// failure; the caller decides whether to [`abort`](Self::abort) it.
    pub async fn finish(
        &self,
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.backend
            .complete_multipart(&self.upload, &self.parts, precondition)
            .await
    }

    /// Discard the upload and its parts.
    ///
    /// # Errors
    ///
    /// As [`StorageBackend::abort_multipart`].
    pub async fn abort(self) -> Result<(), StorageError> {
        self.backend.abort_multipart(&self.upload).await
    }
}

impl<B: StorageBackend> fmt::Debug for ChunkReader<'_, B> {
    /// Show the read position; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkReader")
            .field("key", &self.key)
            .field("chunk", &self.chunk)
            .field("offset", &self.offset)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl<B: StorageBackend> fmt::Debug for MultipartWriter<'_, B> {
    /// Show the upload and its parts; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartWriter")
            .field("upload", &self.upload)
            .field("parts", &self.parts)
            .finish_non_exhaustive()
    }
}
