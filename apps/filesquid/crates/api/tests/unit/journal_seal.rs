//! Unit tests for the durable journal (API-S-032; ADR 033 Decision 7): per-commit entries in
//! the same version, the sealing service over library and installation journal objects (a
//! seal of mixed events, a failed seal followed by a successful one with nothing lost, the
//! cap, the no-op run), segments that are OCFL objects in a storage root, and the
//! `journal_unsealed` health reason.

use filesquid_api::canonical::{create_storage_root, OwnerReadable, ProfileTransform};
use filesquid_api::health::HealthStatus;
use filesquid_api::journal::seal::{
    is_seal_bookkeeping, journal_object_id, read_sealed_entries, JournalObject, JournalRoot,
    OcflJournalObject, SealConfig, SealError, SealFuture, SealOutcome, SealService, Segment,
    JOURNAL_OBJECT_ROOT, SEAL_INTERVAL_MS, SEAL_OPERATION, SEAL_PROCESS_KIND,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::InstallationId;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;

const LIB_A: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";
const LIB_B: &str = "1c1f8a52-3f0e-4c4b-9a59-5b3c0e7d1a02";
const INSTALLATION: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// One in-memory OCFL storage root with leases and staging.
struct Root {
    backend: Arc<MemoryBackend>,
    root: JournalRoot<MemoryBackend, Leases, MemoryBackend>,
}

impl Root {
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
        let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
        let root = JournalRoot {
            backend: backend.clone(),
            reservations: Arc::new(leases),
            staging: Arc::new(MemoryBackend::new()),
            transform,
            holder: "seal".into(),
        };
        Self { backend, root }
    }

    fn object(&self, scope_id: &str) -> Arc<dyn JournalObject> {
        let installation = InstallationId::parse(INSTALLATION).expect("installation");
        Arc::new(OcflJournalObject::new(
            self.root.clone(),
            scope_id,
            &installation,
        ))
    }

    async fn sealed(&self) -> Vec<JournalEntry> {
        read_sealed_entries(self.backend.as_ref(), &OwnerReadable)
            .await
            .expect("segments")
    }
}

/// A host: monitor over an in-memory journal, a clock the test moves.
struct Host {
    monitor: Monitor,
    store: Arc<MemoryJournal>,
    now: Arc<AtomicI64>,
}

impl Host {
    fn new() -> Self {
        let now = Arc::new(AtomicI64::new(1_800_000_000_000));
        let clock = now.clone();
        let store = Arc::new(MemoryJournal::new("desktop"));
        let mut config = MonitorConfig::new("desktop");
        config.clock = Arc::new(move || clock.load(Ordering::SeqCst));
        let monitor = Monitor::start(
            store.clone(),
            Arc::new(StaticHostChecks::healthy()),
            config,
            EventBus::new(),
        );
        Self {
            monitor,
            store,
            now,
        }
    }

    fn record(&self, entries: &[JournalEntry]) -> Vec<i64> {
        self.store.append(entries).expect("append")
    }

    fn named(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.into()),
            ..JournalQuery::default()
        };
        self.store.query(&query).expect("query").entries
    }
}

fn library_event(library: &str, operation: &str) -> JournalEntry {
    JournalEntry::new(operation, Outcome::Succeeded).target(library)
}

fn host_event(operation: &str) -> JournalEntry {
    JournalEntry::new(operation, Outcome::Info)
}

fn ids(entries: &[JournalEntry]) -> Vec<i64> {
    entries.iter().filter_map(|e| e.id).collect()
}

fn service(host: &Host, installation: Arc<dyn JournalObject>) -> SealService {
    SealService::new(host.monitor.clone(), installation, SealConfig::default())
}

#[tokio::test]
async fn a_seal_puts_library_events_in_the_library_object_and_the_rest_in_the_installation_object()
{
    let host = Host::new();
    let library = Root::new().await;
    let installation = Root::new().await;
    let lib = host.record(&[
        library_event(LIB_A, "library.collection.create"),
        library_event(LIB_A, "library.file.hide"),
    ]);
    let other = host.record(&[
        host_event("host.started"),
        library_event(LIB_B, "library.create"),
        JournalEntry::new("api.auth.denied", Outcome::Failed),
    ]);
    let seals = service(&host, installation.object(INSTALLATION));
    assert!(seals.open_library(LIB_A, library.object(LIB_A)));
    let report = seals.seal_all().await;
    assert!(report.all_sealed(), "{report:?}");
    assert_eq!(ids(&library.sealed().await), lib);
    let host_sealed = ids(&installation.sealed().await);
    assert_eq!(
        host_sealed, other,
        "an unopened library's events stay with the host"
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_second_seal_seals_only_what_came_after_the_first() {
    let host = Host::new();
    let installation = Root::new().await;
    let seals = service(&host, installation.object(INSTALLATION));
    let first = host.record(&[host_event("host.started")]);
    seals.seal_all().await;
    let second = host.record(&[host_event("health.changed")]);
    seals.seal_all().await;
    let sealed = installation.sealed().await;
    let real: Vec<i64> = sealed
        .iter()
        .filter(|e| !is_seal_bookkeeping(e))
        .filter_map(|e| e.id)
        .collect();
    assert_eq!(real, [first, second].concat(), "each entry sealed once");
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_run_with_only_the_seals_own_bookkeeping_commits_and_journals_nothing() {
    let host = Host::new();
    let installation = Root::new().await;
    let seals = service(&host, installation.object(INSTALLATION));
    host.record(&[host_event("host.started")]);
    let first = seals.seal_all().await;
    assert!(first.all_sealed());
    host.monitor.journal().flush();
    let entries_before = host.store.count(&JournalQuery::default()).expect("count");
    let versions_before = installation.sealed().await.len();
    let again = seals.seal_all().await;
    assert!(again.nothing_to_seal(), "{again:?}");
    host.monitor.journal().flush();
    assert_eq!(
        host.store.count(&JournalQuery::default()).expect("count"),
        entries_before,
        "a no-op seal journals nothing"
    );
    assert_eq!(installation.sealed().await.len(), versions_before);
    host.monitor.shutdown();
}

/// A journal object whose commits fail while `down` is set.
struct Flaky {
    inner: Arc<dyn JournalObject>,
    down: AtomicBool,
}

impl JournalObject for Flaky {
    fn object_id(&self) -> &str {
        self.inner.object_id()
    }

    fn last_sealed(&self) -> SealFuture<'_, Result<i64, SealError>> {
        self.inner.last_sealed()
    }

    fn commit_segment<'a>(
        &'a self,
        segment: &'a Segment,
    ) -> SealFuture<'a, Result<String, SealError>> {
        if self.down.load(Ordering::SeqCst) {
            return Box::pin(async { Err(SealError::Unavailable("storage root offline".into())) });
        }
        self.inner.commit_segment(segment)
    }
}

#[tokio::test]
async fn a_failed_seal_is_journaled_and_the_next_seal_loses_nothing() {
    let host = Host::new();
    let library = Root::new().await;
    let installation = Root::new().await;
    let flaky = Arc::new(Flaky {
        inner: library.object(LIB_A),
        down: AtomicBool::new(true),
    });
    let seals = service(&host, installation.object(INSTALLATION));
    seals.open_library(LIB_A, flaky.clone());
    let before = host.record(&[library_event(LIB_A, "library.collection.create")]);
    let failed = seals.seal_all().await;
    let outcome = failed
        .outcome(&format!("library:{LIB_A}"))
        .expect("library scope");
    assert!(matches!(outcome, SealOutcome::Failed { .. }), "{outcome:?}");
    assert!(library.sealed().await.is_empty(), "nothing published");
    let refused = host.named(SEAL_OPERATION);
    assert!(refused
        .iter()
        .any(|e| e.outcome == Outcome::Failed && e.error_code.as_deref() == Some("unavailable")));
    let after = host.record(&[library_event(LIB_A, "library.file.hide")]);
    flaky.down.store(false, Ordering::SeqCst);
    let report = seals.seal_all().await;
    assert!(report.all_sealed(), "{report:?}");
    assert_eq!(ids(&library.sealed().await), [before, after].concat());
    host.monitor.shutdown();
}

#[tokio::test]
async fn closing_a_library_seals_it_and_stops_sealing_into_it() {
    let host = Host::new();
    let library = Root::new().await;
    let installation = Root::new().await;
    let seals = service(&host, installation.object(INSTALLATION));
    seals.open_library(LIB_A, library.object(LIB_A));
    let lib = host.record(&[library_event(LIB_A, "library.collection.create")]);
    host.record(&[host_event("host.started")]);
    let report = seals.close_library(LIB_A).await;
    assert!(report.all_sealed(), "{report:?}");
    assert_eq!(ids(&library.sealed().await), lib);
    assert!(
        installation.sealed().await.is_empty(),
        "library close seals the library only"
    );
    assert!(!seals.is_open(LIB_A));
    seals.seal_all().await;
    let installation_ids = ids(&installation.sealed().await);
    assert!(
        lib.iter().all(|id| !installation_ids.contains(id)),
        "a closed library's entries never reach the installation object"
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_scope_that_reads_only_other_scopes_pages_still_moves_on() {
    let host = Host::new();
    let library = Root::new().await;
    let installation = Root::new().await;
    let config = SealConfig {
        max_scan_pages: 1,
        ..SealConfig::default()
    };
    let seals = SealService::new(
        host.monitor.clone(),
        installation.object(INSTALLATION),
        config,
    );
    seals.open_library(LIB_A, library.object(LIB_A));
    let page = filesquid_api::journal::MAX_PAGE;
    host.record(
        &(0..page)
            .map(|_| library_event(LIB_A, "library.file.hide"))
            .collect::<Vec<_>>(),
    );
    let host_ids = host.record(&[host_event("host.started")]);
    for _ in 0..3 {
        seals.seal_all().await;
    }
    let sealed = ids(&installation.sealed().await);
    assert!(sealed.contains(&host_ids[0]), "the host event is sealed");
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_scope_whose_object_cannot_be_read_counts_from_its_oldest_entry() {
    let host = Host::new();
    let recorded_at = host.now.load(Ordering::SeqCst) - 10_000;
    let seals = service(&host, Arc::new(Unreadable));
    host.record(&[host_event("host.started").at_ms(recorded_at)]);
    let report = seals.seal_all().await;
    assert!(!report.all_sealed(), "{report:?}");
    use filesquid_api::journal::seal::SealStatus;
    assert_eq!(seals.oldest_unsealed_ms(), Some(recorded_at));
    host.monitor.shutdown();
}

/// A journal object whose segments cannot be read.
struct Unreadable;

impl JournalObject for Unreadable {
    fn object_id(&self) -> &str {
        "urn:filesquid:journal:unreadable"
    }

    fn last_sealed(&self) -> SealFuture<'_, Result<i64, SealError>> {
        Box::pin(async { Err(SealError::Unavailable("storage root offline".into())) })
    }

    fn commit_segment<'a>(
        &'a self,
        _segment: &'a Segment,
    ) -> SealFuture<'a, Result<String, SealError>> {
        Box::pin(async { Err(SealError::Unavailable("storage root offline".into())) })
    }
}

#[tokio::test]
async fn segments_are_ocfl_objects_in_the_storage_root() {
    let host = Host::new();
    let library = Root::new().await;
    let seals = service(&host, Root::new().await.object(INSTALLATION));
    seals.open_library(LIB_A, library.object(LIB_A));
    host.record(&[library_event(LIB_A, "library.collection.create")]);
    seals.seal_all().await;
    host.record(&[library_event(LIB_A, "library.file.hide")]);
    seals.seal_all().await;
    let report =
        filesquid_ocfl::validate::validate_object(library.backend.as_ref(), JOURNAL_OBJECT_ROOT)
            .await
            .expect("validate");
    assert!(report.errors().is_empty(), "{:?}", report.errors());
    let object = filesquid_ocfl::read::Object::open(library.backend.as_ref(), JOURNAL_OBJECT_ROOT)
        .await
        .expect("object");
    assert_eq!(object.inventory().id, journal_object_id(LIB_A));
    assert_eq!(object.inventory().versions.len(), 2, "one version per seal");
    let listing = library.backend.list("", None, 1000).await.expect("list");
    assert!(
        listing
            .keys
            .iter()
            .all(|k| k.as_str().starts_with(JOURNAL_OBJECT_ROOT) || k.as_str().starts_with("0=")),
        "nothing outside the storage root's objects: {:?}",
        listing.keys
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn sealing_is_a_registered_journaled_process() {
    let host = Host::new();
    let seals = service(&host, Root::new().await.object(INSTALLATION));
    host.record(&[host_event("host.started")]);
    seals.seal_all().await;
    let processes = host.monitor.registry().snapshot();
    let seal = processes
        .iter()
        .find(|p| p.kind == SEAL_PROCESS_KIND)
        .expect("a seal process");
    assert_eq!(format!("{:?}", seal.state), "Completed");
    let sealed = host.named(SEAL_OPERATION);
    assert_eq!(sealed.len(), 1);
    assert_eq!(sealed[0].outcome, Outcome::Succeeded);
    assert!(sealed[0].target_ids.contains(&"installation".to_string()));
    assert!(sealed[0].target_ids.contains(&"entries:1".to_string()));
    assert!(is_seal_bookkeeping(&sealed[0]));
    assert_eq!(
        sealed[0],
        sealed[0].clone().redacted(),
        "journal.seal is redacted"
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn a_run_seals_at_most_its_cap_and_the_next_run_continues() {
    let host = Host::new();
    let installation = Root::new().await;
    let config = SealConfig {
        max_entries_per_scope: 3,
        ..SealConfig::default()
    };
    let seals = SealService::new(
        host.monitor.clone(),
        installation.object(INSTALLATION),
        config,
    );
    let recorded = host.record(
        &(0..5)
            .map(|_| host_event("process.progress"))
            .collect::<Vec<_>>(),
    );
    seals.seal_all().await;
    assert_eq!(ids(&installation.sealed().await), recorded[..3]);
    seals.seal_all().await;
    let real: Vec<i64> = installation
        .sealed()
        .await
        .iter()
        .filter(|e| !is_seal_bookkeeping(e))
        .filter_map(|e| e.id)
        .collect();
    assert_eq!(real, recorded);
    host.monitor.shutdown();
}

#[tokio::test]
async fn segments_hold_the_redacted_entries_only() {
    let host = Host::new();
    let installation = Root::new().await;
    let seals = service(&host, installation.object(INSTALLATION));
    host.record(&[JournalEntry::new("host.started", Outcome::Info).target("Grandma's birthday")]);
    seals.seal_all().await;
    let text = serde_json::to_string(&installation.sealed().await).expect("json");
    assert!(!text.contains("Grandma"), "{text}");
    host.monitor.shutdown();
}

#[tokio::test]
async fn an_entry_left_unsealed_for_two_intervals_turns_health_degraded() {
    let host = Host::new();
    let library = Root::new().await;
    let flaky = Arc::new(Flaky {
        inner: library.object(LIB_A),
        down: AtomicBool::new(true),
    });
    let seals = Arc::new(service(&host, Root::new().await.object(INSTALLATION)));
    seals.open_library(LIB_A, flaky.clone());
    assert!(host.monitor.install_seal_status(seals.clone()));
    let at = host.now.load(Ordering::SeqCst);
    host.record(&[library_event(LIB_A, "library.collection.create").at_ms(at)]);
    seals.seal_all().await;
    assert_eq!(
        host.monitor.health_report().status,
        HealthStatus::Ok,
        "young"
    );
    host.now
        .fetch_add(2 * SEAL_INTERVAL_MS + 1, Ordering::SeqCst);
    seals.seal_all().await;
    let report = host.monitor.health_report();
    assert_eq!(report.status, HealthStatus::Degraded);
    assert!(report.reasons.iter().any(|r| r.code == "journal_unsealed"));
    flaky.down.store(false, Ordering::SeqCst);
    seals.seal_all().await;
    assert_eq!(host.monitor.health_report().status, HealthStatus::Ok);
    host.monitor.shutdown();
}

#[test]
fn journal_objects_are_named_per_scope() {
    assert_eq!(
        journal_object_id(LIB_A),
        format!("urn:filesquid:journal:{LIB_A}")
    );
    assert_eq!(JOURNAL_OBJECT_ROOT, "journal");
    assert_eq!(SEAL_INTERVAL_MS, 3_600_000);
}

/// The provenance context of the per-commit test.
fn provenance() -> filesquid_api::history::ProvenanceContext {
    filesquid_api::history::ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: filesquid_prov::HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    }
}

#[tokio::test]
async fn a_committed_operation_and_the_seal_together_keep_every_entry_once() {
    use filesquid_api::canonical::{commit_snapshot_with, read_commit_journal, SnapshotDelta};
    use filesquid_api::history::{commit_meta, CommitOperation, CommitRecorder};
    use filesquid_ocfl::change::{ChangeModel, StagingArea};
    let host = Host::new();
    let library = Root::new().await;
    let reserved = host.store.allocate_ids(2).expect("ids");
    let mut own = library_event(LIB_A, "library.collection.create");
    own.id = Some(reserved[1]);
    let operation = CommitOperation::new(
        filesquid_core::OperationId::new("op-1").expect("op"),
        filesquid_core::OperationKind::CatalogOrganisation,
        "2026-10-20T10:00:00Z",
    );
    let recorder =
        CommitRecorder::new(provenance(), operation.clone(), reserved[0]).with_entries(vec![own]);
    let staging = MemoryBackend::new();
    let area = StagingArea::new(&staging, "changes/op-1").expect("area");
    let model = ChangeModel::new(
        library.backend.as_ref(),
        library.root.reservations.as_ref(),
        "writer",
        "snapshot",
        format!("urn:uuid:{LIB_A}"),
        area,
    );
    let mut delta = SnapshotDelta::default();
    delta.puts.insert("library.json".into(), b"{}".to_vec());
    let meta = commit_meta(&provenance(), &operation, "2026-10-20T10:00:01Z");
    let receipt = commit_snapshot_with(&model, &OwnerReadable, &delta, meta, Some(&recorder))
        .await
        .expect("commit")
        .expect("a version");
    let committed = recorder.journal_entries(&receipt);
    host.store.append(&committed).expect("journal");
    let in_version: Vec<JournalEntry> =
        read_commit_journal(library.backend.as_ref(), &OwnerReadable, None)
            .await
            .expect("per-commit entries")
            .iter()
            .map(|line| serde_json::from_str(line).expect("entry"))
            .collect();
    assert_eq!(
        ids(&in_version),
        reserved,
        "the version holds its operation's entries"
    );
    let seals = service(&host, Root::new().await.object(INSTALLATION));
    seals.open_library(LIB_A, library.object(LIB_A));
    seals.seal_all().await;
    let mut recovered: Vec<i64> = ids(&in_version);
    recovered.extend(ids(&library.sealed().await));
    recovered.sort_unstable();
    recovered.dedup();
    let stored: Vec<i64> = host
        .store
        .query(&JournalQuery {
            library_id: Some(LIB_A.into()),
            ..JournalQuery::default()
        })
        .expect("query")
        .entries
        .iter()
        .filter_map(|e| e.id)
        .collect();
    assert!(
        stored.iter().all(|id| recovered.contains(id)),
        "{stored:?} {recovered:?}"
    );
    host.monitor.shutdown();
}
