//! Unit tests for the import route (API-S-036): `POST /api/v1/imports` answers the host's job
//! or `cancelled`; refuses a body whose envelope scope is not the library's (`400`), a request
//! without a destination collection (`400`, `destination_required`) or with an unknown one
//! (`404`, `collection_not_found`) before any dialog (Task DE-1.7, ADR 041), an open dialog
//! (`409`), a library that is not open (`404`), a catalog that failed (`500`) and a host without
//! an import dialog (`503`); answers a repeated operation with its job before checking anything;
//! journals each request once without a path; and is authorised before it runs.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportStartError, ImportStarter, COLLECTION_NOT_FOUND,
    DESTINATION_REQUIRED, IMPORT_START_OPERATION,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, Keyed,
    LibraryCatalog, LibraryRecord, LibraryState, MemoryCatalog, PageKey, NEW_LIBRARY_REVISION,
};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::navigation::{HostAccess, MemoryFolderIndex, NavigationService, RepositoryInfo};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_api::{router, ApiState};
use filesquid_core::{
    Collection, CollectionId, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const PATH: &str = "/api/v1/imports";
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
const OTHER_LIBRARY: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
const COLLECTION: &str = "c0ffee00-1234-4abc-8def-0123456789ab";
const FOREIGN_COLLECTION: &str = "f0f0f0f0-1234-4abc-8def-0123456789ab";

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
        let operation = request.envelope.operation_id();
        Ok(match job {
            Some(job) => ImportAnswer::started(operation, job.to_string()),
            None => ImportAnswer::cancelled(operation),
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
        Self::with_navigation(navigation())
    }

    fn with_navigation(navigation: NavigationService) -> Self {
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

/// A navigation service over a catalog with [`LIBRARY`] holding [`COLLECTION`] and
/// [`OTHER_LIBRARY`] holding [`FOREIGN_COLLECTION`].
fn navigation() -> NavigationService {
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

/// An import request body for `scope` into [`COLLECTION`].
fn body(scope: &str) -> Value {
    json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": "op-1",
            "expected_revision": "r1",
            "scope": scope
        },
        "library_id": LIBRARY,
        "collection_id": COLLECTION,
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
        json!({
            "outcome": { "operation_id": "op-1", "kind": "pending", "error": null },
            "import": "started",
            "job_id": "job-1"
        })
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
        json!({
            "outcome": { "operation_id": "op-1", "kind": "committed", "error": null },
            "import": "cancelled"
        })
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
    let mut newer = body(&format!("library:{LIBRARY}"));
    newer["envelope"]["schema_version"] = json!(2);
    let (status, _) = host.post(newer, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "another schema version");
    let mut unnamed = body("library:");
    unnamed["library_id"] = json!("");
    let (status, _) = host.post(unnamed, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no library");
    let (status, _) = host.post(body(&format!("library:{LIBRARY}")), false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(starter.asked.lock().expect("asked").is_empty());
    let codes: Vec<_> = host
        .starts()
        .into_iter()
        .map(|e| e.error_code.unwrap_or_default())
        .collect();
    assert_eq!(
        codes,
        [
            "unavailable",
            "invalid_request",
            "invalid_request",
            "invalid_request",
            "invalid_request"
        ]
    );
    assert!(!format!("{:?}", host.starts()).contains("someone"));
}

#[tokio::test]
async fn the_destination_reaches_the_host() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let mut request = body(&format!("library:{LIBRARY}"));
    request["folder"] = json!("trips/2024");
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let asked = starter.asked.lock().expect("asked");
    assert_eq!(asked[0].collection_id.as_deref(), Some(COLLECTION));
    assert_eq!(asked[0].folder.as_deref(), Some("trips/2024"));
}

#[tokio::test]
async fn an_import_without_a_destination_is_refused_before_any_dialog() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let mut request = body(&format!("library:{LIBRARY}"));
    request
        .as_object_mut()
        .expect("object")
        .remove("collection_id");
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    assert_eq!(answer["code"], "invalid_request");
    assert!(
        answer["message"]
            .as_str()
            .is_some_and(|m| m.contains(DESTINATION_REQUIRED)),
        "{answer}"
    );
    assert!(starter.asked.lock().expect("asked").is_empty(), "no dialog");
    let entries = host.starts();
    assert_eq!(entries.len(), 1, "journaled once");
    assert_eq!(entries[0].error_code.as_deref(), Some(DESTINATION_REQUIRED));
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
}

#[tokio::test]
async fn an_unknown_or_foreign_collection_is_not_found_before_any_dialog() {
    for collection in [
        "9e9e9e9e-1234-4abc-8def-0123456789ab",
        FOREIGN_COLLECTION,
        "../x",
    ] {
        let host = Host::start();
        let starter = ScriptedStarter::new(Ok(Some("job-1")));
        host.state.install_import_starter(starter.clone());
        let mut request = body(&format!("library:{LIBRARY}"));
        request["collection_id"] = json!(collection);
        let (status, answer) = host.post(request, true).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{collection}: {answer}");
        assert!(
            answer["message"]
                .as_str()
                .is_some_and(|m| m.contains(COLLECTION_NOT_FOUND)),
            "{answer}"
        );
        assert!(starter.asked.lock().expect("asked").is_empty(), "no dialog");
        let entries = host.starts();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].error_code.as_deref(), Some(COLLECTION_NOT_FOUND));
    }
}

#[tokio::test]
async fn a_destination_folder_outside_the_path_rule_is_refused_before_any_dialog() {
    for folder in ["../escape", "provenance", ".filesquid/x", "a//b", ""] {
        let host = Host::start();
        let starter = ScriptedStarter::new(Ok(Some("job-1")));
        host.state.install_import_starter(starter.clone());
        let mut request = body(&format!("library:{LIBRARY}"));
        request["folder"] = json!(folder);
        let (status, answer) = host.post(request, true).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{folder:?}: {answer}");
        assert!(starter.asked.lock().expect("asked").is_empty(), "no dialog");
        let entries = host.starts();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].error_code.as_deref(), Some("invalid_path"));
        let journaled = format!("{entries:?}");
        if !folder.is_empty() {
            assert!(!journaled.contains(folder), "{folder:?}: {journaled}");
        }
    }
}

/// A starter that knows the job of every operation already, as after a first start.
struct ReplayingStarter;

impl ImportStarter for ReplayingStarter {
    fn start(&self, _: &str, _: &ImportRequest) -> Result<ImportAnswer, ImportStartError> {
        panic!("a replay shows no dialog");
    }

    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        let operation = request.envelope.operation_id();
        Ok(Some(ImportAnswer::started(operation, "job-1".to_string())))
    }
}

#[tokio::test]
async fn a_repeated_operation_answers_its_job_before_the_destination_is_checked_again() {
    let host = Host::start();
    host.state
        .install_import_starter(Arc::new(ReplayingStarter));
    let mut request = body(&format!("library:{LIBRARY}"));
    request["collection_id"] = json!(FOREIGN_COLLECTION);
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["job_id"], "job-1");
    assert_eq!(answer["import"], "started");
    let entries = host.starts();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
}

/// A catalog whose store fails for every call in a way a retry will not fix.
struct BrokenCatalog;

/// The error every [`BrokenCatalog`] call answers.
fn broken<T>() -> Result<T, CatalogError> {
    Err(CatalogError::Failed("store broken".into()))
}

impl LibraryCatalog for BrokenCatalog {
    fn receipt(&self, _: &OperationId) -> Result<Option<CommitReceipt>, CatalogError> {
        broken()
    }
    fn library(&self, _: &LibraryId) -> Result<Option<LibraryState>, CatalogError> {
        broken()
    }
    fn libraries(
        &self,
        _: &RepositoryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<LibraryRecord>>, CatalogError> {
        broken()
    }
    fn collection(&self, _: &CollectionId) -> Result<Option<Collection>, CatalogError> {
        broken()
    }
    fn collections(
        &self,
        _: &LibraryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<Collection>>, CatalogError> {
        broken()
    }
    fn virtual_collection(
        &self,
        _: &VirtualCollectionId,
    ) -> Result<Option<VirtualCollection>, CatalogError> {
        broken()
    }
    fn virtual_collections(
        &self,
        _: &LibraryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError> {
        broken()
    }
    fn is_virtual_member(&self, _: &VirtualCollectionId, _: &FileId) -> Result<bool, CatalogError> {
        broken()
    }
    fn last_member_order(&self, _: &VirtualCollectionId) -> Result<Option<i64>, CatalogError> {
        broken()
    }
    fn folder(&self, _: &FolderId) -> Result<Option<Folder>, CatalogError> {
        broken()
    }
    fn file(&self, _: &FileId) -> Result<Option<FileEntry>, CatalogError> {
        broken()
    }
    fn files(
        &self,
        _: &FileListing,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError> {
        broken()
    }
    fn commit(&self, _: CatalogCommit) -> Result<CommitReceipt, CatalogError> {
        broken()
    }
}

#[tokio::test]
async fn a_catalog_failure_a_retry_cannot_fix_is_internal_not_unavailable() {
    let navigation = NavigationService::new(
        Arc::new(BrokenCatalog),
        Arc::new(MemoryFolderIndex::new()),
        Arc::new(HostAccess),
        RepositoryInfo::new(RepositoryId::new("repo-1").expect("repository"), None),
    );
    let host = Host::with_navigation(navigation);
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let (status, answer) = host.post(body(&format!("library:{LIBRARY}")), true).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{answer}");
    assert!(starter.asked.lock().expect("asked").is_empty(), "no dialog");
    let entries = host.starts();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].error_code.as_deref(), Some("internal"));
}
