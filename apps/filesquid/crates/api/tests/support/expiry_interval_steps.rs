//! Step definitions of `expiry_interval_API-S-019.feature` (Task API-F2.30; API-S-019
//! **Routes and expiry runs**, DE-S-014): the installation's expiry interval, read and set
//! through the settings routes of a router whose library service keeps its settings in the
//! in-memory reference store, and the pure [`ExpirySchedule`] every host's expiry monitor
//! asks when its next run falls due. The desktop's own monitor over the SQLite operational
//! store is bound in the desktop harness (`desktop_expiry_interval_F-008.feature`).

use crate::library_fixture::Fixture;
use crate::MonitorWorld;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::auth::{Action, Authenticator, Authoriser, Policy, Principal, Resource};
use filesquid_api::library::{
    ExpirySchedule, InstallationSettings, LibraryCommand, MemorySettings, RetentionSettings,
    SETTINGS_OPERATION,
};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// The admin's token (on the desktop, the owner's).
const ADMIN_TOKEN: &str = "expiry-interval-admin";
/// The token of a signed-in caller who is not the admin.
const MEMBER_TOKEN: &str = "expiry-interval-member";
/// The settings path.
const SETTINGS_PATH: &str = "/api/v1/settings/recently-deleted";
/// One minute in milliseconds.
const MINUTE_MS: i64 = 60_000;
/// The scenario's present, on the host's clock (milliseconds).
const NOW_MS: i64 = 1_000 * MINUTE_MS;

/// The installation's callers: the admin, and a member who may do everything but change the
/// installation settings.
struct Callers;

impl Authenticator for Callers {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        match token {
            ADMIN_TOKEN => Some(Principal::Owner),
            MEMBER_TOKEN => Some(Principal::Administrator),
            _ => None,
        }
    }
}

impl Policy for Callers {
    fn permits(&self, principal: &Principal, action: Action, _resource: &Resource) -> bool {
        action != Action::ConfigureRecentlyDeleted || *principal == Principal::Owner
    }
}

/// Per-scenario state of the expiry-interval steps.
#[derive(Default)]
pub struct ExpiryIntervalWorld {
    /// The library fixture whose service the routes run.
    fixture: Option<Fixture>,
    /// The API state of the installation.
    state: Option<ApiState>,
    /// The installation's settings store.
    settings: Option<Arc<MemorySettings>>,
    /// The host's expiry schedule, kept across a settings change (no restart).
    schedule: ExpirySchedule,
    /// The last answer: its status and JSON body.
    answer: Option<(StatusCode, Value)>,
    /// Operation IDs handed out so far.
    operations: u32,
}

impl ExpiryIntervalWorld {
    /// The settings store.
    fn settings(&self) -> &MemorySettings {
        self.settings.as_ref().expect("installation")
    }

    /// The last answer.
    fn answer(&self) -> &(StatusCode, Value) {
        self.answer.as_ref().expect("an answer")
    }

    /// The interval the installed service reads now, as a running host's monitor does.
    fn stored_interval(&self) -> u32 {
        let state = self.state.as_ref().expect("installation");
        let service = state.library_service().expect("library service");
        service
            .installation_settings()
            .expect("settings")
            .expiry_interval_minutes
    }

    /// Send `method` to the settings path as the holder of `token`; keep the answer.
    async fn send(&mut self, method: Method, token: &str, body: Option<Value>) {
        let request = Request::builder()
            .method(method)
            .uri(SETTINGS_PATH)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json");
        let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
        let state = self.state.clone().expect("installation");
        let response = router(state)
            .oneshot(request.body(body).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let bytes = response.into_body().collect().await.expect("body");
        let json = serde_json::from_slice(&bytes.to_bytes()).unwrap_or(Value::Null);
        self.answer = Some((status, json));
    }

    /// `PUT` the default settings with the interval `minutes` (any JSON number) as `token`.
    async fn set_interval(&mut self, token: &str, minutes: &str) {
        let minutes: Value = serde_json::from_str(minutes).expect("a JSON number");
        self.operations += 1;
        let body = json!({
            "envelope": {"schema_version": 1,
                         "operation_id": format!("op-expiry-interval-{}", self.operations),
                         "expected_revision": "0", "scope": "installation"},
            "input": {"default_retention_days": 30, "expiry": "keep_version_history",
                      "expiry_interval_minutes": minutes},
        });
        self.send(Method::PUT, token, Some(body)).await;
    }
}

#[given(expr = "expiry-interval installation with one open library holding a deleted file")]
fn installation(w: &mut MonitorWorld) {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Documents");
    let file = fx.add_file_in("f1", &collection);
    fx.run(LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: vec![file],
    });
    let settings = Arc::new(MemorySettings::new());
    let service = fx.service.clone().with_settings(settings.clone());
    let state = ApiState::new();
    assert!(state.install_authoriser(Authoriser::new(Arc::new(Callers), Arc::new(Callers))));
    assert!(state.install_library_service(Arc::new(service)));
    w.expiry_interval = ExpiryIntervalWorld {
        fixture: Some(fx),
        state: Some(state),
        settings: Some(settings),
        ..ExpiryIntervalWorld::default()
    };
}

#[given(expr = "expiry-interval host ran expiry at launch with the default interval")]
fn ran_at_launch(w: &mut MonitorWorld) {
    let world = &mut w.expiry_interval;
    assert!(
        world.schedule.is_due(world.stored_interval(), NOW_MS),
        "expiry is due at launch"
    );
    world.schedule.record_start(NOW_MS);
    assert!(!world.schedule.is_due(world.stored_interval(), NOW_MS));
}

#[given(expr = "expiry-interval last run started {int} minutes ago with the default interval")]
fn ran_minutes_ago(w: &mut MonitorWorld, minutes: i64) {
    let world = &mut w.expiry_interval;
    world.schedule.record_start(NOW_MS - minutes * MINUTE_MS);
    assert!(
        !world.schedule.is_due(world.stored_interval(), NOW_MS),
        "the default interval has not passed"
    );
}

#[when(expr = "expiry-interval caller reads the Recently deleted settings")]
async fn caller_reads(w: &mut MonitorWorld) {
    w.expiry_interval.send(Method::GET, ADMIN_TOKEN, None).await;
}

#[when(regex = r"^expiry-interval admin sets the expiry interval to ([0-9.]+) minutes$")]
async fn admin_sets(w: &mut MonitorWorld, minutes: String) {
    w.expiry_interval.set_interval(ADMIN_TOKEN, &minutes).await;
}

#[when(
    regex = r"^expiry-interval caller who is not the admin sets the expiry interval to ([0-9]+) minutes$"
)]
async fn member_sets(w: &mut MonitorWorld, minutes: String) {
    w.expiry_interval.set_interval(MEMBER_TOKEN, &minutes).await;
}

#[then(expr = "expiry-interval answer should carry expiry_interval_minutes {int}")]
fn answer_carries(w: &mut MonitorWorld, minutes: u32) {
    let (status, json) = w.expiry_interval.answer();
    assert_eq!(*status, StatusCode::OK, "{json}");
    assert_eq!(json["expiry_interval_minutes"], minutes, "{json}");
}

#[then(regex = r"^expiry-interval settings should (?:still )?hold ([0-9]+) minutes$")]
async fn settings_hold(w: &mut MonitorWorld, minutes: u32) {
    let world = &mut w.expiry_interval;
    let stored = world.settings().read().expect("settings");
    assert_eq!(stored.expiry_interval_minutes, minutes);
    world.send(Method::GET, ADMIN_TOKEN, None).await;
    assert_eq!(world.answer().1["expiry_interval_minutes"], minutes);
}

#[then(
    expr = "expiry-interval change should be journaled once as settings.recently_deleted.set without its values"
)]
fn journaled_once(w: &mut MonitorWorld) {
    let entries = w.expiry_interval.settings().journal_entries();
    assert_eq!(entries.len(), 1, "journaled once");
    assert_eq!(entries[0].operation, SETTINGS_OPERATION);
    assert!(entries[0].target_ids.is_empty(), "no value is journaled");
    let text = serde_json::to_string(&entries[0]).expect("entry");
    assert!(!text.contains("expiry_interval"), "{text}");
}

#[then(expr = "expiry-interval next run should fall due {int} minutes after the last run started")]
fn next_run_due(w: &mut MonitorWorld, minutes: i64) {
    let world = &w.expiry_interval;
    let started = world.schedule.last_started_ms().expect("a last run");
    let due = world.schedule.next_due_ms(world.stored_interval());
    assert_eq!(due, Some(started + minutes * MINUTE_MS));
}

#[then(expr = "expiry-interval host should not need a restart")]
fn no_restart(w: &mut MonitorWorld) {
    // The same service and the same schedule as before the change answer the new interval.
    let world = &w.expiry_interval;
    let interval = world.stored_interval();
    let due = world.schedule.next_due_ms(interval).expect("a last run");
    assert!(!world.schedule.is_due(interval, due - 1));
    assert!(world.schedule.is_due(interval, due));
}

#[then(expr = "expiry-interval next run should start at once")]
fn starts_at_once(w: &mut MonitorWorld) {
    let world = &w.expiry_interval;
    assert!(world.schedule.is_due(world.stored_interval(), NOW_MS));
}

#[then(expr = "expiry-interval store should be unchanged and nothing journaled")]
fn store_unchanged(w: &mut MonitorWorld) {
    let world = &w.expiry_interval;
    let (status, json) = world.answer();
    assert_eq!(*status, StatusCode::OK, "{json}");
    let stored = world.settings().read().expect("settings");
    assert_eq!(stored, InstallationSettings::default());
    assert!(world.settings().journal_entries().is_empty());
    let fixture = world.fixture.as_ref().expect("installation");
    assert!(fixture.failure_journal().is_empty(), "no refusal either");
}

#[then(regex = r"^expiry-interval answer should be refused as (invalid_request|forbidden)$")]
fn refused_as(w: &mut MonitorWorld, code: String) {
    let (status, json) = w.expiry_interval.answer();
    let expected = match code.as_str() {
        "forbidden" => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_REQUEST,
    };
    assert_eq!(*status, expected, "{json}");
    assert_eq!(json["code"], code.as_str(), "{json}");
}
