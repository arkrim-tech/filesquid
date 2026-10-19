//! Shared fixture for the OCFL validation tests (Task API-F2.7). It provides:
//!
//! - one library whose OCFL storage root sits on the write-counting in-memory backend of
//!   [`crate::fixture`];
//! - objects committed by the `filesquid-ocfl` writer, including an E023 object (a stray
//!   file under its head `vN/`, the writer-fencing gap of OCFL-S-018) and a W010 object;
//! - a monitor over an in-memory journal, an in-memory findings store, and a
//!   `ValidationService` on a hand-moved clock.

#![allow(dead_code)]

use crate::fixture::{key, library, ready, CountingBackend, ManualClock, NOW_MS};
use filesquid_api::events::EventSubscription;
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, MAX_PAGE};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::ocfl_validation::{
    MemoryFindingsStore, RunRecord, RunTrigger, ValidationConfig, ValidationError,
    ValidationService,
};
use filesquid_api::{ApiState, EventBus};
use filesquid_core::LibraryId;
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, ObjectWriter, VersionDraft};
use filesquid_ocfl::VersionNum;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// Upper bound on journal pages read by [`ValidationRig::journal`] (AGENTS.md §22).
const MAX_JOURNAL_PAGES: usize = 100;

/// One week in milliseconds: the default schedule.
pub const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// The library, its storage root, the monitor, the findings store and the service.
pub struct ValidationRig {
    /// The storage root of the one library.
    pub root: Arc<CountingBackend>,
    /// Staged bytes for commits.
    pub staging: MemoryBackend,
    /// Writer leases.
    pub leases: LeaseStore<MemoryBackend, ManualClock>,
    /// The lease clock.
    pub lease_clock: ManualClock,
    /// The API state the monitor and the validation are installed in.
    pub state: ApiState,
    /// The monitor (journal, registry, health).
    pub monitor: Monitor,
    /// The journal store behind the monitor.
    pub journal: Arc<MemoryJournal>,
    /// The findings store.
    pub store: Arc<MemoryFindingsStore>,
    /// The service under test.
    pub service: ValidationService<CountingBackend>,
    /// The service clock (Unix ms).
    pub now: Arc<AtomicI64>,
}

impl Drop for ValidationRig {
    /// Stop the journal writer.
    fn drop(&mut self) {
        self.monitor.shutdown();
    }
}

impl ValidationRig {
    /// An empty storage root opened as the library, with the service installed.
    pub fn new() -> Self {
        Self::with_store(Arc::new(MemoryFindingsStore::new()), Arc::new(CountingBackend::new()))
    }

    /// A rig over `store` and `root`: a restarted host keeps both.
    pub fn with_store(store: Arc<MemoryFindingsStore>, root: Arc<CountingBackend>) -> Self {
        let state = ApiState::new();
        let journal = Arc::new(MemoryJournal::new("desktop"));
        let now = Arc::new(AtomicI64::new(NOW_MS));
        let clock_now = now.clone();
        let mut config = MonitorConfig::new("desktop");
        config.clock = Arc::new(move || clock_now.load(Ordering::SeqCst));
        let monitor = Monitor::start(
            journal.clone(),
            Arc::new(StaticHostChecks::healthy()),
            config,
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        let service_now = now.clone();
        let service = ValidationService::new(store.clone(), monitor.clone())
            .with_clock(Arc::new(move || service_now.load(Ordering::SeqCst)));
        if ready(root.list("", None, 1)).expect("list").keys.is_empty() {
            let declaration = filesquid_ocfl::Declaration::storage_root(
                filesquid_ocfl::SpecVersion::V1_1,
            );
            ready(root.put(&key(&declaration.file_name()), declaration.contents().as_bytes()))
                .expect("declare");
        }
        service.open_library(library(), root.clone());
        state.install_validation(Arc::new(service.clone()));
        let lease_clock = ManualClock::at(NOW_MS as u64);
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            lease_clock.clone(),
            LeaseConfig::default(),
        )
        .expect("leases");
        Self {
            root,
            staging: MemoryBackend::new(),
            leases,
            lease_clock,
            state,
            monitor,
            journal,
            store,
            service,
            now,
        }
    }

    /// Commit `files` as the next version of `object_id` at `root` (on top of `expected`).
    pub fn commit(&self, root: &str, object_id: &str, expected: Option<&str>, files: &[(&str, &[u8])]) {
        let mut draft = VersionDraft::new(object_id, "2026-10-18T00:00:00Z");
        for (path, bytes) in files {
            let staged = key(&format!("stage/{root}/{path}"));
            ready(self.staging.put(&staged, bytes)).expect("stage");
            draft.stage(*path, staged);
        }
        let writer = ObjectWriter::new(&*self.root, &self.leases, "test-writer");
        let expected = expected.map(|v| VersionNum::parse(v).expect("version"));
        ready(writer.commit(root, expected.as_ref(), &self.staging, &draft)).expect("commit");
        self.lease_clock.advance(120_000);
    }

    /// A valid sha256 object with one version.
    pub fn object(&self, root: &str, object_id: &str) {
        self.commit(root, object_id, None, &[("a.txt", b"alpha")]);
    }

    /// An object whose head `v1/` holds a stray file its inventory does not list (E023).
    pub fn e023_object(&self, root: &str, object_id: &str) {
        self.object(root, object_id);
        self.put(&format!("{root}/v1/content/stray.tmp"), b"left by a fenced-out writer");
    }

    /// An object whose head has no version inventory or sidecar (W010).
    pub fn w010_object(&self, root: &str, object_id: &str) {
        self.object(root, object_id);
        let mut v2 = VersionDraft::new(object_id, "2026-10-18T00:00:00Z");
        v2.keep("a.txt", DigestAlgorithm::Sha256.digest_hex(b"alpha"));
        let staged = key(&format!("stage/{root}/b.txt"));
        ready(self.staging.put(&staged, b"bravo")).expect("stage");
        v2.stage("b.txt", staged);
        let writer = ObjectWriter::new(&*self.root, &self.leases, "test-writer");
        let v1 = VersionNum::parse("v1").expect("v1");
        ready(writer.commit(root, Some(&v1), &self.staging, &v2)).expect("v2");
        self.lease_clock.advance(120_000);
        self.remove(&format!("{root}/v2/inventory.json"));
        self.remove(&format!("{root}/v2/inventory.json.sha256"));
    }

    /// Store `bytes` at `raw`.
    pub fn put(&self, raw: &str, bytes: &[u8]) {
        ready(self.root.put(&key(raw), bytes)).expect("put");
    }

    /// Delete `raw`.
    pub fn remove(&self, raw: &str) {
        ready(self.root.delete(&key(raw))).expect("delete");
    }

    /// Delete every file of the object at `root`.
    pub fn delete_object(&self, root: &str) {
        let page = ready(self.root.list(&format!("{root}/"), None, 1_000)).expect("list");
        for k in page.keys {
            ready(self.root.delete(&k)).expect("delete");
        }
    }

    /// Run a validation of the library as the owner.
    pub fn run(&self, trigger: RunTrigger, from_start: bool) -> Result<RunRecord, ValidationError> {
        ready_async(self.service.run(&library(), trigger, from_start, "owner"))
    }

    /// Set the object cap of every later run.
    pub fn cap(&self, cap: u64) {
        let config = ValidationConfig {
            interval_ms: WEEK_MS,
            object_cap: cap,
        };
        self.service.configure("owner", config).expect("configure");
    }

    /// The object IDs the store holds findings for, as checked in the last runs.
    pub fn checked_ids(&self, record: &RunRecord) -> u64 {
        record.checked
    }

    /// Every journal entry, oldest first.
    pub fn journal(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
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

    /// Journal entries of `operation`.
    pub fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.journal()
            .into_iter()
            .filter(|e| e.operation == operation)
            .collect()
    }

    /// A subscription to the event channel.
    pub fn events(&self) -> EventSubscription {
        self.state.events.subscription().expect("slot")
    }

    /// Move the service clock forward by `ms`.
    pub fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    /// The library ID.
    pub fn library(&self) -> LibraryId {
        library()
    }
}

/// Drive a future whose backend never waits (the in-memory backends).
pub fn ready_async<F: std::future::Future>(future: F) -> F::Output {
    ready(future)
}

/// An unused bus, for tests that need one.
pub fn bus() -> EventBus {
    EventBus::new()
}
