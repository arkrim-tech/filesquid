//! A source whose bytes change between the stage's hashing read and its copying read (Task
//! DE-1.9; OCFL-S-026): a [`MemoryBackend`] holding the file at [`SOURCE_KEY`], whose ranged
//! reads from the second pass on answer the bytes with the first one flipped, while its entity
//! tag and size stay the same, as a file rewritten in place within one timestamp tick would.

use filesquid_native::import::source::SOURCE_KEY;
use filesquid_ocfl::storage::{
    ByteRange, CompletedPart, ListPage, MaybeSend, MemoryBackend, ObjectMeta, Precondition,
    StorageBackend, StorageError, StorageKey, UploadId,
};
use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};

/// The changing source.
pub struct ChangingSource {
    inner: MemoryBackend,
    passes: AtomicU32,
    fails: bool,
}

impl ChangingSource {
    /// A source holding `bytes` at [`SOURCE_KEY`].
    pub async fn holding(bytes: &[u8]) -> Self {
        let inner = MemoryBackend::new();
        let key = StorageKey::new(SOURCE_KEY).expect("key");
        inner.put(&key, bytes).await.expect("put");
        Self {
            inner,
            passes: AtomicU32::new(0),
            fails: false,
        }
    }

    /// A source holding `bytes` whose reads fail from the second pass on, as a file that
    /// became unreadable while it was staged.
    pub async fn failing(bytes: &[u8]) -> Self {
        Self {
            fails: true,
            ..Self::holding(bytes).await
        }
    }

    /// How many reading passes began (each reads from offset 0).
    pub fn passes(&self) -> u32 {
        self.passes.load(Ordering::SeqCst)
    }

    /// The bytes of `range`, flipped from the second pass on.
    async fn read(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        if range.start() == 0 {
            self.passes.fetch_add(1, Ordering::SeqCst);
        }
        if self.fails && self.passes() > 1 {
            return Err(StorageError::Io {
                key: key.to_string(),
                kind: std::io::ErrorKind::PermissionDenied,
                message: "permission denied".into(),
            });
        }
        let mut bytes = self.inner.get_range(key, range).await?;
        if self.passes() > 1 {
            if let Some(first) = bytes.first_mut() {
                *first ^= 0xff;
            }
        }
        Ok(bytes)
    }
}

impl StorageBackend for ChangingSource {
    fn put(
        &self,
        key: &StorageKey,
        bytes: &[u8],
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        self.inner.put(key, bytes)
    }

    fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        self.inner.put_if(key, bytes, precondition)
    }

    fn get_range(
        &self,
        key: &StorageKey,
        range: ByteRange,
    ) -> impl Future<Output = Result<Vec<u8>, StorageError>> + MaybeSend {
        self.read(key, range)
    }

    fn head(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        self.inner.head(key)
    }

    fn exists(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<bool, StorageError>> + MaybeSend {
        self.inner.exists(key)
    }

    fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> impl Future<Output = Result<ListPage, StorageError>> + MaybeSend {
        self.inner.list(prefix, start_after, limit)
    }

    fn delete(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend {
        self.inner.delete(key)
    }

    fn create_multipart(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<UploadId, StorageError>> + MaybeSend {
        self.inner.create_multipart(key)
    }

    fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> impl Future<Output = Result<CompletedPart, StorageError>> + MaybeSend {
        self.inner.upload_part(upload, part_number, bytes)
    }

    fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        self.inner.complete_multipart(upload, parts, precondition)
    }

    fn abort_multipart(
        &self,
        upload: &UploadId,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend {
        self.inner.abort_multipart(upload)
    }
}
