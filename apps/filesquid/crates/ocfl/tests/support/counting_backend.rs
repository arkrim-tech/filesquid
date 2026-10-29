//! A storage backend wrapper for the streamed-stage tests (IO-1.7): it
//! counts every ranged read and remembers the largest one, so a test can
//! prove a file is read in bounded chunks and never whole, and it can
//! replace a file's bytes after a chosen number of reads, so a test can
//! change a source between the hashing read and the copying read.

#![allow(dead_code)]

use std::sync::Mutex;

use filesquid_ocfl::storage::{
    write_whole, ByteRange, CompletedPart, ListPage, ObjectMeta, Precondition, StorageBackend,
    StorageError, StorageKey, UploadId,
};

/// A planned change: once `after` reads have been served, `key` holds
/// `bytes`.
struct Swap {
    after: usize,
    key: StorageKey,
    bytes: Vec<u8>,
}

/// What the wrapper has seen.
#[derive(Default)]
struct Seen {
    reads: usize,
    largest: u64,
    swap: Option<Swap>,
}

/// Wraps `B`, counting `get_range` calls.
pub struct CountingBackend<B> {
    inner: B,
    seen: Mutex<Seen>,
}

impl<B: StorageBackend> CountingBackend<B> {
    /// Wrap `inner`.
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            seen: Mutex::new(Seen::default()),
        }
    }

    /// The wrapped backend.
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// The number of ranged reads served so far.
    pub fn reads(&self) -> usize {
        self.seen.lock().expect("lock").reads
    }

    /// The length of the largest ranged read asked for.
    pub fn largest_read(&self) -> u64 {
        self.seen.lock().expect("lock").largest
    }

    /// Forget the counts so far.
    pub fn reset_counts(&self) {
        let mut seen = self.seen.lock().expect("lock");
        seen.reads = 0;
        seen.largest = 0;
    }

    /// Replace `key` with `bytes` just before the read after the first
    /// `after` reads (counted from now).
    pub fn swap_after(&self, after: usize, key: &StorageKey, bytes: Vec<u8>) {
        let mut seen = self.seen.lock().expect("lock");
        let after = seen.reads + after;
        seen.swap = Some(Swap {
            after,
            key: key.clone(),
            bytes,
        });
    }

    /// Count one read of `len` bytes and take the planned change when due.
    fn count(&self, len: u64) -> Option<Swap> {
        let mut seen = self.seen.lock().expect("lock");
        seen.largest = seen.largest.max(len);
        let due = seen.swap.as_ref().is_some_and(|s| seen.reads >= s.after);
        seen.reads += 1;
        if due {
            return seen.swap.take();
        }
        None
    }
}

impl<B: StorageBackend> StorageBackend for CountingBackend<B> {
    fn content_addressed_tags(&self) -> bool {
        self.inner.content_addressed_tags()
    }

    async fn check_hierarchy(&self, prefix: &str) -> Result<(), StorageError> {
        self.inner.check_hierarchy(prefix).await
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.inner.put(key, bytes).await
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner.put_if(key, bytes, precondition).await
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        if let Some(swap) = self.count(range.len()) {
            write_whole(&self.inner, &swap.key, &swap.bytes).await?;
        }
        self.inner.get_range(key, range).await
    }

    async fn head(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        self.inner.head(key).await
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        self.inner.exists(key).await
    }

    async fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        self.inner.list(prefix, start_after, limit).await
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.inner.delete(key).await
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        self.inner.create_multipart(key).await
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.inner.upload_part(upload, part_number, bytes).await
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner
            .complete_multipart(upload, parts, precondition)
            .await
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.inner.abort_multipart(upload).await
    }
}
