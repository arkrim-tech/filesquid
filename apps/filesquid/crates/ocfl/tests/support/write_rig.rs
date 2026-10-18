//! A test rig for the write path (IO-0.6): a storage root behind a
//! [`FaultBackend`], a staging backend outside it, a lease store on its own
//! backend sharing the same write log, and a clock the test moves by hand.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{
    complete_immediately, MemoryBackend, StorageBackend, StorageError, StorageKey,
};
use filesquid_ocfl::validate::{validate_object, ValidationReport};
use filesquid_ocfl::write::{
    rfc3339_utc, Clock, CommitError, CommitReceipt, LeaseConfig, LeaseStore, ObjectWriter,
    RepairError, RepairReceipt, VersionDraft,
};
use filesquid_ocfl::VersionNum;

use crate::fault::{FaultBackend, Log};

/// The object every write test uses.
pub const OBJECT_ID: &str = "urn:example:obj";

/// The object root every write test uses.
pub const ROOT: &str = "obj";

/// TTL 60 s, margin 10 s.
pub const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// 2026-10-10T00:00:00Z in Unix milliseconds.
pub const START_MILLIS: u64 = 1_791_590_400_000;

/// A clock the test moves by hand.
#[derive(Debug, Clone)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// A clock at `millis`.
    pub fn at(millis: u64) -> Self {
        Self(Arc::new(AtomicU64::new(millis)))
    }

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

/// The lease store type the rig uses.
pub type Leases = LeaseStore<FaultBackend<MemoryBackend>, ManualClock>;

/// Storage root, staging, leases and clock for one test.
pub struct Rig<B: StorageBackend> {
    pub log: Log,
    pub storage: FaultBackend<B>,
    pub staging: Arc<MemoryBackend>,
    pub leases: Arc<Leases>,
    pub clock: ManualClock,
}

/// Drive a backend future that never waits.
pub fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// A version number from its name.
pub fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// A storage key from its text.
pub fn key(raw: &str) -> StorageKey {
    StorageKey::new(raw).expect("key")
}

/// The sha256 digest of `bytes`.
pub fn sha256(bytes: &[u8]) -> String {
    DigestAlgorithm::Sha256.digest_hex(bytes)
}

impl<B: StorageBackend> Rig<B> {
    /// A rig whose storage root is `inner`.
    pub fn new(inner: B) -> Self {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let clock = ManualClock::at(START_MILLIS);
        let lease_backend = FaultBackend::new(MemoryBackend::new(), "leases", log.clone());
        let leases =
            LeaseStore::new(lease_backend, "leases", clock.clone(), LEASES).expect("lease config");
        Self {
            storage: FaultBackend::new(inner, "storage", log.clone()),
            staging: Arc::new(MemoryBackend::new()),
            leases: Arc::new(leases),
            log,
            clock,
        }
    }

    /// A writer for `holder`.
    pub fn writer(&self, holder: &str) -> ObjectWriter<'_, FaultBackend<B>, Leases> {
        ObjectWriter::new(&self.storage, &*self.leases, holder)
    }

    /// The current time as RFC 3339 UTC.
    pub fn now(&self) -> String {
        rfc3339_utc(self.clock.now_millis().expect("the manual clock"))
    }

    /// A draft that stages every file in `files` under `stage/<tag>/`.
    pub fn draft(&self, tag: &str, files: &[(&str, &[u8])]) -> VersionDraft {
        let mut draft = VersionDraft::new(OBJECT_ID, self.now());
        for (path, bytes) in files {
            let staged = key(&format!("stage/{tag}/{path}"));
            ready(self.staging.put(&staged, bytes)).expect("stage");
            draft.stage(*path, staged);
        }
        draft
    }

    /// Commit `draft` as `holder` on top of `expected` (a version name, or
    /// `None` for a new object). `Err` when the writer "crashed".
    pub fn try_commit(
        &self,
        holder: &str,
        expected: Option<&str>,
        draft: &VersionDraft,
    ) -> Result<Result<CommitReceipt, CommitError>, StorageError> {
        let expected = expected.map(v);
        let writer = self.writer(holder);
        complete_immediately(writer.commit(ROOT, expected.as_ref(), &*self.staging, draft))
    }

    /// Commit `draft`; the writer must not crash.
    pub fn commit(
        &self,
        holder: &str,
        expected: Option<&str>,
        draft: &VersionDraft,
    ) -> Result<CommitReceipt, CommitError> {
        self.try_commit(holder, expected, draft)
            .expect("the commit completed")
    }

    /// Repair the version inventory of the object's head as `holder`
    /// (OCFL-S-020). `Err` when the writer "crashed".
    pub fn try_repair(
        &self,
        holder: &str,
    ) -> Result<Result<RepairReceipt, RepairError>, StorageError> {
        let writer = self.writer(holder);
        complete_immediately(writer.repair_version_inventory(ROOT))
    }

    /// Repair the head's version inventory; the writer must not crash.
    pub fn repair(&self, holder: &str) -> Result<RepairReceipt, RepairError> {
        self.try_repair(holder).expect("the repair completed")
    }

    /// Commit v1 with `a.txt` and `b.txt` and return its receipt.
    pub fn commit_v1(&self) -> CommitReceipt {
        let draft = self.draft("v1", &[("a.txt", b"alpha"), ("b.txt", b"bravo")]);
        self.commit("writer-a", None, &draft).expect("commit v1")
    }

    /// Open the object.
    pub fn open(&self) -> Result<Object<'_, FaultBackend<B>>, ReadError> {
        ready(Object::open(&self.storage, ROOT))
    }

    /// Validate the object.
    pub fn validate(&self) -> ValidationReport {
        ready(validate_object(&self.storage, ROOT)).expect("validation ran")
    }

    /// Assert the object validates with zero errors.
    pub fn assert_valid(&self) {
        let report = self.validate();
        assert!(report.errors().is_empty(), "{:?}", report.errors());
    }

    /// Whether `raw` is stored in the storage root.
    pub fn stored(&self, raw: &str) -> bool {
        ready(self.storage.exists(&key(raw))).expect("exists")
    }

    /// The bytes stored at `raw` in the storage root.
    pub fn bytes(&self, raw: &str) -> Vec<u8> {
        let k = key(raw);
        ready(filesquid_ocfl::storage::read_verified(
            &self.storage,
            &k,
            filesquid_ocfl::storage::MAX_CHUNK_BYTES,
        ))
        .expect("read")
    }

    /// Every key under `prefix` with its entity tag.
    pub fn snapshot(&self, prefix: &str) -> BTreeMap<String, String> {
        let page = ready(self.storage.list(prefix, None, 1_000)).expect("list");
        assert!(page.next.is_none(), "snapshot fits one page");
        page.keys
            .iter()
            .map(|k| {
                let meta = ready(self.storage.head(k)).expect("head");
                (k.as_str().to_string(), meta.etag.as_str().to_string())
            })
            .collect()
    }

    /// The write log so far.
    pub fn writes(&self) -> Vec<String> {
        self.log.lock().expect("lock").clone()
    }

    /// Forget the write log so far.
    pub fn clear_log(&self) {
        self.log.lock().expect("lock").clear();
    }

    /// Let every lease lapse.
    pub fn expire_leases(&self) {
        self.clock.advance(LEASES.ttl_ms);
    }
}
