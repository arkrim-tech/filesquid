//! Copy one file between two storage backends for the snapshot copy (F-042).
//!
//! A file that fits one put is read as one consistent version ([`read_verified`]) and
//! written with `IfAbsent`. A larger file is streamed through a multipart upload in parts of
//! at most [`MAX_PART_BYTES`], and the read gets the same checks as [`read_verified`]: the
//! source's entity tag and size are read before and after, the bytes streamed must match the
//! size, and on a backend whose tags are content SHA-256 digests the streamed bytes must hash
//! to the tag. A source replaced during the copy aborts the upload, so a copy never publishes
//! mixed bytes. Every multipart upload that does not complete is aborted.

use filesquid_ocfl::storage::{
    read_verified, ChunkReader, MultipartWriter, ObjectMeta, Precondition, StorageBackend,
    StorageError, StorageKey, MAX_CHUNK_BYTES, MAX_PART_BYTES, MAX_PUT_BYTES,
};
use filesquid_ocfl::DigestAlgorithm;

/// Copy `key` from `source` to `dest`, never over an existing file, and answer the bytes
/// copied.
///
/// # Errors
///
/// [`StorageError::PreconditionFailed`] when `dest` already has `key`;
/// [`StorageError::Backend`] when the source changed during the copy; any backend error.
pub(super) async fn copy_file<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    key: &StorageKey,
) -> Result<u64, StorageError> {
    let before = source.head(key).await?;
    if before.size <= MAX_PUT_BYTES {
        let bytes = read_verified(source, key, MAX_CHUNK_BYTES).await?;
        dest.put_if(key, &bytes, &Precondition::IfAbsent).await?;
        return Ok(bytes.len() as u64);
    }
    copy_streamed(source, dest, key, &before).await?;
    Ok(before.size)
}

/// Stream a large file through a multipart upload, aborting the upload on any failure.
async fn copy_streamed<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    key: &StorageKey,
    before: &ObjectMeta,
) -> Result<(), StorageError> {
    let mut writer = MultipartWriter::start(dest, key.clone()).await?;
    let uploaded = upload_parts(source, key, &mut writer).await;
    let checked = match uploaded {
        Ok(streamed) => check_unchanged(source, key, before, &streamed).await,
        Err(err) => Err(err),
    };
    let finished = match checked {
        Ok(()) => writer
            .finish(Some(&Precondition::IfAbsent))
            .await
            .map(|_| ()),
        Err(err) => Err(err),
    };
    if let Err(err) = finished {
        if let Err(abort) = writer.abort().await {
            tracing::warn!(target: super::LOG_TARGET, error = %abort, "could not abort a copy upload");
        }
        return Err(err);
    }
    Ok(())
}

/// What was streamed: the byte count and the SHA-256 of the bytes.
struct Streamed {
    bytes: u64,
    sha256: String,
}

/// Read `key` chunk by chunk and upload parts of up to [`MAX_PART_BYTES`]: a part is sent
/// before one more chunk could take it over the bound. The reader stops at the end of the
/// object, and [`MultipartWriter::write_part`] refuses more parts than the backend bound, so
/// the loop is bounded.
async fn upload_parts<S: StorageBackend, D: StorageBackend>(
    source: &S,
    key: &StorageKey,
    writer: &mut MultipartWriter<'_, D>,
) -> Result<Streamed, StorageError> {
    let mut reader = ChunkReader::new(source, key.clone(), MAX_CHUNK_BYTES)?;
    let mut hasher = DigestAlgorithm::Sha256.hasher();
    let mut bytes = 0u64;
    let mut part = Vec::new();
    while let Some(chunk) = reader.next_chunk().await? {
        hasher.update(&chunk);
        bytes += chunk.len() as u64;
        part.extend_from_slice(&chunk);
        if part.len() as u64 + MAX_CHUNK_BYTES > MAX_PART_BYTES {
            writer.write_part(&part).await?;
            part.clear();
        }
    }
    if !part.is_empty() {
        writer.write_part(&part).await?;
    }
    Ok(Streamed {
        bytes,
        sha256: hasher.finalize_hex(),
    })
}

/// Refuse a streamed copy whose source changed: the tag or size moved, fewer or more bytes
/// were read than the size, or (on a content-addressed backend) the bytes do not hash to the
/// tag.
async fn check_unchanged<S: StorageBackend>(
    source: &S,
    key: &StorageKey,
    before: &ObjectMeta,
    streamed: &Streamed,
) -> Result<(), StorageError> {
    let after = source.head(key).await?;
    let same = after.etag == before.etag && after.size == before.size;
    let complete = streamed.bytes == before.size;
    let hashed = !source.content_addressed_tags() || streamed.sha256 == before.etag.as_str();
    if same && complete && hashed {
        return Ok(());
    }
    Err(StorageError::Backend {
        reason: format!("{key} changed while it was being copied"),
    })
}
