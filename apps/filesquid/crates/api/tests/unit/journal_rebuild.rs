//! Unit tests for the journal half of the rebuild from files (CL-1.9; API-S-032, ADR 033
//! Decision 7): with the journal store lost, a rebuild restores into a fresh store every
//! entry a committed operation wrote into its version and every library entry up to the
//! last seal, each exactly once, and reports how many it restored and when the last seal
//! ran. A second rebuild restores nothing new; an ID the fresh store already holds for
//! another entry is reported, never overwritten.

use filesquid_api::canonical::{
    commit_snapshot_with, create_storage_root, OwnerReadable, ProfileTransform, SnapshotDelta,
};
use filesquid_api::history::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use filesquid_api::journal::seal::{
    JournalObject, JournalRoot, OcflJournalObject, SealConfig, SealService,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{JournalRestore, LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::{InstallationId, LibraryId, OperationId, OperationKind};
use filesquid_ocfl::change::{ChangeModel, StagingArea};
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::sync::Arc;

const LIB: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";
const INSTALLATION: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// A library storage root, with what its journal object commits through.
struct Library {
    backend: Arc<MemoryBackend>,
    leases: Arc<Leases>,
}

impl Library {
    async fn new() -> Self {
        let backend = Arc::new(MemoryBackend::new());
        create_storage_root(backend.as_ref()).await.expect("root");
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            SystemClock,
            LeaseConfig::default(),
        )
        .expect("leases");
        Self {
            backend,
            leases: Arc::new(leases),
        }
    }

    /// The library's journal object, in its own storage root.
    fn journal_object(&self) -> Arc<dyn JournalObject> {
        let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
        let root = JournalRoot {
            backend: self.backend.clone(),
            reservations: self.leases.clone(),
            staging: Arc::new(MemoryBackend::new()),
            transform,
            holder: "seal".into(),
        };
        let installation = InstallationId::parse(INSTALLATION).expect("installation");
        Arc::new(OcflJournalObject::new(root, LIB, &installation))
    }

    /// Commit one catalog operation whose version carries `entries` beside its `ocfl.commit`
    /// entry; answer the entries the version holds.
    async fn commit(
        &self,
        operation_id: &str,
        entries: Vec<JournalEntry>,
        ids: Vec<i64>,
    ) -> Vec<JournalEntry> {
        let operation = CommitOperation::new(
            OperationId::new(operation_id).expect("op"),
            OperationKind::CatalogOrganisation,
            "2026-10-20T10:00:00Z",
        );
        let recorder =
            CommitRecorder::new(provenance(), operation.clone(), ids[0]).with_entries(entries);
        let staging = MemoryBackend::new();
        let area = StagingArea::new(&staging, &format!("changes/{operation_id}")).expect("area");
        let model = ChangeModel::new(
            self.backend.as_ref(),
            self.leases.as_ref(),
            "writer",
            "snapshot",
            format!("urn:uuid:{LIB}"),
            area,
        );
        let mut delta = SnapshotDelta::default();
        delta
            .puts
            .insert(format!("{operation_id}.json"), b"{}".to_vec());
        let meta = commit_meta(&provenance(), &operation, "2026-10-20T10:00:01Z");
        let receipt = commit_snapshot_with(&model, &OwnerReadable, &delta, meta, Some(&recorder))
            .await
            .expect("commit")
            .expect("a version");
        recorder.journal_entries(&receipt)
    }
}

/// The host before the loss: its journal store and monitor.
struct Host {
    store: Arc<MemoryJournal>,
    monitor: Monitor,
}

impl Host {
    fn new() -> Self {
        let store = Arc::new(MemoryJournal::new("desktop"));
        let monitor = Monitor::start(
            store.clone(),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("desktop"),
            EventBus::new(),
        );
        Self { store, monitor }
    }

    fn record(&self, entries: &[JournalEntry]) -> Vec<i64> {
        self.store.append(entries).expect("append")
    }
}

/// A rebuild service restoring into `fresh`; its own journal goes to a throwaway store.
struct Rebuilder {
    service: LocalLibraryService,
    _writer: JournalWriter,
}

impl Rebuilder {
    fn new(fresh: Option<Arc<MemoryJournal>>) -> Self {
        let writer = JournalWriter::start(
            Arc::new(MemoryJournal::new("desktop")),
            WriterConfig::default(),
            EventBus::new(),
            Arc::new(Metrics::default()),
        );
        let mut service =
            LocalLibraryService::new(writer.sender(), Arc::new(MemoryRepairLedger::new()));
        if let Some(fresh) = fresh {
            let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
            service = service.with_journal_restore(JournalRestore {
                store: fresh,
                transform,
            });
        }
        Self {
            service,
            _writer: writer,
        }
    }
}

fn provenance() -> ProvenanceContext {
    ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: filesquid_prov::HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    }
}

fn library_id() -> LibraryId {
    LibraryId::new(LIB).expect("library id")
}

fn library_event(operation: &str) -> JournalEntry {
    JournalEntry::new(operation, Outcome::Succeeded).target(LIB)
}

fn all_ids(store: &MemoryJournal) -> Vec<i64> {
    let query = JournalQuery {
        limit: 1000,
        ..JournalQuery::default()
    };
    store
        .query(&query)
        .expect("query")
        .entries
        .iter()
        .filter_map(|e| e.id)
        .collect()
}

/// A library with one committed operation, library events sealed after it, and one more
/// event after the seal; answers what a rebuild must restore and the event it may lose.
async fn lived_in_library(host: &Host, library: &Library) -> (Vec<i64>, i64) {
    let reserved = host.store.allocate_ids(2).expect("ids");
    let mut own = library_event("library.collection.create");
    own.id = Some(reserved[1]);
    let committed = library.commit("op-1", vec![own], reserved.clone()).await;
    host.store.append(&committed).expect("journal the commit");
    let sealed_events = host.record(&[
        library_event("library.file.hide"),
        library_event("api.auth.denied"),
    ]);
    host.monitor.journal().flush();
    let seals = SealService::new(
        host.monitor.clone(),
        library.journal_object(),
        SealConfig::default(),
    );
    seals.open_library(LIB, library.journal_object());
    let report = seals.seal_all().await;
    assert!(report.all_sealed(), "{report:?}");
    let after = host.record(&[library_event("library.file.unhide")]);
    let mut expected: Vec<i64> = committed.iter().filter_map(|e| e.id).collect();
    expected.extend(sealed_events);
    let library_sealed =
        filesquid_api::journal::seal::read_sealed_entries(library.backend.as_ref(), &OwnerReadable)
            .await
            .expect("segments");
    expected.extend(library_sealed.iter().filter_map(|e| e.id));
    expected.sort_unstable();
    expected.dedup();
    (expected, after[0])
}

#[tokio::test]
async fn a_rebuild_restores_every_committed_and_sealed_entry_exactly_once() {
    let host = Host::new();
    let library = Library::new().await;
    let (expected, lost) = lived_in_library(&host, &library).await;
    host.monitor.shutdown();
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let rebuilder = Rebuilder::new(Some(fresh.clone()));
    let report = rebuilder
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild");
    let journal = report.journal.expect("the journal was rebuilt");
    assert_eq!(all_ids(&fresh), expected, "every entry once, in ID order");
    assert_eq!(journal.restored, expected.len() as u64);
    assert_eq!(journal.already_present, 0);
    assert_eq!(journal.conflicting, 0);
    assert_eq!(journal.unreadable, 0);
    assert!(
        journal.last_sealed_at.is_some(),
        "the last seal is reported"
    );
    assert!(
        !all_ids(&fresh).contains(&lost),
        "an event after the last seal that changed nothing is the accepted loss"
    );
}

#[tokio::test]
async fn a_second_rebuild_restores_nothing_new() {
    let host = Host::new();
    let library = Library::new().await;
    let (expected, _) = lived_in_library(&host, &library).await;
    host.monitor.shutdown();
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let rebuilder = Rebuilder::new(Some(fresh.clone()));
    let id = library_id();
    let rebuild = || {
        rebuilder
            .service
            .rebuild("owner", &id, library.backend.as_ref())
    };
    rebuild().await.expect("first");
    let again = rebuild().await.expect("second").journal.expect("journal");
    assert_eq!(again.restored, 0);
    assert_eq!(again.already_present, expected.len() as u64);
    assert_eq!(all_ids(&fresh), expected, "no duplicate");
}

#[tokio::test]
async fn an_id_the_fresh_store_holds_for_another_entry_is_reported_not_overwritten() {
    let host = Host::new();
    let library = Library::new().await;
    let (expected, _) = lived_in_library(&host, &library).await;
    host.monitor.shutdown();
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let mut squatter = JournalEntry::new("host.started", Outcome::Info);
    squatter.id = Some(expected[0]);
    fresh.append(&[squatter]).expect("squatter");
    let rebuilder = Rebuilder::new(Some(fresh.clone()));
    let journal = rebuilder
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild")
        .journal
        .expect("journal");
    assert_eq!(journal.conflicting, 1);
    assert_eq!(journal.restored, expected.len() as u64 - 1);
    let kept = fresh.entry(expected[0]).expect("entry").expect("held");
    assert_eq!(kept.operation, "host.started", "the held entry is kept");
}

#[tokio::test]
async fn new_entries_after_a_rebuild_get_ids_above_the_restored_ones() {
    let host = Host::new();
    let library = Library::new().await;
    let (expected, _) = lived_in_library(&host, &library).await;
    host.monitor.shutdown();
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let rebuilder = Rebuilder::new(Some(fresh.clone()));
    rebuilder
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild");
    let next = fresh
        .append(&[library_event("library.file.hide")])
        .expect("append");
    assert!(next[0] > *expected.last().expect("some"), "{next:?}");
}

#[tokio::test]
async fn a_library_without_commits_or_seals_restores_nothing() {
    let library = Library::new().await;
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let rebuilder = Rebuilder::new(Some(fresh.clone()));
    let journal = rebuilder
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild")
        .journal
        .expect("journal");
    assert_eq!(journal.restored, 0);
    assert_eq!(journal.last_sealed_at, None);
    assert!(all_ids(&fresh).is_empty());
}

#[tokio::test]
async fn a_host_without_a_journal_restore_reports_no_journal() {
    let library = Library::new().await;
    let report = Rebuilder::new(None)
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild");
    assert_eq!(report.journal, None);
}

#[tokio::test]
async fn a_committed_operations_entries_come_back_without_any_seal() {
    let host = Host::new();
    let library = Library::new().await;
    let reserved = host.store.allocate_ids(2).expect("ids");
    let mut own = library_event("library.collection.create");
    own.id = Some(reserved[1]);
    let committed = library.commit("op-1", vec![own], reserved.clone()).await;
    host.monitor.shutdown();
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let journal = Rebuilder::new(Some(fresh.clone()))
        .service
        .rebuild("owner", &library_id(), library.backend.as_ref())
        .await
        .expect("rebuild")
        .journal
        .expect("journal");
    let committed_ids: Vec<i64> = committed.iter().filter_map(|e| e.id).collect();
    assert_eq!(committed_ids, reserved);
    assert_eq!(
        all_ids(&fresh),
        reserved,
        "the version's entries, once each"
    );
    assert_eq!(journal.restored, 2);
    assert_eq!(journal.last_sealed_at, None, "never sealed");
}
