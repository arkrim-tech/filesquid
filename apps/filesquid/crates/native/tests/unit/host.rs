//! Unit tests for the data-directory host checks and [`HostMonitor`] (SRS-F-061 Health and
//! Journal store). The write-probe test moved here from `filesquid-api` with the probe
//! (Task CL-0.1).

use filesquid_api::journal::{JournalQuery, MemoryJournal};
use filesquid_api::monitor::{HostChecks, Monitor, MonitorConfig};
use filesquid_api::ApiState;
use filesquid_native::{DataDirChecks, HostMonitor, JOURNAL_FILE};
use std::path::PathBuf;
use std::sync::Arc;

/// Fresh temporary directory for one test (removed by the caller).
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn the_write_probe_never_touches_an_existing_file() {
    let dir = temp_dir("probe");
    std::fs::write(dir.join(".filesquid-write-probe"), b"keep me").unwrap();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        Arc::new(DataDirChecks::new(&dir)),
        MonitorConfig::new("host-a"),
        filesquid_api::EventBus::new(),
    );
    let report = monitor.health_report();
    assert!(!report
        .reasons
        .iter()
        .any(|r| r.code == "data_dir_not_writable"));
    assert_eq!(
        std::fs::read(dir.join(".filesquid-write-probe")).unwrap(),
        b"keep me"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    monitor.shutdown();
    drop(monitor);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_data_dir_is_not_writable_and_has_no_free_space_reading() {
    let checks = DataDirChecks::new(temp_dir("gone").join("missing"));
    assert!(!checks.data_dir_writable());
    assert_eq!(checks.free_bytes(), None);
}

#[test]
fn an_existing_data_dir_reports_free_space() {
    let dir = temp_dir("free");
    let checks = DataDirChecks::new(&dir);
    assert!(checks.data_dir_writable());
    assert!(checks.free_bytes().is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn host_monitor_opens_the_store_and_journals_start_and_stop() {
    let dir = temp_dir("host").join("data");
    let state = ApiState::new();
    let host = HostMonitor::open(&dir, "host-a", &state, None).expect("open");
    host.tick();
    host.stop();
    host.tick();
    assert!(dir.join(JOURNAL_FILE).exists());
    let entries = host
        .monitor()
        .store()
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    let ops: Vec<_> = entries.iter().map(|e| e.operation.as_str()).collect();
    assert!(ops.contains(&"host.started"), "{ops:?}");
    assert!(ops.contains(&"host.stopped"), "{ops:?}");
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}
