//! Shared fixture for the local library tests (Task CL-1.1): an OCFL storage root on the
//! in-memory backend behind a write-counting wrapper, a lease store on its own backend, a
//! hand-moved clock, objects committed by the `filesquid-ocfl` writer (some with their head
//! version inventory removed, W010), a `LocalLibraryService` with a bounded journal queue over
//! an in-memory journal, and the in-memory repair ledger.

#![allow(dead_code)]

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, MAX_PAGE};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{
    LocalLibraryService, MemoryRepairLedger, RepairConsent, RepairRequest, CONSENT_STATEMENT_ID,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::EventBus;
use filesquid_core::LibraryId;
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{
    complete_immediately, write_whole, ByteRange, CompletedPart, ListPage, MemoryBackend,
    ObjectMeta, Precondition, StorageBackend, StorageError, StorageKey, UploadId,
};
use filesquid_ocfl::write::{
    rfc3339_utc, Clock, CommitError, CommitReceipt, LeaseConfig, LeaseStore, ObjectWriter,
    VersionDraft,
};
use filesquid_ocfl::VersionNum;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The fixed journal clock: 2026-10-18T00:00:00Z in Unix milliseconds.
pub const NOW_MS: i64 = 1_792_281_600_000;

/// The library every test names.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// Lease TTL 60 s with a 10 s margin.
const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// Upper bound on journal pages read by [`Rig::journal`] (AGENTS.md §22).
const MAX_JOURNAL_PAGES: usize = 100;

/// Drive a backend future that never waits.
pub fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("in-memory futures complete at once")
}

/// A storage key from its text.
pub fn key(raw: &str) -> StorageKey {
    StorageKey::new(raw).expect("key")
}

/// A version number from its name.
pub fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// The library ID every test uses.
pub fn library() -> LibraryId {
    LibraryId::new(LIBRARY).expect("library id")
}

/// A clock the test moves by hand.
#[derive(Debug, Clone)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// Move the clock forward by `millis`.
    pub fn advance(&self, millis: u64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_millis(&self) -> Option<u64> {
        Some(self.0.load(Ordering::SeqCst))
    }
}

/// An in-memory backend that counts every write (put, conditional put, delete and every
/// multipart step), so a test can prove an operation wrote nothing.
#[derive(Default)]
pub struct CountingBackend {
    inner: MemoryBackend,
    writes: AtomicU64,
}

impl CountingBackend {
    /// An empty backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes so far.
    pub fn writes(&self) -> u64 {
        self.writes.load(Ordering::SeqCst)
    }

    /// Count one write.
    fn count(&self) {
        self.writes.fetch_add(1, Ordering::SeqCst);
    }

    /// Every key and its bytes, for before/after comparisons.
    pub fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let page = ready(self.inner.list("", None, 1_000)).expect("list");
        assert!(page.next.is_none(), "fixture roots stay below one page");
        page.keys
            .into_iter()
            .map(|k| {
                let size = ready(self.inner.head(&k)).expect("head").size;
                let bytes = match size {
                    0 => Vec::new(),
                    n => ready(
                        self.inner
                            .get_range(&k, ByteRange::new(0, n).expect("range")),
                    )
                    .expect("read"),
                };
                (k.as_str().to_string(), bytes)
            })
            .collect()
    }
}

impl StorageBackend for CountingBackend {
    fn content_addressed_tags(&self) -> bool {
        self.inner.content_addressed_tags()
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.count();
        self.inner.put(key, bytes).await
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.count();
        self.inner.put_if(key, bytes, precondition).await
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
        self.count();
        self.inner.delete(key).await
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        self.count();
        self.inner.create_multipart(key).await
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.count();
        self.inner.upload_part(upload, part_number, bytes).await
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.count();
        self.inner
            .complete_multipart(upload, parts, precondition)
            .await
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.count();
        self.inner.abort_multipart(upload).await
    }
}

/// The lease store the rig uses.
pub type Leases = LeaseStore<MemoryBackend, ManualClock>;

/// A storage root, its leases, a service with its journal, and the repair ledger.
pub struct Rig {
    /// The storage root.
    pub root: CountingBackend,
    /// Staged bytes for commits, outside the storage root.
    pub staging: MemoryBackend,
    /// Fenced leases for the writer and the repair.
    pub leases: Leases,
    /// The lease clock.
    pub clock: ManualClock,
    /// The service under test.
    pub service: LocalLibraryService,
    /// The repair ledger the service records consents and outcomes in.
    pub ledger: Arc<MemoryRepairLedger>,
    /// The journal store behind the bounded queue.
    pub journal: Arc<MemoryJournal>,
    writer: JournalWriter,
}

impl Drop for Rig {
    /// Write what is queued and stop the journal writer thread.
    fn drop(&mut self) {
        self.writer.shutdown();
    }
}

impl Rig {
    /// An empty folder with a service, ledger and journal.
    pub fn new() -> Self {
        let journal = Arc::new(MemoryJournal::new("desktop"));
        let writer = JournalWriter::start(
            journal.clone(),
            WriterConfig::default(),
            EventBus::new(),
            Arc::new(Metrics::default()),
        );
        let ledger = Arc::new(MemoryRepairLedger::new());
        let service = LocalLibraryService::new(writer.sender(), ledger.clone())
            .with_clock(Arc::new(|| NOW_MS));
        let clock = ManualClock(Arc::new(AtomicU64::new(NOW_MS as u64)));
        let leases = LeaseStore::new(MemoryBackend::new(), "leases", clock.clone(), LEASES)
            .expect("lease config");
        Self {
            root: CountingBackend::new(),
            staging: MemoryBackend::new(),
            leases,
            clock,
            service,
            ledger,
            journal,
            writer,
        }
    }

    /// A rig whose folder is already an empty OCFL 1.1 storage root.
    pub fn with_root() -> Self {
        let rig = Self::new();
        ready(rig.service.create_root("owner", &library(), &rig.root)).expect("create root");
        rig
    }

    /// The current lease time as RFC 3339 UTC.
    fn now(&self) -> String {
        rfc3339_utc(self.clock.now_millis().expect("manual clock"))
    }

    /// A draft of `object_id` that stages `files` under `stage/<tag>/`.
    pub fn draft(&self, object_id: &str, tag: &str, files: &[(&str, &[u8])]) -> VersionDraft {
        let mut draft = VersionDraft::new(object_id, self.now());
        for (path, bytes) in files {
            let staged = key(&format!("stage/{tag}/{path}"));
            ready(self.staging.put(&staged, bytes)).expect("stage");
            draft.stage(*path, staged);
        }
        draft
    }

    /// A draft of `object_id` with one file of `bytes`, staged whole (multipart above one put).
    pub fn draft_whole(&self, object_id: &str, path: &str, bytes: &[u8]) -> VersionDraft {
        let mut draft = VersionDraft::new(object_id, self.now());
        let staged = key(&format!("stage/whole/{path}"));
        ready(write_whole(&self.staging, &staged, bytes)).expect("stage");
        draft.stage(path, staged);
        draft
    }

    /// Commit `draft` at the object root `root` on top of `expected`.
    pub fn commit(
        &self,
        root: &str,
        expected: Option<&str>,
        draft: &VersionDraft,
    ) -> Result<CommitReceipt, CommitError> {
        let writer = ObjectWriter::new(&self.root, &self.leases, "test-writer");
        let expected = expected.map(v);
        ready(writer.commit(root, expected.as_ref(), &self.staging, draft))
    }

    /// A sha256 object at `root` with a v1 of two files and a v2 adding a third.
    pub fn object(&self, root: &str, object_id: &str) {
        let v1 = self.draft(object_id, &format!("{root}-v1"), &[("a.txt", b"alpha")]);
        self.commit(root, None, &v1).expect("commit v1");
        let mut v2 = self.draft(object_id, &format!("{root}-v2"), &[("b.txt", b"bravo")]);
        v2.keep("a.txt", DigestAlgorithm::Sha256.digest_hex(b"alpha"));
        self.commit(root, Some("v1"), &v2).expect("commit v2");
        self.clock.advance(120_000);
    }

    /// A sha256 object at `root` whose head (v2) version inventory and sidecar were removed:
    /// the W010 shape.
    pub fn w010_object(&self, root: &str, object_id: &str) {
        self.object(root, object_id);
        self.remove(&format!("{root}/v2/inventory.json"));
        self.remove(&format!("{root}/v2/inventory.json.sha256"));
    }

    /// A W010 object whose head version inventory was put back without its sidecar: an
    /// interrupted repair.
    pub fn interrupted_repair(&self, root: &str, object_id: &str) {
        self.w010_object(root, object_id);
        let bytes = self.bytes(&format!("{root}/inventory.json"));
        ready(
            self.root
                .put(&key(&format!("{root}/v2/inventory.json")), &bytes),
        )
        .expect("put");
    }

    /// Delete one key of the storage root.
    pub fn remove(&self, raw: &str) {
        ready(self.root.delete(&key(raw))).expect("delete");
    }

    /// The bytes at `raw`.
    pub fn bytes(&self, raw: &str) -> Vec<u8> {
        let k = key(raw);
        let size = ready(self.root.head(&k)).expect("head").size;
        ready(
            self.root
                .get_range(&k, ByteRange::new(0, size).expect("range")),
        )
        .expect("read")
    }

    /// Whether `raw` is stored in the storage root.
    pub fn stored(&self, raw: &str) -> bool {
        ready(self.root.exists(&key(raw))).expect("exists")
    }

    /// A consented repair request for `object_ids`.
    pub fn consented(&self, object_ids: &[&str]) -> RepairRequest {
        let ids: Vec<String> = object_ids.iter().map(|s| s.to_string()).collect();
        RepairRequest {
            library_id: library(),
            object_ids: ids.clone(),
            consent: Some(RepairConsent {
                library_id: library(),
                object_ids: ids,
                statement_id: CONSENT_STATEMENT_ID.to_string(),
                confirmed: true,
            }),
        }
    }

    /// Every journal entry written through the bounded queue, oldest first.
    pub fn journal(&self) -> Vec<JournalEntry> {
        self.writer.sender().flush();
        let mut entries = Vec::new();
        let mut query = JournalQuery {
            limit: MAX_PAGE,
            ..JournalQuery::default()
        };
        for _ in 0..MAX_JOURNAL_PAGES {
            let page = self.journal.query(&query).expect("journal query");
            entries.extend(page.entries);
            match page.next_cursor {
                Some(cursor) => query.after_id = Some(cursor),
                None => return entries,
            }
        }
        panic!("journal longer than {MAX_JOURNAL_PAGES} pages");
    }

    /// Journal entries through the queue for `operation`.
    pub fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.journal()
            .into_iter()
            .filter(|e| e.operation == operation)
            .collect()
    }
}
