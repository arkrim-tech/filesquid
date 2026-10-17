//! In-memory storage backend for tests and examples. Portable: it builds on
//! every target, `wasm32` included. It is not a canonical store.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard};

use super::backend::StorageBackend;
use super::error::StorageError;
use super::key::StorageKey;
use super::types::{
    check_list_request, check_part, check_part_order, check_precondition, check_put_size, paginate,
    ByteRange, CompletedPart, ETag, ListPage, ObjectMeta, Precondition, UploadId, MAX_OBJECT_BYTES,
};

/// A [`StorageBackend`] that keeps every object in process memory.
///
/// Every operation takes one lock, so each call is atomic, and conditional
/// writes are true compare-and-swap operations.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    state: Mutex<MemoryState>,
}

/// Objects and open uploads behind the backend lock.
#[derive(Debug, Default)]
struct MemoryState {
    objects: BTreeMap<String, Vec<u8>>,
    uploads: HashMap<String, PendingUpload>,
    next_upload: u64,
}

/// An open multipart upload: its destination and the parts received so far.
#[derive(Debug)]
struct PendingUpload {
    key: StorageKey,
    parts: BTreeMap<u32, Vec<u8>>,
}

impl MemoryBackend {
    /// Create an empty backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the state lock, mapping a poisoned lock to a backend error.
    fn lock(&self) -> Result<MutexGuard<'_, MemoryState>, StorageError> {
        self.state.lock().map_err(|_| StorageError::Backend {
            reason: "in-memory backend lock is poisoned".to_string(),
        })
    }
}

/// Size and tag of `bytes`.
fn meta_of(bytes: &[u8]) -> ObjectMeta {
    ObjectMeta {
        size: bytes.len() as u64,
        etag: ETag::sha256_of(bytes),
    }
}

/// The not-found error for `key`.
fn not_found(key: &StorageKey) -> StorageError {
    StorageError::NotFound {
        key: key.as_str().to_string(),
    }
}

/// The unknown-upload error for `upload`.
fn unknown(upload: &UploadId) -> StorageError {
    StorageError::UnknownUpload {
        upload_id: upload.as_str().to_string(),
    }
}

/// Copy `range` out of `bytes`, clipped to its end.
fn slice_range(bytes: &[u8], range: ByteRange) -> Vec<u8> {
    let len = bytes.len() as u64;
    let start = range.start().min(len) as usize;
    let end = range.start().saturating_add(range.len()).min(len) as usize;
    bytes[start..end].to_vec()
}

/// Join the listed parts, checking each one's tag and the total size.
fn assemble(pending: &PendingUpload, parts: &[CompletedPart]) -> Result<Vec<u8>, StorageError> {
    let mut joined = Vec::new();
    for part in parts {
        let mismatch = StorageError::PartMismatch {
            part_number: part.part_number,
        };
        let bytes = pending
            .parts
            .get(&part.part_number)
            .ok_or(mismatch.clone())?;
        if ETag::sha256_of(bytes) != part.etag {
            return Err(mismatch);
        }
        if (joined.len() + bytes.len()) as u64 > MAX_OBJECT_BYTES {
            return Err(StorageError::LimitExceeded {
                what: "object size",
                limit: MAX_OBJECT_BYTES,
            });
        }
        joined.extend_from_slice(bytes);
    }
    Ok(joined)
}

impl StorageBackend for MemoryBackend {
    /// The SHA-256 of the stored bytes is the tag.
    fn content_addressed_tags(&self) -> bool {
        true
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let mut state = self.lock()?;
        state
            .objects
            .insert(key.as_str().to_string(), bytes.to_vec());
        Ok(meta_of(bytes))
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let mut state = self.lock()?;
        let current = state.objects.get(key.as_str()).map(|b| ETag::sha256_of(b));
        check_precondition(key, current.as_ref(), precondition)?;
        state
            .objects
            .insert(key.as_str().to_string(), bytes.to_vec());
        Ok(meta_of(bytes))
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        let state = self.lock()?;
        let bytes = state
            .objects
            .get(key.as_str())
            .ok_or_else(|| not_found(key))?;
        Ok(slice_range(bytes, range))
    }

    async fn head(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        let state = self.lock()?;
        let bytes = state
            .objects
            .get(key.as_str())
            .ok_or_else(|| not_found(key))?;
        Ok(meta_of(bytes))
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        Ok(self.lock()?.objects.contains_key(key.as_str()))
    }

    async fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        check_list_request(prefix, limit)?;
        let state = self.lock()?;
        paginate(
            state.objects.keys().map(String::as_str),
            prefix,
            start_after,
            limit,
        )
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.lock()?.objects.remove(key.as_str());
        Ok(())
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        let mut state = self.lock()?;
        state.next_upload += 1;
        let id = format!("mem-{}", state.next_upload);
        let pending = PendingUpload {
            key: key.clone(),
            parts: BTreeMap::new(),
        };
        state.uploads.insert(id.clone(), pending);
        Ok(UploadId::new(id))
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        check_part(part_number, bytes)?;
        let mut state = self.lock()?;
        let pending = state
            .uploads
            .get_mut(upload.as_str())
            .ok_or_else(|| unknown(upload))?;
        pending.parts.insert(part_number, bytes.to_vec());
        Ok(CompletedPart {
            part_number,
            etag: ETag::sha256_of(bytes),
        })
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        check_part_order(parts)?;
        let mut state = self.lock()?;
        let pending = state
            .uploads
            .get(upload.as_str())
            .ok_or_else(|| unknown(upload))?;
        let key = pending.key.clone();
        let joined = assemble(pending, parts)?;
        if let Some(condition) = precondition {
            let current = state.objects.get(key.as_str()).map(|b| ETag::sha256_of(b));
            check_precondition(&key, current.as_ref(), condition)?;
        }
        let meta = meta_of(&joined);
        state.objects.insert(key.as_str().to_string(), joined);
        state.uploads.remove(upload.as_str());
        Ok(meta)
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        let removed = self.lock()?.uploads.remove(upload.as_str());
        removed.map(|_| ()).ok_or_else(|| unknown(upload))
    }
}
