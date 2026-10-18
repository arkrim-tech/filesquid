//! Cucumber harness for the `filesquid-api` Gherkin under `tests/features/` (F-061, and the
//! versioned contract, typed errors, deny-by-default authorisation, transport-agnostic routes
//! and event channel of F-056: API-S-002, API-S-003, API-S-004, API-S-009).
//!
//! Scenarios that need domain operations not built yet (import, rename, metadata writes, C2PA
//! scans, sync) or the optional OTLP export (Task FS-3.6) have no step definitions and are
//! reported as skipped until those land.

use axum::body::Body;
use axum::http::{header, Request};
use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::auth::Authoriser;
use filesquid_api::events::EventSubscription;
use filesquid_api::health::HealthStatus;
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::registry::{Clock, ProcessHandle, ProcessSpec, ProcessState};
use filesquid_api::{router, ApiError, ApiEvent, ApiState, EventBus};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

#[path = "support/event_steps.rs"]
mod event_steps;

#[path = "support/auth_steps.rs"]
mod auth_steps;

/// Administrator token accepted by [`admin_state`].
const ADMIN_TOKEN: &str = "admin";

/// API state whose authoriser accepts `token` as the server administrator's.
fn admin_state(token: &str) -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(token));
    state
}

/// State shared by the steps of one scenario.
#[derive(Default, World)]
#[world(init = Self::default)]
struct MonitorWorld {
    capacity: usize,
    produced: u64,
    stored: u64,
    state: Option<ApiState>,
    monitor: Option<Monitor>,
    tokens: Vec<(String, String)>,
    statuses: Vec<u16>,
    now: Option<Arc<AtomicI64>>,
    handle: Option<ProcessHandle>,
    prometheus: String,
    json: serde_json::Value,
    live: Option<tokio::sync::broadcast::Receiver<ApiEvent>>,
    store: Option<Arc<MemoryJournal>>,
    expected_ids: Vec<i64>,
    export: String,
    answers: Vec<(u16, Vec<u8>)>,
    channel: Option<EventBus>,
    subscriptions: Vec<EventSubscription>,
    received: Vec<ApiEvent>,
    refused: Option<ApiError>,
    secret: Option<String>,
    presented: Option<String>,
}

impl std::fmt::Debug for MonitorWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitorWorld")
            .field("capacity", &self.capacity)
            .field("statuses", &self.statuses)
            .finish()
    }
}

#[given(expr = "the journal queue capacity is N events")]
fn capacity(w: &mut MonitorWorld) {
    w.capacity = 16;
}

#[when(expr = "{int} x N job and process events are produced as fast as possible")]
fn burst(w: &mut MonitorWorld, factor: u64) {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig {
            capacity: w.capacity,
        },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let total = factor * w.capacity as u64;
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let sender = writer.sender();
            std::thread::spawn(move || {
                (0..total / 4)
                    .for_each(|_| sender.send(JournalEntry::new("process.progress", Outcome::Info)))
            })
        })
        .collect();
    threads
        .into_iter()
        .for_each(|t| t.join().expect("producer"));
    writer.sender().flush();
    w.produced = total;
    w.stored = store.count(&JournalQuery::default()).expect("count");
    writer.shutdown();
}

#[then(expr = "the journal should contain all {int} x N events")]
fn all_events(w: &mut MonitorWorld, factor: u64) {
    assert_eq!(w.stored, factor * w.capacity as u64);
    assert_eq!(w.stored, w.produced);
}

#[then(expr = "no log file should exist anywhere in the host's data directory")]
fn no_log_files(_w: &mut MonitorWorld) {
    let dir = data_dir("nolog");
    let store =
        filesquid_api::journal::SqliteJournal::open(&dir.join("filesquid.sqlite"), "host-a")
            .expect("open");
    store
        .append(&[JournalEntry::new("import", Outcome::Succeeded)])
        .expect("append");
    let logs: Vec<_> = std::fs::read_dir(&dir)
        .expect("dir")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .collect();
    assert!(logs.is_empty());
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[given(expr = "a filesquid-server with users Ana \\(administrator) and Ben \\(member)")]
fn server_users(w: &mut MonitorWorld) {
    let state = admin_state("ana-admin-token");
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("server")),
        MonitorConfig::new("server", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    w.tokens = vec![
        ("Ana".into(), "ana-admin-token".into()),
        ("Ben".into(), "ben-member-token".into()),
    ];
    w.state = Some(state);
    w.monitor = Some(monitor);
}

#[when(
    expr = "Ben requests GET \\/api\\/v1\\/admin\\/journal and GET \\/api\\/v1\\/admin\\/health"
)]
async fn ben_requests(w: &mut MonitorWorld) {
    let token = w
        .tokens
        .iter()
        .find(|(n, _)| n == "Ben")
        .map(|(_, t)| t.clone())
        .expect("Ben");
    for path in ["/api/v1/admin/journal", "/api/v1/admin/health"] {
        let req = Request::get(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request");
        let res = router(w.state.clone().expect("state"))
            .oneshot(req)
            .await
            .expect("response");
        w.statuses.push(res.status().as_u16());
    }
}

#[then(expr = "both responses should be {int}")]
fn responses_are(w: &mut MonitorWorld, code: u16) {
    assert_eq!(w.statuses, vec![code, code]);
    if let Some(m) = &w.monitor {
        m.shutdown();
    }
}

#[given(
    expr = "an import job that sends heartbeats every {int} seconds with a {int} second timeout"
)]
fn heartbeating_job(w: &mut MonitorWorld, _every: i64, timeout: i64) {
    let now = Arc::new(AtomicI64::new(1_000_000));
    let clock_now = now.clone();
    let clock: Clock = Arc::new(move || clock_now.load(Ordering::SeqCst));
    let state = ApiState::new();
    let mut config = MonitorConfig::new("desktop", std::env::temp_dir());
    config.clock = clock;
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("desktop")),
        config,
        state.events.clone(),
    );
    w.handle = Some(
        monitor
            .registry()
            .register(ProcessSpec::new("import", timeout * 1000)),
    );
    assert_eq!(monitor.tick().status, HealthStatus::Ok);
    w.live = Some(state.events.subscribe());
    w.now = Some(now);
    w.monitor = Some(monitor);
}

#[when(expr = "the job stops sending heartbeats for {int} seconds")]
fn silence(w: &mut MonitorWorld, seconds: i64) {
    w.now
        .as_ref()
        .expect("clock")
        .fetch_add(seconds * 1000, Ordering::SeqCst);
}

#[then(expr = "the process registry should mark the job {string}")]
fn marked(w: &mut MonitorWorld, state: String) {
    let monitor = w.monitor.as_ref().expect("monitor");
    monitor.tick();
    let view = &monitor.registry().snapshot()[0];
    assert_eq!(
        serde_json::to_value(view.state).expect("state"),
        serde_json::Value::String(state)
    );
    assert_eq!(view.state, ProcessState::Stalled);
}

#[then(expr = "health should become {string} with reason {string}")]
fn health_became(w: &mut MonitorWorld, status: String, reason: String) {
    let report = w.monitor.as_ref().expect("monitor").health_report();
    assert_eq!(
        serde_json::to_value(report.status).expect("status"),
        serde_json::Value::String(status)
    );
    assert!(
        report
            .reasons
            .iter()
            .any(|r| r.code.replace('_', " ") == reason),
        "{:?}",
        report.reasons
    );
}

/// The Monitor learns of changes only from the event channel, so assert what was pushed
/// there (subscribed before the job went silent), not what was stored.
#[then(expr = "the Monitor dashboard should show the change without a refresh")]
fn pushed_live(w: &mut MonitorWorld) {
    let monitor = w.monitor.as_ref().expect("monitor");
    monitor.journal().flush();
    let mut live = w.live.take().expect("subscribed before the change");
    let mut pushed = Vec::new();
    while let Ok(event) = live.try_recv() {
        pushed.push(event);
    }
    assert!(
        pushed
            .iter()
            .any(|e| e.topic == "process.update" && e.data["state"] == "stalled"),
        "{pushed:?}"
    );
    assert!(
        pushed
            .iter()
            .any(|e| e.topic == "health.change" && e.data["status"] == "degraded"),
        "{pushed:?}"
    );
    assert!(pushed
        .iter()
        .any(|e| e.topic == "journal.entry" && e.data["operation"] == "process.stalled"));
    w.handle.take();
    monitor.shutdown();
}

/// A host with real activity behind its counters. `/metrics` itself is served by
/// `filesquid-server` from `Metrics::to_prometheus` and is compared over HTTP in that
/// crate's integration tests.
#[given(expr = "filesquid-server is running")]
fn server_running(w: &mut MonitorWorld) {
    let state = admin_state(ADMIN_TOKEN);
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("server")),
        MonitorConfig::new("server", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    monitor
        .registry()
        .register(ProcessSpec::new("import", 30_000))
        .complete();
    monitor
        .journal()
        .send(JournalEntry::new("ocfl.commit", Outcome::Failed));
    monitor.journal().flush();
    w.state = Some(state);
    w.monitor = Some(monitor);
}

#[when(expr = "I read \\/metrics and GET \\/api\\/v1\\/admin\\/metrics")]
async fn read_both(w: &mut MonitorWorld) {
    let req = Request::get("/api/v1/admin/metrics")
        .header(header::AUTHORIZATION, "Bearer admin")
        .body(Body::empty())
        .expect("request");
    let res = router(w.state.clone().expect("state"))
        .oneshot(req)
        .await
        .expect("response");
    let body = http_body_util::BodyExt::collect(res.into_body())
        .await
        .expect("body")
        .to_bytes();
    w.json = serde_json::from_slice(&body).expect("metrics JSON");
    w.prometheus = w
        .monitor
        .as_ref()
        .expect("monitor")
        .metrics()
        .to_prometheus();
}

#[then(expr = "every counter should report the same value in both")]
fn same_values(w: &mut MonitorWorld) {
    for (name, value) in w.json.as_object().expect("object") {
        let line = w
            .prometheus
            .lines()
            .find(|l| l.starts_with(&format!("{name} ")))
            .expect("metric line");
        assert_eq!(
            line.split_whitespace().nth(1),
            Some(value.to_string().as_str())
        );
    }
    assert_eq!(w.json["filesquid_processes_completed_total"], 1);
    assert_eq!(w.json["filesquid_errors_total"], 1);
    if let Some(m) = w.monitor.take() {
        m.shutdown();
    }
}

#[given(expr = "journal entries from the last {int} days")]
fn entries_over_days(w: &mut MonitorWorld, days: i64) {
    let store = Arc::new(MemoryJournal::new("server"));
    let now = filesquid_api::journal::now_ms();
    let entries: Vec<JournalEntry> = (0..days)
        .flat_map(|day| {
            let at = now - day * 86_400_000;
            [
                JournalEntry::new("ocfl.commit", Outcome::Succeeded).at_ms(at),
                JournalEntry::new("ocfl.commit", Outcome::Failed).at_ms(at),
                JournalEntry::new("import", Outcome::Failed).at_ms(at),
            ]
        })
        .collect();
    let ids = store.append(&entries).expect("append");
    w.expected_ids = entries
        .iter()
        .zip(ids)
        .filter(|(e, _)| e.operation == "ocfl.commit" && e.outcome == Outcome::Failed)
        .map(|(_, id)| id)
        .collect();
    let state = admin_state(ADMIN_TOKEN);
    let monitor = Monitor::start(
        store.clone(),
        MonitorConfig::new("server", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    w.store = Some(store);
    w.state = Some(state);
    w.monitor = Some(monitor);
}

#[when(expr = "the administrator filters by operation {string} and outcome {string} and exports")]
async fn filter_and_export(w: &mut MonitorWorld, operation: String, outcome: String) {
    let req = Request::get(format!(
        "/api/v1/admin/journal/export?operation={operation}&outcome={outcome}"
    ))
    .header(header::AUTHORIZATION, "Bearer admin")
    .body(Body::empty())
    .expect("request");
    let res = router(w.state.clone().expect("state"))
        .oneshot(req)
        .await
        .expect("response");
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/x-ndjson");
    let body = http_body_util::BodyExt::collect(res.into_body())
        .await
        .expect("body")
        .to_bytes();
    w.export = String::from_utf8(body.to_vec()).expect("UTF-8");
}

#[then(expr = "the export should be JSON Lines containing exactly the matching entries")]
fn export_matches(w: &mut MonitorWorld) {
    let entries: Vec<JournalEntry> = w
        .export
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON entry per line"))
        .collect();
    assert!(entries
        .iter()
        .all(|e| e.operation == "ocfl.commit" && e.outcome == Outcome::Failed));
    let ids: Vec<i64> = entries.iter().filter_map(|e| e.id).collect();
    assert_eq!(ids, w.expected_ids);
    assert_eq!(ids.len(), 30);
    if let Some(m) = w.monitor.take() {
        m.shutdown();
    }
}

// ---- Versioned API contract (F-056, API-S-002, API-S-009) ----

/// Send `GET path` through the router (optionally with a bearer token); record the status
/// in `statuses` and the JSON body in `json`.
async fn send_get(w: &mut MonitorWorld, path: &str, token: Option<&str>) {
    let mut req = Request::get(path);
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let res = router(w.state.clone().expect("state"))
        .oneshot(req.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    w.statuses.push(res.status().as_u16());
    let body = http_body_util::BodyExt::collect(res.into_body())
        .await
        .expect("body")
        .to_bytes();
    w.json = serde_json::from_slice(&body).expect("response body is JSON");
}

/// The checked-in contract as JSON.
fn contract_json() -> serde_json::Value {
    serde_json::from_str(filesquid_api::CONTRACT).expect("contract JSON")
}

#[given(expr = "a FileSquid API router")]
fn api_router(w: &mut MonitorWorld) {
    w.state = Some(admin_state(ADMIN_TOKEN));
}

#[given(expr = "a FileSquid API router whose monitor has not started")]
fn api_router_without_monitor(w: &mut MonitorWorld) {
    w.state = Some(admin_state(ADMIN_TOKEN));
}

#[given(expr = "the API contract")]
fn the_contract(w: &mut MonitorWorld) {
    w.json = contract_json();
}

#[given(expr = "a FileSquid server API that requires an administrator token")]
fn server_with_token(w: &mut MonitorWorld) {
    let state = admin_state(ADMIN_TOKEN);
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("server")),
        MonitorConfig::new("server", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    w.state = Some(state);
    w.monitor = Some(monitor);
}

#[when(expr = "a client asks for the API contract")]
async fn ask_contract(w: &mut MonitorWorld) {
    send_get(w, "/api/v1/openapi.json", Some(ADMIN_TOKEN)).await;
}

#[when(expr = "a client asks for the activity journal without a token")]
async fn ask_journal_anonymous(w: &mut MonitorWorld) {
    send_get(w, "/api/v1/admin/journal", None).await;
}

#[when(expr = "a client asks for the health report")]
async fn ask_health_report(w: &mut MonitorWorld) {
    send_get(w, "/api/v1/admin/health", Some(ADMIN_TOKEN)).await;
}

#[when(expr = "an administrator asks for the activity journal with a malformed cursor")]
async fn ask_journal_bad_cursor(w: &mut MonitorWorld) {
    send_get(
        w,
        "/api/v1/admin/journal?after=not-a-cursor",
        Some(ADMIN_TOKEN),
    )
    .await;
}

#[when(expr = "an administrator asks for the activity journal")]
async fn ask_journal(w: &mut MonitorWorld) {
    send_get(w, "/api/v1/admin/journal", Some(ADMIN_TOKEN)).await;
}

#[then(expr = "the contract should be OpenAPI 3.1 at the crate's API version")]
fn contract_is_versioned(w: &mut MonitorWorld) {
    assert_eq!(w.statuses.last(), Some(&200));
    assert_eq!(w.json["openapi"], "3.1.0");
    assert_eq!(w.json["info"]["version"], filesquid_api::API_VERSION);
}

#[then(expr = "every path the router serves should be documented in the contract")]
fn routed_are_documented(_w: &mut MonitorWorld) {
    let doc = contract_json();
    for path in filesquid_api::routed_paths() {
        assert!(doc["paths"].get(&path).is_some(), "{path} is undocumented");
    }
}

#[then(expr = "every documented path should be served by the router or marked as a host route")]
fn documented_are_routed(_w: &mut MonitorWorld) {
    let doc = contract_json();
    let routed = filesquid_api::routed_paths();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        let host = item.get("x-filesquid-host-route") == Some(&serde_json::Value::Bool(true));
        assert!(host || routed.contains(path), "{path} is not routed");
    }
}

#[then(
    expr = "a command envelope should require an operation id, an expected revision, a scope and a schema version"
)]
fn command_envelope_required(w: &mut MonitorWorld) {
    let required = &w.json["components"]["schemas"]["CommandEnvelope"]["required"];
    for name in [
        "operation_id",
        "expected_revision",
        "scope",
        "schema_version",
    ] {
        assert!(
            required
                .as_array()
                .expect("required")
                .contains(&name.into()),
            "{name}"
        );
    }
}

#[then(expr = "a query page size should be bounded between {int} and {int}")]
fn page_size_bounded(w: &mut MonitorWorld, min: u32, max: u32) {
    let schemas = &w.json["components"]["schemas"];
    let parameter = &w.json["components"]["parameters"]["PageSize"]["schema"];
    for schema in [
        &schemas["QueryEnvelope"]["properties"]["page_size"],
        parameter,
    ] {
        assert_eq!(schema["minimum"], min);
        assert_eq!(schema["maximum"], max);
    }
    assert_eq!(max, filesquid_api::operation::MAX_PAGE_SIZE);
}

#[then(expr = "an API error should carry a code, a message and whether it is retryable")]
fn api_error_fields(w: &mut MonitorWorld) {
    let required = &w.json["components"]["schemas"]["ApiError"]["required"];
    assert_eq!(
        *required,
        serde_json::json!(["code", "message", "retryable"])
    );
}

#[then(expr = "the response should be an {string} error with a message")]
fn typed_error(w: &mut MonitorWorld, code: String) {
    assert!(w.statuses.last().is_some_and(|s| *s >= 400));
    assert_eq!(w.json["code"], code.as_str(), "{}", w.json);
    assert!(w.json["message"].as_str().is_some_and(|m| !m.is_empty()));
}

#[then(expr = "the error should be retryable")]
fn error_retryable(w: &mut MonitorWorld) {
    assert_eq!(w.json["retryable"], true);
}

#[then(expr = "the error should not be retryable")]
fn error_not_retryable(w: &mut MonitorWorld) {
    assert_eq!(w.json["retryable"], false);
    shutdown_monitor(w);
}

#[then(expr = "the response should be a journal page")]
fn journal_page(w: &mut MonitorWorld) {
    assert_eq!(w.statuses.last(), Some(&200));
    assert!(w.json["entries"].is_array());
    shutdown_monitor(w);
}

/// Stop the scenario's monitor, if one was started.
fn shutdown_monitor(w: &mut MonitorWorld) {
    if let Some(m) = w.monitor.take() {
        m.shutdown();
    }
}

/// Fresh temporary directory.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-api-features-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

#[tokio::main]
async fn main() {
    let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    let _ = std::fs::remove_dir_all(&results);
    std::fs::create_dir_all(&results).expect("allure-results");
    let junit = std::fs::File::create(results.join("cucumber-junit.xml")).expect("junit file");
    MonitorWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<MonitorWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .run_and_exit(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features"))
        .await;
}
