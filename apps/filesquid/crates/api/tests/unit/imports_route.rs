//! Unit tests for the import route (API-S-036): `POST /api/v1/imports` answers the host's job
//! or `cancelled`; refuses a body whose envelope scope is not the library's (`400`), an open
//! dialog (`409`), a library that is not open (`404`) and a host without an import dialog
//! (`503`); journals each request once without a path; and is authorised before it runs.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::imports::{
    ImportAnswer, ImportOutcome, ImportRequest, ImportStartError, ImportStarter,
    COLLECTION_NOT_SUPPORTED, IMPORT_START_OPERATION,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const PATH: &str = "/api/v1/imports";
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A starter that answers `answer` (job `job-1` when started) and keeps the requests.
struct ScriptedStarter {
    answer: Result<Option<&'static str>, ImportStartError>,
    asked: Mutex<Vec<ImportRequest>>,
}

impl ScriptedStarter {
    fn new(answer: Result<Option<&'static str>, ImportStartError>) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }
}

impl ImportStarter for ScriptedStarter {
    fn start(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        self.asked.lock().expect("asked").push(request.clone());
        let job = self.answer.clone()?;
        Ok(ImportAnswer {
            operation_id: request.envelope.operation_id().as_str().to_string(),
            outcome: match job {
                Some(_) => ImportOutcome::Started,
                None => ImportOutcome::Cancelled,
            },
            job_id: job.map(str::to_string),
        })
    }
}

/// A desktop host with the owner's token and a journal.
struct Host {
    state: ApiState,
    monitor: Monitor,
    token: String,
}

impl Host {
    fn start() -> Self {
        let state = ApiState::new();
        let token = LaunchToken::generate().expect("token");
        state.install_authoriser(Authoriser::desktop_owner(&token));
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("host-a")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        Self {
            state,
            monitor,
            token: token.secret().to_string(),
        }
    }

    async fn post(&self, body: Value, owner: bool) -> (StatusCode, Value) {
        let mut request = Request::post(PATH).header(header::CONTENT_TYPE, "application/json");
        if owner {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        }
        let request = request.body(Body::from(body.to_string())).expect("request");
        let response = router(self.state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn starts(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(IMPORT_START_OPERATION.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }
}

/// An import request body for `scope`.
fn body(scope: &str) -> Value {
    json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": "op-1",
            "expected_revision": "r1",
            "scope": scope
        },
        "library_id": LIBRARY,
        "source": "folder"
    })
}

#[tokio::test]
async fn a_started_import_answers_its_job_and_journals_it_once() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    assert!(host.state.install_import_starter(starter.clone()));
    let (status, answer) = host.post(body(&format!("library:{LIBRARY}")), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer,
        json!({ "operation_id": "op-1", "outcome": "started", "job_id": "job-1" })
    );
    assert_eq!(starter.asked.lock().expect("asked")[0].library_id, LIBRARY);
    let entries = host.starts();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].target_ids, ["job-1"]);
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
}

#[tokio::test]
async fn a_cancelled_dialog_answers_cancelled_without_a_job() {
    let host = Host::start();
    host.state
        .install_import_starter(ScriptedStarter::new(Ok(None)));
    let (status, answer) = host.post(body(&format!("library:{LIBRARY}")), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer,
        json!({ "operation_id": "op-1", "outcome": "cancelled" })
    );
    assert_eq!(host.starts()[0].outcome, Outcome::Cancelled);
}

#[tokio::test]
async fn refusals_are_typed_and_journaled_once_with_their_code() {
    let cases = [
        (ImportStartError::Busy, StatusCode::CONFLICT, "dialog_open"),
        (
            ImportStartError::LibraryNotFound,
            StatusCode::NOT_FOUND,
            "library_not_found",
        ),
        (
            ImportStartError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
        ),
    ];
    for (err, status, code) in cases {
        let host = Host::start();
        host.state
            .install_import_starter(ScriptedStarter::new(Err(err)));
        let (got, answer) = host.post(body(&format!("library:{LIBRARY}")), true).await;
        assert_eq!(got, status, "{code}: {answer}");
        let entries = host.starts();
        assert_eq!(entries.len(), 1, "{code}");
        assert_eq!(entries[0].error_code.as_deref(), Some(code));
    }
}

#[tokio::test]
async fn a_wrong_scope_no_starter_or_no_token_asks_nothing() {
    let host = Host::start();
    let (status, _) = host.post(body(&format!("library:{LIBRARY}")), true).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no starter installed"
    );
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let (status, _) = host.post(body("library:another"), true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = host.post(json!({ "path": "/home/someone" }), true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = host.post(body(&format!("library:{LIBRARY}")), false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(starter.asked.lock().expect("asked").is_empty());
    let codes: Vec<_> = host
        .starts()
        .into_iter()
        .map(|e| e.error_code.unwrap_or_default())
        .collect();
    assert_eq!(codes, ["unavailable", "invalid_request", "invalid_request"]);
    assert!(!format!("{:?}", host.starts()).contains("someone"));
}

#[tokio::test]
async fn a_collection_is_refused_before_any_dialog() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let mut request = body(&format!("library:{LIBRARY}"));
    request["collection_id"] = json!("col-1");
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    assert_eq!(answer["code"], "invalid_request");
    assert!(
        answer["message"]
            .as_str()
            .is_some_and(|m| m.contains(COLLECTION_NOT_SUPPORTED)),
        "{answer}"
    );
    assert!(starter.asked.lock().expect("asked").is_empty(), "no dialog");
    let entries = host.starts();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].error_code.as_deref(),
        Some(COLLECTION_NOT_SUPPORTED)
    );
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
}
