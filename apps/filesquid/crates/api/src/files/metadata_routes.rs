//! The metadata property routes (Task API-F1.4): statements, term placement, the namespace
//! registry and the import-record projection of one file. Reads take `files.read` and are not
//! journaled when they succeed; edits take `files.write` and are journaled with opaque IDs
//! only, through the same answers as the other file writes.

use super::command::{self, FileCommand};
use super::error::FileError;
use super::provenance::{Caller, Committed};
use super::routes::{body, files, query, read_answer, recorded, refuse};
use super::statements::{
    FileRecordQuery, MetadataCommitView, RegisterNamespaceRequest, RemoveStatementRequest,
    RenameRequest, SetStatementRequest, TermQuery,
};
use super::view::LibraryQuery;
use super::{
    FACTS_OPERATION, METADATA_READ_OPERATION, METADATA_REMOVE_OPERATION, METADATA_SET_OPERATION,
    NAMESPACES_OPERATION, NAMESPACE_OPERATION, RENAME_OPERATION, TERMS_OPERATION,
};
use crate::auth::{journal_entry, Action, Principal};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json};

/// The metadata property routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/files/metadata", get(statements), Action::ReadFiles),
        (
            "/files/metadata/statements",
            put(set_statement),
            Action::WriteFiles,
        ),
        (
            "/files/metadata/removals",
            post(remove_statement),
            Action::WriteFiles,
        ),
        ("/files/metadata/terms", get(terms), Action::ReadFiles),
        (
            "/files/metadata/namespaces",
            get(namespaces),
            Action::ReadFiles,
        ),
        (
            "/files/metadata/registrations",
            post(register_namespace),
            Action::WriteFiles,
        ),
        ("/files/facts", get(facts), Action::ReadFiles),
        ("/files/renames", post(rename), Action::WriteFiles),
    ]
}

/// `GET /api/v1/files/metadata`: a file's statements.
async fn statements(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<FileRecordQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.statements(query(request)?).await }.await;
    read_answer(&state, METADATA_READ_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/facts`: the read-only projection of a file's import record.
async fn facts(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<FileRecordQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.facts(query(request)?).await }.await;
    read_answer(&state, FACTS_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/metadata/terms`: where a bare term name can go, in the add order.
async fn terms(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<TermQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.terms(query(request)?).await }.await;
    read_answer(&state, TERMS_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/metadata/namespaces`: the library's namespace registry.
async fn namespaces(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<LibraryQuery>, QueryRejection>,
) -> Response {
    let outcome = async {
        let library = query(request)?;
        files(&state)?.namespaces(library.library_id).await
    }
    .await;
    read_answer(&state, NAMESPACES_OPERATION, &principal, outcome).await
}

/// `PUT /api/v1/files/metadata/statements`: set one statement.
async fn set_statement(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<SetStatementRequest>, JsonRejection>,
) -> Response {
    let caller = Caller::new(principal.actor());
    let outcome = async { files(&state)?.set_statement(body(request)?, caller).await }.await;
    edit_answer(&state, METADATA_SET_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/metadata/removals`: remove one statement.
async fn remove_statement(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<RemoveStatementRequest>, JsonRejection>,
) -> Response {
    let caller = Caller::new(principal.actor());
    let outcome = async {
        files(&state)?
            .remove_statement(body(request)?, caller)
            .await
    }
    .await;
    edit_answer(&state, METADATA_REMOVE_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/metadata/registrations`: register a namespace as the owner.
async fn register_namespace(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<RegisterNamespaceRequest>, JsonRejection>,
) -> Response {
    let caller = Caller::new(principal.actor());
    let outcome = async {
        files(&state)?
            .register_namespace(body(request)?, caller)
            .await
    }
    .await;
    edit_answer(&state, NAMESPACE_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/renames`: change a file's display name only, as a command
/// ([`super::command`]) whose expected revision is the file object's head.
async fn rename(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<RenameRequest>>, JsonRejection>,
) -> Response {
    let command = body(request).and_then(|c| command::check(c.envelope, c.input));
    let principal = &principal;
    command::run(
        &state,
        principal,
        RENAME_OPERATION,
        command,
        |files, c, actor| async move {
            let caller = Caller::new(actor).with_operation(c.operation_id);
            Ok(recorded(files.rename(c.input, caller, c.expected).await?))
        },
    )
    .await
}

/// Answer a metadata edit: journal its version with opaque IDs only, or the refusal.
async fn edit_answer(
    state: &ApiState,
    operation: &str,
    principal: &Principal,
    outcome: Result<Committed<MetadataCommitView>, FileError>,
) -> Response {
    let committed = match outcome {
        Ok(committed) => committed,
        Err(err) => return refuse(state, operation, principal.actor(), err).await,
    };
    for entry in committed.entries {
        journal_entry(state, entry).await;
    }
    Json(committed.view).into_response()
}
