//! Cucumber harness for the `filesquid-native` Gherkin under `tests/features/` (F-061: the
//! SQLite journal store and the data-directory checks that the hosts inject into
//! `filesquid-api`). Writes JUnit to `tests/allure-results/cucumber-junit.xml`.

use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::health::{HealthReport, HealthStatus};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::monitor::{HostChecks, Monitor, MonitorConfig};
use filesquid_api::{ApiState, EventBus};
use filesquid_native::{DataDirChecks, HostMonitor, SqliteJournal, JOURNAL_FILE};
use log_names::log_files_under;
use std::path::PathBuf;
use std::sync::Arc;

#[path = "support/log_names.rs"]
#[allow(dead_code)]
mod log_names;

/// State shared by the steps of one scenario.
#[derive(Debug, Default, World)]
struct NativeWorld {
    dir: Option<PathBuf>,
    operations: Vec<String>,
    writable: Option<bool>,
    health: Option<HealthReport>,
}

/// Fresh temporary directory named after `name` (I/O).
fn fresh_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fs-native-features-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

#[given(expr = "a host data directory with the SQLite journal store")]
fn store_dir(w: &mut NativeWorld) {
    w.dir = Some(fresh_dir("nolog"));
}

#[when(expr = "an import is journaled")]
fn journal_import(w: &mut NativeWorld) {
    let dir = w.dir.as_ref().expect("dir");
    let store = SqliteJournal::open(&dir.join(JOURNAL_FILE), "host-a").expect("open");
    store
        .append(&[JournalEntry::new("import", Outcome::Succeeded)])
        .expect("append");
}

#[then(expr = "no log file should exist anywhere in the host's data directory")]
fn no_log_files(w: &mut NativeWorld) {
    let dir = w.dir.take().expect("dir");
    let logs = log_files_under(&dir).expect("walk data directory");
    assert!(logs.is_empty(), "log files: {logs:?}");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[given(expr = "an empty host data directory")]
fn empty_dir(w: &mut NativeWorld) {
    w.dir = Some(fresh_dir("host").join("data"));
}

#[when(expr = "the host monitor opens and stops")]
fn open_and_stop(w: &mut NativeWorld) {
    let dir = w.dir.as_ref().expect("dir");
    let host = HostMonitor::open(dir, "host-a", &ApiState::new(), None).expect("open");
    host.stop();
    let page = host
        .monitor()
        .store()
        .query(&JournalQuery::default())
        .expect("query");
    w.operations = page.entries.into_iter().map(|e| e.operation).collect();
}

#[then(expr = "the data directory should hold the SQLite operational store")]
fn holds_store(w: &mut NativeWorld) {
    assert!(w.dir.as_ref().expect("dir").join(JOURNAL_FILE).exists());
}

#[then(expr = "the journal should record the host starting and stopping")]
fn started_and_stopped(w: &mut NativeWorld) {
    assert!(w.operations.iter().any(|o| o == "host.started"));
    assert!(w.operations.iter().any(|o| o == "host.stopped"));
    let dir = w.dir.take().expect("dir");
    std::fs::remove_dir_all(dir.parent().expect("parent")).expect("cleanup");
}

#[given(expr = "a host data directory that does not exist")]
fn missing_dir(w: &mut NativeWorld) {
    w.dir = Some(fresh_dir("missing").join("absent"));
}

#[when(expr = "the data-directory checks run in the host's health report")]
fn run_checks(w: &mut NativeWorld) {
    let checks = Arc::new(DataDirChecks::new(w.dir.as_ref().expect("dir")));
    w.writable = Some(checks.data_dir_writable());
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        checks,
        MonitorConfig::new("host-a"),
        EventBus::new(),
    );
    w.health = Some(monitor.health_report());
    monitor.shutdown();
}

#[then(expr = "the data directory should be reported as not writable")]
fn not_writable(w: &mut NativeWorld) {
    assert_eq!(w.writable, Some(false));
}

#[then(expr = "health should be failing with the reason {string}")]
fn health_failing(w: &mut NativeWorld, reason: String) {
    let report = w.health.take().expect("health report");
    assert_eq!(report.status, HealthStatus::Failing);
    assert!(
        report.reasons.iter().any(|r| r.code == reason),
        "{report:?}"
    );
    let dir = w.dir.take().expect("dir");
    std::fs::remove_dir_all(dir.parent().expect("parent")).expect("cleanup");
}

#[tokio::main]
async fn main() {
    let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    let _ = std::fs::remove_dir_all(&results);
    std::fs::create_dir_all(&results).expect("allure-results");
    let junit = std::fs::File::create(results.join("cucumber-junit.xml")).expect("junit file");
    NativeWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<NativeWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .fail_on_skipped()
        .run_and_exit(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features"))
        .await;
}
