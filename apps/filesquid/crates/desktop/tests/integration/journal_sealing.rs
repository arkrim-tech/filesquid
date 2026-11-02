//! Task DE-0.11: the desktop seals its activity journal (DE-S-013, API-S-032, ADR 033
//! Decision 7). A launch catches up a seal the last launch missed; the hourly seal runs
//! without a call and ends with the app; the close-time seal seals what is pending; a served
//! library is sealed into its own storage root; a launch that found no operational store
//! refills it from the sealed journal under the same journal IDs; a library rebuild restores
//! the library's journal through the SQLite journal store. Runs against SQLite files and OCFL
//! storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::canonical::{create_storage_root, OwnerReadable};
use filesquid_api::journal::seal::{
    read_sealed_entries, SealOutcome, SEAL_OPERATION, SEAL_PROCESS_KIND,
};
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::local_library::{LibraryRoot, MemoryRepairLedger};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId};
use filesquid_desktop_lib::sealing::{
    launch_sealing, open_library, refill_before_open, seal_before_close, spawn_hourly,
    start_sealing, REFILL_OPERATION, REFILL_PENDING_FILE,
};
use filesquid_desktop_lib::{load_installation, local_library_service, open_host_monitor};
use filesquid_native::journal_objects::{FAILED_REFILL_FLOOR, INSTALLATION_JOURNAL_DIR};
use filesquid_native::{HostLeases, HostMonitor, HostRoot, SqliteJournal, JOURNAL_FILE};
use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const LIBRARY: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";

/// A fresh scratch directory (data directory at `<dir>/data`), removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fs-desktop-sealing-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).expect("data dir");
        Self(dir)
    }

    fn data(&self) -> PathBuf {
        self.0.join("data")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("scratch directory not removed: {err}");
        }
    }
}

/// One launch's host monitor and installation over `data`.
fn launch(data: &Path) -> (HostMonitor, InstallationId) {
    let host = open_host_monitor(&ApiState::new(), data, None).expect("monitor");
    let installation = load_installation(data, &host).expect("installation");
    (host, installation)
}

/// Every entry of the journal in `data` (read after the monitor stopped).
fn journal_of(data: &Path) -> Vec<JournalEntry> {
    let store = SqliteJournal::open(&data.join(JOURNAL_FILE), "desktop").expect("journal");
    journal::all_entries(&store)
}

/// What the installation's journal object in `data` holds.
async fn sealed_in(data: &Path) -> Vec<JournalEntry> {
    let backend = FsBackend::new(
        data.join(INSTALLATION_JOURNAL_DIR),
        data.join("journal-work").join("root-parts"),
    )
    .expect("backend");
    read_sealed_entries(&backend, &OwnerReadable)
        .await
        .expect("segments")
}

/// What the installation's journal object in `data` holds, or nothing while a seal is still
/// committing (the object is read while the hourly seal may write it).
async fn settled_in(data: &Path) -> Vec<JournalEntry> {
    let Ok(backend) = FsBackend::new(
        data.join(INSTALLATION_JOURNAL_DIR),
        data.join("journal-work").join("root-parts"),
    ) else {
        return Vec::new();
    };
    read_sealed_entries(&backend, &OwnerReadable)
        .await
        .unwrap_or_default()
}

/// Remove the operational store of `data`, as an owner deleting it would.
fn delete_store(data: &Path) {
    for name in [JOURNAL_FILE, "filesquid.sqlite-wal", "filesquid.sqlite-shm"] {
        let _ = std::fs::remove_file(data.join(name));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_catches_up_a_missed_seal_and_refills_a_lost_store() {
    let scratch = Scratch::new("catch-up");
    let data = scratch.data();
    let (host, _) = launch(&data);
    host.monitor()
        .journal()
        .send(JournalEntry::new("library.create", Outcome::Succeeded).target("lib-opaque"));
    host.stop(); // a forced quit: no close-time seal
    let before: Vec<JournalEntry> = journal_of(&data);
    let (host, installation) = launch(&data);
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = tokio::runtime::Handle::current();
    launch_sealing(&host, &data, &installation, (&runtime, stop.clone(), None))
        .await
        .expect("sealing");
    stop.store(true, Ordering::SeqCst);
    host.stop();
    let sealed = sealed_in(&data).await;
    assert!(
        sealed.iter().any(|e| e.operation == "library.create"),
        "the missed seal was caught up: {sealed:?}"
    );
    let after = journal_of(&data);
    let seals: Vec<_> = after
        .iter()
        .filter(|e| e.operation == SEAL_OPERATION)
        .collect();
    assert_eq!(seals.len(), 1, "{after:?}");
    assert_eq!(seals[0].outcome, Outcome::Succeeded);
    assert!(
        after.iter().any(|e| e
            .process_id
            .as_deref()
            .is_some_and(|p| p.starts_with(SEAL_PROCESS_KIND))),
        "the run is a registered process"
    );
    delete_store(&data);
    let refill = refill_before_open(&data, true).await;
    let (host, installation) = launch(&data);
    launch_sealing(&host, &data, &installation, (&runtime, stop, refill))
        .await
        .expect("sealing after the loss");
    host.stop();
    let refilled = journal_of(&data);
    assert!(before.iter().any(|e| e.operation == "library.create"));
    for old in &before {
        let back = refilled.iter().find(|e| e.id == old.id);
        assert_eq!(
            back,
            Some(old),
            "entry {:?} comes back whole under its journal ID, and no new entry took it",
            old.id
        );
    }
    let refills: Vec<_> = refilled
        .iter()
        .filter(|e| e.operation == REFILL_OPERATION)
        .collect();
    assert_eq!(refills.len(), 1, "{refilled:?}");
    assert_eq!(refills[0].outcome, Outcome::Succeeded);
    let text = serde_json::to_string(&refilled).expect("json");
    assert!(
        !text.contains(data.to_string_lossy().as_ref()),
        "no path: {text}"
    );
}

/// A refill that fails leaves its marker, so the next launch refills again even though the
/// failed refill already recreated `filesquid.sqlite`; the marker goes once a refill succeeds.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_refill_runs_again_at_the_next_launch() {
    let scratch = Scratch::new("refill-retry");
    let data = scratch.data();
    let (host, installation) = launch(&data);
    host.monitor()
        .journal()
        .send(JournalEntry::new("library.create", Outcome::Succeeded).target("lib-opaque"));
    host.monitor().journal().flush();
    let sealing = start_sealing(&host, &data, &installation)
        .await
        .expect("sealing");
    sealing.service.seal_all().await;
    host.stop();
    let before = journal_of(&data);
    let inventory = data
        .join(INSTALLATION_JOURNAL_DIR)
        .join("journal")
        .join("inventory.json");
    let kept = std::fs::read(&inventory).expect("inventory");
    std::fs::write(&inventory, b"not an inventory").expect("corrupt");
    delete_store(&data);
    let failed = refill_before_open(&data, true).await;
    assert!(failed.is_some_and(|r| r.is_err()), "the refill fails");
    assert!(
        data.join(JOURNAL_FILE).exists(),
        "a new, incomplete store is left"
    );
    assert!(
        data.join(REFILL_PENDING_FILE).is_file(),
        "the refill is still pending"
    );
    let (host, _) = launch(&data);
    host.monitor()
        .journal()
        .send(JournalEntry::new("host.event", Outcome::Succeeded));
    host.stop(); // the launch goes on over the incomplete store
    let written = journal_of(&data);
    assert!(!written.is_empty());
    assert!(
        written.iter().all(|e| e.id >= Some(FAILED_REFILL_FLOOR)),
        "no entry takes a sealed ID after a failed refill: {written:?}"
    );
    std::fs::write(&inventory, kept).expect("repair");
    let retried = refill_before_open(&data, false).await;
    assert!(
        retried.is_some_and(|r| r.is_ok()),
        "the next launch refills again"
    );
    assert!(!data.join(REFILL_PENDING_FILE).exists(), "the marker goes");
    assert!(
        refill_before_open(&data, false).await.is_none(),
        "nothing more to refill"
    );
    let refilled = journal_of(&data);
    let old = before
        .iter()
        .find(|e| e.operation == "library.create")
        .expect("journaled");
    assert_eq!(refilled.iter().find(|e| e.id == old.id), Some(old));
    for entry in &written {
        let kept = refilled.iter().find(|e| e.id == entry.id);
        assert_eq!(kept, Some(entry), "the launch's own entries stay");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hourly_seal_runs_without_a_call_and_ends_with_the_app() {
    let scratch = Scratch::new("hourly");
    let data = scratch.data();
    let (host, installation) = launch(&data);
    let sealing = start_sealing(&host, &data, &installation)
        .await
        .expect("sealing");
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = tokio::runtime::Handle::current();
    let hourly = spawn_hourly(&runtime, sealing, stop.clone(), Duration::from_millis(200));
    host.monitor()
        .journal()
        .send(JournalEntry::new("host.event", Outcome::Succeeded));
    host.monitor().journal().flush();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut sealed = Vec::new();
    while Instant::now() < deadline
        && !sealed
            .iter()
            .any(|e: &JournalEntry| e.operation == "host.event")
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
        sealed = settled_in(&data).await;
    }
    assert!(
        sealed.iter().any(|e| e.operation == "host.event"),
        "{sealed:?}"
    );
    stop.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), hourly)
        .await
        .expect("the loop ends with the app")
        .expect("no panic");
    host.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_close_seal_seals_what_is_pending() {
    let scratch = Scratch::new("close");
    let data = scratch.data();
    let (host, installation) = launch(&data);
    let sealing = start_sealing(&host, &data, &installation)
        .await
        .expect("sealing");
    host.monitor()
        .journal()
        .send(JournalEntry::new("host.event", Outcome::Succeeded));
    host.monitor().journal().flush();
    let report = seal_before_close(&sealing, Duration::from_secs(30))
        .await
        .expect("finished in time");
    assert!(
        matches!(
            report.outcome("installation"),
            Some(SealOutcome::Sealed { .. })
        ),
        "{report:?}"
    );
    host.stop();
    assert!(sealed_in(&data)
        .await
        .iter()
        .any(|e| e.operation == "host.event"));
}

/// A new OCFL storage root at `<dir>/library` with its leases, as the host serves a folder.
async fn library_root(dir: &Path) -> HostRoot {
    let folder = dir.join("library");
    std::fs::create_dir_all(&folder).expect("folder");
    let backend = FsBackend::new(&folder, dir.join("library-parts")).expect("backend");
    create_storage_root(&backend).await.expect("root");
    let leases: HostLeases = LeaseStore::new(
        FsBackend::new(dir.join("leases"), dir.join("leases-parts")).expect("leases"),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("lease store");
    LibraryRoot {
        backend: Arc::new(backend),
        reservations: Arc::new(leases),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_served_library_is_sealed_into_its_own_root_and_restored_by_its_rebuild() {
    let scratch = Scratch::new("library");
    let data = scratch.data();
    let (host, installation) = launch(&data);
    let sealing = start_sealing(&host, &data, &installation)
        .await
        .expect("sealing");
    let root = library_root(&scratch.0).await;
    let library = LibraryId::new(LIBRARY).expect("library");
    assert!(open_library(&sealing, (&library, &root), &data));
    assert!(
        !open_library(&sealing, (&library, &root), &data),
        "opened once"
    );
    host.monitor()
        .journal()
        .send(JournalEntry::new("file.upload", Outcome::Succeeded).target(LIBRARY));
    host.monitor().journal().flush();
    let report = sealing.service.seal_all().await;
    let scope = format!("library:{LIBRARY}");
    assert!(
        matches!(report.outcome(&scope), Some(SealOutcome::Sealed { .. })),
        "{report:?}"
    );
    let in_library = read_sealed_entries(root.backend.as_ref(), &OwnerReadable)
        .await
        .expect("library segments");
    let sealed = in_library
        .iter()
        .find(|e| e.operation == "file.upload")
        .expect("sealed into the library's root")
        .clone();
    host.stop();
    delete_store(&data);
    assert!(refill_before_open(&data, true)
        .await
        .is_some_and(|r| r.is_ok()));
    let (host, installation) = launch(&data);
    let service = local_library_service(&host, &installation, Arc::new(MemoryRepairLedger::new()));
    let rebuilt = service
        .rebuild("owner", &library, root.backend.as_ref())
        .await
        .expect("rebuilt");
    let restored = rebuilt.journal.expect("the journal was restored");
    assert!(restored.restored >= 1, "{restored:?}");
    host.stop();
    let back = journal_of(&data);
    let entry = back
        .iter()
        .find(|e| e.operation == "file.upload")
        .expect("restored into the SQLite journal");
    assert_eq!(entry.id, sealed.id, "same journal ID");
}
