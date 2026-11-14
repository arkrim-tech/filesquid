//! Steps of `desktop_expiry_interval_F-008.feature` (Task API-F2.30; DE-S-014, API-S-019):
//! the desktop's expiry monitor over the real launch functions (`open_host_monitor`,
//! `open_library_catalog`, `install_recently_deleted`, `open_jobs`), with the installation's
//! settings in the SQLite operational store of a scratch data directory and the interval
//! changed through the `filesquid://` scheme with the per-launch token. Only the monitor's
//! clock is the scenario's, so an hour passes without waiting for one.

use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::auth::LaunchToken;
use filesquid_api::jobs::{JobFilter, JobState, Jobs};
use filesquid_api::library::EXPIRY_JOB_KIND;
use filesquid_api::ApiState;
use filesquid_core::InstallationId;
use filesquid_desktop_lib::api_protocol::handle;
use filesquid_desktop_lib::recently_deleted::{
    install_recently_deleted, ExpiryHandle, ExpiryMonitor, ExpiryTask,
};
use filesquid_desktop_lib::{install_launch_token, open_host_monitor, open_library_catalog};
use filesquid_native::workers::history_backfill::open_jobs;
use filesquid_native::HostMonitor;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::Arc;

/// The installation of these scenarios.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The settings path.
const SETTINGS_PATH: &str = "/api/v1/settings/recently-deleted";

/// One minute in milliseconds.
const MINUTE_MS: i64 = 60_000;

/// Scenario counter, so concurrent scenarios get separate scratch directories.
static SCENARIOS: AtomicU32 = AtomicU32::new(0);

/// One launch of the desktop: its API state, token, host monitor, jobs and expiry monitor.
struct Launch {
    state: ApiState,
    token: LaunchToken,
    host: Option<HostMonitor>,
    jobs: Jobs,
    monitor: Arc<ExpiryMonitor>,
}

impl Launch {
    /// Launch over the data directory `data`, the expiry monitor reading `clock`.
    fn start(data: &Path, clock: Arc<AtomicI64>) -> Self {
        let installation = InstallationId::parse(INSTALLATION).expect("installation");
        let state = ApiState::new();
        let token = install_launch_token(&state).expect("token");
        let host = open_host_monitor(&state, data, None).expect("monitor");
        let catalog = Arc::new(open_library_catalog(data, &host, &installation).expect("catalog"));
        let service =
            install_recently_deleted(&state, &host, catalog.clone(), data).expect("settings");
        let jobs = open_jobs(data, &host).expect("jobs");
        let task = ExpiryTask::new(service, catalog, jobs.clone(), &installation);
        let handle = ExpiryHandle::new(task, Arc::new(AtomicBool::new(false)));
        let now = Arc::new(move || clock.load(Ordering::SeqCst));
        Self {
            state,
            token,
            host: Some(host),
            jobs,
            monitor: Arc::new(ExpiryMonitor::with_clock(handle, now)),
        }
    }

    /// Stop the host monitor (flushing the journal) and drop every SQLite connection.
    fn stop(&mut self) {
        if let Some(host) = self.host.take() {
            host.stop();
        }
        self.state = ApiState::new();
    }
}

/// What one expiry scenario launched and saw.
pub struct ExpiryScenario {
    dir: PathBuf,
    clock: Arc<AtomicI64>,
    launch: Launch,
    answer: Option<(u16, Value)>,
    operations: u32,
}

impl std::fmt::Debug for ExpiryScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpiryScenario")
            .field("dir", &self.dir)
            .finish()
    }
}

impl Drop for ExpiryScenario {
    fn drop(&mut self) {
        self.launch.stop();
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("scenario directory not removed: {err}");
        }
    }
}

impl ExpiryScenario {
    /// A desktop launched over a fresh scratch data directory.
    fn start() -> Self {
        let n = SCENARIOS.fetch_add(1, Ordering::SeqCst);
        let name = format!("fs-desktop-expiry-feature-{}-{n}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("stale scratch directory");
        }
        std::fs::create_dir_all(dir.join("data")).expect("data dir");
        let clock = Arc::new(AtomicI64::new(0));
        let launch = Launch::start(&dir.join("data"), clock.clone());
        Self {
            dir,
            clock,
            launch,
            answer: None,
            operations: 0,
        }
    }

    /// Send `method` to the settings path as the owner; keep the answer.
    async fn send(&mut self, method: &str, body: Option<Value>) {
        let bearer = format!("Bearer {}", self.launch.token.secret());
        let request = http::Request::builder()
            .method(method)
            .uri(format!("filesquid://localhost{SETTINGS_PATH}"))
            .header(http::header::ORIGIN, "tauri://localhost")
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(http::header::AUTHORIZATION, bearer);
        let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
        let request = request.body(bytes).expect("request");
        let response = handle(self.launch.state.clone(), request).await;
        let status = response.status().as_u16();
        let json = serde_json::from_slice(response.body()).unwrap_or(Value::Null);
        self.answer = Some((status, json));
    }

    /// The last answer.
    fn answer(&self) -> &(u16, Value) {
        self.answer.as_ref().expect("an answer")
    }

    /// Tick the monitor on a blocking thread, as the app's loop does; answer whether it ran.
    async fn tick(&self) -> bool {
        let monitor = self.launch.monitor.clone();
        tokio::task::spawn_blocking(move || monitor.tick())
            .await
            .expect("tick")
    }

    /// Move the monitor's clock to `minutes` after the start of the last run.
    fn at_minutes_after_last_run(&self, minutes: i64) {
        let started = self.launch.monitor.last_started_ms().expect("a last run");
        self.clock
            .store(started + minutes * MINUTE_MS, Ordering::SeqCst);
    }

    /// The completed `library.expiry` jobs.
    fn completed_jobs(&self) -> usize {
        let filter = JobFilter {
            kind: Some(EXPIRY_JOB_KIND.to_string()),
            state: Some(JobState::Completed),
            limit: 100,
        };
        self.launch.jobs.list(&filter).expect("jobs").jobs.len()
    }
}

/// The scenario's desktop.
fn scenario(world: &mut DesktopWorld) -> &mut ExpiryScenario {
    world.expiry.as_mut().expect("a launched desktop")
}

#[given(
    expr = "desktop-expiry host launched with its Recently deleted settings in the operational store"
)]
fn launched(world: &mut DesktopWorld) {
    world.expiry = Some(ExpiryScenario::start());
}

#[given(expr = "desktop-expiry monitor ran at launch")]
#[when(expr = "desktop-expiry monitor ticks at launch")]
async fn ticks_at_launch(world: &mut DesktopWorld) {
    let scenario = scenario(world);
    assert!(scenario.tick().await, "expiry runs at launch");
    assert!(!scenario.tick().await, "and not again at the next tick");
}

#[given(expr = "desktop-expiry {int} minutes have passed")]
fn minutes_passed(world: &mut DesktopWorld, minutes: i64) {
    scenario(world).at_minutes_after_last_run(minutes);
}

#[given(regex = r"^desktop-expiry owner sets the expiry interval to ([0-9.]+) minutes$")]
#[when(regex = r"^desktop-expiry owner sets the expiry interval to ([0-9.]+) minutes$")]
async fn owner_sets(world: &mut DesktopWorld, minutes: String) {
    let scenario = scenario(world);
    let minutes: Value = serde_json::from_str(&minutes).expect("a JSON number");
    scenario.operations += 1;
    let body = json!({
        "envelope": {"schema_version": 1,
                     "operation_id": format!("op-desktop-expiry-{}", scenario.operations),
                     "expected_revision": "0", "scope": "installation"},
        "input": {"default_retention_days": 30, "expiry": "keep_version_history",
                  "expiry_interval_minutes": minutes},
    });
    scenario.send("PUT", Some(body)).await;
}

#[when(expr = "desktop-expiry host is relaunched")]
fn relaunched(world: &mut DesktopWorld) {
    let scenario = scenario(world);
    assert_eq!(scenario.answer().0, 200, "the interval was stored");
    scenario.launch.stop();
    scenario.launch = Launch::start(&scenario.dir.join("data"), scenario.clock.clone());
}

#[then(expr = "desktop-expiry monitor should have completed {int} expiry job(s)")]
fn completed(world: &mut DesktopWorld, jobs: usize) {
    assert_eq!(scenario(world).completed_jobs(), jobs);
}

#[then(expr = "desktop-expiry monitor should not run {int} minutes after the last run started")]
async fn does_not_run(world: &mut DesktopWorld, minutes: i64) {
    let scenario = scenario(world);
    scenario.at_minutes_after_last_run(minutes);
    assert!(!scenario.tick().await, "not due after {minutes} minutes");
}

#[then(expr = "desktop-expiry monitor should run {int} minutes after the last run started")]
async fn runs(world: &mut DesktopWorld, minutes: i64) {
    let scenario = scenario(world);
    scenario.at_minutes_after_last_run(minutes);
    assert!(scenario.tick().await, "due after {minutes} minutes");
}

#[then(expr = "desktop-expiry monitor should run at its next tick")]
async fn runs_at_next_tick(world: &mut DesktopWorld) {
    assert!(scenario(world).tick().await, "due at once");
}

#[then(expr = "desktop-expiry next run should fall due {int} minutes after the last run started")]
fn next_due(world: &mut DesktopWorld, minutes: i64) {
    let monitor = scenario(world).launch.monitor.clone();
    let started = monitor.last_started_ms().expect("a last run");
    assert_eq!(monitor.next_due_ms(), Some(started + minutes * MINUTE_MS));
}

#[then(expr = "desktop-expiry settings should answer expiry_interval_minutes {int}")]
async fn settings_answer(world: &mut DesktopWorld, minutes: u32) {
    let scenario = scenario(world);
    scenario.send("GET", None).await;
    let (status, json) = scenario.answer();
    assert_eq!(*status, 200, "{json}");
    assert_eq!(json["expiry_interval_minutes"], minutes, "{json}");
}

#[then(expr = "desktop-expiry answer should be refused as invalid_request")]
fn refused(world: &mut DesktopWorld) {
    let (status, json) = scenario(world).answer();
    assert_eq!(*status, 400, "{json}");
    assert_eq!(json["code"], "invalid_request", "{json}");
}
