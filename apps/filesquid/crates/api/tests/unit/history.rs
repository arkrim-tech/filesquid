//! Task API-F2.9: PROV-O records of commits, the history sources, the history routes and
//! redaction for shared outputs (API-S-028, API-S-029, API-S-032; PRD F-067, F-046; ADR 040
//! Decisions 3 to 9).

#[path = "../support/history_fixture.rs"]
mod history_fixture;

use axum::body::Body;
use axum::http::{header, Request};
use filesquid_api::auth::Authoriser;
use filesquid_api::history::{
    commit_meta, entry_of, journal_path, redact_record, version_user, CommitOperation,
    CommitRecorder, HistoryError, HistorySource, MemoryHistory, OcflHistory, ProvenanceContext,
    Redactor, INSTALLATION_USER_NAME, JSON_LD, PROV_RECORD_OPERATION, REDACTED_INSTALLATION,
};
use filesquid_api::journal::{JournalEntry, Level, Outcome, REDACTED};
use filesquid_api::{router, ApiState};
use filesquid_core::{InstallationId, OperationId, OperationKind, OperationMessage};
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{
    complete_immediately, MemoryBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::write::{rfc3339_utc, CommitReceipt, LeaseConfig, LeaseStore, SystemClock};
use filesquid_ocfl::{Declaration, SpecVersion, VersionNum};
use filesquid_prov::ocfl::{read_record, record_path};
use filesquid_prov::{build_record, parse_record, HostFacts, Record, VersionFacts};
use history_fixture::{sample_record, SAMPLE_FILE, SAMPLE_INSTALLATION};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

const OBJECT_ID: &str = "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
const ROOT: &str = "objects/a";
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const TOKEN: &str = "history-token";

/// Drive a future of the in-memory backend.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("in-memory futures complete at once")
}

fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// `created` time number `n`, a minute apart.
fn created(n: u64) -> String {
    rfc3339_utc(1_791_590_400_000 + n * 60_000)
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

fn operation(id: &str, kind: OperationKind) -> CommitOperation {
    CommitOperation::new(OperationId::new(id).expect("id"), kind, created(0))
}

/// A storage root on the in-memory backend with leases and staging.
struct Rig {
    storage: MemoryBackend,
    leases: LeaseStore<MemoryBackend, SystemClock>,
    staging: MemoryBackend,
}

impl Rig {
    fn new() -> Self {
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            SystemClock,
            LeaseConfig::default(),
        )
        .expect("leases");
        Self {
            storage: MemoryBackend::new(),
            leases,
            staging: MemoryBackend::new(),
        }
    }

    fn model(
        &self,
        holder: &str,
    ) -> ChangeModel<'_, MemoryBackend, LeaseStore<MemoryBackend, SystemClock>, MemoryBackend> {
        let staging = StagingArea::new(&self.staging, &format!("changes/{holder}")).unwrap();
        ChangeModel::new(
            &self.storage,
            &self.leases,
            holder,
            ROOT,
            OBJECT_ID,
            staging,
        )
    }

    /// Declare the backend an OCFL 1.1 storage root.
    fn storage_root(&self) {
        let declaration = Declaration::storage_root(SpecVersion::V1_1);
        let key = StorageKey::new(&declaration.file_name()).expect("key");
        ready(self.storage.put(&key, declaration.contents().as_bytes())).expect("declare");
    }

    fn object(&self) -> Object<'_, MemoryBackend> {
        ready(Object::open(&self.storage, ROOT)).expect("open")
    }

    /// Commit `files` without a record, as before the history existed.
    fn commit_plain(&self, files: &[(&str, &[u8])], n: u64) -> VersionNum {
        let model = self.model("plain");
        for (path, bytes) in files {
            ready(model.stage_bytes(path, bytes)).unwrap();
        }
        ready(model.commit(VersionMeta::new(created(n))))
            .unwrap()
            .version
    }

    /// The bytes of the logical file `path` in `version`.
    fn logical(&self, version: &str, path: &str) -> Vec<u8> {
        let object = self.object();
        let mut reader = object
            .stream_file(&v(version), path, MAX_CHUNK_BYTES)
            .expect("logical file");
        let mut bytes = Vec::new();
        for _ in 0..64 {
            match ready(reader.next_chunk()).expect("chunk") {
                Some(chunk) => bytes.extend_from_slice(&chunk),
                None => return bytes,
            }
        }
        panic!("logical file longer than the test reads");
    }
}

/// The journal entries of a `.journal.jsonl` file.
fn journal_lines(bytes: &[u8]) -> Vec<JournalEntry> {
    String::from_utf8(bytes.to_vec())
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect()
}

// ---- Version metadata ----

#[test]
fn the_version_user_names_the_installation_and_never_a_person() {
    let installation = InstallationId::parse(INSTALLATION).unwrap();
    let user = version_user(&installation);
    assert_eq!(user.name, INSTALLATION_USER_NAME);
    assert_eq!(
        user.address.as_deref(),
        Some(format!("urn:uuid:{INSTALLATION}").as_str())
    );
}

#[test]
fn commit_meta_sets_the_user_and_the_operation_message() {
    let op = operation("op-1", OperationKind::MetadataEdit).with_label("Final cut");
    let meta = commit_meta(&context(), &op, created(2));
    assert_eq!(meta.created, created(2));
    assert_eq!(meta.user, Some(version_user(&context().installation)));
    let message = OperationMessage::parse(meta.message.as_deref().unwrap())
        .unwrap()
        .expect("a FileSquid message");
    assert_eq!(message.operation_id().as_str(), "op-1");
    assert_eq!(message.kind(), OperationKind::MetadataEdit);
    assert_eq!(message.label(), Some("Final cut"));
}

// ---- The recorder ----

#[test]
fn a_commit_writes_its_record_and_journal_entries_in_the_same_version() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], 1);
    let model = rig.model("writer");
    ready(model.stage_bytes("a.txt", b"alpha two")).unwrap();
    let op = operation("op-2", OperationKind::Upload).with_actor("owner");
    let recorder = CommitRecorder::new(context(), op.clone(), 41);
    let meta = commit_meta(&context(), &op, created(2));
    let receipt = ready(model.commit_with(meta, Some(&recorder))).unwrap();
    assert_eq!(receipt.version, v("v2"));
    let record = ready(read_record(&rig.object(), &v("v2")))
        .unwrap()
        .expect("record");
    assert_eq!(record.activity().operation(), "op-2");
    assert_eq!(record.activity().kind(), "upload");
    assert_eq!(record.activity().journal_entries(), ["41"]);
    assert!(record
        .agents()
        .iter()
        .any(|agent| agent.id() == format!("urn:uuid:{INSTALLATION}")));
    let lines = journal_lines(&rig.logical("v2", &journal_path("v2")));
    assert_eq!(lines.len(), 1);
    let entry = &lines[0];
    assert_eq!(entry.id, Some(41));
    assert_eq!(entry.operation, "ocfl.commit");
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(entry.actor, "owner");
    assert_eq!(entry.operation_id.as_deref(), Some("op-2"));
    assert_eq!(
        entry.object_ref.as_deref(),
        Some(format!("{OBJECT_ID}@v2").as_str())
    );
    let appended = recorder.journal_entries(&receipt);
    assert_eq!(
        appended, lines,
        "the caller appends what the version carries"
    );
}

#[test]
fn a_rebased_commit_regenerates_both_files_for_the_new_version() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], 1);
    let mine = rig.model("mine");
    ready(mine.stage_bytes("b.txt", b"bravo")).unwrap();
    rig.commit_plain(&[("c.txt", b"charlie")], 2);
    let op = operation("op-3", OperationKind::Import);
    let recorder = CommitRecorder::new(context(), op.clone(), 9);
    let receipt =
        ready(mine.commit_with(commit_meta(&context(), &op, created(3)), Some(&recorder))).unwrap();
    assert_eq!(receipt.version, v("v3"));
    let state = rig.object().state(&v("v3")).unwrap();
    assert!(state.get(&record_path("v3")).is_some());
    assert!(state.get(&journal_path("v3")).is_some());
    assert!(state.get(&journal_path("v2")).is_none());
    let lines = journal_lines(&rig.logical("v3", &journal_path("v3")));
    assert_eq!(
        lines[0].object_ref.as_deref(),
        Some(format!("{OBJECT_ID}@v3").as_str())
    );
    assert_eq!(recorder.journal_entries(&receipt), lines);
}

#[test]
fn a_takeover_adds_a_warning_after_the_commit_entry() {
    let rig = Rig::new();
    let model = rig.model("writer");
    ready(model.stage_bytes("a.txt", b"alpha")).unwrap();
    let op = operation("op-4", OperationKind::Import);
    let recorder = CommitRecorder::new(context(), op.clone(), 5);
    let mut receipt =
        ready(model.commit_with(commit_meta(&context(), &op, created(1)), Some(&recorder)))
            .unwrap();
    receipt.took_over = true;
    let entries = recorder.journal_entries(&receipt);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, Some(5));
    assert_eq!(entries[1].operation, "ocfl.lease.takeover");
    assert_eq!(entries[1].level, Level::Warn);
    assert_eq!(entries[1].id, None, "the store assigns the warning's id");
    assert_eq!(entries[1].object_ref, entries[0].object_ref);
}

#[test]
fn a_receipt_for_another_version_still_journals_the_commit() {
    let recorder = CommitRecorder::new(context(), operation("op-5", OperationKind::Import), 3);
    let receipt = CommitReceipt {
        object_id: OBJECT_ID.into(),
        version: v("v7"),
        created: created(7),
        inventory_digest: "0".repeat(64),
        written: Vec::new(),
        took_over: false,
    };
    let entries = recorder.journal_entries(&receipt);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].id, None,
        "never reuse an id another version carries"
    );
    assert_eq!(
        entries[0].object_ref.as_deref(),
        Some(format!("{OBJECT_ID}@v7").as_str())
    );
}

#[test]
fn a_record_that_cannot_be_built_stops_the_commit() {
    let rig = Rig::new();
    let model = rig.model("writer");
    ready(model.stage_bytes("a.txt", b"alpha")).unwrap();
    let mut op = operation("op-6", OperationKind::Import);
    op.started = created(9);
    let recorder = CommitRecorder::new(context(), op.clone(), 1);
    let refused =
        ready(model.commit_with(commit_meta(&context(), &op, created(1)), Some(&recorder)));
    assert!(refused.is_err(), "a start after the commit is refused");
    assert!(
        ready(Object::open(&rig.storage, ROOT)).is_err(),
        "nothing published"
    );
}

// ---- Journal redaction of every new event type ----

#[test]
fn every_new_journal_event_keeps_its_opaque_fields_and_loses_paths() {
    let events = [
        JournalEntry::new("ocfl.commit", Outcome::Succeeded),
        JournalEntry::new(PROV_RECORD_OPERATION, Outcome::Info).level(Level::Warn),
        JournalEntry::new(PROV_RECORD_OPERATION, Outcome::Failed),
        JournalEntry::new("prov.backfill", Outcome::Started),
        JournalEntry::new("prov.backfill", Outcome::Failed),
        JournalEntry::new("prov.backfill", Outcome::Succeeded),
        JournalEntry::new("prov.backfill", Outcome::Cancelled),
    ];
    for event in events {
        let operation = event.operation.clone();
        let kept = event
            .operation_id("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b")
            .object_ref(format!("{OBJECT_ID}@v3"))
            .error_code("provenance_commit_failed")
            .target(OBJECT_ID)
            .target("Holiday/Anna.jpg")
            .redacted();
        assert_eq!(kept.operation, operation);
        assert_eq!(
            kept.object_ref.as_deref(),
            Some(format!("{OBJECT_ID}@v3").as_str())
        );
        assert_eq!(kept.target_ids, [OBJECT_ID, REDACTED], "{operation}");
        let text = serde_json::to_string(&kept).unwrap();
        assert!(!text.contains("Anna"), "{text}");
    }
}

// ---- Redaction of records (PRD F-046) ----

/// A record whose activity carries a label and whose entities carry identity-bearing paths.
fn personal_record(version: &str, base: Option<&str>) -> Record {
    let mut facts = VersionFacts {
        object_id: OBJECT_ID.into(),
        version: version.into(),
        created: created(2),
        started: created(1),
        operation_id: "op-9".into(),
        kind: "rename".into(),
        new_state: [("Holiday/Anna Smith.jpg".to_string(), "b".repeat(64))].into(),
        installation: Some(INSTALLATION.into()),
        message: Some(r#"filesquid.operation.v1 {"operation_id":"op-9","kind":"rename","label":"Anna's birthday"}"#.into()),
        journal_entries: vec!["12".into()],
        ..VersionFacts::default()
    };
    if let Some(base) = base {
        facts.base_version = Some(base.into());
        facts.base_state = [("Holiday/Anna Smith.jpg".to_string(), "a".repeat(64))].into();
    }
    build_record(&facts).expect("record")
}

#[test]
fn a_redacted_record_names_no_installation_message_or_path() {
    let record = personal_record("v2", Some("v1"));
    let text = filesquid_prov::to_json_ld(&record);
    assert!(text.contains(INSTALLATION) && text.contains("Anna"));
    let redacted = redact_record(text.as_bytes()).expect("redacted");
    assert!(!redacted.contains(INSTALLATION), "{redacted}");
    assert!(!redacted.contains("Anna"), "{redacted}");
    assert!(!redacted.contains("Holiday"), "{redacted}");
    assert!(!redacted.contains("fsq:message"), "{redacted}");
    assert!(redacted.contains(REDACTED_INSTALLATION));
    assert!(redacted.contains(OBJECT_ID), "object IDs are kept");
    assert!(redacted.contains("op-9"), "operation IDs are kept");
    let value: Value = serde_json::from_str(&redacted).unwrap();
    assert!(value["@graph"].is_array(), "the JSON-LD shape is kept");
}

#[test]
fn redaction_keeps_derivations_linked_and_paths_numbered_across_records() {
    let mut redactor = Redactor::new();
    let first = redactor
        .redact(filesquid_prov::to_json_ld(&personal_record("v2", Some("v1"))).as_bytes())
        .unwrap();
    let second = redactor
        .redact(filesquid_prov::to_json_ld(&personal_record("v3", Some("v2"))).as_bytes())
        .unwrap();
    let paths = |doc: &Value| -> Vec<String> {
        doc["@graph"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|node| node["fsq:path"].as_str().map(str::to_string))
            .collect()
    };
    assert!(paths(&first).iter().all(|p| p == "redacted-1"));
    assert!(paths(&second).iter().all(|p| p == "redacted-1"));
    let generated = format!("urn:filesquid:entity:{OBJECT_ID}:v2:redacted-1");
    assert!(first.to_string().contains(&generated));
    assert!(
        second.to_string().contains(&generated),
        "v3 derives from the same v2 entity"
    );
}

// ---- History sources ----

/// A record of `version` of `object` by operation `op`, committed at minute `n`.
fn record_of(object: &str, version: &str, op: &str, n: u64) -> Record {
    let number: u64 = version[1..].parse().unwrap();
    let mut facts = VersionFacts {
        object_id: object.into(),
        version: version.into(),
        created: created(n),
        started: created(n),
        operation_id: op.into(),
        kind: "import".into(),
        new_state: [(format!("f{number}.txt"), "c".repeat(64))].into(),
        installation: Some(INSTALLATION.into()),
        journal_entries: vec![number.to_string()],
        ..VersionFacts::default()
    };
    if number > 1 {
        facts.base_version = Some(format!("v{}", number - 1));
    }
    build_record(&facts).expect("record")
}

#[test]
fn a_memory_history_indexes_its_records_and_refuses_conflicts() {
    let history = MemoryHistory::new();
    history
        .insert(record_of(OBJECT_ID, "v1", "op-1", 1))
        .unwrap();
    history
        .insert(record_of(OBJECT_ID, "v1", "op-1", 1))
        .expect("the same record again");
    assert!(history
        .insert(record_of(OBJECT_ID, "v1", "op-1", 2))
        .is_err());
    let index = history.index().unwrap();
    assert_eq!(index.len(), 1);
    let bytes = history.record(OBJECT_ID, "v1").unwrap().expect("record");
    assert_eq!(
        parse_record(&bytes).unwrap(),
        record_of(OBJECT_ID, "v1", "op-1", 1)
    );
    assert_eq!(history.record(OBJECT_ID, "v2").unwrap(), None);
}

#[test]
fn an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root() {
    let rig = Rig::new();
    rig.storage_root();
    rig.commit_plain(&[("a.txt", b"alpha")], 1);
    let model = rig.model("writer");
    ready(model.stage_bytes("b.txt", b"bravo")).unwrap();
    let op = operation("op-7", OperationKind::Import);
    let recorder = CommitRecorder::new(context(), op.clone(), 2);
    ready(model.commit_with(commit_meta(&context(), &op, created(2)), Some(&recorder))).unwrap();
    drop(model);
    let Rig { storage, .. } = rig;
    let history = OcflHistory::new(storage);
    let index = history.index().expect("index");
    assert_eq!(index.len(), 1, "v1 has no record yet");
    let bytes = history.record(OBJECT_ID, "v2").unwrap().expect("v2 record");
    assert_eq!(parse_record(&bytes).unwrap().activity().operation(), "op-7");
    assert_eq!(history.record(OBJECT_ID, "v1").unwrap(), None);
    assert_eq!(history.record("urn:uuid:other", "v2").unwrap(), None);
}

#[test]
fn an_ocfl_history_without_a_storage_root_is_unavailable() {
    let history = OcflHistory::new(MemoryBackend::new());
    assert_eq!(history.index().err(), Some(HistoryError::Unavailable));
}

#[test]
fn a_history_entry_carries_the_label_and_marks_reconstruction() {
    let record = personal_record("v2", Some("v1"));
    let entry = entry_of(record.activity()).unwrap();
    assert_eq!(entry.kind, OperationKind::Rename);
    assert_eq!(entry.label.as_deref(), Some("Anna's birthday"));
    assert_eq!(entry.version.object_ref(), format!("{OBJECT_ID}@v2"));
    assert_eq!(entry.journal.journal_entry_ids(), ["12"]);
    assert!(entry.started_at.is_some());
    assert!(!entry.reconstructed);
}

// ---- Routes ----

/// Authorised state with `records` installed as the history.
fn state_with(records: Vec<Record>) -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    let history = MemoryHistory::new();
    for record in records {
        history.insert(record).unwrap();
    }
    state.install_history(Arc::new(history));
    state
}

/// GET `path`: status, content type and body.
async fn get(state: &ApiState, path: &str) -> (u16, String, Vec<u8>) {
    let request = Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let response = router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, kind, body.to_vec())
}

fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("json")
}

const OTHER: &str = "urn:uuid:11111111-1111-4111-8111-111111111111";

fn three_records() -> Vec<Record> {
    vec![
        record_of(OBJECT_ID, "v1", "op-a", 1),
        record_of(OBJECT_ID, "v2", "op-b", 2),
        record_of(OTHER, "v1", "op-b", 3),
    ]
}

#[tokio::test]
async fn the_history_lists_everything_or_one_filter_in_cursor_pages() {
    let state = state_with(three_records());
    let (status, _, body) = get(&state, "/api/v1/history").await;
    assert_eq!(status, 200);
    assert_eq!(json(&body)["items"].as_array().unwrap().len(), 3);
    let (_, _, body) = get(&state, "/api/v1/history?operation=op-b").await;
    let items = json(&body)["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|item| item["operation_id"] == "op-b"));
    let agent = format!("/api/v1/history?agent=urn:uuid:{INSTALLATION}");
    assert_eq!(
        json(&get(&state, &agent).await.2)["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let range = format!("/api/v1/history?from={}&to={}", created(2), created(3));
    let (_, _, body) = get(&state, &range).await;
    assert_eq!(json(&body)["items"][0]["version"]["version"], "v2");
    let (_, _, body) = get(&state, "/api/v1/history?limit=2").await;
    let page = json(&body);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let cursor = page["next_cursor"].as_str().expect("more");
    let next = format!(
        "/api/v1/history?limit=2&cursor={}",
        cursor.replace(' ', "%20")
    );
    let (_, _, body) = get(&state, &next).await;
    let last = json(&body);
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert!(last["next_cursor"].is_null());
}

#[tokio::test]
async fn bad_history_parameters_answer_invalid_request() {
    let state = state_with(three_records());
    for query in [
        "?object=a&operation=b",
        "?agent=a&from=2026-01-01T00:00:00Z",
        "?limit=0",
        "?limit=1001",
        "?cursor=nope",
        "?from=yesterday",
    ] {
        let (status, _, body) = get(&state, &format!("/api/v1/history{query}")).await;
        assert_eq!(status, 400, "{query}");
        assert_eq!(json(&body)["code"], "invalid_request", "{query}");
    }
}

#[tokio::test]
async fn a_file_history_lists_its_object_only() {
    let state = state_with(three_records());
    let path = format!("/api/v1/files/{}/history", &OBJECT_ID[9..]);
    let (status, _, body) = get(&state, &path).await;
    assert_eq!(status, 200);
    let items = json(&body)["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .all(|item| item["version"]["object_id"] == OBJECT_ID));
    let (status, _, _) = get(&state, "/api/v1/files/a%20b/history").await;
    assert_eq!(status, 400, "an id that is not opaque");
}

#[tokio::test]
async fn a_version_record_is_raw_json_ld_or_not_found() {
    let state = state_with(three_records());
    let path = format!("/api/v1/files/{}/versions/v2/prov", &OBJECT_ID[9..]);
    let (status, kind, body) = get(&state, &path).await;
    assert_eq!(status, 200);
    assert_eq!(kind, JSON_LD);
    assert_eq!(
        parse_record(&body).unwrap(),
        record_of(OBJECT_ID, "v2", "op-b", 2)
    );
    let missing = format!("/api/v1/files/{}/versions/v9/prov", &OBJECT_ID[9..]);
    let (status, _, body) = get(&state, &missing).await;
    assert_eq!(status, 404);
    assert_eq!(json(&body)["code"], "not_found");
    let bad = format!("/api/v1/files/{}/versions/latest/prov", &OBJECT_ID[9..]);
    assert_eq!(get(&state, &bad).await.0, 400);
}

#[tokio::test]
async fn the_export_is_a_json_ld_array_and_can_be_redacted() {
    let record = sample_record();
    let state = state_with(vec![record]);
    let path = format!("/api/v1/files/{SAMPLE_FILE}/history/export");
    let (status, kind, body) = get(&state, &path).await;
    assert_eq!(status, 200);
    assert_eq!(kind, JSON_LD);
    let records = json(&body);
    assert_eq!(records.as_array().unwrap().len(), 1);
    assert!(String::from_utf8_lossy(&body).contains(SAMPLE_INSTALLATION));
    let (status, _, body) = get(&state, &format!("{path}?redact=true")).await;
    assert_eq!(status, 200);
    let text = String::from_utf8_lossy(&body);
    assert!(!text.contains(SAMPLE_INSTALLATION), "{text}");
    assert!(!text.contains("photo.jpg"), "{text}");
}

#[tokio::test]
async fn history_routes_answer_unavailable_before_the_host_installs_a_source() {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    for path in [
        "/api/v1/history".to_string(),
        format!("/api/v1/files/{SAMPLE_FILE}/history"),
        format!("/api/v1/files/{SAMPLE_FILE}/history/export"),
        format!("/api/v1/files/{SAMPLE_FILE}/versions/v1/prov"),
    ] {
        let (status, _, body) = get(&state, &path).await;
        assert_eq!(status, 503, "{path}");
        assert_eq!(json(&body)["retryable"], true, "{path}");
    }
}
