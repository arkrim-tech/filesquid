//! Step definitions for the sealing scenarios of `durable_journal_and_journal_tab_F-061.feature`
//! (Task API-F2.11; API-S-032): the seal service of the background's desktop host, sealing
//! the seeded library `lib-a` into a library journal object and every other event into the
//! installation's journal object, both OCFL objects in in-memory storage roots.

use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::canonical::{create_storage_root, OwnerReadable, ProfileTransform};
use filesquid_api::health::HealthStatus;
use filesquid_api::journal::seal::{
    is_seal_bookkeeping, read_sealed_entries, JournalObject, JournalRoot, OcflJournalObject,
    SealConfig, SealError, SealFuture, SealReport, SealService, Segment, JOURNAL_OBJECT_ROOT,
    SEAL_INTERVAL_MS, SEAL_OPERATION, SEAL_PROCESS_KIND,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, Outcome};
use filesquid_api::monitor::Monitor;
use filesquid_core::InstallationId;
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The seeded library of the background.
const LIBRARY: &str = "lib-a";
const INSTALLATION: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// A journal object whose commits fail while `down` is set.
struct Switch {
    inner: Arc<dyn JournalObject>,
    down: AtomicBool,
}

impl JournalObject for Switch {
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
            return Box::pin(async { Err(SealError::Unavailable("not writable".into())) });
        }
        self.inner.commit_segment(segment)
    }
}

/// Per-scenario state of the sealing steps.
#[derive(Default)]
pub struct SealWorld {
    service: Option<Arc<SealService>>,
    monitor: Option<Monitor>,
    library: Option<Arc<MemoryBackend>>,
    installation: Option<Arc<MemoryBackend>>,
    switch: Option<Arc<Switch>>,
    expected: Vec<i64>,
    report: Option<SealReport>,
    entries_before: u64,
    versions_before: usize,
}

impl std::fmt::Debug for SealWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealWorld").finish_non_exhaustive()
    }
}

/// An in-memory OCFL storage root and its journal object for `scope_id`.
async fn root(scope_id: &str) -> (Arc<MemoryBackend>, Arc<dyn JournalObject>) {
    let backend = Arc::new(MemoryBackend::new());
    create_storage_root(backend.as_ref()).await.expect("root");
    let leases: Leases = LeaseStore::new(
        MemoryBackend::new(),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("leases");
    let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
    let journal_root = JournalRoot {
        backend: backend.clone(),
        reservations: Arc::new(leases),
        staging: Arc::new(MemoryBackend::new()),
        transform,
        holder: "seal".into(),
    };
    let installation = InstallationId::parse(INSTALLATION).expect("installation");
    let object = OcflJournalObject::new(journal_root, scope_id, &installation);
    (backend, Arc::new(object))
}

impl SealWorld {
    /// Start the seal service over the background's monitor, once.
    async fn start(&mut self, monitor: Monitor) {
        if self.service.is_some() {
            return;
        }
        let (library, library_object) = root(LIBRARY).await;
        let (installation, installation_object) = root(INSTALLATION).await;
        let switch = Arc::new(Switch {
            inner: library_object,
            down: AtomicBool::new(false),
        });
        let service = Arc::new(SealService::new(
            monitor.clone(),
            installation_object,
            SealConfig::default(),
        ));
        service.open_library(LIBRARY, switch.clone());
        monitor.install_seal_status(service.clone());
        self.service = Some(service);
        self.monitor = Some(monitor);
        self.library = Some(library);
        self.installation = Some(installation);
        self.switch = Some(switch);
    }

    fn service(&self) -> &SealService {
        self.service.as_deref().expect("a seal service")
    }

    fn monitor(&self) -> &Monitor {
        self.monitor.as_ref().expect("a monitor")
    }

    fn record(&mut self, entries: &[JournalEntry]) {
        let ids = self.monitor().store().append(entries).expect("append");
        self.expected.extend(ids);
    }
}

/// The seal world, started over the background's monitor.
async fn world(w: &mut MonitorWorld) -> &mut SealWorld {
    let monitor = w
        .journal
        .monitor_handle()
        .expect("the background's monitor");
    w.seal.start(monitor).await;
    &mut w.seal
}

/// The non-bookkeeping journal IDs sealed in `backend`.
async fn sealed_ids(backend: &MemoryBackend) -> Vec<i64> {
    read_sealed_entries(backend, &OwnerReadable)
        .await
        .expect("segments")
        .iter()
        .filter(|e| !is_seal_bookkeeping(e))
        .filter_map(|e| e.id)
        .collect()
}

/// The number of versions of the journal object in `backend`.
async fn versions(backend: &MemoryBackend) -> usize {
    match filesquid_ocfl::read::Object::open(backend, JOURNAL_OBJECT_ROOT).await {
        Ok(object) => object.inventory().versions.len(),
        Err(_) => 0,
    }
}

#[given(expr = "an authorisation refusal and a health change were journaled")]
async fn refusal_and_health_change(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.expected.clear();
    seal.record(&[
        JournalEntry::new("api.auth.denied", Outcome::Failed).error_code("not_permitted"),
        JournalEntry::new("health.changed", Outcome::Info),
    ]);
}

#[when(expr = "the hourly seal runs")]
async fn hourly_seal(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.report = Some(seal.service().seal_all().await);
}

#[then(
    expr = "the installation's journal object should gain one JSON Lines segment with those entries"
)]
async fn installation_gained_segment(w: &mut MonitorWorld) {
    let seal = world(w).await;
    let installation = seal.installation.clone().expect("installation root");
    assert_eq!(versions(&installation).await, 1, "one segment, one version");
    let sealed = sealed_ids(&installation).await;
    for id in &seal.expected {
        assert!(sealed.contains(id), "entry {id} sealed: {sealed:?}");
    }
}

#[then(expr = "the seal should be a registered, journaled process")]
async fn registered_process(w: &mut MonitorWorld) {
    let seal = world(w).await;
    let monitor = seal.monitor().clone();
    let processes = monitor.registry().snapshot();
    assert!(processes.iter().any(|p| p.kind == SEAL_PROCESS_KIND));
    monitor.journal().flush();
    let query = JournalQuery {
        operation: Some(SEAL_OPERATION.into()),
        ..JournalQuery::default()
    };
    assert!(monitor.store().count(&query).expect("count") >= 1);
}

#[given(expr = "library entries that are not yet sealed")]
async fn library_entries(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.expected.clear();
    seal.record(&[JournalEntry::new("library.file.hide", Outcome::Succeeded).target(LIBRARY)]);
}

#[when(expr = "I close the library")]
async fn close_library(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.report = Some(seal.service().close_library(LIBRARY).await);
}

#[then(expr = "they should be sealed into the library's journal object")]
async fn sealed_into_library(w: &mut MonitorWorld) {
    let seal = world(w).await;
    let library = seal.library.clone().expect("library root");
    let sealed = sealed_ids(&library).await;
    assert!(!seal.expected.is_empty());
    assert!(
        seal.expected.iter().all(|id| sealed.contains(id)),
        "{sealed:?}"
    );
    assert!(!seal.service().is_open(LIBRARY), "closed for sealing");
}

#[given(expr = "host entries that are not yet sealed")]
async fn host_entries(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.expected.clear();
    seal.record(&[JournalEntry::new("host.stopping", Outcome::Info)]);
}

#[when(expr = "I close the app")]
async fn close_app(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.report = Some(seal.service().seal_all().await);
}

#[then(expr = "they should be sealed into the installation's journal object")]
async fn sealed_into_installation(w: &mut MonitorWorld) {
    let seal = world(w).await;
    let installation = seal.installation.clone().expect("installation root");
    let sealed = sealed_ids(&installation).await;
    assert!(
        seal.expected.iter().all(|id| sealed.contains(id)),
        "{sealed:?}"
    );
}

#[given(expr = "the library's storage root is not writable during a seal")]
async fn library_not_writable(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.expected.clear();
    // Recorded two sealing intervals and a moment ago, as if the seals had been failing
    // since then; the next steps cannot move the system clock.
    let at = seal.monitor().now_ms() - 2 * SEAL_INTERVAL_MS - 1;
    seal.record(&[JournalEntry::new("library.file.hide", Outcome::Succeeded)
        .target(LIBRARY)
        .at_ms(at)]);
    seal.switch
        .as_ref()
        .expect("switch")
        .down
        .store(true, Ordering::SeqCst);
    let report = seal.service().seal_all().await;
    assert!(!report.all_sealed(), "the library's seal failed");
}

#[when(expr = "the next seal runs after the storage root is writable again")]
async fn next_seal_writable(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.record(&[JournalEntry::new("library.file.unhide", Outcome::Succeeded).target(LIBRARY)]);
    seal.switch
        .as_ref()
        .expect("switch")
        .down
        .store(false, Ordering::SeqCst);
    seal.report = Some(seal.service().seal_all().await);
}

#[then(expr = "every entry that was not sealed should be in the new segment")]
async fn nothing_lost(w: &mut MonitorWorld) {
    let seal = world(w).await;
    assert!(seal.report.as_ref().expect("report").all_sealed());
    let library = seal.library.clone().expect("library root");
    let sealed = sealed_ids(&library).await;
    assert!(
        seal.expected.iter().all(|id| sealed.contains(id)),
        "{sealed:?}"
    );
}

#[given(expr = "the hourly seal has run")]
async fn hourly_seal_has_run(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.service().seal_all().await;
    let monitor = seal.monitor().clone();
    monitor.journal().flush();
    seal.entries_before = monitor
        .store()
        .count(&JournalQuery::default())
        .expect("count");
    let installation = seal.installation.clone().expect("installation root");
    seal.versions_before = versions(&installation).await;
}

#[when(expr = "the hourly seal runs again with nothing new")]
async fn seal_again(w: &mut MonitorWorld) {
    let seal = world(w).await;
    seal.report = Some(seal.service().seal_all().await);
}

#[then(expr = "no journal version should be added and nothing should be journaled")]
async fn nothing_added(w: &mut MonitorWorld) {
    let seal = world(w).await;
    assert!(seal.report.as_ref().expect("report").nothing_to_seal());
    let monitor = seal.monitor().clone();
    monitor.journal().flush();
    let count = monitor
        .store()
        .count(&JournalQuery::default())
        .expect("count");
    assert_eq!(count, seal.entries_before);
    let installation = seal.installation.clone().expect("installation root");
    assert_eq!(versions(&installation).await, seal.versions_before);
}

#[when(expr = "two sealing intervals pass and the seal fails again")]
async fn two_intervals_pass(w: &mut MonitorWorld) {
    let seal = world(w).await;
    let report = seal.service().seal_all().await;
    assert!(!report.all_sealed(), "the library's seal fails again");
}

#[then(expr = "health should be degraded with the reason {string}")]
async fn degraded_with(w: &mut MonitorWorld, reason: String) {
    let seal = world(w).await;
    let report = seal.monitor().health_report();
    assert_eq!(report.status, HealthStatus::Degraded);
    assert!(
        report.reasons.iter().any(|r| r.code == reason),
        "{:?}",
        report.reasons
    );
}
