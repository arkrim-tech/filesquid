//! The desktop host of the import route tests (API-S-036): the owner's token, a journal, a
//! navigation service whose catalog holds two libraries with one collection each, scripted
//! starters, and the request body the tests send.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportStartError, ImportStarter, PlanBook, PlanFacts, PlanItem,
    PlanItemPage,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, LibraryCatalog, LibraryRecord, MemoryCatalog,
    NEW_LIBRARY_REVISION,
};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::navigation::{HostAccess, MemoryFolderIndex, NavigationService, RepositoryInfo};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_api::{router, ApiState};
use filesquid_core::{Collection, CollectionId, Library, LibraryId, RepositoryId};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// The import route.
pub const PATH: &str = "/api/v1/imports";
/// The library the tests import into.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
/// Another open library.
pub const OTHER_LIBRARY: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
/// The destination collection, of [`LIBRARY`].
pub const COLLECTION: &str = "c0ffee00-1234-4abc-8def-0123456789ab";
/// A collection of [`OTHER_LIBRARY`].
pub const FOREIGN_COLLECTION: &str = "f0f0f0f0-1234-4abc-8def-0123456789ab";

/// A starter without the review that answers `answer` (job `job-1` when started) and keeps
/// the requests, as the desktop does until Task DE-1.8.
pub struct ScriptedStarter {
    pub answer: Result<Option<&'static str>, ImportStartError>,
    pub asked: Mutex<Vec<ImportRequest>>,
}

impl ScriptedStarter {
    /// A starter answering `answer`.
    pub fn new(answer: Result<Option<&'static str>, ImportStartError>) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// The requests the dialog was shown for.
    pub fn asked(&self) -> Vec<ImportRequest> {
        self.asked.lock().expect("asked").clone()
    }
}

impl ImportStarter for ScriptedStarter {
    /// Scripted: every request the tests send is a first request.
    fn replay(&self, _request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        Ok(None)
    }

    fn start(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        self.asked.lock().expect("asked").push(request.clone());
        let job = self.answer.clone()?;
        let operation = request.envelope.operation_id();
        Ok(match job {
            Some(job) => ImportAnswer::started(operation, job.to_string()),
            None => ImportAnswer::cancelled(operation),
        })
    }
}

/// A desktop host with the owner's token and a journal.
pub struct Host {
    pub state: ApiState,
    pub monitor: Monitor,
    pub token: String,
}

impl Host {
    /// A host whose navigation catalog is [`navigation`].
    pub fn start() -> Self {
        Self::with_navigation(navigation())
    }

    /// A host over `navigation`.
    pub fn with_navigation(navigation: NavigationService) -> Self {
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
        state.install_navigation(Arc::new(navigation));
        Self {
            state,
            monitor,
            token: token.secret().to_string(),
        }
    }

    /// `POST /api/v1/imports` with `body`, with the owner's token when `owner`.
    pub async fn post(&self, body: Value, owner: bool) -> (StatusCode, Value) {
        self.send(Method::POST, PATH, Some(body), owner).await
    }

    /// Send `method` `path` with `body`, with the owner's token when `owner`; answer the
    /// status and the JSON body (`Null` when there is none).
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        owner: bool,
    ) -> (StatusCode, Value) {
        let token = owner.then_some(self.token.as_str());
        send_as(self.state.clone(), token, method, path, body).await
    }

    /// The journal entries of `operation`, oldest first.
    pub fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }

    /// Every journal entry as one JSON text, for redaction checks.
    pub fn journal_text(&self) -> String {
        self.monitor.journal().flush();
        let entries = self
            .monitor
            .store()
            .query(&JournalQuery::default())
            .expect("query")
            .entries;
        serde_json::to_string(&entries).expect("json")
    }
}

/// A navigation service over a catalog with [`LIBRARY`] holding [`COLLECTION`] and
/// [`OTHER_LIBRARY`] holding [`FOREIGN_COLLECTION`].
pub fn navigation() -> NavigationService {
    let catalog = Arc::new(MemoryCatalog::new());
    let repository = RepositoryId::new("repo-1").expect("repository");
    for (n, (library, collection)) in [(LIBRARY, COLLECTION), (OTHER_LIBRARY, FOREIGN_COLLECTION)]
        .into_iter()
        .enumerate()
    {
        let library_id = LibraryId::new(library).expect("library");
        let record = LibraryRecord {
            library: Library::new(library_id.clone(), repository.clone()),
            name: "Family".into(),
        };
        let collection = Collection::new(
            CollectionId::new(collection).expect("collection"),
            library_id.clone(),
            "Trips",
        )
        .expect("collection");
        let commit = CatalogCommit {
            operation_id: OperationId::new(format!("seed-{n}")).expect("op"),
            library_id,
            expected_revision: ExpectedRevision::new(NEW_LIBRARY_REVISION).expect("rev"),
            changes: vec![
                CatalogChange::CreateLibrary(record),
                CatalogChange::PutCollection(collection),
            ],
            journal: JournalEntry::new("library.create", Outcome::Succeeded),
            created_id: None,
            deletion: None,
        };
        catalog.commit(commit).expect("seed");
    }
    NavigationService::new(
        catalog,
        Arc::new(MemoryFolderIndex::new()),
        Arc::new(HostAccess),
        RepositoryInfo::new(repository, None),
    )
}

/// An import request body for `scope` into [`COLLECTION`], operation `op-1`.
pub fn body(scope: &str) -> Value {
    body_of("op-1", scope)
}

/// An import request body of `operation` for `scope` into [`COLLECTION`].
pub fn body_of(operation: &str, scope: &str) -> Value {
    json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": operation,
            "expected_revision": "r1",
            "scope": scope
        },
        "library_id": LIBRARY,
        "collection_id": COLLECTION,
        "source": "folder"
    })
}

/// `POST /api/v1/imports` with `body` and the bearer `token` to the router over `state`, so a
/// test can send it from a spawned task.
pub async fn post_as(state: ApiState, token: &str, body: Value) -> (StatusCode, Value) {
    send_as(state, Some(token), Method::POST, PATH, Some(body)).await
}

/// Send `method` `path` with `body`, with the bearer `token` when given, to the router over
/// `state`; answer the status and the JSON body (`Null` when there is none).
pub async fn send_as(
    state: ApiState,
    token: Option<&str>,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let bytes = body.map(|b| b.to_string()).unwrap_or_default();
    let request = request.body(Body::from(bytes)).expect("request");
    let response = router(state).oneshot(request).await.expect("answer");
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

/// The scope of [`LIBRARY`].
pub fn scope() -> String {
    format!("library:{LIBRARY}")
}

/// The paths the fake dialog "picks": they stay inside the starter and must never leave it.
pub const PICKED: [&str; 3] = [
    "/Volumes/EOS_DIGITAL/DCIM/IMG_4102.CR3",
    "/Volumes/EOS_DIGITAL/DCIM/IMG_4102.JPG",
    "/Volumes/EOS_DIGITAL/DCIM/IMG_4021.JPG",
];

/// A starter with the review: a scripted dialog, plans kept in a [`PlanBook`], a clock the
/// tests advance and a source fingerprint the tests change.
pub struct PlanningStarter {
    pub book: PlanBook,
    clock: AtomicI64,
    fingerprint: Mutex<String>,
    /// What the dialog answers, in order: `true` picks [`PICKED`], `false` cancels.
    dialog: Mutex<Vec<bool>>,
    dialogs: AtomicUsize,
    plans: AtomicUsize,
    pub jobs: AtomicUsize,
}

impl PlanningStarter {
    pub fn new(dialog: Vec<bool>) -> Arc<Self> {
        Arc::new(Self {
            book: PlanBook::new(),
            clock: AtomicI64::new(1_760_000_000_000),
            fingerprint: Mutex::new("fp-1".into()),
            dialog: Mutex::new(dialog),
            dialogs: AtomicUsize::new(0),
            plans: AtomicUsize::new(0),
            jobs: AtomicUsize::new(0),
        })
    }

    pub fn now(&self) -> i64 {
        self.clock.load(Ordering::SeqCst)
    }

    pub fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn change_source(&self) {
        *self.fingerprint.lock().expect("fingerprint") = "fp-2".into();
    }

    pub fn dialogs_shown(&self) -> usize {
        self.dialogs.load(Ordering::SeqCst)
    }

    /// The plan of the picked files: counts and sizes, never a path.
    pub fn facts() -> PlanFacts {
        PlanFacts {
            source_volume: Some("EOS_DIGITAL".into()),
            files_found: 3,
            files_new: 2,
            files_present: 1,
            bytes_to_copy: 31_200_000,
            names_numbered: 1,
            free_bytes_after: Some(408_900_000_000),
            staging_limit_bytes: 10 * 1024 * 1024 * 1024,
        }
    }

    /// The planned files, by base name.
    pub fn items() -> Vec<PlanItem> {
        PICKED
            .iter()
            .enumerate()
            .map(|(n, path)| PlanItem {
                item_id: format!("file-{n}"),
                name: path.rsplit('/').next().expect("name").to_string(),
                bytes: [24_800_000, 6_400_000, 5_900_000][n],
                present: n == 2,
                numbered: n == 0,
            })
            .collect()
    }
}

impl ImportStarter for PlanningStarter {
    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        self.book.replay(request, self.now())
    }

    fn plan(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        self.dialogs.fetch_add(1, Ordering::SeqCst);
        let mut dialog = self.dialog.lock().expect("dialog");
        let picked = dialog
            .first()
            .copied()
            .ok_or(ImportStartError::Unavailable)?;
        dialog.remove(0);
        if !picked {
            return Ok(self.book.cancelled(request));
        }
        let n = self.plans.fetch_add(1, Ordering::SeqCst) + 1;
        let fingerprint = self.fingerprint.lock().expect("fingerprint").clone();
        Ok(self.book.open(
            &format!("plan-{n}"),
            request,
            Self::facts(),
            Self::items(),
            &fingerprint,
            self.now(),
        ))
    }

    fn start_plan(&self, _actor: &str, plan_id: &str) -> Result<ImportAnswer, ImportStartError> {
        let fingerprint = self.fingerprint.lock().expect("fingerprint").clone();
        self.book.start(plan_id, &fingerprint, self.now(), |_, _| {
            let n = self.jobs.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(format!("job-{n}"))
        })
    }

    fn discard(&self, _actor: &str, plan_id: &str) -> Result<(), ImportStartError> {
        self.book.discard(plan_id, self.now())
    }

    fn plan_items(
        &self,
        plan_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<PlanItemPage, ImportStartError> {
        self.book.items(plan_id, cursor, limit, self.now())
    }
}
