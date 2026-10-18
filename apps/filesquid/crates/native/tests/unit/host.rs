//! Unit tests for the data-directory host checks and [`HostMonitor`] (SRS-F-061 Health and
//! Journal store). The write-probe test moved here from `filesquid-api` with the probe
//! (Task CL-0.1).

use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{HostChecks, Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::ApiState;
use filesquid_native::{
    probe_name, DataDirChecks, HostMonitor, ALREADY_INSTALLED, JOURNAL_FILE, MAX_PROBE_ATTEMPTS,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Fresh temporary directory for one test (removed by the caller).
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Probe names seeded by the collision test: fewer than [`MAX_PROBE_ATTEMPTS`], and covering
/// every sequence number this test binary's other probes can have used before it.
const SEEDED_PROBES: u64 = 16;

#[test]
fn the_write_probe_never_touches_an_existing_file() {
    assert!((SEEDED_PROBES as usize) < MAX_PROBE_ATTEMPTS);
    let dir = temp_dir("probe");
    for sequence in 0..SEEDED_PROBES {
        std::fs::write(dir.join(probe_name(sequence)), b"keep me").unwrap();
    }
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
    for sequence in 0..SEEDED_PROBES {
        assert_eq!(
            std::fs::read(dir.join(probe_name(sequence))).unwrap(),
            b"keep me"
        );
    }
    let left = std::fs::read_dir(&dir).unwrap().count() as u64;
    assert_eq!(left, SEEDED_PROBES, "the probe removes only its own file");
    monitor.shutdown();
    drop(monitor);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_data_dir_is_not_writable_and_has_no_free_space_reading() {
    let parent = temp_dir("gone");
    let checks = DataDirChecks::new(parent.join("missing"));
    assert!(!checks.data_dir_writable());
    assert_eq!(checks.free_bytes(), None);
    std::fs::remove_dir_all(parent).unwrap();
}

#[test]
fn a_second_host_monitor_is_refused_before_any_file_is_created() {
    let parent = temp_dir("second-open");
    let state = ApiState::new();
    let first = HostMonitor::open(&parent.join("a"), "host-a", &state, None).expect("first");
    let second = HostMonitor::open(&parent.join("b"), "host-a", &state, None);
    assert_eq!(second.err().as_deref(), Some(ALREADY_INSTALLED));
    assert!(
        !parent.join("b").exists(),
        "no data directory for the refused monitor"
    );
    first.stop();
    std::fs::remove_dir_all(parent).unwrap();
}

#[test]
fn a_second_started_monitor_is_shut_down_and_never_installed() {
    let state = ApiState::new();
    let installed = Arc::new(MemoryJournal::new("host-a"));
    let extra = Arc::new(MemoryJournal::new("host-a"));
    let checks = Arc::new(StaticHostChecks::healthy());
    let first = HostMonitor::start(installed.clone(), checks.clone(), "host-a", &state, None)
        .expect("first");
    let second = HostMonitor::start(extra.clone(), checks, "host-a", &state, None);
    assert_eq!(second.err().as_deref(), Some(ALREADY_INSTALLED));
    let none = extra.count(&JournalQuery::default()).expect("count");
    assert_eq!(none, 0, "the refused monitor journals nothing");
    first.stop();
    let ops: Vec<String> = state
        .monitor()
        .expect("installed")
        .store()
        .query(&JournalQuery::default())
        .expect("query")
        .entries
        .into_iter()
        .map(|e| e.operation)
        .collect();
    assert!(ops.iter().any(|o| o == "host.started"), "{ops:?}");
    let first_count = installed.count(&JournalQuery::default()).expect("count");
    assert_eq!(
        first_count,
        ops.len() as u64,
        "the installed monitor is the first one"
    );
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
