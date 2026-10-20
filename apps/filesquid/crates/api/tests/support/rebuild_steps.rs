//! Step definitions for "Database deleted, rebuild restores the journal" in
//! `durable_journal_and_journal_tab_F-061.feature` (Task CL-1.9; API-S-032): a library
//! storage root in memory with one committed operation and library events sealed into its
//! journal object, a journal store that is then lost, and the rebuild from files restoring
//! the journal into a fresh store.

use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::canonical::{
    commit_snapshot_with, create_storage_root, OwnerReadable, ProfileTransform, SnapshotDelta,
};
use filesquid_api::history::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use filesquid_api::journal::seal::{
    is_seal_bookkeeping, read_sealed_entries, JournalObject, JournalRoot, OcflJournalObject,
    SealConfig, SealService,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{
    JournalRebuild, JournalRestore, LocalLibraryService, MemoryRepairLedger,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::{InstallationId, LibraryId, OperationId, OperationKind};
use filesquid_ocfl::change::{ChangeModel, StagingArea};
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::collections::BTreeSet;
use std::sync::Arc;

/// The library of the scenario.
const LIBRARY: &str = "lib-a";
const INSTALLATION: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// Per-scenario state of the rebuild steps.
#[derive(Default)]
pub struct RebuildWorld {
    backend: Option<Arc<MemoryBackend>>,
    leases: Option<Arc<Leases>>,
    lost: Option<Arc<MemoryJournal>>,
    committed: BTreeSet<i64>,
    sealed: BTreeSet<i64>,
    after_seal: BTreeSet<i64>,
    fresh: Option<Arc<MemoryJournal>>,
    report: Option<JournalRebuild>,
}

impl std::fmt::Debug for RebuildWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RebuildWorld").finish_non_exhaustive()
    }
}

impl RebuildWorld {
    fn backend(&self) -> &MemoryBackend {
        self.backend.as_deref().expect("a library storage root")
    }

    fn lost(&self) -> &MemoryJournal {
        self.lost
            .as_deref()
            .expect("the journal store before the loss")
    }

    fn fresh_ids(&self) -> Vec<i64> {
        ids_of(self.fresh.as_deref().expect("a rebuild"))
    }

    /// The library's journal object, in its own storage root.
    fn journal_object(&self) -> Arc<dyn JournalObject> {
        let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
        let root = JournalRoot {
            backend: self.backend.clone().expect("root"),
            reservations: self.leases.clone().expect("leases"),
            staging: Arc::new(MemoryBackend::new()),
            transform,
            holder: "seal".into(),
        };
        let installation = InstallationId::parse(INSTALLATION).expect("installation");
        Arc::new(OcflJournalObject::new(root, LIBRARY, &installation))
    }

    /// Commit one catalog operation whose version carries one library entry; answer the
    /// entries the version holds.
    async fn commit(&self, ids: Vec<i64>) -> Vec<JournalEntry> {
        let operation = CommitOperation::new(
            OperationId::new("op-rebuild").expect("op"),
            OperationKind::CatalogOrganisation,
            "2026-10-20T10:00:00Z",
        );
        let mut own = library_event("library.collection.create");
        own.id = Some(ids[1]);
        let recorder =
            CommitRecorder::new(provenance(), operation.clone(), ids[0]).with_entries(vec![own]);
        let staging = MemoryBackend::new();
        let area = StagingArea::new(&staging, "changes/op-rebuild").expect("area");
        let model = ChangeModel::new(
            self.backend(),
            self.leases.as_deref().expect("leases"),
            "writer",
            "snapshot",
            format!("urn:uuid:{INSTALLATION}"),
            area,
        );
        let mut delta = SnapshotDelta::default();
        delta.puts.insert("library.json".into(), b"{}".to_vec());
        let meta = commit_meta(&provenance(), &operation, "2026-10-20T10:00:01Z");
        let receipt = commit_snapshot_with(&model, &OwnerReadable, &delta, meta, Some(&recorder))
            .await
            .expect("commit")
            .expect("a version");
        recorder.journal_entries(&receipt)
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

fn library_event(operation: &str) -> JournalEntry {
    JournalEntry::new(operation, Outcome::Succeeded).target(LIBRARY)
}

fn ids_of(store: &MemoryJournal) -> Vec<i64> {
    let query = JournalQuery {
        limit: 1000,
        ..JournalQuery::default()
    };
    let page = store.query(&query).expect("query");
    page.entries.iter().filter_map(|e| e.id).collect()
}

/// A storage root for the library and its writers' leases.
async fn library_root() -> (Arc<MemoryBackend>, Arc<Leases>) {
    let backend = Arc::new(MemoryBackend::new());
    create_storage_root(backend.as_ref()).await.expect("root");
    let leases = LeaseStore::new(
        MemoryBackend::new(),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("leases");
    (backend, Arc::new(leases))
}

/// The installation's journal object, in a storage root of its own.
async fn installation_object() -> Arc<dyn JournalObject> {
    let (backend, leases) = library_root().await;
    let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
    let root = JournalRoot {
        backend,
        reservations: leases,
        staging: Arc::new(MemoryBackend::new()),
        transform,
        holder: "seal".into(),
    };
    let installation = InstallationId::parse(INSTALLATION).expect("installation");
    Arc::new(OcflJournalObject::new(root, INSTALLATION, &installation))
}

/// Seal the library's entries of `monitor`'s store into its journal object, and every other
/// entry into the installation's.
async fn seal(w: &RebuildWorld, monitor: &Monitor) {
    let installation = installation_object().await;
    let seals = SealService::new(monitor.clone(), installation, SealConfig::default());
    seals.open_library(LIBRARY, w.journal_object());
    let report = seals.seal_all().await;
    assert!(report.all_sealed(), "{report:?}");
}

#[given(expr = "committed operations and other entries sealed at the last hourly seal")]
async fn committed_and_sealed(w: &mut MonitorWorld) {
    let (backend, leases) = library_root().await;
    let lost = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        lost.clone(),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        EventBus::new(),
    );
    let r = &mut w.rebuild;
    r.backend = Some(backend);
    r.leases = Some(leases);
    let committed = r.commit(lost.allocate_ids(2).expect("ids")).await;
    lost.append(&committed).expect("journal the commit");
    r.committed = committed.iter().filter_map(|e| e.id).collect();
    lost.append(&[
        library_event("library.file.hide"),
        library_event("api.auth.denied"),
    ])
    .expect("other entries");
    monitor.journal().flush();
    seal(r, &monitor).await;
    let sealed = read_sealed_entries(r.backend(), &OwnerReadable)
        .await
        .expect("segments");
    r.sealed = sealed.iter().filter_map(|e| e.id).collect();
    monitor.shutdown();
    r.lost = Some(lost);
}

#[given(expr = "entries that changed nothing written after that seal")]
fn after_the_seal(w: &mut MonitorWorld) {
    let r = &mut w.rebuild;
    let ids = r
        .lost()
        .append(&[JournalEntry::new("library.view.open", Outcome::Info).target(LIBRARY)])
        .expect("after the seal");
    r.after_seal = ids.into_iter().collect();
}

#[when(expr = "the operational database is deleted and I rebuild the library from its files")]
async fn delete_and_rebuild(w: &mut MonitorWorld) {
    let r = &mut w.rebuild;
    let fresh = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        Arc::new(MemoryJournal::new("desktop")),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let transform: Arc<dyn ProfileTransform> = Arc::new(OwnerReadable);
    let service = LocalLibraryService::new(writer.sender(), Arc::new(MemoryRepairLedger::new()))
        .with_journal_restore(JournalRestore {
            store: fresh.clone(),
            transform,
        });
    let library = LibraryId::new(LIBRARY).expect("library id");
    let report = service
        .rebuild("owner", &library, r.backend())
        .await
        .expect("rebuild");
    r.report = report.journal;
    r.fresh = Some(fresh);
}

#[then(expr = "the journal should contain every committed operation's entries")]
fn has_committed(w: &mut MonitorWorld) {
    let r = &w.rebuild;
    let fresh: BTreeSet<i64> = r.fresh_ids().into_iter().collect();
    assert!(!r.committed.is_empty());
    assert!(r.committed.is_subset(&fresh), "{:?} {fresh:?}", r.committed);
}

#[then(expr = "every entry up to the last seal")]
fn has_sealed(w: &mut MonitorWorld) {
    let r = &w.rebuild;
    let fresh: BTreeSet<i64> = r.fresh_ids().into_iter().collect();
    assert!(!r.sealed.is_empty());
    assert!(r.sealed.is_subset(&fresh), "{:?} {fresh:?}", r.sealed);
    let report = r.report.as_ref().expect("a journal rebuild");
    assert!(report.last_sealed_at.is_some(), "the last seal is reported");
}

#[then(expr = "no entry twice")]
fn no_entry_twice(w: &mut MonitorWorld) {
    let r = &w.rebuild;
    let ids = r.fresh_ids();
    let distinct: BTreeSet<i64> = ids.iter().copied().collect();
    assert_eq!(ids.len(), distinct.len());
    let report = r.report.as_ref().expect("a journal rebuild");
    assert_eq!(
        report.restored,
        ids.len() as u64,
        "the report counts each once"
    );
}

#[then(expr = "only entries that changed nothing after the last seal should be missing")]
fn only_after_seal_missing(w: &mut MonitorWorld) {
    let r = &w.rebuild;
    let fresh: BTreeSet<i64> = r.fresh_ids().into_iter().collect();
    let query = JournalQuery {
        library_id: Some(LIBRARY.into()),
        limit: 1000,
        ..JournalQuery::default()
    };
    let before = r.lost().query(&query).expect("query").entries;
    let missing: Vec<&JournalEntry> = before
        .iter()
        .filter(|e| e.id.is_some_and(|id| !fresh.contains(&id)))
        .collect();
    assert!(
        !missing.is_empty(),
        "the event after the seal is the accepted loss"
    );
    for entry in missing {
        let after = entry.id.is_some_and(|id| r.after_seal.contains(&id));
        assert!(after || is_seal_bookkeeping(entry), "lost {entry:?}");
    }
}
