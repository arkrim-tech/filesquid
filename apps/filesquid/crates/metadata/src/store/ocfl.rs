//! `ocfl` store: `metadata.md` through a `filesquid-ocfl` storage backend.
//!
//! The store runs over any [`StorageBackend`] the host supplies (local
//! filesystem, in-memory, later S3-compatible), so it reaches bytes only
//! through `filesquid-ocfl` ([ADR 025](../../../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! [`OcflStore`] is the [`MetadataStore`] over a backend. With the `ocfl-fs`
//! feature on a native target, [`load`] and [`save`] are the path-based form
//! used by the CLI: they apply the Git guard and run the same code over a
//! local filesystem backend.

use filesquid_ocfl::storage::{
    read_verified, write_whole, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::WriteError;

use crate::file_ref::FileRef;
use crate::markdown::{from_markdown, to_markdown};
use crate::record::Record;
use crate::store::{MetadataStore, StoreError, METADATA_MD};

/// Return the storage key of `metadata.md` under `object_prefix` (an object's
/// key prefix, with or without a trailing `/`; empty for the backend root).
///
/// # Errors
///
/// [`WriteError::InvalidPath`] when the prefix is absolute or could escape
/// the root.
pub fn metadata_key(object_prefix: &str) -> Result<StorageKey, WriteError> {
    if object_prefix.starts_with('/') {
        return Err(WriteError::InvalidPath);
    }
    let prefix = object_prefix.strip_suffix('/').unwrap_or(object_prefix);
    let raw = match prefix {
        "" => METADATA_MD.to_string(),
        _ => format!("{prefix}/{METADATA_MD}"),
    };
    StorageKey::new(&raw).map_err(|_| WriteError::InvalidPath)
}

/// Read the record stored at `key`. A missing `metadata.md`, including one
/// deleted during the read, is an empty record. The file is streamed in
/// bounded chunks through `read_verified`, so a concurrent
/// replace never yields a record mixed from two versions.
///
/// # Errors
///
/// Any backend failure other than a missing object, as a [`WriteError`].
pub async fn load_from<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
) -> Result<Record, WriteError> {
    match read_verified(backend, key, MAX_CHUNK_BYTES).await {
        Ok(bytes) => Ok(from_markdown(&String::from_utf8_lossy(&bytes))),
        Err(StorageError::NotFound { .. }) => Ok(Record::default()),
        Err(err) => Err(err.into()),
    }
}

/// Write `record` as `metadata.md` at `key`, replacing any earlier record.
/// Records larger than one put are written through multipart upload.
///
/// # Errors
///
/// Any backend failure, as a [`WriteError`].
pub async fn save_to<B: StorageBackend>(
    backend: &B,
    key: &StorageKey,
    record: &Record,
) -> Result<(), WriteError> {
    let bytes = to_markdown(record).into_bytes();
    write_whole(backend, key, &bytes).await?;
    Ok(())
}

/// Map a `filesquid-ocfl` failure onto the store's error type.
fn store_error(err: WriteError) -> StoreError {
    match err {
        WriteError::InvalidPath => StoreError::InvalidPath,
        WriteError::Git(_) => StoreError::Git,
        other => StoreError::Backend(Box::new(other)),
    }
}

/// Stores records as `metadata.md` through a `filesquid-ocfl` backend.
///
/// The record describes the whole OCFL object, so the [`FileRef`]'s object
/// identifier is the object's key prefix in the backend and its logical path
/// and version do not change the key.
#[derive(Debug)]
pub struct OcflStore<B: StorageBackend> {
    /// Backend the host supplies.
    backend: B,
}

impl<B: StorageBackend> OcflStore<B> {
    /// Create a store over `backend`.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: StorageBackend> MetadataStore for OcflStore<B> {
    async fn load(&self, file: &FileRef) -> Result<Record, StoreError> {
        let key = metadata_key(file.object_id()).map_err(store_error)?;
        load_from(&self.backend, &key).await.map_err(store_error)
    }

    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError> {
        let key = metadata_key(file.object_id()).map_err(store_error)?;
        save_to(&self.backend, &key, record)
            .await
            .map_err(store_error)
    }
}

#[cfg(all(feature = "ocfl-fs", not(target_arch = "wasm32")))]
pub use native::{load, save};

/// Path-based helpers over a local filesystem backend (native only).
#[cfg(all(feature = "ocfl-fs", not(target_arch = "wasm32")))]
mod native {
    use std::path::Path;

    use filesquid_ocfl::storage::{complete_immediately, FsBackend};
    use filesquid_ocfl::{reject_git_in_ocfl_root, WriteError};

    use super::{load_from, metadata_key, save_to};
    use crate::record::Record;

    /// Open a local filesystem backend on an object directory after the Git
    /// guard, with the backend's default staging directory, which is always
    /// outside the object directory (see `FsBackend::with_default_staging`).
    fn object_backend(object_root: &Path) -> Result<FsBackend, WriteError> {
        reject_git_in_ocfl_root(object_root)?;
        Ok(FsBackend::with_default_staging(object_root)?)
    }

    /// Read `metadata.md` from an object directory. Missing file is an empty record.
    ///
    /// # Errors
    ///
    /// [`WriteError::Git`] when the directory holds `.git`, or a backend failure.
    pub fn load(object_root: &Path) -> Result<Record, WriteError> {
        let key = metadata_key("")?;
        let backend = object_backend(object_root)?;
        complete_immediately(load_from(&backend, &key))?
    }

    /// Write `metadata.md` into an object directory (Git-guarded).
    ///
    /// # Errors
    ///
    /// [`WriteError::Git`] when the directory holds `.git`, or a backend failure.
    pub fn save(object_root: &Path, record: &Record) -> Result<(), WriteError> {
        let key = metadata_key("")?;
        let backend = object_backend(object_root)?;
        complete_immediately(save_to(&backend, &key, record))?
    }
}
