//! Steps of `desktop_journal_sealing_F-061.feature` (Task DE-0.11; DE-S-013, SRS-F-061): the
//! desktop's journal sealing through the real launch functions (`open_host_monitor`,
//! `load_installation`, `refill_before_open`, `launch_sealing`, `start_sealing`,
//! `spawn_hourly`, `seal_before_close`, `open_library`, `local_library_service`), against
//! SQLite files and OCFL storage roots in a scratch directory removed when the scenario ends.

use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::canonical::{create_storage_root, OwnerReadable};
use filesquid_api::journal::seal::{read_sealed_entries, SEAL_OPERATION, SEAL_PROCESS_KIND};
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::local_library::{LibraryRoot, MemoryRepairLedger};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId};
use filesquid_desktop_lib::sealing::{
    launch_sealing, open_library, refill_before_open, seal_before_close, spawn_hourly,
    start_sealing, DesktopSealing, REFILL_OPERATION,
};
use filesquid_desktop_lib::{load_installation, local_library_service, open_host_monitor};
use filesquid_native::journal_objects::INSTALLATION_JOURNAL_DIR;
use filesquid_native::{HostLeases, HostMonitor, HostRoot, SqliteJournal, JOURNAL_FILE};
use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The library of the library scenario.
const LIBRARY: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";

/// Scenario counter, so concurrent scenarios get separate scratch directories.
static SCENARIOS: AtomicU32 = AtomicU32::new(0);

/// What one sealing scenario set up and saw.
#[derive(Default)]
pub struct SealingScenario {
    dir: Option<PathBuf>,
    host: Option<HostMonitor>,
    sealing: Option<DesktopSealing>,
    stop: Option<Arc<AtomicBool>>,
    hourly: Option<tokio::task::JoinHandle<()>>,
    first_launch: Vec<JournalEntry>,
    root: Option<HostRoot>,
    sealed_upload: Option<JournalEntry>,
}

impl std::fmt::Debug for SealingScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealingScenario")
            .field("dir", &self.dir)
            .finish()
    }
}

impl Drop for SealingScenario {
    fn drop(&mut self) {
        if let Some(stop) = &self.stop {
            stop.store(true, Ordering::SeqCst);
        }
        if let Some(host) = self.host.take() {
            host.stop();
        }
        let Some(dir) = &self.dir else { return };
        if let Err(err) = std::fs::remove_dir_all(dir) {
            eprintln!("scenario directory not removed: {err}");
        }
    }
}

/// The scenario's state.
fn scenario(world: &mut DesktopWorld) -> &mut SealingScenario {
    world.sealing.get_or_insert_with(SealingScenario::default)
}

/// The scenario's scratch directory (data directory at `<dir>/data`).
fn dir(world: &mut DesktopWorld) -> PathBuf {
    scenario(world)
        .dir
        .get_or_insert_with(|| {
            let n = SCENARIOS.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir()
                .join(format!("fs-desktop-sealing-bdd-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("data")).expect("data dir");
            dir
        })
        .clone()
}

/// The data directory.
fn data(world: &mut DesktopWorld) -> PathBuf {
    dir(world).join("data")
}

/// Open the host monitor and read the installation, as a launch does.
fn open(data: &Path) -> (HostMonitor, InstallationId) {
    let host = open_host_monitor(&ApiState::new(), data, None).expect("monitor");
    let installation = load_installation(data, &host).expect("installation");
    (host, installation)
}

/// Every entry of the journal in `data` (read after the monitor stopped).
fn journal_of(data: &Path) -> Vec<JournalEntry> {
    let store = SqliteJournal::open(&data.join(JOURNAL_FILE), "desktop").expect("journal");
    crate::journal::all_entries(&store)
}

/// What the installation's journal object holds; nothing while a seal is still committing.
async fn sealed_in(data: &Path) -> Vec<JournalEntry> {
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

/// Stop the scenario's host, as an app exit or a forced quit does.
fn stop_host(world: &mut DesktopWorld) {
    let state = scenario(world);
    if let Some(stop) = &state.stop {
        stop.store(true, Ordering::SeqCst);
    }
    if let Some(host) = state.host.take() {
        host.stop();
    }
}

/// A full launch: refill (when `store_lost`), monitor, installation and the launch's sealing.
async fn full_launch(world: &mut DesktopWorld, store_lost: bool) {
    let data = data(world);
    let refill = refill_before_open(&data, store_lost).await;
    let (host, installation) = open(&data);
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = tokio::runtime::Handle::current();
    let sealing = launch_sealing(
        &host,
        &data,
        &installation,
        (&runtime, stop.clone(), refill),
    )
    .await
    .expect("sealing");
    let state = scenario(world);
    state.host = Some(host);
    state.sealing = Some(sealing);
    state.stop = Some(stop);
}

/// Start sealing on the scenario's running host.
async fn sealing_on_running_host(world: &mut DesktopWorld) {
    let data = data(world);
    let (host, installation) = open(&data);
    let sealing = start_sealing(&host, &data, &installation)
        .await
        .expect("sealing");
    let state = scenario(world);
    state.host = Some(host);
    state.sealing = Some(sealing);
}

/// Journal `entry` through the running host and wait until it is stored.
fn journal(world: &mut DesktopWorld, entry: JournalEntry) {
    let host = scenario(world).host.as_ref().expect("a running host");
    host.monitor().journal().send(entry);
    host.monitor().journal().flush();
}

#[given("a desktop whose sealing runs on a short hourly interval")]
async fn short_hourly(world: &mut DesktopWorld) {
    sealing_on_running_host(world).await;
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = tokio::runtime::Handle::current();
    let state = scenario(world);
    let sealing = state.sealing.clone().expect("sealing");
    state.hourly = Some(spawn_hourly(
        &runtime,
        sealing,
        stop.clone(),
        Duration::from_millis(200),
    ));
    state.stop = Some(stop);
}

#[given("a running desktop with its sealing started")]
async fn running(world: &mut DesktopWorld) {
    sealing_on_running_host(world).await;
}

#[given("a desktop that journaled a library creation and was forced to quit without sealing")]
fn forced_quit(world: &mut DesktopWorld) {
    let data = data(world);
    let (host, _) = open(&data);
    scenario(world).host = Some(host);
    journal(
        world,
        JournalEntry::new("library.create", Outcome::Succeeded).target("lib-opaque"),
    );
    stop_host(world);
    scenario(world).first_launch = journal_of(&data);
}

#[given("the desktop started again, which caught up the seal")]
#[when("the desktop starts again")]
async fn starts_again(world: &mut DesktopWorld) {
    full_launch(world, false).await;
    stop_host(world);
}

#[given("a running desktop serving a library from a folder the owner chose")]
async fn serving_library(world: &mut DesktopWorld) {
    sealing_on_running_host(world).await;
    let dir = dir(world);
    let root = library_root(&dir).await;
    let data = data(world);
    let state = scenario(world);
    let library = LibraryId::new(LIBRARY).expect("library");
    let sealing = state.sealing.as_ref().expect("sealing");
    assert!(open_library(sealing, (&library, &root), &data));
    state.root = Some(root);
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

#[when("a host event is journaled")]
fn host_event(world: &mut DesktopWorld) {
    journal(world, JournalEntry::new("host.event", Outcome::Succeeded));
}

#[when("a host event is journaled and the app closes")]
async fn event_and_close(world: &mut DesktopWorld) {
    journal(world, JournalEntry::new("host.event", Outcome::Succeeded));
    let sealing = scenario(world).sealing.clone().expect("sealing");
    let report = seal_before_close(&sealing, Duration::from_secs(30)).await;
    assert!(report.is_some_and(|r| r.all_sealed()));
    stop_host(world);
}

#[when("the operational store is deleted and the desktop starts again")]
async fn store_deleted(world: &mut DesktopWorld) {
    let data = data(world);
    for name in [JOURNAL_FILE, "filesquid.sqlite-wal", "filesquid.sqlite-shm"] {
        let _ = std::fs::remove_file(data.join(name));
    }
    full_launch(world, true).await;
    stop_host(world);
}

#[when("an upload into that library is journaled and the journal is sealed")]
async fn library_upload(world: &mut DesktopWorld) {
    journal(
        world,
        JournalEntry::new("file.upload", Outcome::Succeeded).target(LIBRARY),
    );
    let sealing = scenario(world).sealing.clone().expect("sealing");
    let report = sealing.service.seal_all().await;
    assert!(report.all_sealed(), "{report:?}");
}

#[then("it should be sealed into the installation's journal object without being asked")]
async fn sealed_unasked(world: &mut DesktopWorld) {
    let data = data(world);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut sealed = sealed_in(&data).await;
    while Instant::now() < deadline && !sealed.iter().any(|e| e.operation == "host.event") {
        tokio::time::sleep(Duration::from_millis(100)).await;
        sealed = sealed_in(&data).await;
    }
    assert!(
        sealed.iter().any(|e| e.operation == "host.event"),
        "{sealed:?}"
    );
}

#[then("the hourly seal should end when the app exits")]
async fn hourly_ends(world: &mut DesktopWorld) {
    let state = scenario(world);
    if let Some(stop) = &state.stop {
        stop.store(true, Ordering::SeqCst);
    }
    let hourly = state.hourly.take().expect("the hourly seal");
    tokio::time::timeout(Duration::from_secs(5), hourly)
        .await
        .expect("it ends with the app")
        .expect("no panic");
}

#[then("the library creation should be sealed into the installation's journal object")]
#[then("the close-time seal should have sealed it into the installation's journal object")]
async fn sealed_in_installation(world: &mut DesktopWorld) {
    let data = data(world);
    let sealed = sealed_in(&data).await;
    let expected = ["host.event", "library.create"];
    assert!(
        sealed
            .iter()
            .any(|e| expected.contains(&e.operation.as_str())),
        "{sealed:?}"
    );
}

#[then(expr = "the journal should record one {string} run that succeeded, as a registered process")]
fn one_seal_run(world: &mut DesktopWorld, operation: String) {
    assert_eq!(operation, SEAL_OPERATION);
    let entries = journal_of(&data(world));
    let runs: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(runs.len(), 1, "{entries:?}");
    assert_eq!(runs[0].outcome, Outcome::Succeeded);
    let registered = entries.iter().any(|e| {
        e.process_id
            .as_deref()
            .is_some_and(|p| p.starts_with(SEAL_PROCESS_KIND))
    });
    assert!(registered, "{entries:?}");
}

#[then("every entry of the first launch should be back under its own journal ID")]
fn back_under_ids(world: &mut DesktopWorld) {
    let entries = journal_of(&data(world));
    let first = scenario(world).first_launch.clone();
    assert!(!first.is_empty());
    for old in &first {
        // Both sides were read back from the store, so redaction and the host default are
        // applied to each: the whole entry must come back, not only its ID.
        let back = entries.iter().find(|e| e.id == old.id);
        assert_eq!(back, Some(old), "{:?}", old.id);
    }
}

#[then(expr = "the journal should record {string} succeeded once")]
fn recorded_once(world: &mut DesktopWorld, operation: String) {
    assert_eq!(operation, REFILL_OPERATION);
    let entries = journal_of(&data(world));
    let found: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(found.len(), 1, "{entries:?}");
    assert_eq!(found[0].outcome, Outcome::Succeeded);
}

#[then("no journal entry should name the data directory")]
fn names_no_path(world: &mut DesktopWorld) {
    let dir = dir(world);
    let text = serde_json::to_string(&journal_of(&dir.join("data"))).expect("json");
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
}

#[then("the upload should be sealed into the library's own storage root")]
async fn sealed_in_library(world: &mut DesktopWorld) {
    let root = scenario(world).root.clone().expect("the library root");
    let sealed = read_sealed_entries(root.backend.as_ref(), &OwnerReadable)
        .await
        .expect("library segments");
    let upload = sealed
        .iter()
        .find(|e| e.operation == "file.upload")
        .cloned()
        .expect("sealed into the library's root");
    scenario(world).sealed_upload = Some(upload);
}

#[then("after the operational store is lost, the library's rebuild should restore it under its journal ID")]
async fn rebuild_restores(world: &mut DesktopWorld) {
    stop_host(world);
    let data = data(world);
    for name in [JOURNAL_FILE, "filesquid.sqlite-wal", "filesquid.sqlite-shm"] {
        let _ = std::fs::remove_file(data.join(name));
    }
    assert!(refill_before_open(&data, true)
        .await
        .is_some_and(|r| r.is_ok()));
    let (host, installation) = open(&data);
    let service = local_library_service(&host, &installation, Arc::new(MemoryRepairLedger::new()));
    let root = scenario(world).root.clone().expect("the library root");
    let library = LibraryId::new(LIBRARY).expect("library");
    let rebuilt = service
        .rebuild("owner", &library, root.backend.as_ref())
        .await
        .expect("rebuilt");
    assert!(rebuilt.journal.is_some_and(|j| j.restored >= 1));
    host.stop();
    let upload = scenario(world)
        .sealed_upload
        .clone()
        .expect("the sealed upload");
    let back = journal_of(&data);
    let entry = back
        .iter()
        .find(|e| e.operation == "file.upload")
        .expect("restored");
    assert_eq!(entry.id, upload.id);
}
