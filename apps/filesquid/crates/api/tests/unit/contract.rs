//! Contract drift tests (T-048, ADR 030 Decision 2, API-S-002): the Rust routes, the Rust
//! payload types and the checked-in OpenAPI 3.1 document MUST agree. The generated
//! TypeScript types are checked against the same document by the UI package's Vitest test
//! (`apps/filesquid/ui/tests/unit/contract.test.ts`). Both run in `scripts/pre-commit.sh`.

#[path = "schema_check.rs"]
mod schema_check;

#[path = "../support/files_contract.rs"]
mod files_contract;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

#[path = "../support/history_fixture.rs"]
mod history_fixture;

#[path = "../support/library_contract.rs"]
mod library_contract;

#[path = "../support/jobs_contract.rs"]
mod jobs_contract;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use filesquid_api::auth::{
    Action, Authoriser, DenialReason, HostPolicy, Principal, TokenAuthenticator,
};
use filesquid_api::events::{topic, EventResync};
use filesquid_api::health::{HealthInputs, HealthReason, HealthReport, HealthStatus};
use filesquid_api::history::HistoryPage;
use filesquid_api::host::{
    EnrolmentPort, HostIdentity, HostInfo, HostType, HostedService, NotEnrolled, OperatorName,
};
use filesquid_api::journal::{JournalEntry, JournalPage, Level, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig};
use filesquid_api::ocfl_validation::{
    Finding, FindingView, FindingsView, LibraryRuns, MemoryFindingsStore, RunAccepted, RunOutcome,
    RunRecord, RunRefusal, RunRequest, RunTrigger, RunView, Severity, ValidationConfig,
    ValidationService,
};
use filesquid_api::operation::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationId,
    OperationScope, PageSize, PortError, QueryEnvelope, MAX_PAGE_SIZE, PROTOCOL_SCHEMA_VERSION,
};
use filesquid_api::registry::{ProcessSpec, ProcessState, ProcessView};
use filesquid_api::{
    route_actions, routed_paths, router, ApiError, ApiEvent, ApiState, ErrorCode, API_VERSION,
    CONTRACT,
};
use history_fixture::{concrete, install_sample_history, sample_record};
use http_body_util::BodyExt;
use schema_check::{required, unsupported_keywords, Checker};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;
use tower::ServiceExt;

/// OpenAPI extension naming the content route's credential, the handle in its path.
const CREDENTIAL: &str = "x-filesquid-credential";

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
    assert_eq!(API_VERSION, "0.36.0");
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
            // The content route's only credential is the handle in its path (API-S-018).
            Some("content.read") => {
                assert_eq!(op["security"], json!([]), "{method} {path}");
                assert_eq!(op[CREDENTIAL], "content-handle", "{method} {path}");
            }
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
        if op[ACTION] == "content.read" {
            let responses = op["responses"].as_object().expect("responses");
            assert!(
                !responses.contains_key("401"),
                "{method} {path} never answers 401"
            );
            let denied = &op["responses"]["403"]["$ref"];
            assert_eq!(
                denied, "#/components/responses/ContentDenied",
                "{method} {path}"
            );
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

/// API state whose authoriser accepts [`TOKEN`] as the desktop owner's, the one caller the
/// policy grants every action, the repair included.
fn owner_authorised_state() -> ApiState {
    let state = ApiState::new();
    let owner = TokenAuthenticator::new(TOKEN, Principal::Owner);
    state.install_authoriser(Authoriser::new(Arc::new(owner), Arc::new(HostPolicy)));
    state
}

/// The owner's API state with a running monitor, so admin routes answer 200, a file service
/// whose library holds the objects the sample requests name, and library commands whose
/// sample library is open.
async fn owner_state() -> (ApiState, Monitor) {
    let state = owner_authorised_state();
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        std::sync::Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        MonitorConfig::new("host-a"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    state.describe_host(HostIdentity::desktop(), Arc::new(NotEnrolled));
    files_contract::install(&state, &monitor).await;
    let validation: ValidationService<filesquid_ocfl::storage::MemoryBackend> =
        ValidationService::new(Arc::new(MemoryFindingsStore::new()), monitor.clone());
    state.install_validation(Arc::new(validation));
    install_sample_history(&state);
    library_contract::install(&state, &monitor).await;
    state.install_navigation(navigation_service());
    state.install_preferences(Arc::new(
        filesquid_api::preferences::MemoryPreferenceStore::new(),
    ));
    state.install_history_backfill(Arc::new(StartingLauncher));
    jobs_contract::install(&state, &monitor);
    install_recently_deleted(&state, &monitor);
    (state, monitor)
}

/// A backfill launcher that always starts.
struct StartingLauncher;

impl filesquid_api::history::launch::BackfillLauncher for StartingLauncher {
    fn launch(&self, _actor: &str) -> Result<(), filesquid_api::history::launch::LaunchError> {
        Ok(())
    }
}

/// A navigation service over a catalog holding the sample library, one collection and one
/// virtual collection, with the sample file in both: every node route answers 200.
fn navigation_service() -> Arc<filesquid_api::navigation::NavigationService> {
    let drive = drive_catalog::DriveCatalog::new(files_contract::LIBRARY);
    drive.file_in_collection(files_contract::FILE);
    drive.add_to_virtual(files_contract::FILE);
    drive.navigation
}

/// Send one request carrying [`TOKEN`] through the router, with the sample query and body of
/// its route ([`files_contract::sample`]); return the status and body bytes.
async fn call(state: &ApiState, method: &Method, path: &str) -> (StatusCode, Vec<u8>) {
    let (uri, body) = library_contract::sample(method, path)
        .unwrap_or_else(|| files_contract::sample(state, method, path, TOKEN));
    let mut request = Request::builder()
        .method(method.clone())
        .uri(uri)
        .header("authorization", format!("Bearer {TOKEN}"));
    if let Some((content_type, _)) = &body {
        request = request.header("content-type", *content_type);
    }
    let trash_json = *method != Method::GET
        && matches!(
            path,
            "/api/v1/recently-deleted/commands" | "/api/v1/settings/recently-deleted"
        );
    if body.is_none() && trash_json {
        request = request.header("content-type", "application/json");
    }
    let body = match body {
        Some((_, bytes)) => Body::from(with_revision(state, bytes).await),
        None => sample_body(method, path),
    };
    let request = request.body(body).expect("request");
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

/// `bytes` with a sample command's head placeholder ([`files_contract::HEAD_OF`]) replaced by
/// the head of the object it names on `state`, read through the versions route; `v1` when
/// `state` cannot answer it (no authoriser or no file service), where the command is refused
/// before its revision is compared.
async fn with_revision(state: &ApiState, bytes: Vec<u8>) -> Vec<u8> {
    let text = String::from_utf8_lossy(&bytes).to_string();
    let Some((object, marker)) = files_contract::head_marker(&text) else {
        return bytes;
    };
    let uri = format!(
        "/api/v1/files/versions?library_id={}&object_id={}",
        files_contract::LIBRARY,
        object.replace(':', "%3A")
    );
    let request = Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = router(state.clone())
        .oneshot(request)
        .await
        .expect("router answers");
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let answer: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let head = answer["head"].as_str().unwrap_or("v1");
    text.replace(&marker, head).into_bytes()
}

/// The body a documented operation needs to answer 200: the default settings for the settings
/// route, nothing otherwise.
fn sample_body(method: &Method, path: &str) -> Body {
    if *method == Method::PUT && path == "/api/v1/admin/ocfl/validation-settings" {
        return Body::from(serde_json::to_vec(&ValidationConfig::default()).expect("json"));
    }
    if *method == Method::PUT && path == "/api/v1/settings/recently-deleted" {
        let settings =
            serde_json::json!({"default_retention_days": 30, "expiry": "keep_version_history"});
        return Body::from(settings.to_string());
    }
    if path == "/api/v1/recently-deleted/commands" {
        let command = serde_json::json!({
            "envelope": {"schema_version": 1, "operation_id": "op-trash", "expected_revision": "1",
                         "scope": format!("library:{TRASH_LIBRARY}")},
            "input": {"command": "set_library_retention", "library_id": TRASH_LIBRARY, "retention_days": 7},
        });
        return Body::from(command.to_string());
    }
    Body::empty()
}

/// The library of the contract's Recently deleted library service.
const TRASH_LIBRARY: &str = "5e0c1b2a-3948-4f57-8a6b-7c8d9e0f1a2b";

/// Install a library service over an in-memory catalog holding [`TRASH_LIBRARY`] at
/// revision 1, with in-memory Recently deleted settings.
fn install_recently_deleted(state: &ApiState, monitor: &Monitor) {
    use filesquid_api::library::{
        CatalogChange, CatalogCommit, LibraryCatalog, LibraryRecord, LibraryService, MemoryCatalog,
        MemorySettings,
    };
    let catalog = Arc::new(MemoryCatalog::new());
    let library = filesquid_core::LibraryId::new(TRASH_LIBRARY).expect("library");
    let record = LibraryRecord {
        library: filesquid_core::Library::new(
            library.clone(),
            filesquid_core::RepositoryId::new("repository").expect("repository"),
        ),
        name: "Trash library".into(),
    };
    catalog
        .commit(CatalogCommit {
            operation_id: filesquid_api::operation::OperationId::new("seed").expect("op"),
            library_id: library,
            expected_revision: filesquid_api::operation::ExpectedRevision::new("0").expect("rev"),
            changes: vec![CatalogChange::CreateLibrary(record)],
            journal: JournalEntry::new("seed", filesquid_api::journal::Outcome::Succeeded),
            created_id: None,
            deletion: None,
            restored_into: Vec::new(),
        })
        .expect("seed");
    let service = LibraryService::new(catalog, monitor.journal())
        .with_settings(Arc::new(MemorySettings::new()));
    assert!(state.install_library_service(Arc::new(service)));
}

/// Assert `status` is one the operation documents and, when the documented response has a
/// JSON schema, that `body` conforms to it. A `HEAD` answer has no body, so only its status
/// is checked. Returns whether a body was checked.
fn assert_documented_answer(
    doc: &Value,
    (method, op): (&Method, &Value),
    status: StatusCode,
    body: &[u8],
) -> bool {
    let checker = Checker::new(doc);
    let documented = &op["responses"][status.as_str()];
    assert!(documented.is_object(), "status {status} is not documented");
    let response = checker.resolve(documented).expect("response $ref");
    let Some(schema) = response.pointer("/content/application~1json/schema") else {
        return false;
    };
    if *method == Method::HEAD {
        assert!(body.is_empty(), "a HEAD answer has no body");
        return false;
    }
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
    let (ready, monitor) = owner_state().await;
    let mut checked_ok = 0;
    for (method, path, op) in &operations {
        let path = &jobs_contract::concrete_job(&concrete(path));
        let (status, body) = call(&ApiState::new(), method, path).await;
        let expected = match op[ACTION].as_str() {
            Some("public") => StatusCode::OK,
            _ => StatusCode::FORBIDDEN,
        };
        assert_eq!(status, expected, "{method} {path} without an authoriser");
        assert_documented_answer(&doc, (method, op), status, &body);
        let (status, body) = call(&authorised_state(), method, path).await;
        assert_documented_answer(&doc, (method, op), status, &body);
        let (status, body) = call(&ready, method, path).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{method} {path} when the host is ready"
        );
        checked_ok += usize::from(assert_documented_answer(&doc, (method, op), status, &body));
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
            let at = format!("{method} {path}");
            if path.starts_with("/api/v1/content/") {
                // The content route answers its one denial to every other method (API-S-018).
                assert_eq!(status, StatusCode::FORBIDDEN, "{at}");
                assert_fallback_body(&body, "forbidden", &at);
                continue;
            }
            assert_eq!(status.as_u16(), FALLBACKS[1].0, "{at}");
            assert_fallback_body(&body, FALLBACKS[1].1, &at);
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
                let conflicts = ["FileCommandConflict", "LibraryCommandConflict"];
                if status == "409" && conflicts.iter().any(|name| *body == *schema(&doc, name)) {
                    assert_carries_api_error(body, api_error, &format!("{method} {path}"));
                    continue;
                }
                assert_eq!(body, api_error, "{method} {path} {status} body is ApiError");
            }
        }
    }
}

/// Assert the 409 answer `body` of a file write command (Task API-F2.13) or a library
/// command (Task API-F2.19) carries every
/// `ApiError` field a denial does not add, with the same schema and required, beside its
/// outcome fields.
fn assert_carries_api_error(body: &Value, api_error: &Value, at: &str) {
    for field in ["code", "message", "retryable"] {
        assert_eq!(
            body["properties"][field], api_error["properties"][field],
            "{at} 409 {field}"
        );
        let required = body["required"].as_array().expect("required");
        assert!(
            required.iter().any(|r| r == field),
            "{at} 409 requires {field}"
        );
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
        (ErrorCode::PayloadTooLarge, 413, false),
        (ErrorCode::RangeNotSatisfiable, 416, false),
        (ErrorCode::RateLimited, 429, true),
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
        ("ProvenanceLink", provenance_samples()),
        ("JournalEntryView", vec![to_value(sample_view())]),
        ("JournalViewPage", vec![to_value(sample_view_page())]),
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
    all.extend(host_samples());
    all.extend(validation_samples());
    all.extend(history_samples());
    all.extend(navigation_samples());
    all.extend(preference_samples());
    all.extend(operation_samples());
    all.extend(files_contract::samples());
    all.extend(library_contract::samples());
    all.extend(client_event_samples());
    all.extend(jobs_contract::samples());
    all.extend(recently_deleted_samples());
    all
}

/// Serialized samples of the preference schemas: the defaults, a value with every choice
/// changed and a pin of every kind, and every enum value.
fn preference_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::preferences::{
        FolderPlacement, Layout, NavigationPreferences, PinKind, PinnedDestination, Preferences,
        SortDirection, SortKey, StartView,
    };
    let pin = |kind| PinnedDestination {
        node: "n1.YzE6Yw".into(),
        kind,
    };
    let kinds = [
        PinKind::Repository,
        PinKind::Library,
        PinKind::Collection,
        PinKind::VirtualCollection,
        PinKind::Folder,
        PinKind::RecentlyDeleted,
    ];
    let navigation = NavigationPreferences {
        version: 1,
        pinned: kinds.map(pin).to_vec(),
        order: vec!["libraries".into()],
        collapsed: vec!["n1.YzE6Yw".into()],
    };
    let changed = Preferences {
        start_view: StartView::PersonalRoot,
        sort_key: SortKey::OpenedByMe,
        sort_direction: SortDirection::Descending,
        folder_placement: FolderPlacement::Mixed,
        layout: Layout::Grid,
        hover_card: false,
        navigation: navigation.clone(),
        ..Preferences::default()
    };
    let sort_keys = [
        SortKey::Name,
        SortKey::Size,
        SortKey::Modified,
        SortKey::ModifiedByMe,
        SortKey::OpenedByMe,
    ];
    vec![
        (
            "Preferences",
            vec![to_value(Preferences::default()), to_value(changed)],
        ),
        ("NavigationPreferences", vec![to_value(&navigation)]),
        (
            "PinnedDestination",
            navigation.pinned.iter().map(to_value).collect(),
        ),
        (
            "StartView",
            [StartView::Home, StartView::PersonalRoot]
                .map(to_value)
                .to_vec(),
        ),
        ("SortKey", sort_keys.map(to_value).to_vec()),
        (
            "SortDirection",
            [SortDirection::Ascending, SortDirection::Descending]
                .map(to_value)
                .to_vec(),
        ),
        (
            "FolderPlacement",
            [FolderPlacement::OnTop, FolderPlacement::Mixed]
                .map(to_value)
                .to_vec(),
        ),
        (
            "Layout",
            [Layout::List, Layout::Grid].map(to_value).to_vec(),
        ),
    ]
}

/// Both shapes of the PROV-O link: a committed entry's and a journal-only one.
fn provenance_samples() -> Vec<Value> {
    use filesquid_api::journal::link::{provenance_of, ProvenanceLink};
    let mut committed = full_entry();
    committed.outcome = Outcome::Succeeded;
    vec![
        to_value(provenance_of(&committed)),
        to_value(ProvenanceLink::journal_only()),
    ]
}

/// A named view of a committed entry.
fn sample_view() -> filesquid_api::admin::JournalEntryView {
    let mut entry = full_entry();
    entry.outcome = Outcome::Succeeded;
    filesquid_api::admin::JournalEntryView {
        provenance: filesquid_api::journal::link::provenance_of(&entry),
        entry,
        display_name: Some("photo.jpg".into()),
    }
}

fn sample_view_page() -> filesquid_api::admin::JournalViewPage {
    filesquid_api::admin::JournalViewPage {
        entries: vec![sample_view()],
        next_cursor: Some(7),
    }
}

/// The client-events request schemas, as values the route's parser accepts, and its
/// acknowledgement.
fn client_event_samples() -> Vec<(&'static str, Vec<Value>)> {
    let console = json!({
        "kind": "console",
        "id": "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab",
        "level": "error",
        "component": "fs-file-browser",
        "error_code": "fetch_failed",
        "ids": ["urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b"],
        "time": "2026-10-19T10:00:00Z",
        "principal": "owner"
    });
    let overflow = json!({
        "kind": "overflow",
        "id": "6a1d4f55-3b8c-4d2e-8f90-123456789abc",
        "component": "fs-console-queue",
        "count": 3,
        "principal": "owner"
    });
    let request = json!({ "events": [console.clone(), overflow.clone()] });
    let body = serde_json::to_vec(&request).expect("json");
    filesquid_api::client_events::parse_batch(&body, i64::MAX / 2).expect("the parser accepts it");
    let ack = filesquid_api::client_events::ClientEventsAck {
        acknowledged: vec!["5f0c3e44-2a7b-4c1d-9e8f-0123456789ab".into()],
        journaled: 1,
        principal: "owner".into(),
    };
    vec![
        ("ClientConsoleEvent", vec![console]),
        ("ClientOverflowEvent", vec![overflow]),
        ("ClientEventsRequest", vec![request]),
        ("ClientEventsAck", vec![to_value(ack)]),
    ]
}

/// Serialized samples of the navigation schemas: a page of every kind of node and both
/// shapes of the change event.
fn navigation_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::navigation::{
        FolderScope, NavigationChange, NavigationRequest, NodeKind, NodeRef,
    };
    let service = navigation_service();
    let page = |node: Option<String>| {
        let request = NavigationRequest {
            node,
            ..NavigationRequest::default()
        };
        service
            .list(&filesquid_api::auth::Principal::Owner, &request)
            .expect("page")
    };
    let root = page(None);
    let repository = to_value(&root);
    let kinds = [
        NodeKind::Repository,
        NodeKind::Library,
        NodeKind::Collection,
        NodeKind::VirtualCollection,
        NodeKind::RecentlyDeleted,
        NodeKind::Folder,
    ];
    let folder = NodeRef::folder(
        FolderScope::Collection(filesquid_core::CollectionId::new("c").expect("id")),
        "a/b",
    );
    let handle = folder.to_handle().expect("handle");
    let nodes = to_value(filesquid_api::navigation::NodeView {
        node: handle,
        kind: NodeKind::Folder,
        name: Some("b".to_string()),
        has_children: false,
    });
    let page_with_ancestors = serde_json::json!({
        "nodes": [nodes.clone()], "ancestors": [nodes.clone()], "next_cursor": "c1.3330"
    });
    vec![
        ("NavigationKind", kinds.map(to_value).to_vec()),
        ("NavigationNode", vec![nodes]),
        ("NavigationPage", vec![repository, page_with_ancestors]),
        (
            "NavigationChange",
            vec![
                to_value(NavigationChange::of(&[folder])),
                to_value(NavigationChange::everything()),
            ],
        ),
    ]
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

/// Serialized samples of the host descriptor schemas (API-S-020): every host type, a named
/// and an unnamed operator, and both enrolment states.
fn host_samples() -> Vec<(&'static str, Vec<Value>)> {
    let types = [HostType::Desktop, HostType::Server, HostType::Browser];
    let enrolled = HostedService::enrolled_at("backup.filesquid.tech", true, false).expect("host");
    let services = [HostedService::not_enrolled(), enrolled];
    let named = OperatorName::parse(&"o".repeat(256)).expect("valid");
    let identities = [
        HostIdentity::desktop(),
        HostIdentity::browser(),
        HostIdentity::server(named),
        HostIdentity::server(None),
    ];
    let descriptors = identities
        .into_iter()
        .map(|identity| HostInfo::new(identity, Arc::new(NotEnrolled), Default::default()))
        .map(|host| to_value(host.descriptor()))
        .collect();
    vec![
        ("HostType", types.map(to_value).to_vec()),
        ("HostedService", services.map(to_value).to_vec()),
        ("HostDescriptor", descriptors),
    ]
}

/// Serialized samples of the OCFL validation schemas (API-S-022): every severity, trigger and
/// outcome, a finding with and without its location, a library before and after its first
/// run, and run requests and answers.
fn validation_samples() -> Vec<(&'static str, Vec<Value>)> {
    let library = filesquid_core::LibraryId::new("lib-1").expect("id");
    let finding = Finding {
        library_id: library.clone(),
        object_id: "urn:uuid:b".into(),
        version: Some("v1".into()),
        code: "E023".into(),
        severity: Severity::Error,
        location: "obj-b/v1/content/stray.tmp".into(),
    };
    let unread = Finding {
        version: None,
        ..finding.clone()
    };
    let findings = [
        FindingView::of(finding.clone(), true),
        FindingView::of(unread, false),
    ];
    let run = RunView::from(RunRecord {
        library_id: library.clone(),
        trigger: RunTrigger::Scheduled,
        started_ms: 0,
        ended_ms: 1,
        outcome: RunOutcome::Failed,
        checked: 0,
        not_checked: 0,
        error_code: Some("unavailable".into()),
    });
    let fresh = LibraryRuns {
        library_id: library.clone(),
        running: false,
        last_run: None,
        next_run: None,
    };
    let ran = LibraryRuns {
        last_run: Some(run.clone()),
        next_run: Some("2026-10-25T00:00:00.000Z".into()),
        running: true,
        ..fresh.clone()
    };
    let view = FindingsView {
        findings: findings.to_vec(),
        truncated: false,
        settings: ValidationConfig::default(),
        libraries: vec![fresh, ran.clone()],
    };
    let request = RunRequest {
        library_id: Some(library.clone()),
        from_start: true,
    };
    let refusal = RunRefusal {
        library_id: library.clone(),
        error_code: "already_running".into(),
    };
    let accepted = RunAccepted {
        started: vec![library],
        refused: vec![refusal.clone()],
    };
    vec![
        (
            "FindingSeverity",
            [Severity::Error, Severity::Warning].map(to_value).to_vec(),
        ),
        ("FindingView", findings.map(to_value).to_vec()),
        (
            "RunTrigger",
            [RunTrigger::Scheduled, RunTrigger::Requested]
                .map(to_value)
                .to_vec(),
        ),
        ("RunOutcome", all_run_outcomes().map(to_value).to_vec()),
        ("RunView", vec![to_value(run)]),
        ("LibraryRuns", vec![to_value(ran)]),
        ("FindingsView", vec![to_value(view)]),
        (
            "RunRequest",
            vec![to_value(request), to_value(RunRequest::default())],
        ),
        ("RunRefusal", vec![to_value(refusal)]),
        ("RunAccepted", vec![to_value(accepted)]),
        (
            "ValidationSettings",
            vec![to_value(ValidationConfig::default())],
        ),
    ]
}

fn all_run_outcomes() -> [RunOutcome; 4] {
    use RunOutcome::*;
    let all = [Completed, Capped, Cancelled, Failed];
    all.iter().for_each(|o| match o {
        Completed | Capped | Cancelled | Failed => (),
    });
    all
}

/// Serialized samples of the history schemas (API-S-029): a recorded and a reconstructed
/// entry, every operation kind, and a page with and without a next cursor, complete and
/// truncated.
fn history_samples() -> Vec<(&'static str, Vec<Value>)> {
    let recorded = filesquid_api::history::entry_of(sample_record().activity()).expect("entry");
    let mut reconstructed = recorded.clone();
    reconstructed.started_at = None;
    reconstructed.reconstructed = true;
    let mut labelled = recorded.clone();
    labelled.label = Some("Final cut".into());
    let pages = [
        HistoryPage {
            items: vec![recorded.clone(), labelled.clone()],
            next_cursor: Some("2026-10-19T10:00:01Z urn:filesquid:activity:op-1".into()),
            truncated: false,
        },
        HistoryPage {
            items: Vec::new(),
            next_cursor: None,
            truncated: true,
        },
    ];
    let started = filesquid_api::history::launch::BackfillStarted {
        status: "started".into(),
    };
    vec![
        ("BackfillStarted", vec![to_value(started)]),
        (
            "OperationKind",
            filesquid_core::OperationKind::ALL.map(to_value).to_vec(),
        ),
        ("VersionRef", vec![to_value(&recorded.version)]),
        ("JournalLink", vec![to_value(&recorded.journal)]),
        (
            "HistoryEntry",
            [recorded, reconstructed, labelled].map(to_value).to_vec(),
        ),
        ("HistoryPage", pages.map(to_value).to_vec()),
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
        .duration_ms(12)
        .operation_id("op-1")
        .object_ref("urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b@v3")
        .client_event("5f0c3e44-2a7b-4c1d-9e8f-0123456789ab");
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
        code: "ocfl_validation_errors".into(),
        severity: HealthStatus::Degraded,
        detail: "OCFL validation errors (E023) in 1 object(s)".into(),
        codes: vec!["E023".into()],
        objects: Some(1),
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
        ocfl_error_codes: vec!["E023".into()],
        ocfl_error_objects: 1,
        journal_unsealed_age_ms: None,
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
        | ErrorCode::PayloadTooLarge
        | ErrorCode::RangeNotSatisfiable
        | ErrorCode::RateLimited
        | ErrorCode::Internal
        | ErrorCode::Unavailable => (),
    });
    assert_eq!(ErrorCode::ALL.len(), 11);
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
    let (state, monitor) = owner_state().await;
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
    events.extend(host_change_event().await);
    events.extend(navigation_change_event().await);
    events.extend(jobs_contract::job_update_event().await);
    events
}

/// The `navigation.change` event a navigation publisher sends.
async fn navigation_change_event() -> Option<ApiEvent> {
    use filesquid_api::navigation::{NavigationPublisher, NodeRef};
    let bus = filesquid_api::EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    let library = filesquid_core::LibraryId::new("library").expect("id");
    NavigationPublisher::new(bus).publish(&[NodeRef::Library(library)]);
    sub.next().await
}

/// An enrolment port that reports enrolled once switched on.
#[derive(Default)]
struct SwitchedPort(std::sync::atomic::AtomicBool);

impl EnrolmentPort for SwitchedPort {
    fn hosted_service(&self) -> HostedService {
        if self.0.load(std::sync::atomic::Ordering::SeqCst) {
            HostedService::enrolled_at("backup.filesquid.tech", true, true).expect("host")
        } else {
            HostedService::not_enrolled()
        }
    }
}

/// The `host.change` event a described host publishes when its enrolment changes.
async fn host_change_event() -> Option<ApiEvent> {
    let bus = filesquid_api::EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    let port = Arc::new(SwitchedPort::default());
    let host = HostInfo::new(HostIdentity::server(None), port.clone(), bus);
    port.0.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(host.refresh_hosted_service());
    sub.next().await
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

/// Serialized samples of the Recently deleted schemas (Task API-F2.15): the settings answer,
/// every expiry setting and deletion cause, a command answer with a restored collection and
/// a deletion, and each command the route takes.
fn recently_deleted_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::library::{
        CommandResult, Deletion, DeletionCause, ExpiryMode, InstallationSettings, LibraryCommand,
        PurgeConfirmation, RestoredInto,
    };
    use filesquid_api::operation::{CommandOutcome, CommandOutcomeKind, OperationId};
    use filesquid_api::recently_deleted::{LibraryRetentionView, RecentlyDeletedSettingsView};
    use filesquid_core::{CollectionId, FileId, LibraryId};
    let library = LibraryId::new("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b").expect("library");
    let collection = CollectionId::new("c0ffee00-1234-4abc-8def-0123456789ab").expect("id");
    let file = FileId::new("0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19").expect("file");
    let settings = InstallationSettings {
        default_retention_days: 30,
        expiry: ExpiryMode::KeepVersionHistory,
    };
    let view = RecentlyDeletedSettingsView {
        default_retention_days: 30,
        expiry: ExpiryMode::DeleteAllVersionHistory,
        may_change: true,
        libraries: vec![LibraryRetentionView {
            library_id: library.as_str().into(),
            name: "Studio work".into(),
            retention_days: Some(90),
            revision: "4".into(),
            may_change: true,
        }],
        complete: true,
    };
    let deletion = Deletion {
        deleted_at_ms: 1_000,
        purge_after_ms: 2_000,
        cause: DeletionCause::WithCollection,
    };
    let result = CommandResult {
        outcome: CommandOutcome::new(
            OperationId::new("op-1").expect("op"),
            CommandOutcomeKind::Committed,
        ),
        library_id: Some(library.clone()),
        revision: Some("5".into()),
        created_id: Some(collection.as_str().into()),
        deletion: Some(deletion),
        restored_into: vec![RestoredInto {
            from_collection_id: collection.clone(),
            collection_id: collection.clone(),
            title: "untitled collection 2".into(),
        }],
    };
    let commands = vec![
        LibraryCommand::DeleteFiles {
            library_id: library.clone(),
            file_ids: vec![file.clone()],
        },
        LibraryCommand::DeleteCollection {
            library_id: library.clone(),
            collection_id: collection.clone(),
        },
        LibraryCommand::RestoreFiles {
            library_id: library.clone(),
            file_ids: vec![file.clone()],
        },
        LibraryCommand::PurgeFiles {
            library_id: library.clone(),
            file_ids: vec![file],
            confirmation: PurgeConfirmation {
                library_id: library.clone(),
                file_count: 1,
            },
        },
        LibraryCommand::SetLibraryRetention {
            library_id: library,
            retention_days: Some(7),
        },
    ];
    let commands_view = commands.clone();
    let outcome = serde_json::json!({
        "operation_id": "op-1", "kind": "committed", "error": null, "result": to_value(&result)
    });
    vec![
        (
            "ExpiryMode",
            [
                ExpiryMode::KeepVersionHistory,
                ExpiryMode::DeleteAllVersionHistory,
            ]
            .map(to_value)
            .to_vec(),
        ),
        (
            "DeletionCause",
            [DeletionCause::OneByOne, DeletionCause::WithCollection]
                .map(to_value)
                .to_vec(),
        ),
        ("InstallationSettings", vec![to_value(settings)]),
        ("RecentlyDeletedSettings", vec![to_value(&view)]),
        ("Deletion", vec![to_value(deletion)]),
        (
            "DeletedRow",
            vec![to_value(filesquid_api::files::drive::DeletedRow {
                library_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
                library_name: "Family archive".into(),
                deleted_at: "2026-10-20T00:00:00.000Z".into(),
                purge_after: "2026-11-19T00:00:00.000Z".into(),
                days_left: 19,
                cause: DeletionCause::OneByOne,
                home_collection_id: "c0ffee00-1234-4abc-8def-0123456789ab".into(),
                home_collection_title: Some("Documents".into()),
                home_collection_deleted: false,
                folder: None,
            })],
        ),
        ("LibraryRetention", vec![to_value(&view.libraries[0])]),
        ("RestoredInto", vec![to_value(&result.restored_into[0])]),
        (
            "RecentlyDeletedCommand",
            vec![serde_json::json!({
                "envelope": {"schema_version": 1, "operation_id": "op-1", "expected_revision": "1", "scope": "library:x"},
                "input": to_value(&commands[0]),
            })],
        ),
        ("LibraryCommandResult", vec![to_value(&result)]),
        (
            "RecentlyDeletedCommandInput",
            commands_view.into_iter().map(to_value).collect(),
        ),
        ("RecentlyDeletedCommandOutcome", vec![outcome]),
    ]
}
