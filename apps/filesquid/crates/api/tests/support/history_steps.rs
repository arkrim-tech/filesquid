//! Step definitions for `prov_o_history_records_routes_and_backfill_API-S-029.feature`
//! (Task API-F2.9): the provenance follow-up of a repair, the backfill job over an in-memory
//! OCFL storage root, and the history routes through the router over a `MemoryHistory`.

use crate::ocfl_fixture::{ready, v, Rig, NOW_MS};
use crate::MonitorWorld;
use axum::body::Body;
use axum::http::{header, Request};
use cucumber::{given, then, when};
use filesquid_api::auth::{Authoriser, Principal};
use filesquid_api::history::backfill::{Backfill, BackfillReport};
use filesquid_api::history::{journal_path, MemoryHistory, ProvenanceContext};
use filesquid_api::journal::JournalEntry;
use filesquid_api::local_library::RepairAnswer;
use filesquid_api::metrics::Metrics;
use filesquid_api::registry::{ProcessRegistry, ProcessState};
use filesquid_api::{router, ApiState, EventBus};
use filesquid_core::InstallationId;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::MAX_CHUNK_BYTES;
use filesquid_prov::ocfl::{missing_records, read_record};
use filesquid_prov::{build_record, parse_record, Derivation, HostFacts, Record, VersionFacts};
use http_body_util::BodyExt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tower::ServiceExt;

/// The W010 object's root and identifier.
const W010: (&str, &str) = ("obj-b", "urn:uuid:b");

/// The installation of every record.
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The first and the second file of the route scenarios.
const FIRST: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
const SECOND: &str = "11111111-1111-4111-8111-111111111111";

/// The bearer token the route scenarios present.
const TOKEN: &str = "history-token";

/// Largest journal file the steps read, in chunks (AGENTS.md §22).
const MAX_CHUNKS: usize = 64;

/// Per-scenario state of the history steps.
#[derive(Default)]
pub struct HistoryWorld {
    rig: Option<Rig>,
    answer: Option<RepairAnswer>,
    registry: Option<ProcessRegistry>,
    reports: Vec<BackfillReport>,
    commits_before: usize,
    state: Option<ApiState>,
    pages: Vec<serde_json::Value>,
    body: Vec<u8>,
}

impl std::fmt::Debug for HistoryWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryWorld")
            .field("reports", &self.reports.len())
            .finish_non_exhaustive()
    }
}

impl HistoryWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }

    fn answer(&self) -> &RepairAnswer {
        self.answer.as_ref().expect("repair answer")
    }

    fn state(&self) -> &ApiState {
        self.state.as_ref().expect("api state")
    }
}

fn context() -> ProvenanceContext {
    ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    }
}

/// The record of `version` of the object at `root`.
fn record_at(rig: &Rig, root: &str, version: &str) -> Option<Record> {
    let object = ready(Object::open(&rig.root, root)).expect("open");
    ready(read_record(&object, &v(version))).expect("read")
}

/// The journal entries the version `version` of the object at `root` carries.
fn carried(rig: &Rig, root: &str, version: &str) -> Vec<JournalEntry> {
    let object = ready(Object::open(&rig.root, root)).expect("open");
    let path = journal_path(version);
    let mut reader = object
        .stream_file(&v(version), &path, MAX_CHUNK_BYTES)
        .expect("journal file");
    let mut bytes = Vec::new();
    for _ in 0..MAX_CHUNKS {
        match ready(reader.next_chunk()).expect("chunk") {
            Some(chunk) => bytes.extend_from_slice(&chunk),
            None => break,
        }
    }
    String::from_utf8(bytes)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect()
}

#[given(
    expr = "a library with an object whose head has no version inventory, recording provenance"
)]
fn w010_recording(w: &mut MonitorWorld) {
    let mut rig = Rig::with_root();
    let ids = rig.journal.clone();
    rig.service = rig.service.clone().with_provenance(context(), ids);
    rig.w010_object(W010.0, W010.1);
    w.history.rig = Some(rig);
}

#[given(expr = "a library with an object whose head has no version inventory, without provenance")]
fn w010_unrecorded(w: &mut MonitorWorld) {
    let rig = Rig::with_root();
    rig.w010_object(W010.0, W010.1);
    w.history.rig = Some(rig);
}

#[when(expr = "the owner repairs that object")]
fn owner_repairs(w: &mut MonitorWorld) {
    let rig = w.history.rig();
    let request = rig.consented(&[W010.1]);
    let answer = ready(rig.service.repair_version_inventory(
        &Principal::Owner,
        request,
        &rig.root,
        &rig.leases,
    ))
    .expect("answered");
    w.history.answer = Some(answer);
}

#[then(expr = "a new version of the object should hold a PROV-O record of kind {string}")]
fn record_of_kind(w: &mut MonitorWorld, kind: String) {
    let one = &w.history.answer().objects[0];
    let version = one
        .provenance_version
        .as_deref()
        .expect("a provenance version");
    assert_eq!(version, "v3");
    let record = record_at(w.history.rig(), W010.0, version).expect("record");
    assert_eq!(record.activity().kind(), kind);
}

#[then(expr = "the record, the consent and the outcome should share the repair run ID")]
fn shared_run_id(w: &mut MonitorWorld) {
    let rig = w.history.rig();
    let run_id = w.history.answer().run_id.as_str();
    let record = record_at(rig, W010.0, "v3").expect("record");
    assert_eq!(record.activity().operation(), run_id);
    assert_eq!(
        rig.ledger.consents()[0].operation_id.as_deref(),
        Some(run_id)
    );
    let outcome = &rig.ledger.outcomes()[0].journal;
    assert_eq!(outcome.operation_id.as_deref(), Some(run_id));
}

#[then(
    expr = "the new version should carry its {string} journal entry under the ID the journal stored"
)]
fn carried_entry(w: &mut MonitorWorld, operation: String) {
    let rig = w.history.rig();
    let carried = carried(rig, W010.0, "v3");
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].operation, operation);
    let stored = rig.journaled(&operation);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, carried[0].id);
    assert!(stored[0].id.is_some());
}

#[then(expr = "the repair answer should name the provenance error {string}")]
fn provenance_error(w: &mut MonitorWorld, code: String) {
    let one = &w.history.answer().objects[0];
    assert_eq!(one.provenance_error_code.as_deref(), Some(code.as_str()));
    assert_eq!(one.provenance_version, None);
}

#[then(expr = "a {string} warning should be journaled under the repair run ID")]
fn warning_journaled(w: &mut MonitorWorld, operation: String) {
    let rig = w.history.rig();
    let entries = rig.journaled(&operation);
    assert_eq!(entries.len(), 1);
    let run_id = w.history.answer().run_id.as_str();
    assert_eq!(entries[0].operation_id.as_deref(), Some(run_id));
    assert_eq!(entries[0].level, filesquid_api::journal::Level::Warn);
}

#[given(expr = "a library of {int} objects committed before the history existed")]
fn plain_library(w: &mut MonitorWorld, count: usize) {
    let rig = Rig::with_root();
    for n in 0..count {
        rig.object(&format!("obj-{n}"), &format!("urn:uuid:{n}"));
    }
    w.history.rig = Some(rig);
}

/// Run the backfill once, capped at `cap` records, and keep its report.
fn run_backfill(w: &mut MonitorWorld, cap: usize) {
    let world = &mut w.history;
    let rig = world.rig.as_ref().expect("rig");
    let registry = world.registry.get_or_insert_with(|| {
        ProcessRegistry::new(
            rig.sender(),
            EventBus::new(),
            Arc::new(Metrics::default()),
            Arc::new(|| NOW_MS),
        )
    });
    world.commits_before = rig.journaled("ocfl.commit").len();
    let job = Backfill::new(
        context(),
        rig.journal.clone(),
        rig.sender(),
        registry.clone(),
    )
    .with_clock(Arc::new(|| NOW_MS + 600_000))
    .with_max_versions(cap);
    let report = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("run");
    world.reports.push(report);
}

#[when(expr = "the history backfill runs")]
fn backfill_runs(w: &mut MonitorWorld) {
    run_backfill(w, 1_000);
}

#[when(expr = "the history backfill runs again")]
fn backfill_runs_again(w: &mut MonitorWorld) {
    run_backfill(w, 1_000);
}

#[when(expr = "the history backfill runs with a cap of {int} records")]
fn backfill_capped(w: &mut MonitorWorld, cap: usize) {
    run_backfill(w, cap);
}

#[then(
    expr = "every version should have a record and the earlier ones should be marked reconstructed"
)]
fn all_recorded(w: &mut MonitorWorld) {
    let rig = w.history.rig();
    for n in 0..2 {
        let root = format!("obj-{n}");
        let object = ready(Object::open(&rig.root, &root)).expect("open");
        assert!(missing_records(&object).is_empty(), "{root}");
        let first = record_at(rig, &root, "v1").expect("v1 record");
        assert_eq!(
            first.activity().derivation(),
            Some(Derivation::Reconstructed)
        );
    }
}

#[then(expr = "the backfill process should end {string} with {int} of {int} objects done")]
fn process_ended(w: &mut MonitorWorld, state: String, done: u64, total: u64) {
    let report = w.history.reports.last().expect("report");
    let registry = w.history.registry.as_ref().expect("registry");
    let process = registry
        .snapshot()
        .into_iter()
        .find(|p| p.id == report.process_id)
        .expect("registered");
    let expected = match state.as_str() {
        "completed" => ProcessState::Completed,
        "cancelled" => ProcessState::Cancelled,
        other => panic!("unknown process state {other}"),
    };
    assert_eq!(process.state, expected);
    assert_eq!((process.done, process.total), (done, Some(total)));
}

#[then(expr = "the second run should commit nothing")]
fn commits_nothing(w: &mut MonitorWorld) {
    let report = w.history.reports.last().expect("report");
    assert!(report.objects.is_empty() && report.failures.is_empty());
    let commits = w.history.rig().journaled("ocfl.commit").len();
    assert_eq!(commits, w.history.commits_before);
}

#[then(expr = "the run should report that it reached its cap")]
fn reached_cap(w: &mut MonitorWorld) {
    assert!(w.history.reports.last().expect("report").reached_cap);
}

/// The record of `version` of `file`, committed at minute `minute`.
fn record_of(file: &str, version: &str, minute: u64) -> Record {
    let number: u64 = version[1..].parse().expect("version number");
    let at = filesquid_ocfl::write::rfc3339_utc(1_791_590_400_000 + minute * 60_000);
    let facts = VersionFacts {
        object_id: format!("urn:uuid:{file}"),
        version: version.into(),
        base_version: (number > 1).then(|| format!("v{}", number - 1)),
        created: at.clone(),
        started: at,
        operation_id: format!("op-{minute}"),
        kind: "import".into(),
        new_state: [(format!("Holiday/photo {number}.jpg"), "d".repeat(64))].into(),
        installation: Some(INSTALLATION.into()),
        journal_entries: vec![minute.to_string()],
        ..VersionFacts::default()
    };
    build_record(&facts).expect("record")
}

#[given(expr = "a history holding {int} versions of one file and {int} version of another")]
fn history_of_two_files(w: &mut MonitorWorld, first: u64, second: u64) {
    let history = MemoryHistory::new();
    for n in 1..=first {
        history
            .insert(record_of(FIRST, &format!("v{n}"), n))
            .expect("insert");
    }
    for n in 1..=second {
        history
            .insert(record_of(SECOND, &format!("v{n}"), first + n))
            .expect("insert");
    }
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    state.install_history(Arc::new(history));
    w.history.state = Some(state);
}

#[given(expr = "a host that has not installed its history yet")]
fn no_history(w: &mut MonitorWorld) {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    w.history.state = Some(state);
}

/// GET `path` through the router; keep the status, the JSON body (if any) and the bytes.
async fn get(w: &mut MonitorWorld, path: &str) {
    let request = Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = router(w.history.state().clone())
        .oneshot(request)
        .await
        .expect("router answers");
    w.statuses.push(response.status().as_u16());
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    w.json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    w.history.body = body.to_vec();
}

#[when(expr = "the caller lists the history of the first file")]
async fn list_first(w: &mut MonitorWorld) {
    get(w, &format!("/api/v1/files/{FIRST}/history")).await;
}

#[then(expr = "the answer should hold {int} entries of that file, oldest first")]
fn entries_of_file(w: &mut MonitorWorld, count: usize) {
    assert_eq!(w.statuses.last(), Some(&200));
    let items = w.json["items"].as_array().expect("items");
    assert_eq!(items.len(), count);
    let versions: Vec<&str> = items
        .iter()
        .map(|item| item["version"]["version"].as_str().expect("version"))
        .collect();
    assert_eq!(versions, ["v1", "v2"]);
    assert!(items
        .iter()
        .all(|item| item["version"]["object_id"] == format!("urn:uuid:{FIRST}")));
}

#[when(expr = "the caller lists the whole history {int} entries at a time")]
async fn list_paged(w: &mut MonitorWorld, limit: usize) {
    let mut path = format!("/api/v1/history?limit={limit}");
    for _ in 0..10 {
        get(w, &path).await;
        let page = w.json.clone();
        w.history.pages.push(page.clone());
        let Some(cursor) = page["next_cursor"].as_str() else {
            return;
        };
        path = format!(
            "/api/v1/history?limit={limit}&cursor={}",
            cursor.replace(' ', "%20")
        );
    }
    panic!("more pages than the scenario holds");
}

#[then(expr = "the pages should hold {int} and then {int} entries and the last page no cursor")]
fn paged(w: &mut MonitorWorld, first: usize, second: usize) {
    let sizes: Vec<usize> = w
        .history
        .pages
        .iter()
        .map(|page| page["items"].as_array().expect("items").len())
        .collect();
    assert_eq!(sizes, [first, second]);
    assert!(w.history.pages.last().expect("page")["next_cursor"].is_null());
}

#[when(expr = "the caller reads the record of version {string} of the first file")]
async fn read_version(w: &mut MonitorWorld, version: String) {
    get(w, &format!("/api/v1/files/{FIRST}/versions/{version}/prov")).await;
}

#[then(expr = "the answer should be the JSON-LD record of that version")]
fn json_ld_record(w: &mut MonitorWorld) {
    assert_eq!(w.statuses.last(), Some(&200));
    let record = parse_record(&w.history.body).expect("a filesquid-prov/1 record");
    assert_eq!(record, record_of(FIRST, "v2", 2));
}

#[when(expr = "the caller lists the history by object and by operation at once")]
async fn two_filters(w: &mut MonitorWorld) {
    get(
        w,
        &format!("/api/v1/history?object=urn:uuid:{FIRST}&operation=op-1"),
    )
    .await;
}

#[when(expr = "the caller exports the history of the first file redacted")]
async fn export_redacted(w: &mut MonitorWorld) {
    get(
        w,
        &format!("/api/v1/files/{FIRST}/history/export?redact=true"),
    )
    .await;
}

#[then(
    expr = "the export should be a JSON-LD array of {int} records without the installation or any path"
)]
fn redacted_export(w: &mut MonitorWorld, count: usize) {
    assert_eq!(w.statuses.last(), Some(&200));
    assert_eq!(w.json.as_array().expect("array").len(), count);
    let text = String::from_utf8_lossy(&w.history.body);
    assert!(!text.contains(INSTALLATION), "{text}");
    assert!(
        !text.contains("Holiday") && !text.contains("photo"),
        "{text}"
    );
}
