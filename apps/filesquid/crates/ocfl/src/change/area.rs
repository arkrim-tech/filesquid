//! [`StagingArea`]: where staged changes live, on any backend the caller
//! chooses, under a caller-chosen prefix outside every OCFL root.
//!
//! Layout under the prefix `P` (format in [`super::index`]):
//!
//! - `P/index.json`: the staging index;
//! - `P/blobs/<sha256>`: staged bytes the object's `manifest` lacks, named
//!   by their digest, so the same bytes are staged once.
//!
//! Before its first write of each key, the staging area refuses a key below
//! an OCFL object root or storage root of any version (the copy guard of
//! OCFL-S-019), so staging is never inside the object and never an OCFL
//! extension. Reading or recovering an object never reads it.

use std::collections::BTreeSet;

use super::error::ChangeError;
use super::index::{StagingIndex, MAX_INDEX_BYTES};
use crate::digest::DigestAlgorithm;
use crate::read::copy_guard::refuse_copy_into_ocfl_root;
use crate::read::small::read_small;
use crate::read::ReadError;
use crate::storage::{
    write_whole, ChunkReader, MultipartWriter, StorageBackend, StorageError, StorageKey,
    MAX_CHUNK_BYTES, MAX_LIST_PAGE, MAX_OBJECT_BYTES, MAX_PART_BYTES, MAX_WALK_ENTRIES,
};

/// The staging index's name under the prefix.
const INDEX_FILE: &str = "index.json";

/// The directory of staged bytes under the prefix.
const BLOBS_DIR: &str = "blobs";

/// Most chunks one working file is read in.
const MAX_FILE_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Most listing pages one walk of a prefix reads.
pub(crate) const MAX_LIST_PAGES: usize = MAX_WALK_ENTRIES / MAX_LIST_PAGE + 1;

/// One object's staging area: a prefix on a caller backend.
#[derive(Debug)]
pub struct StagingArea<'s, S> {
    backend: &'s S,
    prefix: String,
}

impl<'s, S: StorageBackend> StagingArea<'s, S> {
    /// The staging area under `prefix` of `backend`. Use one prefix per
    /// object and writer; it MUST be outside the storage root, which the
    /// first write checks when `backend` can see that root.
    ///
    /// # Errors
    ///
    /// [`ChangeError::Storage`] when `prefix` is not a valid storage key.
    pub fn new(backend: &'s S, prefix: &str) -> Result<Self, ChangeError> {
        let prefix = prefix.trim_end_matches('/');
        StorageKey::new(prefix)?;
        Ok(Self {
            backend,
            prefix: prefix.to_string(),
        })
    }

    /// The prefix the area lives under.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The backend the area lives on.
    pub(crate) fn backend(&self) -> &'s S {
        self.backend
    }

    /// The key of the staging index.
    fn index_key(&self) -> Result<StorageKey, ChangeError> {
        Ok(StorageKey::new(&format!("{}/{INDEX_FILE}", self.prefix))?)
    }

    /// The key that holds the staged bytes of `digest`.
    pub(crate) fn blob_key(&self, digest: &str) -> Result<StorageKey, ChangeError> {
        Ok(StorageKey::new(&format!(
            "{}/{BLOBS_DIR}/{digest}",
            self.prefix
        ))?)
    }

    /// Read the staging index, or `None` when nothing is staged.
    ///
    /// # Errors
    ///
    /// [`ChangeError::StagingCorrupt`] for an index that cannot be read as
    /// one, or a backend error.
    pub(crate) async fn read_index(&self) -> Result<Option<StagingIndex>, ChangeError> {
        let key = self.index_key()?;
        let bytes = match read_small(self.backend, &key, MAX_INDEX_BYTES, "staging index").await {
            Ok(bytes) => bytes,
            Err(ReadError::Storage(StorageError::NotFound { .. })) => return Ok(None),
            Err(ReadError::Format(err)) => return Err(corrupt(&key, err.to_string())),
            Err(err) => return Err(err.into()),
        };
        StagingIndex::parse(&bytes)
            .map(Some)
            .map_err(|reason| corrupt(&key, reason))
    }

    /// Write the staging index.
    ///
    /// # Errors
    ///
    /// [`ChangeError::Read`] for an area inside an OCFL root, or a backend
    /// error.
    pub(crate) async fn write_index(&self, index: &StagingIndex) -> Result<(), ChangeError> {
        let key = self.index_key()?;
        self.guard(&key).await?;
        write_whole(self.backend, &key, &index.to_json()).await?;
        Ok(())
    }

    /// Stage `bytes` unless `known` (the object's manifest) or the area
    /// already holds them, and return their sha256 digest.
    ///
    /// # Errors
    ///
    /// [`ChangeError::Read`] for an area inside an OCFL root, or a backend
    /// error.
    pub(crate) async fn put_bytes(
        &self,
        bytes: &[u8],
        known: &BTreeSet<String>,
    ) -> Result<String, ChangeError> {
        let digest = DigestAlgorithm::ENGINE.digest_hex(bytes);
        if known.contains(&digest) {
            return Ok(digest);
        }
        let key = self.blob_key(&digest)?;
        self.guard(&key).await?;
        if !self.backend.exists(&key).await? {
            write_whole(self.backend, &key, bytes).await?;
        }
        Ok(digest)
    }

    /// Stage the working file at `key` of `source` unless `known` or the
    /// area already holds its bytes, and return their sha256 digest. The
    /// file is hashed first and copied only when its bytes are new; the copy
    /// is hashed again and refused when the file changed in between.
    ///
    /// # Errors
    ///
    /// [`ChangeError::SourceChanged`], [`ChangeError::Read`] for an area
    /// inside an OCFL root, or a backend error.
    pub(crate) async fn put_file<D: StorageBackend>(
        &self,
        source: &D,
        key: &StorageKey,
        known: &BTreeSet<String>,
    ) -> Result<String, ChangeError> {
        let digest = hash_file(source, key).await?;
        if known.contains(&digest) {
            return Ok(digest);
        }
        let blob = self.blob_key(&digest)?;
        self.guard(&blob).await?;
        if self.backend.exists(&blob).await? {
            return Ok(digest);
        }
        let copied = copy_file(source, key, self.backend, &blob).await?;
        if copied != digest {
            self.backend.delete(&blob).await?;
            return Err(ChangeError::SourceChanged {
                key: key.to_string(),
            });
        }
        Ok(digest)
    }

    /// Delete everything staged: the blobs, then the index.
    ///
    /// # Errors
    ///
    /// A backend error, or [`StorageError::LimitExceeded`] when the area
    /// holds more keys than one bounded walk reads.
    pub(crate) async fn clear(&self) -> Result<(), ChangeError> {
        let blobs = format!("{}/{BLOBS_DIR}/", self.prefix);
        for key in list_all(self.backend, &blobs).await? {
            self.backend.delete(&key).await?;
        }
        self.backend.delete(&self.index_key()?).await?;
        Ok(())
    }

    /// Refuse a key below an OCFL object root or storage root.
    async fn guard(&self, key: &StorageKey) -> Result<(), ChangeError> {
        refuse_copy_into_ocfl_root(self.backend, key).await?;
        Ok(())
    }
}

/// A damaged-index error for `key`.
fn corrupt(key: &StorageKey, reason: String) -> ChangeError {
    ChangeError::StagingCorrupt {
        key: key.to_string(),
        reason,
    }
}

/// Every key under `prefix` of `backend`, in order, in bounded pages.
///
/// # Errors
///
/// A backend error, or [`StorageError::LimitExceeded`] past
/// [`MAX_LIST_PAGES`] pages.
pub(crate) async fn list_all<B: StorageBackend>(
    backend: &B,
    prefix: &str,
) -> Result<Vec<StorageKey>, StorageError> {
    let mut keys = Vec::new();
    let mut after: Option<StorageKey> = None;
    for _ in 0..MAX_LIST_PAGES {
        let page = backend.list(prefix, after.as_ref(), MAX_LIST_PAGE).await?;
        keys.extend(page.keys);
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(keys),
        }
    }
    Err(StorageError::LimitExceeded {
        what: "keys listed under one prefix",
        limit: MAX_WALK_ENTRIES as u64,
    })
}

/// The sha256 digest of the file at `key`, read in bounded chunks.
///
/// # Errors
///
/// A backend error, or [`StorageError::LimitExceeded`] for a file above the
/// largest object a backend stores.
async fn hash_file<D: StorageBackend>(source: &D, key: &StorageKey) -> Result<String, ChangeError> {
    let mut reader = ChunkReader::new(source, key.clone(), MAX_CHUNK_BYTES)?;
    let mut hasher = DigestAlgorithm::ENGINE.hasher();
    for _ in 0..MAX_FILE_CHUNKS {
        match reader.next_chunk().await? {
            Some(bytes) => hasher.update(&bytes),
            None => return Ok(hasher.finalize_hex()),
        }
    }
    Err(too_large().into())
}

/// Copy the file at `key` of `source` to `dest` at `dest_key` through a
/// multipart upload, and return the sha256 digest of the bytes copied. A
/// failed upload is aborted.
///
/// # Errors
///
/// A backend error on either side.
async fn copy_file<D: StorageBackend, S: StorageBackend>(
    source: &D,
    key: &StorageKey,
    dest: &S,
    dest_key: &StorageKey,
) -> Result<String, ChangeError> {
    let mut writer = MultipartWriter::start(dest, dest_key.clone()).await?;
    let copied = upload_parts(source, key, &mut writer).await;
    let finished = match copied {
        Ok(digest) => writer.finish(None).await.map(|_| digest),
        Err(err) => Err(err),
    };
    if finished.is_err() {
        if let Err(abort) = writer.abort().await {
            tracing::warn!(error = %abort, "could not abort the staging upload");
        }
    }
    Ok(finished?)
}

/// Read `key` of `source` in chunks into parts of at most
/// [`MAX_PART_BYTES`], hashing as it goes. Always uploads at least one
/// part, so an empty file is staged too.
async fn upload_parts<D: StorageBackend, S: StorageBackend>(
    source: &D,
    key: &StorageKey,
    writer: &mut MultipartWriter<'_, S>,
) -> Result<String, StorageError> {
    let mut reader = ChunkReader::new(source, key.clone(), MAX_CHUNK_BYTES)?;
    let mut hasher = DigestAlgorithm::ENGINE.hasher();
    let mut part = Vec::new();
    let mut parts = 0usize;
    for _ in 0..MAX_FILE_CHUNKS {
        let Some(chunk) = reader.next_chunk().await? else {
            if !part.is_empty() || parts == 0 {
                writer.write_part(&part).await?;
            }
            return Ok(hasher.finalize_hex());
        };
        hasher.update(&chunk);
        part.extend_from_slice(&chunk);
        if part.len() as u64 + MAX_CHUNK_BYTES > MAX_PART_BYTES {
            writer.write_part(&part).await?;
            part.clear();
            parts += 1;
        }
    }
    Err(too_large())
}

/// The error for a working file above the largest object a backend stores.
fn too_large() -> StorageError {
    StorageError::LimitExceeded {
        what: "chunks read from one working file",
        limit: MAX_FILE_CHUNKS,
    }
}
