//! Unit tests for the folder-choice route (API-S-025): `POST /api/v1/libraries/location-choices`
//! answers the opaque location ID the host's dialog registered, or that the owner cancelled;
//! refuses while a dialog is open (`409`), a folder the host cannot register (`409`), an
//! unreadable body (`400`) and a host without a dialog (`503`); journals each request once
//! without a path; and is authorised before the handler runs. Task API-F2.31 (API-S-034
//! **Folder and volume names**): a chosen folder is answered with its own name and its
//! volume's name, each `null` when the host cannot read it, after API-S-005's safe
//! transformation; neither name reaches the journal, an event or a log line.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::local_library::choice::{
    ChoicePurpose, ChooseError, ChosenFolder, LocationChooser, CHOOSE_OPERATION,
};
use filesquid_api::local_library::FolderNames;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const PATH: &str = "/api/v1/libraries/location-choices";

/// What a scripted dialog answers.
type Scripted = Result<Option<ChosenFolder>, ChooseError>;

/// A dialog that answers `answer` and keeps the purposes it was asked for.
struct ScriptedChooser {
    answer: Scripted,
    asked: Mutex<Vec<ChoicePurpose>>,
}

/// A chosen folder registered as `location` whose host reads no name.
fn chosen(location: &str) -> Option<ChosenFolder> {
    Some(ChosenFolder::new(location, FolderNames::default()))
}

impl ScriptedChooser {
    fn new(answer: Scripted) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }
}

impl LocationChooser for ScriptedChooser {
    fn choose(&self, purpose: ChoicePurpose) -> Scripted {
        self.asked.lock().expect("asked").push(purpose);
        self.answer.clone()
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

    /// POST `body`, with the owner's token when `owner`.
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

    /// Every stored folder-choice entry.
    fn choices(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(CHOOSE_OPERATION.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }
}

#[tokio::test]
async fn a_picked_folder_answers_its_opaque_location_id_only() {
    let host = Host::start();
    let chooser = ScriptedChooser::new(Ok(chosen("loc-1234")));
    assert!(host.state.install_location_chooser(chooser.clone()));
    let (status, body) = host
        .post(json!({ "purpose": "copy_destination" }), true)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "outcome": "chosen", "location_id": "loc-1234",
                "folder_name": null, "volume_name": null })
    );
    assert_eq!(
        chooser.asked.lock().expect("asked").as_slice(),
        [ChoicePurpose::CopyDestination]
    );
    let entries = host.choices();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].target_ids, ["loc-1234"]);
    assert!(!host
        .state
        .install_location_chooser(ScriptedChooser::new(Ok(None))));
}

#[tokio::test]
async fn a_cancelled_dialog_answers_cancelled_and_journals_it() {
    let host = Host::start();
    host.state
        .install_location_chooser(ScriptedChooser::new(Ok(None)));
    let (status, body) = host.post(json!({ "purpose": "open" }), true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "outcome": "cancelled", "folder_name": null, "volume_name": null })
    );
    assert_eq!(host.choices()[0].outcome, Outcome::Cancelled);
}

#[tokio::test]
async fn refusals_are_typed_and_journaled_once_with_their_code() {
    let cases = [
        (Err(ChooseError::Busy), StatusCode::CONFLICT, "dialog_open"),
        (
            Err(ChooseError::Refused),
            StatusCode::CONFLICT,
            "location_refused",
        ),
        (
            Err(ChooseError::Unavailable),
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
        ),
    ];
    for (answer, status, code) in cases {
        let host = Host::start();
        host.state
            .install_location_chooser(ScriptedChooser::new(answer));
        let (got, body) = host.post(json!({ "purpose": "create" }), true).await;
        assert_eq!(got, status, "{code}: {body}");
        let entries = host.choices();
        assert_eq!(entries.len(), 1, "{code}");
        assert_eq!(entries[0].error_code.as_deref(), Some(code));
    }
}

#[tokio::test]
async fn without_a_dialog_or_with_an_unreadable_body_nothing_is_asked() {
    let host = Host::start();
    let (status, _) = host.post(json!({ "purpose": "create" }), true).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let chooser = ScriptedChooser::new(Ok(chosen("loc-1")));
    host.state.install_location_chooser(chooser.clone());
    let (status, _) = host.post(json!({ "path": "/home/someone" }), true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = host.post(json!({ "purpose": "create" }), false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(chooser.asked.lock().expect("asked").is_empty());
    let codes: Vec<_> = host
        .choices()
        .into_iter()
        .map(|e| e.error_code.unwrap_or_default())
        .collect();
    assert_eq!(codes, ["unavailable", "invalid_request"]);
    assert!(!format!("{:?}", host.choices()).contains("someone"));
}

/// A log line recorder: every event's fields, as text.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<String>>>);

/// Writes each field of an event into one line.
struct LineVisitor<'a>(&'a mut String);

impl tracing::field::Visit for LineVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Lines {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut line = String::new();
        event.record(&mut LineVisitor(&mut line));
        self.0.lock().expect("lines").push(line);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_chosen_folder_is_answered_with_its_own_name_and_volume_and_nothing_else_names_them() {
    use tracing_subscriber::layer::SubscriberExt;
    let lines = Lines::default();
    let _logs =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(lines.clone()));
    let host = Host::start();
    let mut events = host.state.events.subscribe();
    let names = FolderNames::safe(Some("Tax 2026"), Some("Backup"));
    let chooser = ScriptedChooser::new(Ok(Some(ChosenFolder::new("loc-tax", names))));
    host.state.install_location_chooser(chooser);
    let (status, body) = host.post(json!({ "purpose": "create" }), true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "outcome": "chosen", "location_id": "loc-tax",
                "folder_name": "Tax 2026", "volume_name": "Backup" })
    );
    let entries = host.choices();
    assert_eq!(entries[0].target_ids, ["loc-tax"], "the opaque ID only");
    let journal = serde_json::to_string(&entries).expect("journal");
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(serde_json::to_string(&event).expect("event"));
    }
    tracing::info!(
        probe = "capture-live",
        "the recorder sees this thread's log lines"
    );
    let logged = lines.0.lock().expect("lines").join("\n");
    assert!(
        logged.contains("capture-live"),
        "the log capture is live: {logged}"
    );
    for text in [&journal, &seen.join("\n"), &logged] {
        for name in ["Tax 2026", "Backup"] {
            assert!(!text.contains(name), "{name} leaked: {text}");
        }
    }
}

/// A host that hands the route a name it did not make safe.
struct CarelessChooser;

impl LocationChooser for CarelessChooser {
    fn choose(&self, _purpose: ChoicePurpose) -> Scripted {
        let names = FolderNames {
            folder_name: Some("../home/someone\u{7}".into()),
            volume_name: Some("   ".into()),
        };
        Ok(Some(ChosenFolder::new("loc-careless", names)))
    }
}

#[tokio::test]
async fn the_route_answers_names_only_after_the_safe_transformation() {
    let host = Host::start();
    host.state
        .install_location_chooser(Arc::new(CarelessChooser));
    let (status, body) = host.post(json!({ "purpose": "open" }), true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["folder_name"], ".._home_someone");
    assert_eq!(
        body["volume_name"],
        Value::Null,
        "nothing is left of a blank name"
    );
}
