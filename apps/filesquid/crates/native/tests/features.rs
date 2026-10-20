//! Cucumber harness for the `filesquid-native` Gherkin under `tests/features/` (F-061: the
//! SQLite journal store and the data-directory checks that the hosts inject into
//! `filesquid-api`; NFR-003: the SQLite job store under the API's durable jobs; TC-005: which
//! packages may link `filesquid-native`). Writes JUnit to `tests/allure-results/cucumber-junit.xml`.

use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::health::{HealthReport, HealthStatus};
use filesquid_api::jobs::{JobError, JobSpec, JobState, JobStore, Jobs, RunningJob, JOB_RECOVERED};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::monitor::{HostChecks, Monitor, MonitorConfig};
use filesquid_api::{ApiState, EventBus};
use filesquid_native::{DataDirChecks, HostMonitor, SqliteJobs, SqliteJournal, JOURNAL_FILE};
use log_names::log_files_under;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
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
    jobs: Option<JobRig>,
    links: Vec<(String, String, Vec<String>)>,
}

/// Number of the next job rig, so concurrent scenarios never share a data directory.
static NEXT_RIG: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Lease length of the job scenarios.
const JOB_TTL_MS: i64 = 1_000;

/// One host's durable jobs over the SQLite store in a scenario's data directory: the
/// service of the current launch, its in-memory journal and monitor, a hand-moved clock,
/// the job and the run in progress.
struct JobRig {
    dir: PathBuf,
    journal: Arc<MemoryJournal>,
    monitor: Monitor,
    clock: Arc<AtomicI64>,
    job_id: String,
    running: Option<RunningJob>,
}

impl std::fmt::Debug for JobRig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRig")
            .field("job_id", &self.job_id)
            .finish_non_exhaustive()
    }
}

impl JobRig {
    /// A rig over a fresh data directory (filesystem I/O).
    fn new() -> Self {
        let journal = Arc::new(MemoryJournal::new("host-a"));
        let monitor = Monitor::start(
            journal.clone(),
            Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            EventBus::new(),
        );
        Self {
            dir: fresh_dir(&format!("jobs-{}", NEXT_RIG.fetch_add(1, Ordering::SeqCst))),
            journal,
            monitor,
            clock: Arc::new(AtomicI64::new(1_760_000_000_000)),
            job_id: String::new(),
            running: None,
        }
    }

    /// The job service of the launch `holder` over a newly opened store (database I/O).
    fn launch(&self, holder: &str) -> Jobs {
        let store = SqliteJobs::open(&self.dir.join(JOURNAL_FILE)).expect("job store");
        let clock = self.clock.clone();
        Jobs::new(
            Arc::new(store),
            self.monitor.journal(),
            self.monitor.registry().clone(),
            holder,
        )
        .with_clock(Arc::new(move || clock.load(Ordering::SeqCst)))
        .with_lease_ttl(JOB_TTL_MS)
    }

    /// Submit a job for `source` and start it in the launch `holder`.
    fn start_new(&mut self, holder: &str, source: &str) -> RunningJob {
        let jobs = self.launch(holder);
        let spec = JobSpec {
            kind: "import".into(),
            operation_id: "op-1".into(),
            source: source.into(),
        };
        self.job_id = jobs.submit(spec).expect("submitted").id;
        jobs.start(&self.job_id, source).expect("started")
    }

    /// The stored job (database I/O).
    fn stored(&self) -> filesquid_api::jobs::Job {
        let store = SqliteJobs::open(&self.dir.join(JOURNAL_FILE)).expect("job store");
        store.get(&self.job_id).expect("read").expect("stored")
    }

    /// Move the clock past the lease.
    fn expire(&self) {
        self.clock.fetch_add(JOB_TTL_MS + 1, Ordering::SeqCst);
    }

    /// Stop the monitor and remove the data directory.
    fn finish(self) {
        self.monitor.shutdown();
        std::fs::remove_dir_all(&self.dir).expect("cleanup");
    }
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

#[given(expr = "a host data directory with the SQLite job store")]
fn job_store(w: &mut NativeWorld) {
    w.jobs = Some(JobRig::new());
}

#[given(expr = "a running job that checkpointed {string} as done")]
fn running_with_checkpoint(w: &mut NativeWorld, done: String) {
    let rig = w.jobs.as_mut().expect("rig");
    let mut running = rig.start_new("launch-1", "s");
    running.checkpoint(&done, 1, Some(2)).expect("checkpoint");
    rig.running = Some(running);
}

#[when(expr = "the app quits")]
fn app_quits(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    rig.running
        .take()
        .expect("running")
        .suspend()
        .expect("suspended");
}

#[when(expr = "the host starts again")]
fn host_starts_again(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    let jobs = rig.launch("launch-2");
    let recovered = jobs.recover().expect("recovered");
    assert_eq!(recovered.len(), 1);
    rig.running = Some(jobs.start(&rig.job_id, "s").expect("resumed"));
}

#[then(expr = "the job should be resumable with the checkpoint {string}")]
fn resumed_with(w: &mut NativeWorld, done: String) {
    let mut rig = w.jobs.take().expect("rig");
    let running = rig.running.take().expect("running");
    assert_eq!(running.checkpoint_text(), Some(done.as_str()));
    running.complete().expect("completed");
    assert_eq!(rig.stored().state, JobState::Completed);
    rig.finish();
}

#[given(expr = "a running job whose host crashed")]
fn crashed_job(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    let running = rig.start_new("launch-1", "s");
    drop(running);
}

#[when(expr = "the lease expires and the host recovers its jobs")]
fn recover_after_expiry(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    rig.expire();
    rig.launch("launch-2").recover().expect("recovered");
}

#[then(expr = "the job should be pending again")]
fn pending_again(w: &mut NativeWorld) {
    let rig = w.jobs.as_ref().expect("rig");
    let job = rig.stored();
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.holder, None);
}

#[then(expr = "the journal should record the job as recovered")]
fn journaled_recovered(w: &mut NativeWorld) {
    let rig = w.jobs.take().expect("rig");
    rig.monitor.journal().flush();
    let page = rig
        .journal
        .query(&JournalQuery::default())
        .expect("journal");
    assert!(page.entries.iter().any(|e| e.operation == JOB_RECOVERED));
    rig.finish();
}

#[given(expr = "a running job whose lease expired")]
fn expired_job(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    let running = rig.start_new("launch-1", "s");
    rig.running = Some(running);
    rig.expire();
}

#[when(expr = "another worker takes the job over")]
fn take_over(w: &mut NativeWorld) {
    let rig = w.jobs.as_mut().expect("rig");
    let replacement = rig
        .launch("launch-2")
        .start(&rig.job_id, "s")
        .expect("taken over");
    drop(replacement);
}

#[then(expr = "a checkpoint from the expired worker should be refused as {string}")]
fn stale_refused(w: &mut NativeWorld, code: String) {
    let mut rig = w.jobs.take().expect("rig");
    let mut stale = rig.running.take().expect("running");
    let err = stale.checkpoint("{}", 1, None).expect_err("refused");
    assert_eq!(err, JobError::Fenced);
    assert_eq!(err.error_code(), code);
    assert_eq!(rig.stored().checkpoint, None, "nothing stored");
    rig.finish();
}

#[given(expr = "a job suspended at quit for the source {string}")]
fn suspended_for(w: &mut NativeWorld, source: String) {
    let rig = w.jobs.as_mut().expect("rig");
    let running = rig.start_new("launch-1", &source);
    running.suspend().expect("suspended");
}

#[when(expr = "the host starts it for the source {string}")]
fn start_for(w: &mut NativeWorld, source: String) {
    let rig = w.jobs.as_mut().expect("rig");
    let started = rig.launch("launch-2").start(&rig.job_id, &source);
    assert_eq!(started.err(), Some(JobError::SourceChanged));
}

#[then(expr = "the job should fail with {string}")]
fn failed_with(w: &mut NativeWorld, code: String) {
    let rig = w.jobs.take().expect("rig");
    let job = rig.stored();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.error_code.as_deref(), Some(code.as_str()));
    rig.finish();
}

/// Every direct dependency in the workspace as (package, dependency, enabled features), read
/// from `cargo metadata` (I/O: spawns the `cargo` running the tests).
fn workspace_links() -> Vec<(String, String, Vec<String>)> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = std::process::Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let mut links = Vec::new();
    for package in &packages {
        let name = package["name"].as_str().unwrap_or_default();
        for dep in package["dependencies"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let features = dep["features"].as_array().cloned().unwrap_or_default();
            let features = features
                .iter()
                .filter_map(|f| f.as_str().map(str::to_string))
                .collect();
            let dep_name = dep["name"].as_str().unwrap_or_default().to_string();
            links.push((name.to_string(), dep_name, features));
        }
    }
    links
}

#[given(expr = "the workspace's Cargo metadata")]
fn cargo_metadata(w: &mut NativeWorld) {
    w.links = workspace_links();
}

#[then(expr = "{string} should link {string} with the feature {string}")]
fn links_with_feature(w: &mut NativeWorld, package: String, dep: String, feature: String) {
    let found = feature_sets(&w.links, &package, &dep);
    assert!(!found.is_empty(), "{package} does not link {dep}");
    assert!(found.iter().all(|f| **f == [feature.clone()]), "{found:?}");
}

#[then(expr = "{string} should link {string} with no feature")]
fn links_without_feature(w: &mut NativeWorld, package: String, dep: String) {
    let found = feature_sets(&w.links, &package, &dep);
    assert!(!found.is_empty(), "{package} does not link {dep}");
    assert!(found.iter().all(|f| f.is_empty()), "{found:?}");
}

/// The features of every entry by which `package` links `dep` (normal, dev, build and
/// target-specific entries alike). Pure.
fn feature_sets<'a>(
    links: &'a [(String, String, Vec<String>)],
    package: &str,
    dep: &str,
) -> Vec<&'a Vec<String>> {
    links
        .iter()
        .filter(|(p, d, _)| p == package && d == dep)
        .map(|(_, _, features)| features)
        .collect()
}

#[then(expr = "no package other than {string} and {string} should link {string}")]
fn only_two_link(w: &mut NativeWorld, first: String, second: String, dep: String) {
    let others: Vec<&String> = w
        .links
        .iter()
        .filter(|(p, d, _)| *d == dep && *p != first && *p != second)
        .map(|(p, _, _)| p)
        .collect();
    assert!(others.is_empty(), "{others:?}");
}

#[then(expr = "no package other than {string} should link {string}")]
fn only_one_links(w: &mut NativeWorld, only: String, dep: String) {
    let others: Vec<&String> = w
        .links
        .iter()
        .filter(|(p, d, _)| *d == dep && *p != only)
        .map(|(p, _, _)| p)
        .collect();
    assert!(others.is_empty(), "{others:?}");
}

#[tokio::main]
async fn main() {
    let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    // Only this harness's report is replaced (`File::create` truncates it), so the report of
    // the catalog harness (`catalog_features.rs`) in the same directory survives.
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
