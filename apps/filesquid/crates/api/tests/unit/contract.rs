//! Contract drift tests (T-048, ADR 030 Decision 2, API-S-002): the Rust routes, the Rust
//! payload types and the checked-in OpenAPI 3.1 document MUST agree. The generated
//! TypeScript types are checked against the same document by the UI package's Vitest test
//! (`apps/filesquid/ui/tests/unit/contract.test.ts`). Both run in `scripts/pre-commit.sh`.

#[path = "schema_check.rs"]
mod schema_check;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use filesquid_api::auth::{Action, Authoriser, DenialReason};
use filesquid_api::events::{topic, EventResync};
use filesquid_api::health::{HealthInputs, HealthReason, HealthReport, HealthStatus};
use filesquid_api::journal::{JournalEntry, JournalPage, Level, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::operation::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationId,
    OperationScope, PageSize, PortError, QueryEnvelope, MAX_PAGE_SIZE, PROTOCOL_SCHEMA_VERSION,
};
use filesquid_api::registry::{ProcessSpec, ProcessState, ProcessView};
use filesquid_api::{
    route_actions, routed_paths, router, ApiError, ApiEvent, ApiState, ErrorCode, API_VERSION,
    CONTRACT,
};
use http_body_util::BodyExt;
use schema_check::{required, unsupported_keywords, Checker};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;
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
    assert_eq!(API_VERSION, "0.5.0");
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

/// OpenAPI extension naming the authorisation action an operation requires.
const ACTION: &str = "x-filesquid-action";

/// Every documented operation, host routes included, as (method, path, operation).
fn all_operations(doc: &Value) -> Vec<(String, String, &Value)> {
    let mut operations = Vec::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            operations.push((method.to_string(), path.clone(), &item[*method]));
        }
    }
    operations
}

#[test]
fn every_operation_requires_the_bearer_scheme_and_none_makes_it_optional() {
    let doc = contract();
    assert_eq!(doc["security"], json!([{ "bearerAuth": [] }]));
    let scheme = &doc["components"]["securitySchemes"]["bearerAuth"];
    assert_eq!(scheme["type"], "http");
    assert_eq!(scheme["scheme"], "bearer");
    for (method, path, op) in all_operations(&doc) {
        match op[ACTION].as_str() {
            Some("public") => assert_eq!(op["security"], json!([]), "{method} {path}"),
            _ => assert!(
                op.get("security").is_none(),
                "{method} {path} overrides security"
            ),
        }
    }
}

#[test]
fn the_only_public_operation_is_the_liveness_probe() {
    let doc = contract();
    let public: Vec<String> = all_operations(&doc)
        .into_iter()
        .filter(|(_, _, op)| op[ACTION] == "public")
        .map(|(method, path, _)| format!("{method} {path}"))
        .collect();
    assert_eq!(public, vec!["get /api/v1/health/live".to_string()]);
}

#[test]
fn every_operation_documents_both_denials() {
    let doc = contract();
    for (method, path, op) in all_operations(&doc) {
        if op[ACTION] == "public" {
            let responses = op["responses"].as_object().expect("responses");
            assert!(!responses.contains_key("401") && !responses.contains_key("403"));
            continue;
        }
        for (status, response) in [("401", "Unauthorized"), ("403", "Forbidden")] {
            let reference = format!("#/components/responses/{response}");
            assert_eq!(
                op["responses"][status]["$ref"], reference,
                "{method} {path} documents {status}"
            );
        }
    }
}

#[test]
fn every_operation_names_the_action_the_router_authorises() {
    let doc = contract();
    let names: BTreeSet<&str> = Action::ALL.iter().map(|a| a.as_str()).collect();
    let routed = route_actions();
    for (method, path, op) in all_operations(&doc) {
        let documented = op[ACTION].as_str().unwrap_or_default();
        assert!(
            names.contains(documented),
            "{method} {path}: {documented:?}"
        );
        let expected = routed
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, action)| action.as_str())
            .unwrap_or(Action::SubscribeEvents.as_str());
        assert_eq!(documented, expected, "{method} {path}");
    }
}

#[test]
fn authentication_before_routing_is_documented_as_a_wire_convention() {
    let doc = contract();
    let conventions = doc["info"]["description"].as_str().expect("description");
    for stated in [
        "401 unauthorized",
        "deny-by-default",
        "x-filesquid-action",
        "/api/v1/health/live",
    ] {
        assert!(
            conventions.contains(stated),
            "info.description states {stated}"
        );
    }
    let api_error = schema(&doc, "ApiError");
    assert_eq!(
        api_error["properties"]["reason"]["$ref"],
        "#/components/schemas/DenialReason"
    );
}

/// Every documented operation of a routed path, as (method, path).
fn routed_operations(doc: &Value) -> Vec<(Method, String, &Value)> {
    let mut operations = Vec::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        if item.get(HOST_ROUTE).is_some() {
            continue;
        }
        for name in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            let method = Method::from_bytes(name.to_uppercase().as_bytes()).expect("method");
            operations.push((method, path.clone(), &item[*name]));
        }
    }
    operations
}

/// Bearer token that [`authorised_state`] accepts.
const TOKEN: &str = "contract-token";

/// API state whose authoriser accepts [`TOKEN`]; no monitor yet, so admin routes answer 503.
fn authorised_state() -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    state
}

/// Authorised API state whose monitor is running, so admin routes answer 200.
fn owner_state() -> (ApiState, Monitor) {
    let state = authorised_state();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        MonitorConfig::new("host-a", std::env::temp_dir()),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    (state, monitor)
}

/// Send one empty-bodied request carrying [`TOKEN`] through the router; return the status
/// and body bytes.
async fn call(state: &ApiState, method: &Method, path: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method.clone())
        .uri(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = router(state.clone())
        .oneshot(request)
        .await
        .expect("router answers");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, body.to_vec())
}

/// Assert `status` is one the operation documents and, when the documented response has a
/// JSON schema, that `body` conforms to it. Returns whether a body was checked.
fn assert_documented_answer(doc: &Value, op: &Value, status: StatusCode, body: &[u8]) -> bool {
    let checker = Checker::new(doc);
    let documented = &op["responses"][status.as_str()];
    assert!(documented.is_object(), "status {status} is not documented");
    let response = checker.resolve(documented).expect("response $ref");
    let Some(schema) = response.pointer("/content/application~1json/schema") else {
        return false;
    };
    let value: Value = serde_json::from_slice(body).expect("documented JSON body");
    if let Err(e) = checker.check(&value, schema, status.as_str()) {
        panic!("body drifted from the documented {status} schema: {e}");
    }
    true
}

#[tokio::test]
async fn every_documented_operation_answers_a_documented_status_and_body() {
    let doc = contract();
    let operations = routed_operations(&doc);
    assert!(!operations.is_empty());
    let (ready, monitor) = owner_state();
    let mut checked_ok = 0;
    for (method, path, op) in &operations {
        let (status, body) = call(&ApiState::new(), method, path).await;
        let expected = match op[ACTION].as_str() {
            Some("public") => StatusCode::OK,
            _ => StatusCode::FORBIDDEN,
        };
        assert_eq!(status, expected, "{method} {path} without an authoriser");
        assert_documented_answer(&doc, op, status, &body);
        let (status, body) = call(&authorised_state(), method, path).await;
        assert_documented_answer(&doc, op, status, &body);
        let (status, body) = call(&ready, method, path).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{method} {path} when the host is ready"
        );
        checked_ok += usize::from(assert_documented_answer(&doc, op, status, &body));
    }
    monitor.shutdown();
    let checker = Checker::new(&doc);
    let json_ok = operations
        .iter()
        .filter_map(|(_, _, op)| checker.resolve(&op["responses"]["200"]).ok())
        .filter(|ok| ok.pointer("/content/application~1json/schema").is_some())
        .count();
    assert!(json_ok >= 5, "operations with a JSON 200 schema: {json_ok}");
    assert_eq!(checked_ok, json_ok, "every JSON 200 body was checked");
}

/// The router-wide fallbacks, as the contract states them: (status, ErrorCode value).
const FALLBACKS: [(u16, &str); 2] = [(404, "not_found"), (405, "method_not_allowed")];

#[test]
fn router_fallbacks_are_documented_as_wire_conventions() {
    let doc = contract();
    let conventions = doc["info"]["description"]
        .as_str()
        .expect("info.description");
    let codes = schema(&doc, "ErrorCode");
    let code_doc = codes["description"]
        .as_str()
        .expect("ErrorCode description");
    for (status, code) in FALLBACKS {
        let stated = format!("{status} {code}");
        assert!(
            conventions.contains(&stated),
            "info.description states {stated}"
        );
        assert!(
            code_doc.contains(&stated),
            "ErrorCode description states {stated}"
        );
        assert!(codes["enum"]
            .as_array()
            .expect("enum")
            .contains(&json!(code)));
    }
    assert!(
        conventions.contains("ApiError"),
        "fallbacks carry the ApiError body"
    );
}

/// Assert `body` is an `ApiError` with `code`, as the router fallbacks answer.
fn assert_fallback_body(body: &[u8], code: &str, at: &str) {
    let doc = contract();
    let value: Value = serde_json::from_slice(body).expect("fallback body is JSON");
    Checker::new(&doc)
        .check(&value, schema(&doc, "ApiError"), at)
        .expect("fallback body is an ApiError");
    assert_eq!(value["code"], code, "{at}");
}

#[tokio::test]
async fn router_fallbacks_answer_as_documented() {
    let doc = contract();
    let state = authorised_state();
    let (status, body) = call(&state, &Method::GET, "/api/v1/not-in-the-contract").await;
    assert_eq!(status.as_u16(), FALLBACKS[0].0);
    assert_fallback_body(&body, FALLBACKS[0].1, "unknown path");
    // GET routes also answer HEAD in axum, so HEAD is not an undocumented method here.
    let undocumented = ["put", "post", "delete", "patch"];
    for (_, path, _) in routed_operations(&doc) {
        let item = &doc["paths"][path.as_str()];
        for name in undocumented.iter().filter(|m| item.get(**m).is_none()) {
            let method = Method::from_bytes(name.to_uppercase().as_bytes()).expect("method");
            let (status, body) = call(&state, &method, &path).await;
            assert_eq!(status.as_u16(), FALLBACKS[1].0, "{method} {path}");
            assert_fallback_body(&body, FALLBACKS[1].1, &format!("{method} {path}"));
        }
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
    let (status, body) = call(&authorised_state(), &Method::GET, "/api/v1/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, CONTRACT.as_bytes());
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
    assert_eq!(MAX_PAGE_SIZE, 1000, "owner decision 2026-10-17: 1-1000");
    assert_eq!(filesquid_api::journal::MAX_PAGE, MAX_PAGE_SIZE as usize);
}

#[test]
fn every_limit_parameter_is_the_shared_page_size() {
    let doc = contract();
    let shared = "#/components/parameters/PageSize";
    let mut uses = 0;
    for (path, item) in doc["paths"].as_object().expect("paths") {
        for method in METHODS.iter().filter(|m| item.get(**m).is_some()) {
            let params = item[*method]["parameters"].as_array().cloned();
            for param in params.unwrap_or_default() {
                let is_shared = param.get("$ref").and_then(Value::as_str) == Some(shared);
                uses += usize::from(is_shared);
                let inline_limit = param.get("name").and_then(Value::as_str) == Some("limit");
                assert!(!inline_limit, "{method} {path}: `limit` must use {shared}");
            }
        }
    }
    assert!(uses >= 1, "at least one operation pages with PageSize");
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
        (ErrorCode::MethodNotAllowed, 405, false),
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
        ("Liveness", vec![to_value(filesquid_api::Liveness::up())]),
        ("ApiEvent", vec![to_value(sample_event())]),
        ("EventTopic", topic::ALL.map(to_value).to_vec()),
        ("EventResync", vec![to_value(EventResync { missed: 1 })]),
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
            vec![
                to_value(ApiError::new(ErrorCode::Unavailable, "Not ready yet.")),
                to_value(ApiError::denied(DenialReason::NotPermitted)),
            ],
        ),
        ("DenialReason", DenialReason::ALL.map(to_value).to_vec()),
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
    ApiEvent::new(topic::PROCESS_UPDATE, json!({ "done": 1 }))
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
fn the_checker_accepts_only_truly_unconstrained_objects() {
    let doc = json!({});
    let checker = Checker::new(&doc);
    let empty = json!({ "description": "any payload" });
    checker
        .check(&json!({ "a": "x" }), &empty, "empty")
        .expect("`{}` accepts any object");
    let map = json!({ "additionalProperties": { "type": "integer" } });
    checker
        .check(&json!({ "a": 1 }), &map, "map")
        .expect("integer values conform");
    assert!(checker.check(&json!({ "a": "x" }), &map, "map").is_err());
    let needs = json!({ "required": ["a"] });
    assert!(checker.check(&json!({}), &needs, "needs").is_err());
}

#[test]
fn error_code_all_lists_every_variant() {
    ErrorCode::ALL.iter().for_each(|c| match c {
        ErrorCode::InvalidRequest
        | ErrorCode::Unauthorized
        | ErrorCode::Forbidden
        | ErrorCode::NotFound
        | ErrorCode::MethodNotAllowed
        | ErrorCode::Conflict
        | ErrorCode::Internal
        | ErrorCode::Unavailable => (),
    });
    assert_eq!(ErrorCode::ALL.len(), 8);
}

// ---- Event channel: documented topics and payloads (API-S-003) ----

/// OpenAPI extension on the event channel mapping each topic to its payload schema.
const EVENT_PAYLOADS: &str = "x-filesquid-event-payloads";

#[test]
fn event_topics_in_the_contract_match_the_rust_topics() {
    let doc = contract();
    assert_eq!(
        doc["components"]["schemas"]["ApiEvent"]["properties"]["topic"]["$ref"],
        "#/components/schemas/EventTopic"
    );
    let documented: Vec<&str> = schema(&doc, "EventTopic")["enum"]
        .as_array()
        .expect("EventTopic enum")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(documented, topic::ALL.to_vec());
}

#[test]
fn every_event_topic_documents_its_payload_schema() {
    let doc = contract();
    let payloads = doc["paths"]["/api/v1/events"][EVENT_PAYLOADS]
        .as_object()
        .expect("event payload map");
    let keys: BTreeSet<&str> = payloads.keys().map(String::as_str).collect();
    let topics: BTreeSet<&str> = topic::ALL.into_iter().collect();
    assert_eq!(keys, topics, "documented payloads versus Rust topics");
    let checker = Checker::new(&doc);
    for (name, payload) in payloads {
        let target = checker.resolve(payload).expect("payload $ref resolves");
        assert!(target.is_object(), "{name} payload is a schema");
    }
}

/// Read every event already published to `sub`, stopping after a short quiet period.
async fn drain(sub: &mut filesquid_api::events::EventSubscription) -> Vec<ApiEvent> {
    let mut out = Vec::new();
    for _ in 0..1_000 {
        let quiet = std::time::Duration::from_millis(200);
        match tokio::time::timeout(quiet, sub.next()).await {
            Ok(Some(event)) => out.push(event),
            Ok(None) | Err(_) => break,
        }
    }
    out
}

/// Events emitted by every publisher in the crate: registry, monitor, journal writer, lag.
async fn events_from_every_publisher() -> Vec<ApiEvent> {
    let (state, monitor) = owner_state();
    let mut sub = state.events.subscription().expect("slot");
    let handle = monitor
        .registry()
        .register(ProcessSpec::new("import", 60_000));
    handle.progress(1, 2);
    monitor.tick();
    monitor
        .journal()
        .send(JournalEntry::new("ocfl.commit", Outcome::Succeeded));
    monitor.journal().flush();
    let mut events = drain(&mut sub).await;
    monitor.shutdown();
    let lagging = filesquid_api::EventBus::new();
    let mut slow = lagging.subscription().expect("slot");
    (0..=filesquid_api::events::EVENT_BUFFER).for_each(|_| {
        lagging.publish(sample_event());
    });
    events.extend(slow.next().await);
    events
}

#[tokio::test]
async fn events_from_every_publisher_match_their_documented_payloads() {
    let doc = contract();
    let checker = Checker::new(&doc);
    let payloads = &doc["paths"]["/api/v1/events"][EVENT_PAYLOADS];
    let events = events_from_every_publisher().await;
    let mut seen = BTreeSet::new();
    for event in &events {
        let envelope = to_value(event);
        if let Err(e) = checker.check(&envelope, schema(&doc, "ApiEvent"), "ApiEvent") {
            panic!("published event drifted from ApiEvent: {e}");
        }
        let payload = &payloads[event.topic.as_str()];
        if let Err(e) = checker.check(&event.data, payload, &event.topic) {
            panic!("{} payload drifted from the contract: {e}", event.topic);
        }
        seen.insert(event.topic.clone());
    }
    let all: BTreeSet<String> = topic::ALL.iter().map(|t| t.to_string()).collect();
    assert_eq!(seen, all, "every documented topic is published by Rust");
}
