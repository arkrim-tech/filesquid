//! Contract drift tests (T-048, ADR 030 Decision 2, API-S-002): the Rust routes, the Rust
//! payload types and the checked-in OpenAPI 3.1 document MUST agree. The generated
//! TypeScript types are checked against the same document by the UI package's Vitest test
//! (`apps/filesquid/ui/tests/unit/contract.test.ts`). Both run in `scripts/pre-commit.sh`.

#[path = "schema_check.rs"]
mod schema_check;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use filesquid_api::health::{HealthInputs, HealthReason, HealthReport, HealthStatus};
use filesquid_api::journal::{JournalEntry, JournalPage, Level, Outcome};
use filesquid_api::operation::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationId,
    OperationScope, PageSize, PortError, QueryEnvelope, MAX_PAGE_SIZE, PROTOCOL_SCHEMA_VERSION,
};
use filesquid_api::registry::{ProcessState, ProcessView};
use filesquid_api::{
    routed_paths, router, ApiError, ApiEvent, ApiState, ErrorCode, API_VERSION, CONTRACT,
};
use schema_check::{required, unsupported_keywords, Checker};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tower::ServiceExt;

/// OpenAPI extension that marks a path served by the hosts, not by [`router`].
const HOST_ROUTE: &str = "x-filesquid-host-route";

/// HTTP methods an OpenAPI path item may document.
const METHODS: &[&str] = &["get", "put", "post", "delete", "patch", "head", "options"];

/// Parse the embedded OpenAPI contract.
fn contract() -> Value {
    serde_json::from_str(CONTRACT).expect("openapi.json is valid JSON")
}

fn schema<'a>(doc: &'a Value, name: &str) -> &'a Value {
    let found = &doc["components"]["schemas"][name];
    assert!(found.is_object(), "components.schemas.{name} is documented");
    found
}

fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("serialize")
}

#[test]
fn contract_is_openapi_3_1_with_the_crate_api_version() {
    let doc = contract();
    assert_eq!(doc["openapi"], "3.1.0");
    assert_eq!(doc["info"]["version"], API_VERSION);
    assert_eq!(API_VERSION, "0.3.0");
}

#[test]
fn routed_and_documented_paths_agree_both_ways() {
    let doc = contract();
    let documented: BTreeSet<String> = doc["paths"]
        .as_object()
        .expect("paths")
        .iter()
        .filter(|(_, item)| item.get(HOST_ROUTE) != Some(&Value::Bool(true)))
        .map(|(path, _)| path.clone())
        .collect();
    let routed: BTreeSet<String> = routed_paths().into_iter().collect();
    assert!(!routed.is_empty(), "the router serves at least one path");
    assert_eq!(documented, routed, "contract paths versus router paths");
}

#[test]
fn host_routes_are_the_event_channel_only() {
    let doc = contract();
    let host: Vec<&String> = doc["paths"]
        .as_object()
        .expect("paths")
        .iter()
        .filter(|(_, item)| item.get(HOST_ROUTE) == Some(&Value::Bool(true)))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(host, vec!["/api/v1/events"]);
}

/// Every documented operation of a routed path, as (method, path).
fn routed_operations(doc: &Value) -> Vec<(Method, String)> {
    let mut operations = Vec::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        if item.get(HOST_ROUTE).is_some() {
            continue;
        }
        for method in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            let method = Method::from_bytes(method.to_uppercase().as_bytes()).expect("method");
            operations.push((method, path.clone()));
        }
    }
    operations
}

#[tokio::test]
async fn every_documented_operation_is_answered_by_the_router() {
    let doc = contract();
    let operations = routed_operations(&doc);
    assert!(!operations.is_empty());
    for (method, path) in operations {
        let request = Request::builder()
            .method(method.clone())
            .uri(path.as_str())
            .body(Body::empty())
            .expect("request");
        let response = router(ApiState::new())
            .oneshot(request)
            .await
            .expect("router answers");
        let status = response.status();
        assert_ne!(
            status,
            StatusCode::NOT_FOUND,
            "{method} {path} is not routed"
        );
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path}: wrong method"
        );
    }
}

#[test]
fn every_documented_operation_has_a_unique_operation_id() {
    let doc = contract();
    let mut seen = BTreeSet::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            let id = item[*method]["operationId"]
                .as_str()
                .unwrap_or_else(|| panic!("{method} {path} has an operationId"));
            assert!(seen.insert(id.to_string()), "operationId {id} is unique");
        }
    }
}

#[test]
fn every_error_response_carries_the_api_error_schema() {
    let doc = contract();
    let checker = Checker::new(&doc);
    let api_error = schema(&doc, "ApiError");
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            let responses = item[*method]["responses"].as_object().expect("responses");
            for (status, response) in responses {
                if status.starts_with('2') || status == "101" {
                    continue;
                }
                let response = checker.resolve(response).expect("response $ref");
                let body = checker
                    .resolve(&response["content"]["application/json"]["schema"])
                    .expect("schema $ref");
                assert_eq!(body, api_error, "{method} {path} {status} body is ApiError");
            }
        }
    }
}

#[test]
fn the_contract_uses_only_checked_schema_keywords() {
    let doc = contract();
    let mut found = Vec::new();
    for (name, schema) in doc["components"]["schemas"].as_object().expect("schemas") {
        found.extend(unsupported_keywords(schema, name));
    }
    for (name, param) in doc["components"]["parameters"]
        .as_object()
        .expect("parameters")
    {
        found.extend(unsupported_keywords(&param["schema"], name));
    }
    assert!(
        found.is_empty(),
        "keywords the drift test cannot check: {found:?}"
    );
}

#[test]
fn health_schema_matches_the_core_payload_and_requires_every_field() {
    let doc = contract();
    let health = schema(&doc, "HealthStatus");
    let payload = to_value(filesquid_core::health());
    Checker::new(&doc)
        .check(&payload, health, "HealthStatus")
        .expect("conforms");
    let mut fields: Vec<&str> = payload
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut documented = required(health);
    fields.sort_unstable();
    documented.sort_unstable();
    assert_eq!(documented, fields);
}

#[tokio::test]
async fn router_serves_its_own_contract() {
    let response = router(ApiState::new())
        .oneshot(
            Request::get("/api/v1/openapi.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router answers");
    assert_eq!(response.status(), StatusCode::OK);
}

// ---- Operation schemas (CORE-S-005) ----

#[test]
fn command_envelope_carries_idempotency_key_expected_revision_and_scope() {
    let doc = contract();
    let envelope = schema(&doc, "CommandEnvelope");
    let mut names = required(envelope);
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "expected_revision",
            "operation_id",
            "schema_version",
            "scope"
        ]
    );
    assert_eq!(
        envelope["properties"]["schema_version"]["const"],
        PROTOCOL_SCHEMA_VERSION
    );
    for name in ["operation_id", "expected_revision", "scope"] {
        assert_eq!(envelope["properties"][name]["minLength"], 1, "{name}");
    }
}

#[test]
fn query_envelope_and_page_size_parameter_are_bounded_like_core() {
    let doc = contract();
    let envelope = schema(&doc, "QueryEnvelope");
    let mut names = required(envelope);
    names.sort_unstable();
    assert_eq!(names, ["page_size", "schema_version", "scope"]);
    let parameter = &doc["components"]["parameters"]["PageSize"];
    assert_eq!(parameter["in"], "query");
    for page_size in [&envelope["properties"]["page_size"], &parameter["schema"]] {
        assert_eq!(page_size["type"], "integer");
        assert_eq!(page_size["minimum"], 1);
        assert_eq!(page_size["maximum"], MAX_PAGE_SIZE);
    }
    assert_eq!(doc["components"]["parameters"]["Cursor"]["in"], "query");
}

#[test]
fn errors_carry_code_and_retryability() {
    let doc = contract();
    for (name, fields) in [
        ("PortError", vec!["code", "retryable"]),
        ("ApiError", vec!["code", "message", "retryable"]),
    ] {
        let mut names = required(schema(&doc, name));
        names.sort_unstable();
        assert_eq!(names, fields, "{name}");
    }
}

#[test]
fn error_codes_match_the_rust_enum_with_status_and_retryability() {
    let doc = contract();
    let documented: BTreeSet<String> = schema(&doc, "ErrorCode")["enum"]
        .as_array()
        .expect("enum")
        .iter()
        .map(Value::to_string)
        .collect();
    let rust: BTreeSet<String> = ErrorCode::ALL
        .iter()
        .map(|c| to_value(*c).to_string())
        .collect();
    assert_eq!(documented, rust);
    let expected = [
        (ErrorCode::InvalidRequest, 400, false),
        (ErrorCode::Unauthorized, 401, false),
        (ErrorCode::Forbidden, 403, false),
        (ErrorCode::NotFound, 404, false),
        (ErrorCode::Conflict, 409, false),
        (ErrorCode::Internal, 500, false),
        (ErrorCode::Unavailable, 503, true),
    ];
    assert_eq!(expected.len(), ErrorCode::ALL.len());
    for (code, status, retryable) in expected {
        assert_eq!(code.status().as_u16(), status, "{code:?}");
        assert_eq!(code.retryable(), retryable, "{code:?}");
    }
}

// ---- Every component schema against real Rust values ----

/// Serialized Rust values for every component schema. Enum schemas list every variant.
fn samples() -> Vec<(&'static str, Vec<Value>)> {
    let mut all = vec![
        ("HealthStatus", vec![to_value(filesquid_core::health())]),
        ("ApiEvent", vec![to_value(sample_event())]),
        ("Level", all_levels().map(to_value).to_vec()),
        ("Outcome", all_outcomes().map(to_value).to_vec()),
        ("JournalEntry", vec![to_value(full_entry())]),
        ("JournalPage", vec![to_value(sample_page())]),
        ("ProcessState", all_process_states().map(to_value).to_vec()),
        ("ProcessView", vec![to_value(sample_process())]),
        (
            "MonitorStatus",
            all_monitor_statuses().map(to_value).to_vec(),
        ),
        ("HealthReason", vec![to_value(sample_reason())]),
        ("HealthInputs", vec![to_value(sample_inputs())]),
        ("HealthReport", vec![to_value(sample_report())]),
    ];
    all.extend(operation_samples());
    all
}

/// Serialized samples of the operation and error schemas.
fn operation_samples() -> Vec<(&'static str, Vec<Value>)> {
    vec![
        ("ErrorCode", ErrorCode::ALL.map(to_value).to_vec()),
        (
            "ApiError",
            vec![to_value(ApiError::new(
                ErrorCode::Unavailable,
                "Not ready yet.",
            ))],
        ),
        ("PortError", vec![to_value(port_error())]),
        ("CommandEnvelope", vec![to_value(command_envelope())]),
        ("QueryEnvelope", vec![to_value(query_envelope())]),
        (
            "CommandOutcomeKind",
            all_outcome_kinds().map(to_value).to_vec(),
        ),
        ("CommandOutcome", command_outcomes().map(to_value).to_vec()),
    ]
}

#[test]
fn every_component_schema_has_a_rust_sample() {
    let doc = contract();
    let documented: BTreeSet<&str> = doc["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .keys()
        .map(String::as_str)
        .collect();
    let sampled: BTreeSet<&str> = samples().iter().map(|(name, _)| *name).collect();
    assert_eq!(documented, sampled);
}

#[test]
fn every_component_schema_matches_its_serialized_rust_type() {
    let doc = contract();
    let checker = Checker::new(&doc);
    for (name, values) in samples() {
        let schema = schema(&doc, name);
        for value in &values {
            if let Err(e) = checker.check(value, schema, name) {
                panic!("Rust {name} drifted from the contract: {e}");
            }
        }
        assert_documented_members_are_produced(name, schema, &values);
    }
}

/// Every documented property (objects) or enum value (enums) MUST be produced by Rust.
fn assert_documented_members_are_produced(name: &str, schema: &Value, values: &[Value]) {
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        let documented: BTreeSet<String> = options.iter().map(Value::to_string).collect();
        let produced: BTreeSet<String> = values.iter().map(Value::to_string).collect();
        assert_eq!(
            documented, produced,
            "{name}: enum values versus Rust variants"
        );
        return;
    }
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for prop in props.keys() {
        let produced = values.iter().any(|v| v.get(prop).is_some());
        assert!(
            produced,
            "{name}.{prop} is documented but Rust never serializes it"
        );
    }
}

fn sample_event() -> ApiEvent {
    ApiEvent::new("job.progress", json!({ "done": 1 }))
}

fn full_entry() -> JournalEntry {
    let mut entry = JournalEntry::new("ocfl.commit", Outcome::Failed)
        .actor("owner")
        .target("file-1")
        .process("proc-1")
        .level(Level::Error)
        .error_code("store_unavailable")
        .duration_ms(12);
    entry.id = Some(7);
    entry.host = "host-a".into();
    entry
}

fn sample_page() -> JournalPage {
    JournalPage {
        entries: vec![full_entry()],
        next_cursor: Some(7),
    }
}

fn sample_process() -> ProcessView {
    ProcessView {
        id: "proc-1".into(),
        kind: "import".into(),
        state: ProcessState::Running,
        done: 3,
        total: Some(10),
        started: "2026-10-17T10:00:00.000Z".into(),
        last_heartbeat: "2026-10-17T10:00:01.000Z".into(),
        heartbeat_timeout_ms: 30_000,
        rate_per_second: Some(1.5),
        eta_seconds: Some(5),
        error_code: Some("none".into()),
    }
}

fn sample_reason() -> HealthReason {
    HealthReason {
        code: "disk_low".into(),
        severity: HealthStatus::Degraded,
        detail: "Less than 5 GiB free.".into(),
    }
}

fn sample_inputs() -> HealthInputs {
    HealthInputs {
        stalled: 0,
        store_ok: true,
        data_dir_writable: true,
        free_bytes: Some(1 << 40),
        queue_fill: 0.25,
        errors_last_5m: 0,
    }
}

fn sample_report() -> HealthReport {
    filesquid_api::health::evaluate(&sample_inputs())
}

fn port_error() -> PortError {
    PortError::new("busy", true).expect("port error")
}

fn command_envelope() -> CommandEnvelope {
    CommandEnvelope::new(
        OperationId::new("op-1").expect("id"),
        ExpectedRevision::new("rev-3").expect("revision"),
        OperationScope::new("library:lib-1").expect("scope"),
    )
}

fn query_envelope() -> QueryEnvelope {
    QueryEnvelope::new(
        PageSize::new(MAX_PAGE_SIZE).expect("page size"),
        OperationScope::new("library:lib-1").expect("scope"),
    )
}

fn command_outcomes() -> [CommandOutcome; 2] {
    let id = OperationId::new("op-1").expect("id");
    [
        CommandOutcome::new(id.clone(), CommandOutcomeKind::Committed),
        CommandOutcome::failed(id, port_error()),
    ]
}

// Exhaustive variant lists: adding a variant without updating the list fails to compile.

fn all_levels() -> [Level; 3] {
    let all = [Level::Info, Level::Warn, Level::Error];
    all.iter().for_each(|l| match l {
        Level::Info | Level::Warn | Level::Error => (),
    });
    all
}

fn all_outcomes() -> [Outcome; 7] {
    use Outcome::*;
    let all = [
        Info, Started, Succeeded, Failed, Stalled, Resumed, Cancelled,
    ];
    all.iter().for_each(|o| match o {
        Info | Started | Succeeded | Failed | Stalled | Resumed | Cancelled => (),
    });
    all
}

fn all_process_states() -> [ProcessState; 5] {
    use ProcessState::*;
    let all = [Running, Stalled, Completed, Failed, Cancelled];
    all.iter().for_each(|s| match s {
        Running | Stalled | Completed | Failed | Cancelled => (),
    });
    all
}

fn all_monitor_statuses() -> [HealthStatus; 3] {
    use HealthStatus::*;
    let all = [Ok, Degraded, Failing];
    all.iter().for_each(|s| match s {
        Ok | Degraded | Failing => (),
    });
    all
}

fn all_outcome_kinds() -> [CommandOutcomeKind; 4] {
    use CommandOutcomeKind::*;
    let all = [Committed, Pending, Conflict, Error];
    all.iter().for_each(|k| match k {
        Committed | Pending | Conflict | Error => (),
    });
    all
}

#[test]
fn error_code_all_lists_every_variant() {
    ErrorCode::ALL.iter().for_each(|c| match c {
        ErrorCode::InvalidRequest
        | ErrorCode::Unauthorized
        | ErrorCode::Forbidden
        | ErrorCode::NotFound
        | ErrorCode::Conflict
        | ErrorCode::Internal
        | ErrorCode::Unavailable => (),
    });
    assert_eq!(ErrorCode::ALL.len(), 7);
}
