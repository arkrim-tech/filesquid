//! Step definitions for the client-event, journal query and export scenarios of
//! `durable_journal_and_journal_tab_F-061.feature` (Task API-F2.10; API-S-030, API-S-031),
//! driven through the router of a desktop host whose journal holds a seeded history. The
//! sealing, rebuild and UI scenarios are bound by the later FS-2.9 tasks.

use crate::MonitorWorld;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::admin::JournalNames;
use filesquid_api::auth::{Authoriser, LaunchToken, Principal};
use filesquid_api::client_events::RATE_LIMIT;
use filesquid_api::files::NameFuture;
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{
    Monitor, MonitorConfig, StaticHostChecks, METRICS_SNAPSHOT_OPERATION,
};
use filesquid_api::trace_layer::{activity_span, JournalLayer};
use filesquid_api::{router, ApiState, EventBus};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

/// The file of the seeded history.
const FILE: &str = "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab";
/// The client event ID of the steps that send one event.
const EVENT_ID: &str = "0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a01";
/// The display name the host resolves for [`FILE`].
const NAME: &str = "Lisbon 2026.jpg";

/// Per-scenario state of the journal steps.
#[derive(Default)]
pub struct JournalWorld {
    state: Option<ApiState>,
    monitor: Option<Monitor>,
    token: String,
    answers: Vec<(StatusCode, Option<String>, String)>,
    page: Value,
    export: String,
    entry: Option<JournalEntry>,
}

impl std::fmt::Debug for JournalWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalWorld").finish_non_exhaustive()
    }
}

impl Drop for JournalWorld {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            monitor.shutdown();
        }
    }
}

fn object() -> String {
    format!("urn:uuid:{FILE}")
}

/// The seeded history: a committed rename (op-rename), a failed move (op-move), a lease
/// warning in lib-a, and a host event.
fn seed(store: &MemoryJournal) {
    let entries = [
        JournalEntry::new("file.rename", Outcome::Succeeded)
            .operation_id("op-rename")
            .object_ref(format!("{}@v2", object()))
            .target("lib-a"),
        JournalEntry::new("file.move", Outcome::Failed)
            .operation_id("op-move")
            .object_ref(format!("{}@v2", object()))
            .target("lib-a")
            .error_code("conflict"),
        JournalEntry::new("ocfl.lease.takeover", Outcome::Info)
            .level(filesquid_api::journal::Level::Warn)
            .target("lib-a")
            .error_code("lease_taken_over"),
        JournalEntry::new("host.started", Outcome::Succeeded),
    ];
    store.append(&entries).expect("seed");
}

impl JournalWorld {
    fn state(&self) -> &ApiState {
        self.state.as_ref().expect("a desktop library")
    }

    fn monitor(&self) -> &Monitor {
        self.monitor.as_ref().expect("a desktop library")
    }

    /// Send one request with `token`; record and return the answer.
    async fn send(&mut self, request: Request<Body>) -> (StatusCode, Option<String>, String) {
        let response = router(self.state().clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let retry = response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().expect("ascii").to_string());
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let answer = (status, retry, String::from_utf8_lossy(&bytes).to_string());
        self.answers.push(answer.clone());
        answer
    }

    async fn get(&mut self, uri: &str) -> (StatusCode, Option<String>, String) {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .body(Body::empty())
            .expect("request");
        self.send(request).await
    }

    async fn post_events(&mut self, events: Vec<Value>) -> (StatusCode, Option<String>, String) {
        let body = serde_json::to_vec(&json!({ "events": events })).expect("json");
        let request = Request::post("/api/v1/client-events")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("request");
        self.send(request).await
    }

    /// Every stored entry named `operation`.
    fn named(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor().journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor().store().query(&query).expect("query").entries
    }

    fn page_ids(&self) -> Vec<i64> {
        self.page["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|e| e["id"].as_i64().expect("id"))
            .collect()
    }
}

/// A console event with the `n`th client event ID.
fn console(n: u32, level: &str) -> Value {
    json!({
        "kind": "console",
        "id": format!("{n:08x}-0000-4000-8000-000000000000"),
        "level": level,
        "component": "fs-file-browser",
        "error_code": "fetch_failed",
        "ids": [object()]
    })
}

fn overflow(count: u64) -> Value {
    json!({ "kind": "overflow", "id": EVENT_ID, "component": "fs-console-queue", "count": count })
}

#[given(expr = "a desktop library with recorded activity")]
fn desktop_library(w: &mut MonitorWorld) {
    let state = ApiState::new();
    let token = LaunchToken::generate().expect("token");
    state.install_authoriser(Authoriser::desktop_owner(&token));
    let store = MemoryJournal::new("desktop");
    seed(&store);
    let monitor = Monitor::start(
        Arc::new(store),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    let mut world = JournalWorld::default();
    world.state = Some(state);
    world.monitor = Some(monitor);
    world.token = token.secret().to_string();
    w.journal = world;
}

#[when(expr = "the UI reports a console warning")]
async fn reports_warning(w: &mut MonitorWorld) {
    let (status, _, body) = w.journal.post_events(vec![console(1, "warn")]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[when(expr = "the UI reports a console error with a message that contains a file name")]
async fn reports_error_with_text(w: &mut MonitorWorld) {
    let mut event = console(2, "error");
    event["message"] = json!(format!("Could not open {NAME}"));
    event["stack"] = json!("at open (app.js:1:1)");
    let (status, _, body) = w.journal.post_events(vec![event]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[then(
    expr = "the journal should hold one {string} entry with the component, error code and opaque IDs"
)]
fn one_console_entry(w: &mut MonitorWorld, operation: String) {
    let entries = w.journal.named(&operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = &entries[0];
    assert!(entry
        .target_ids
        .contains(&"component:fs-file-browser".to_string()));
    assert!(entry.target_ids.contains(&object()));
    assert_eq!(entry.error_code.as_deref(), Some("fetch_failed"));
    w.journal.entry = Some(entry.clone());
}

#[then(expr = "the entry should not contain the message text")]
fn no_message_text(w: &mut MonitorWorld) {
    let entry = w.journal.entry.as_ref().expect("an entry");
    let text = serde_json::to_string(entry).expect("json");
    assert!(
        !text.contains("Lisbon") && !text.contains("app.js"),
        "{text}"
    );
}

#[when(expr = "the UI sends {int} events in one request")]
async fn sends_many(w: &mut MonitorWorld, count: u32) {
    let events = (0..count).map(|n| console(100 + n, "warn")).collect();
    w.journal.post_events(events).await;
}

#[then(expr = "the request should be refused with an error code")]
fn refused_with_code(w: &mut MonitorWorld) {
    let (status, _, body) = w.journal.answers.last().expect("an answer").clone();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(error["code"], "invalid_request");
}

#[then(expr = "a caller over the rate bound should receive status 429 with a Retry-After header")]
async fn over_the_rate_bound(w: &mut MonitorWorld) {
    use_up_rate(w).await;
    let (status, retry, _) = w.journal.post_events(vec![console(999, "warn")]).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(retry.expect("Retry-After").parse::<u64>().expect("seconds") >= 1);
}

#[given(expr = "the caller has used up the rate bound")]
async fn use_up_rate(w: &mut MonitorWorld) {
    let already = w.journal.answers.len() as u32;
    for n in 0..RATE_LIMIT as u32 {
        w.journal
            .post_events(vec![console(1000 + already + n, "warn")])
            .await;
    }
}

#[when(expr = "the UI sends a console warning twice with the same client event ID")]
async fn sends_twice(w: &mut MonitorWorld) {
    w.journal.post_events(vec![console(7, "warn")]).await;
    w.journal.post_events(vec![console(7, "warn")]).await;
}

#[then(expr = "both requests should be acknowledged")]
fn both_acknowledged(w: &mut MonitorWorld) {
    let ids = format!("{:08x}-0000-4000-8000-000000000000", 7);
    for (status, _, body) in &w.journal.answers {
        assert_eq!(*status, StatusCode::OK, "{body}");
        let ack: Value = serde_json::from_str(body).expect("json");
        assert_eq!(ack["acknowledged"], json!([ids]));
    }
}

#[then(expr = "the journal should hold one {string} entry for that client event ID")]
fn one_entry_for_the_id(w: &mut MonitorWorld, operation: String) {
    assert_eq!(w.journal.named(&operation).len(), 1);
}

#[when(expr = "the UI sends a console warning raised by another principal")]
async fn foreign_principal(w: &mut MonitorWorld) {
    let mut event = console(8, "warn");
    event["principal"] = json!("administrator");
    w.journal.post_events(vec![event]).await;
}

#[then(expr = "the request should be refused as not permitted and journaled as a refusal")]
fn refused_not_permitted(w: &mut MonitorWorld) {
    let (status, _, body) = w.journal.answers.last().expect("an answer").clone();
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let error: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(error["reason"], "not_permitted");
    assert_eq!(w.journal.named("api.auth.denied").len(), 1);
}

#[then(expr = "no {string} entry should be written")]
fn none_written(w: &mut MonitorWorld, operation: String) {
    assert!(w.journal.named(&operation).is_empty());
}

#[when(expr = "the UI sends one overflow event with the count {int}")]
async fn sends_overflow(w: &mut MonitorWorld, count: u64) {
    let (status, _, body) = w.journal.post_events(vec![overflow(count)]).await;
    assert_eq!(status, StatusCode::OK, "a lone overflow is exempt: {body}");
}

#[then(expr = "the journal should hold one {string} entry with the count {int}")]
fn overflow_entry(w: &mut MonitorWorld, operation: String, count: u64) {
    let entries = w.journal.named(&operation);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].target_ids.contains(&format!("count:{count}")));
}

#[then(expr = "an overflow event with the count {int} should be refused with an error code")]
async fn overflow_refused(w: &mut MonitorWorld, count: u64) {
    let mut event = overflow(count);
    event["id"] = json!("6a1d4f55-3b8c-4d2e-8f90-123456789abc");
    let (status, _, _) = w.journal.post_events(vec![event]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[when(expr = "I filter the journal by the file")]
async fn filter_by_file(w: &mut MonitorWorld) {
    let (status, _, body) = w
        .journal
        .get(&format!("/api/v1/admin/journal?file={FILE}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    w.journal.page = serde_json::from_str(&body).expect("json");
}

#[then(expr = "I should see only the entries about that file")]
fn only_the_file(w: &mut MonitorWorld) {
    assert_eq!(w.journal.page_ids(), [1, 2]);
}

#[when(expr = "I filter the journal by the operation ID of the rename")]
async fn filter_by_operation(w: &mut MonitorWorld) {
    let (_, _, body) = w
        .journal
        .get("/api/v1/admin/journal?operation_id=op-rename")
        .await;
    w.journal.page = serde_json::from_str(&body).expect("json");
}

#[then(expr = "I should see only the entries of that operation")]
fn only_the_operation(w: &mut MonitorWorld) {
    assert_eq!(w.journal.page_ids(), [1]);
}

#[when(expr = "I open the journal entry of the committed rename")]
async fn open_rename(w: &mut MonitorWorld) {
    let (_, _, body) = w.journal.get("/api/v1/admin/journal/entries/1").await;
    w.journal.page = serde_json::from_str(&body).expect("json");
}

#[then(expr = "it should link to the PROV-O record of its object version")]
fn links_to_record(w: &mut MonitorWorld) {
    let link = &w.journal.page["provenance"];
    assert_eq!(link["journal_only"], false);
    assert_eq!(
        link["record"],
        format!("/api/v1/files/{FILE}/versions/v2/prov")
    );
    assert_eq!(link["operation_id"], "op-rename");
}

#[when(expr = "I open the journal entry of the failed move")]
async fn open_move(w: &mut MonitorWorld) {
    let (_, _, body) = w.journal.get("/api/v1/admin/journal/entries/2").await;
    w.journal.page = serde_json::from_str(&body).expect("json");
}

#[then(expr = "it should be marked journal-only")]
fn journal_only(w: &mut MonitorWorld) {
    assert_eq!(w.journal.page["provenance"]["journal_only"], true);
}

/// Names [`FILE`] for the owner.
struct OwnerNames;

impl JournalNames for OwnerNames {
    fn display_name<'a>(&'a self, principal: &'a Principal, object_id: &'a str) -> NameFuture<'a> {
        let name =
            (*principal == Principal::Owner && object_id == object()).then(|| NAME.to_string());
        Box::pin(async move { name })
    }
}

#[given(expr = "the host resolves display names for the owner")]
fn resolves_names(w: &mut MonitorWorld) {
    assert!(w
        .journal
        .state()
        .install_journal_names(Arc::new(OwnerNames)));
}

#[when(expr = "I read the journal with names and export it")]
async fn read_and_export(w: &mut MonitorWorld) {
    let (_, _, body) = w.journal.get("/api/v1/admin/journal?names=true").await;
    w.journal.page = serde_json::from_str(&body).expect("json");
    let (_, _, export) = w
        .journal
        .get("/api/v1/admin/journal/export?names=true")
        .await;
    w.journal.export = export;
}

#[then(expr = "the page should show the file's display name")]
fn shows_name(w: &mut MonitorWorld) {
    assert_eq!(w.journal.page["entries"][0]["display_name"], NAME);
}

#[then(expr = "the export should carry opaque IDs only")]
fn export_is_opaque(w: &mut MonitorWorld) {
    assert!(!w.journal.export.contains("Lisbon"), "{}", w.journal.export);
    assert!(w.journal.export.contains(&object()));
}

#[when(expr = "I export the journal filtered by library and level {string} as CSV")]
async fn export_csv(w: &mut MonitorWorld, level: String) {
    let uri = format!("/api/v1/admin/journal/export?library=lib-a&level={level}&format=csv");
    let (status, _, body) = w.journal.get(&uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    w.journal.export = body;
}

#[then(
    expr = "a CSV file with one row per matching entry and opaque IDs only should be downloaded"
)]
fn csv_downloaded(w: &mut MonitorWorld) {
    let rows: Vec<&str> = w.journal.export.split_terminator("\r\n").collect();
    assert_eq!(rows[0], filesquid_api::journal::csv::CSV_HEADER);
    assert_eq!(
        rows.len(),
        2,
        "header and the one warning of lib-a: {rows:?}"
    );
    assert!(rows[1].contains("ocfl.lease.takeover"));
}

#[then(expr = "the journal should record a {string} entry")]
fn records_export(w: &mut MonitorWorld, operation: String) {
    let entries = w.journal.named(&operation);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor, "owner");
}

#[given(expr = "a caller without the owner credential")]
fn without_owner_credential(w: &mut MonitorWorld) {
    w.journal.token = "0".repeat(64);
}

#[when(expr = "it requests the journal export")]
async fn requests_export(w: &mut MonitorWorld) {
    w.journal.get("/api/v1/admin/journal/export").await;
}

#[then(expr = "the request should be refused and journaled as a refusal")]
fn refused_and_journaled(w: &mut MonitorWorld) {
    let (status, _, _) = w.journal.answers.last().expect("an answer");
    assert_eq!(*status, StatusCode::FORBIDDEN);
    let denied = w.journal.named("api.auth.denied");
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].target_ids, ["monitor.read"]);
}

#[when(expr = "the OCFL engine warns about a lease takeover inside an operation")]
fn engine_warns(w: &mut MonitorWorld) {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        let span = activity_span("op-rename", None);
        let _entered = span.enter();
        tracing::warn!(
            target: "filesquid_ocfl::write::lease",
            object = object().as_str(),
            version = "v3",
            "took over an expired reservation"
        );
    });
    writer.sender().flush();
    writer.shutdown();
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    w.journal.entry = entries.into_iter().next();
}

#[then(
    expr = "the journal entry should name the engine operation, the operation ID and the object version"
)]
fn engine_entry(w: &mut MonitorWorld) {
    let entry = w.journal.entry.as_ref().expect("an engine entry");
    assert_eq!(entry.operation, "ocfl.engine.write.lease");
    assert_eq!(entry.operation_id.as_deref(), Some("op-rename"));
    assert_eq!(
        entry.object_ref.as_deref(),
        Some(format!("{}@v3", object()).as_str())
    );
}

#[when(expr = "the monitor ticks with its default settings")]
fn ticks(w: &mut MonitorWorld) {
    let monitor = w.journal.monitor();
    monitor.tick();
    monitor.tick();
    assert!(w.journal.named(METRICS_SNAPSHOT_OPERATION).is_empty());
}
