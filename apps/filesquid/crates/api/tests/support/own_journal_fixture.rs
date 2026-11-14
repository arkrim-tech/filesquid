//! Fixture of the self-scoped journal tests (Task API-F2.27; API-S-031): a host with two
//! callers and a seeded in-memory journal, driven through the router.
//!
//! Server accounts arrive with API-F1.9, so a caller who is not an administrator is built
//! from the authoriser's other principal: "ana" presents [`ANA_TOKEN`] and is the
//! [`Principal::Owner`] under [`AdministratorsOnly`], a policy that grants that principal
//! nothing; "ben" presents [`BEN_TOKEN`] and is the [`Principal::Administrator`], whom the
//! policy grants everything. Journal entries name a caller by `Principal::actor`.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{
    Action, Authenticator, Authoriser, Policy, Principal, Resource, TokenAuthenticator,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

/// The token of "ana", the caller who is not an administrator.
pub const ANA_TOKEN: &str = "ana-token";
/// The token of "ben", the administrator.
pub const BEN_TOKEN: &str = "ben-token";
/// The file "ana" uploaded and may read.
pub const FILE: &str = "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab";
/// The file "ana" uploaded and may no longer read.
pub const LOST_FILE: &str = "6a1d4f55-3b8c-4d2e-8f90-123456789abc";

/// The object ID of the file `uuid`.
pub fn object(uuid: &str) -> String {
    format!("urn:uuid:{uuid}")
}

/// The journal actor of the caller the scenarios call `name` ("ana" or "ben").
pub fn actor_of(name: &str) -> &'static str {
    principal_of(name).actor()
}

/// The principal of the caller the scenarios call `name`.
fn principal_of(name: &str) -> Principal {
    match name {
        "ana" => Principal::Owner,
        "ben" => Principal::Administrator,
        other => panic!("no caller named {other}"),
    }
}

/// The token of the caller the scenarios call `name`.
pub fn token_of(name: &str) -> &'static str {
    match name {
        "ana" => ANA_TOKEN,
        "ben" => BEN_TOKEN,
        other => panic!("no caller named {other}"),
    }
}

/// Accepts [`ANA_TOKEN`] and [`BEN_TOKEN`], each as its own principal.
struct TwoCallers {
    ana: TokenAuthenticator,
    ben: TokenAuthenticator,
}

impl Authenticator for TwoCallers {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        self.ana
            .authenticate(token)
            .or_else(|| self.ben.authenticate(token))
    }
}

/// Grants the administrator everything and every other principal nothing.
struct AdministratorsOnly;

impl Policy for AdministratorsOnly {
    fn permits(&self, principal: &Principal, _action: Action, _resource: &Resource) -> bool {
        *principal == Principal::Administrator
    }
}

/// The authoriser of a host with "ana" (not an administrator) and "ben" (administrator).
pub fn two_callers() -> Authoriser {
    let callers = TwoCallers {
        ana: TokenAuthenticator::new(ANA_TOKEN, principal_of("ana")),
        ben: TokenAuthenticator::new(BEN_TOKEN, principal_of("ben")),
    };
    Authoriser::new(Arc::new(callers), Arc::new(AdministratorsOnly))
}

/// One entry by `actor`.
pub fn entry_by(actor: &str, operation: &str, outcome: Outcome) -> JournalEntry {
    JournalEntry::new(operation, outcome).actor(actor)
}

/// The seeded history, ids 1 to 9: four entries by "ana" (three uploads, one of them of
/// [`FILE`] and one of [`LOST_FILE`], and a failed move), three by "ben", one of the host
/// itself and one refusal of an anonymous caller.
pub fn seeded() -> Vec<JournalEntry> {
    let (ana, ben) = (actor_of("ana"), actor_of("ben"));
    vec![
        entry_by(ana, "file.upload", Outcome::Succeeded)
            .operation_id("op-1")
            .object_ref(format!("{}@v1", object(FILE)))
            .target("lib-a"),
        entry_by(ben, "file.upload", Outcome::Succeeded)
            .operation_id("op-2")
            .target("lib-b"),
        entry_by(ana, "file.upload", Outcome::Succeeded)
            .operation_id("op-3")
            .object_ref(format!("{}@v1", object(LOST_FILE)))
            .target("lib-a"),
        entry_by("system", "host.started", Outcome::Succeeded),
        entry_by(ben, "file.rename", Outcome::Succeeded).operation_id("op-5"),
        entry_by(ana, "file.move", Outcome::Failed)
            .operation_id("op-6")
            .object_ref(format!("{}@v1", object(FILE)))
            .target("lib-a")
            .error_code("conflict"),
        entry_by(ben, "file.upload", Outcome::Succeeded).operation_id("op-7"),
        entry_by("anonymous", "api.auth.denied", Outcome::Failed)
            .error_code("missing_credentials")
            .target("health.read"),
        entry_by(ana, "file.upload", Outcome::Succeeded)
            .operation_id("op-9")
            .target("lib-b"),
    ]
}

/// A host with a running monitor over an in-memory journal.
pub struct OwnHost {
    /// The API state the router serves.
    pub state: ApiState,
    /// The running monitor; shut down when the host is dropped.
    pub monitor: Monitor,
    /// The journal store behind the monitor.
    pub store: Arc<MemoryJournal>,
}

impl Drop for OwnHost {
    fn drop(&mut self) {
        self.monitor.shutdown();
    }
}

impl OwnHost {
    /// Start a host with `authoriser` whose journal holds `entries`.
    pub fn start(authoriser: Authoriser, entries: &[JournalEntry]) -> Self {
        let state = ApiState::new();
        state.install_authoriser(authoriser);
        let store = Arc::new(MemoryJournal::new("host-a"));
        store.append(entries).expect("seed");
        let monitor = Monitor::start(
            store.clone(),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        Self {
            state,
            monitor,
            store,
        }
    }

    /// `GET uri` with `token` as the Bearer credential; the status and the JSON body
    /// (`Null` when the body is not JSON).
    pub async fn get(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::get(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
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

    /// Every stored entry, after the journal queue is flushed.
    pub fn stored(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let all = JournalQuery {
            limit: 1000,
            ..Default::default()
        };
        self.store.query(&all).expect("query").entries
    }
}

/// The `entries` array of a journal page.
pub fn entries_of(page: &Value) -> &Vec<Value> {
    page["entries"].as_array().expect("entries")
}
