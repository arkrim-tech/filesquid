//! The desktop's journal objects (Task DE-0.11; DE-S-013, API-S-032, ADR 033 Decision 7): the
//! installation's journal object for host events at `<data dir>/journal.ocfl/`, a library's
//! journal object in that library's own storage root, and the refill of a lost operational
//! store from the installation's sealed segments, de-duplicated by journal ID.

use filesquid_api::canonical::OwnerReadable;
use filesquid_api::journal::seal::{read_sealed_entries, SealConfig, SealOutcome, SealService};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::InstallationId;
use filesquid_native::journal_objects::{
    open_installation_journal, restore_installation_journal, INSTALLATION_JOURNAL_DIR,
};
use filesquid_native::{SqliteJournal, JOURNAL_FILE};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const INSTALLATION: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A fresh data directory, removed when dropped.
struct DataDir(PathBuf);

impl DataDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "fs-native-journal-objects-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("data dir");
        Self(dir)
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove the test directory: {err}");
        }
    }
}

fn installation() -> InstallationId {
    InstallationId::parse(INSTALLATION).expect("installation")
}

/// A monitor over the SQLite journal store of `dir`.
fn monitor(dir: &Path) -> (Monitor, Arc<SqliteJournal>) {
    let store = Arc::new(SqliteJournal::open(&dir.join(JOURNAL_FILE), "desktop").expect("store"));
    let monitor = Monitor::start(
        store.clone(),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        EventBus::new(),
    );
    (monitor, store)
}

/// Every entry of `store`, oldest first (bounded: the store is finite and each page advances).
fn all(store: &dyn JournalStore) -> Vec<JournalEntry> {
    let mut query = JournalQuery::default();
    let mut entries = Vec::new();
    loop {
        let page = store.query(&query).expect("query");
        entries.extend(page.entries);
        match page.next_cursor {
            Some(cursor) => query.after_id = Some(cursor),
            None => return entries,
        }
    }
}

/// The installation's journal storage root in `dir`.
fn journal_root(dir: &Path) -> FsBackend {
    FsBackend::new(
        dir.join(INSTALLATION_JOURNAL_DIR),
        dir.join("journal-root-parts"),
    )
    .expect("backend")
}

#[tokio::test]
async fn host_events_are_sealed_into_the_installation_object_in_the_data_directory() {
    let dir = DataDir::new("installation");
    let (monitor, _store) = monitor(&dir.0);
    let object = open_installation_journal(&dir.0, &installation())
        .await
        .expect("installation journal");
    assert!(dir
        .0
        .join(INSTALLATION_JOURNAL_DIR)
        .join("0=ocfl_1.1")
        .is_file());
    monitor
        .journal()
        .send(JournalEntry::new("host.started", Outcome::Succeeded));
    monitor.journal().flush();
    let service = SealService::new(monitor.clone(), object, SealConfig::default());
    let report = service.seal_all().await;
    assert!(
        matches!(
            report.outcome("installation"),
            Some(SealOutcome::Sealed { .. })
        ),
        "{report:?}"
    );
    let sealed = read_sealed_entries(&journal_root(&dir.0), &OwnerReadable)
        .await
        .expect("segments");
    assert!(
        sealed.iter().any(|e| e.operation == "host.started"),
        "{sealed:?}"
    );
    monitor.shutdown();
    let again = open_installation_journal(&dir.0, &installation()).await;
    assert!(again.is_ok(), "a second launch reopens the same object");
}

#[tokio::test]
async fn a_lost_store_is_refilled_from_the_sealed_segments_once_under_the_same_ids() {
    let dir = DataDir::new("refill");
    let (monitor, store) = monitor(&dir.0);
    let object = open_installation_journal(&dir.0, &installation())
        .await
        .expect("installation journal");
    for operation in ["host.started", "host.installation.created"] {
        monitor
            .journal()
            .send(JournalEntry::new(operation, Outcome::Succeeded));
    }
    monitor.journal().flush();
    SealService::new(monitor.clone(), object, SealConfig::default())
        .seal_all()
        .await;
    monitor.journal().flush();
    let before = all(store.as_ref());
    monitor.shutdown();
    drop(monitor);
    drop(store);
    for name in [JOURNAL_FILE, "filesquid.sqlite-wal", "filesquid.sqlite-shm"] {
        let _ = std::fs::remove_file(dir.0.join(name));
    }
    let fresh = SqliteJournal::open(&dir.0.join(JOURNAL_FILE), "desktop").expect("fresh store");
    let restored = restore_installation_journal(&dir.0, &fresh)
        .await
        .expect("restored");
    assert!(restored.restored >= 2, "{restored:?}");
    let after = all(&fresh);
    for operation in ["host.started", "host.installation.created"] {
        let old = before
            .iter()
            .find(|e| e.operation == operation)
            .expect("old");
        let new = after
            .iter()
            .find(|e| e.operation == operation)
            .expect("restored");
        assert_eq!(old.id, new.id, "{operation} keeps its journal ID");
    }
    let again = restore_installation_journal(&dir.0, &fresh)
        .await
        .expect("again");
    assert_eq!(again.restored, 0, "{again:?}");
    assert_eq!(all(&fresh).len(), after.len(), "nothing stored twice");
}

#[tokio::test]
async fn a_data_directory_without_a_journal_object_restores_nothing() {
    let dir = DataDir::new("none");
    let store = SqliteJournal::open(&dir.0.join(JOURNAL_FILE), "desktop").expect("store");
    let restored = restore_installation_journal(&dir.0, &store)
        .await
        .expect("nothing to restore");
    assert_eq!(restored.restored, 0);
    assert!(
        !dir.0.join(INSTALLATION_JOURNAL_DIR).exists(),
        "nothing is created"
    );
}
