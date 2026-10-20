//! Unit tests for the history backfill route (API-S-029): `POST /api/v1/admin/history/backfill`
//! starts the host's backfill and answers `200`, refuses while one runs (`409`) and before the
//! host installs a launcher (`503`), journals each request once, and is authorised before the
//! handler runs.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Action, Authoriser, LaunchToken};
use filesquid_api::history::launch::{BackfillLauncher, LaunchError, BACKFILL_REQUEST_OPERATION};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

const PATH: &str = "/api/v1/admin/history/backfill";

/// A launcher that counts launches and refuses while `running` is set.
#[derive(Default)]
struct CountingLauncher {
    launches: AtomicUsize,
    running: AtomicBool,
    actors: std::sync::Mutex<Vec<String>>,
}

impl BackfillLauncher for CountingLauncher {
    fn launch(&self, actor: &str) -> Result<(), LaunchError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(LaunchError::Running);
        }
        self.launches.fetch_add(1, Ordering::SeqCst);
        self.actors.lock().expect("actors").push(actor.to_string());
        Ok(())
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

    /// POST the route, with the owner's token when `owner`.
    async fn post(&self, owner: bool) -> (StatusCode, Value) {
        let mut request = Request::post(PATH);
        if owner {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        }
        let response = router(self.state.clone())
            .oneshot(request.body(Body::empty()).expect("request"))
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

    /// Every stored entry named `operation`.
    fn entries_named(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }
}

#[tokio::test]
async fn the_owner_starts_one_backfill_and_a_second_request_is_refused_while_it_runs() {
    let host = Host::start();
    let launcher = Arc::new(CountingLauncher::default());
    assert!(host.state.install_history_backfill(launcher.clone()));
    let (status, body) = host.post(true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "started");
    let (status, body) = host.post(true).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "conflict");
    assert_eq!(launcher.launches.load(Ordering::SeqCst), 1);
    assert_eq!(
        launcher.actors.lock().expect("actors").as_slice(),
        ["owner"]
    );
    let entries = host.entries_named(BACKFILL_REQUEST_OPERATION);
    assert_eq!(entries.len(), 2, "each request is journaled once");
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[1].outcome, Outcome::Failed);
    assert_eq!(entries[1].error_code.as_deref(), Some("backfill_running"));
    assert!(!host
        .state
        .install_history_backfill(Arc::new(CountingLauncher::default())));
}

#[tokio::test]
async fn without_a_launcher_the_route_is_unavailable_and_journaled_once() {
    let host = Host::start();
    let (status, body) = host.post(true).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "unavailable");
    assert_eq!(body["retryable"], true);
    let entries = host.entries_named(BACKFILL_REQUEST_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].error_code.as_deref(), Some("unavailable"));
}

#[tokio::test]
async fn an_unauthenticated_request_never_reaches_the_launcher() {
    let host = Host::start();
    let launcher = Arc::new(CountingLauncher::default());
    host.state.install_history_backfill(launcher.clone());
    let (status, _) = host.post(false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(launcher.launches.load(Ordering::SeqCst), 0);
    assert!(host.entries_named(BACKFILL_REQUEST_OPERATION).is_empty());
    assert_eq!(host.entries_named("api.auth.denied").len(), 1);
}

#[test]
fn the_route_performs_the_history_backfill_action() {
    assert_eq!(Action::RunHistoryBackfill.as_str(), "history.backfill");
}
