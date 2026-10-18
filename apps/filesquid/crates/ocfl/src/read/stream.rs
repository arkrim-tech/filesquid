//! Streaming and copying one logical file in bounded chunks, checked
//! against its inventory digest (OCFL-S-004, OCFL-S-010 (a); ADR 023
//! Decision 4: no whole object in memory).

use std::fmt;

use super::error::ReadError;
use super::state::LogicalFile;
use crate::digest::{DigestAlgorithm, DigestHasher};
use crate::storage::{
    ChunkReader, MultipartWriter, ObjectMeta, Precondition, StorageBackend, StorageError,
    StorageKey, MAX_CHUNK_BYTES, MAX_OBJECT_BYTES, MAX_PART_BYTES,
};

/// Reads one logical file chunk by chunk and checks the bytes against the
/// inventory digest when the stream ends.
///
/// A chunk is handed out before the whole file has been checked: callers
/// MUST treat the bytes as unverified until [`next_chunk`](Self::next_chunk)
/// returns `Ok(None)`. [`Object::copy_file`](super::Object::copy_file) does
/// this for a copy, and never publishes one that fails.
pub struct LogicalFileReader<'a, B: StorageBackend> {
    reader: ChunkReader<'a, B>,
    hasher: Option<DigestHasher>,
    algorithm: DigestAlgorithm,
    digest: String,
    content_key: String,
    failed: Option<ReadError>,
}

impl<'a, B: StorageBackend> LogicalFileReader<'a, B> {
    /// Prepare to read `file`, stored at `key`, in chunks of `chunk_bytes`
    /// (1 to [`MAX_CHUNK_BYTES`]). No I/O happens yet.
    ///
    /// # Errors
    ///
    /// [`ReadError::Storage`] for a chunk size outside the bound.
    pub(crate) fn new(
        backend: &'a B,
        key: StorageKey,
        file: &LogicalFile,
        algorithm: DigestAlgorithm,
        chunk_bytes: u64,
    ) -> Result<Self, ReadError> {
        Ok(Self {
            hasher: Some(algorithm.hasher()),
            algorithm,
            digest: file.digest.clone(),
            content_key: key.as_str().to_string(),
            failed: None,
            reader: ChunkReader::new(backend, key, chunk_bytes)?,
        })
    }

    /// Read the next chunk, or `None` once the whole file has been read and
    /// its digest matched. Every call advances the stream or ends it, so a
    /// caller's loop over this ends after at most `size / chunk + 2` calls.
    ///
    /// # Errors
    ///
    /// [`ReadError::Format`] with E092 when the content file is missing or
    /// its bytes do not match the inventory digest; a backend error. Once a
    /// call fails, every later call returns the same error, so a failed
    /// stream can never end as if it had been verified.
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ReadError> {
        if let Some(err) = &self.failed {
            return Err(err.clone());
        }
        let result = self.advance().await;
        if let Err(err) = &result {
            self.failed = Some(err.clone());
        }
        result
    }

    /// Read and hash the next chunk, or check the digest at the end.
    async fn advance(&mut self) -> Result<Option<Vec<u8>>, ReadError> {
        let Some(hasher) = self.hasher.as_mut() else {
            return Ok(None);
        };
        match self.reader.next_chunk().await {
            Ok(Some(bytes)) => {
                hasher.update(&bytes);
                Ok(Some(bytes))
            }
            Ok(None) => self.finish().map(|()| None),
            Err(StorageError::NotFound { .. }) => Err(self.content_fault("is missing")),
            Err(err) => Err(err.into()),
        }
    }

    /// Check the digest of everything read against the inventory.
    fn finish(&mut self) -> Result<(), ReadError> {
        let Some(hasher) = self.hasher.take() else {
            return Ok(());
        };
        let actual = hasher.finalize_hex();
        if self.algorithm.matches(&actual, &self.digest) {
            return Ok(());
        }
        Err(self.content_fault("does not match its inventory digest"))
    }

    /// The E092 refusal for this file's content key.
    fn content_fault(&self, what: &str) -> ReadError {
        ReadError::invalid(
            "E092",
            self.content_key.clone(),
            format!("the content file {what}"),
        )
    }
}

impl<B: StorageBackend> fmt::Debug for LogicalFileReader<'_, B> {
    /// Show what is read; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogicalFileReader")
            .field("reader", &self.reader)
            .field("content_key", &self.content_key)
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

/// Stream `source` into a multipart upload at `dest_key` on `dest`, and
/// publish it only when the whole file matched its digest. On any failure
/// the upload is aborted, so nothing is published at `dest_key`.
///
/// # Errors
///
/// As [`LogicalFileReader::next_chunk`], or a destination backend error
/// (such as [`StorageError::PreconditionFailed`]).
pub(crate) async fn copy_to<B: StorageBackend, D: StorageBackend>(
    mut source: LogicalFileReader<'_, B>,
    dest: &D,
    dest_key: StorageKey,
    precondition: Option<&Precondition>,
) -> Result<ObjectMeta, ReadError> {
    let mut writer = MultipartWriter::start(dest, dest_key).await?;
    let copied = upload_parts(&mut source, &mut writer).await;
    let published = match copied {
        Ok(()) => writer.finish(precondition).await.map_err(ReadError::from),
        Err(err) => Err(err),
    };
    if published.is_err() {
        if let Err(abort) = writer.abort().await {
            tracing::warn!(error = %abort, "could not abort the copy's multipart upload");
        }
    }
    published
}

/// Most chunks one copy reads: enough for the largest object a backend
/// stores, read in the smallest chunks a copy uses.
const MAX_COPY_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Read every chunk of `source` (at most [`MAX_CHUNK_BYTES`] each) into
/// parts of at most [`MAX_PART_BYTES`]. Always uploads at least one part, so
/// an empty file copies too.
async fn upload_parts<B: StorageBackend, D: StorageBackend>(
    source: &mut LogicalFileReader<'_, B>,
    writer: &mut MultipartWriter<'_, D>,
) -> Result<(), ReadError> {
    let mut part = Vec::new();
    let mut parts = 0usize;
    for _ in 0..MAX_COPY_CHUNKS {
        let Some(chunk) = source.next_chunk().await? else {
            if !part.is_empty() || parts == 0 {
                writer.write_part(&part).await?;
            }
            return Ok(());
        };
        part.extend_from_slice(&chunk);
        // Flush while one more chunk could still overflow the part bound.
        if part.len() as u64 + MAX_CHUNK_BYTES > MAX_PART_BYTES {
            writer.write_part(&part).await?;
            part.clear();
            parts += 1;
        }
    }
    Err(StorageError::LimitExceeded {
        what: "chunks read by one copy",
        limit: MAX_COPY_CHUNKS,
    }
    .into())
}
