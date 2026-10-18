//! A storage backend wrapper for the write-order tests (IO-0.6): it records
//! every write in a log shared with other wrappers, can "crash" the writer
//! at a chosen key, and can run a hook right after a chosen key is written.
//!
//! A crash is a write that never completes: the wrapper returns a future
//! that stays pending, so a writer driven with `complete_immediately` is
//! dropped mid-write like a killed process. No cleanup code runs, and the
//! writer's reservation stays held until it expires.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use filesquid_ocfl::storage::{
    ByteRange, CompletedPart, ListPage, ObjectMeta, Precondition, StorageBackend, StorageError,
    StorageKey, UploadId,
};

/// The write log, shared by every wrapper of one test.
pub type Log = Arc<Mutex<Vec<String>>>;

/// A hook run after a write of its key succeeded.
pub type Hook = Box<dyn FnOnce() + Send>;

/// Wraps `B`, logging writes as `"<label> <op> <key>"`.
pub struct FaultBackend<B> {
    inner: B,
    label: &'static str,
    log: Log,
    crash_at: Mutex<Option<String>>,
    hooks: Mutex<Vec<(String, Hook)>>,
    uploads: Mutex<HashMap<String, String>>,
}

impl<B: StorageBackend> FaultBackend<B> {
    /// Wrap `inner`, logging into `log` under `label`.
    pub fn new(inner: B, label: &'static str, log: Log) -> Self {
        Self {
            inner,
            label,
            log,
            crash_at: Mutex::new(None),
            hooks: Mutex::new(Vec::new()),
            uploads: Mutex::new(HashMap::new()),
        }
    }

    /// The wrapped backend.
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// Crash the next write of `key`: it never completes. One crash only.
    pub fn crash_at(&self, key: &str) {
        *self.crash_at.lock().expect("lock") = Some(key.to_string());
    }

    /// Run `hook` once, right after the next successful write of `key`.
    pub fn after_write(&self, key: &str, hook: Hook) {
        self.hooks
            .lock()
            .expect("lock")
            .push((key.to_string(), hook));
    }

    /// Whether the next write of `key` crashes; disarms the crash if so.
    fn crashes(&self, key: &str) -> bool {
        let mut armed = self.crash_at.lock().expect("lock");
        if armed.as_deref() == Some(key) {
            *armed = None;
            return true;
        }
        false
    }

    /// Log a write, crash when armed for `key`, and otherwise run `write`.
    async fn write<T>(
        &self,
        op: &str,
        key: &str,
        write: impl std::future::Future<Output = Result<T, StorageError>>,
    ) -> Result<T, StorageError> {
        if self.crashes(key) {
            self.record(&format!("crash {key}"));
            std::future::pending::<()>().await;
        }
        let result = write.await;
        if result.is_ok() {
            self.record(&format!("{op} {key}"));
            self.run_hook(key);
        }
        result
    }

    /// Append one entry to the shared log.
    fn record(&self, entry: &str) {
        let line = format!("{} {entry}", self.label);
        self.log.lock().expect("lock").push(line);
    }

    /// Run and remove the first hook registered for `key`.
    fn run_hook(&self, key: &str) {
        let hook = {
            let mut hooks = self.hooks.lock().expect("lock");
            let index = hooks.iter().position(|(k, _)| k == key);
            index.map(|i| hooks.remove(i).1)
        };
        if let Some(hook) = hook {
            hook();
        }
    }
}

impl<B: StorageBackend> StorageBackend for FaultBackend<B> {
    fn content_addressed_tags(&self) -> bool {
        self.inner.content_addressed_tags()
    }

    async fn check_hierarchy(&self, prefix: &str) -> Result<(), StorageError> {
        self.inner.check_hierarchy(prefix).await
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.write("put", key.as_str(), self.inner.put(key, bytes))
            .await
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        let write = self.inner.put_if(key, bytes, precondition);
        self.write("put_if", key.as_str(), write).await
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
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
        self.write("delete", key.as_str(), self.inner.delete(key))
            .await
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        let upload = self.inner.create_multipart(key).await?;
        let mut uploads = self.uploads.lock().expect("lock");
        uploads.insert(upload.as_str().to_string(), key.as_str().to_string());
        Ok(upload)
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
        let key = self
            .uploads
            .lock()
            .expect("lock")
            .get(upload.as_str())
            .cloned();
        let key = key.unwrap_or_default();
        let write = self.inner.complete_multipart(upload, parts, precondition);
        self.write("complete", &key, write).await
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.inner.abort_multipart(upload).await
    }
}
