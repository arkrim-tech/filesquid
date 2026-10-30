//! Unit tests for the import route on a host without the review (API-S-036): `POST
//! /api/v1/imports` answers the host's job or `cancelled`; refuses a body whose envelope scope
//! is not the library's (`400`), a request without a destination collection (`400`,
//! `destination_required`) or with an unknown one (`404`, `collection_not_found`) before any
//! dialog (Task DE-1.7, ADR 041), an open dialog (`409`), a library that is not open (`404`), a
//! catalog that failed (`500`) and a host without an import dialog (`503`); answers a repeated
//! operation with its job before checking anything; journals each request once without a path
//! (`import.plan` for the request, `import.start` for a job started at once); and is
//! authorised before it runs. The review itself is tested in `import_review.rs`.

#[path = "../support/import_fixture.rs"]
mod import_fixture;

use axum::http::StatusCode;
use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportStartError, ImportStarter, COLLECTION_NOT_FOUND,
    DESTINATION_REQUIRED, IMPORT_PLAN_OPERATION, IMPORT_START_OPERATION,
};
use filesquid_api::journal::Outcome;
use filesquid_api::library::{
    CatalogCommit, CatalogError, CommitReceipt, FileEntry, FileListing, Keyed, LibraryCatalog,
    LibraryRecord, LibraryState, PageKey,
};
use filesquid_api::navigation::{HostAccess, MemoryFolderIndex, NavigationService, RepositoryInfo};
use filesquid_api::operation::OperationId;
use filesquid_core::{
    Collection, CollectionId, FileId, Folder, FolderId, LibraryId, RepositoryId, VirtualCollection,
    VirtualCollectionId,
};
use import_fixture::{body, scope, Host, ScriptedStarter, COLLECTION, FOREIGN_COLLECTION, LIBRARY};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn a_started_import_answers_its_job_and_journals_it_once() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    assert!(host.state.install_import_starter(starter.clone()));
    let (status, answer) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer,
        json!({
            "outcome": { "operation_id": "op-1", "kind": "pending", "error": null },
            "import": "started",
            "job_id": "job-1"
        })
    );
    assert_eq!(starter.asked()[0].library_id, LIBRARY);
    let entries = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].target_ids, ["job-1"]);
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
    assert!(host.journaled(IMPORT_PLAN_OPERATION).is_empty());
}

#[tokio::test]
async fn a_cancelled_dialog_answers_cancelled_without_a_job() {
    let host = Host::start();
    host.state
        .install_import_starter(ScriptedStarter::new(Ok(None)));
    let (status, answer) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer,
        json!({
            "outcome": { "operation_id": "op-1", "kind": "committed", "error": null },
            "import": "cancelled"
        })
    );
    let entries = host.journaled(IMPORT_PLAN_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Cancelled);
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
        let (got, answer) = host.post(body(&scope()), true).await;
        assert_eq!(got, status, "{code}: {answer}");
        let entries = host.journaled(IMPORT_PLAN_OPERATION);
        assert_eq!(entries.len(), 1, "{code}");
        assert_eq!(entries[0].error_code.as_deref(), Some(code));
    }
}

#[tokio::test]
async fn a_wrong_scope_no_starter_or_no_token_asks_nothing() {
    let host = Host::start();
    let (status, _) = host.post(body(&scope()), true).await;
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
    let mut newer = body(&scope());
    newer["envelope"]["schema_version"] = json!(2);
    let (status, _) = host.post(newer, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "another schema version");
    let mut unnamed = body("library:");
    unnamed["library_id"] = json!("");
    let (status, _) = host.post(unnamed, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no library");
    let (status, _) = host.post(body(&scope()), false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(starter.asked().is_empty());
    let codes: Vec<_> = host
        .journaled(IMPORT_PLAN_OPERATION)
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
    assert!(!host.journal_text().contains("someone"));
}

#[tokio::test]
async fn the_destination_reaches_the_host() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let mut request = body(&scope());
    request["folder"] = json!("trips/2024");
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let asked = starter.asked();
    assert_eq!(asked[0].collection_id.as_deref(), Some(COLLECTION));
    assert_eq!(asked[0].folder.as_deref(), Some("trips/2024"));
}

#[tokio::test]
async fn an_import_without_a_destination_is_refused_before_any_dialog() {
    let host = Host::start();
    let starter = ScriptedStarter::new(Ok(Some("job-1")));
    host.state.install_import_starter(starter.clone());
    let mut request = body(&scope());
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
    assert!(starter.asked().is_empty(), "no dialog");
    let entries = host.journaled(IMPORT_PLAN_OPERATION);
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
        let mut request = body(&scope());
        request["collection_id"] = json!(collection);
        let (status, answer) = host.post(request, true).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{collection}: {answer}");
        assert!(
            answer["message"]
                .as_str()
                .is_some_and(|m| m.contains(COLLECTION_NOT_FOUND)),
            "{answer}"
        );
        assert!(starter.asked().is_empty(), "no dialog");
        let entries = host.journaled(IMPORT_PLAN_OPERATION);
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
        let mut request = body(&scope());
        request["folder"] = json!(folder);
        let (status, answer) = host.post(request, true).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{folder:?}: {answer}");
        assert!(starter.asked().is_empty(), "no dialog");
        let entries = host.journaled(IMPORT_PLAN_OPERATION);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].error_code.as_deref(), Some("invalid_path"));
        if !folder.is_empty() {
            let journaled = host.journal_text();
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
    let mut request = body(&scope());
    request["collection_id"] = json!(FOREIGN_COLLECTION);
    let (status, answer) = host.post(request, true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["job_id"], "job-1");
    assert_eq!(answer["import"], "started");
    let entries = host.journaled(IMPORT_START_OPERATION);
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
    let (status, answer) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{answer}");
    assert!(starter.asked().is_empty(), "no dialog");
    let entries = host.journaled(IMPORT_PLAN_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].error_code.as_deref(), Some("internal"));
}
