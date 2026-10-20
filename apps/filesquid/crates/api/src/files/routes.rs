//! The file operation routes: one handler per route, each parsing its request, calling the
//! installed [`FileOperations`](super::FileOperations), journaling the outcome and answering
//! JSON or bytes. Every handler runs only after the router's guard has authorised its action
//! and put the [`Principal`] in the request extensions (API-S-004).

use super::error::FileError;
use super::names::{content_disposition, Disposition};
use super::nodes::listing_query;
use super::range::{RangePlan, RangeRequest};
use super::service::{ContentAnswer, ContentRequest};
use super::versions::Committed;
use super::view::{
    ContentQuery, FileQuery, FolderRequest, HistoryQuery, LabelRequest, LibraryQuery, ListingQuery,
    ListingRequest, MoveRequest, ObjectQuery, RemoveRequest, RestoreRequest, UploadQuery,
};
use super::{
    DETAILS_OPERATION, DOWNLOAD_OPERATION, FOLDER_OPERATION, HISTORY_OPERATION,
    HISTORY_SIZE_OPERATION, LABEL_OPERATION, LIBRARY_HISTORY_SIZE_OPERATION, LIST_OPERATION,
    MAX_UPLOAD_BYTES, MOVE_OPERATION, READ_OPERATION, REMOVE_OPERATION, RESTORE_OPERATION,
    UPLOAD_OPERATION, VERSIONS_OPERATION,
};
use crate::auth::{journal_entry, Action, Principal};
use crate::error::ErrorCode;
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::ApiState;
use axum::body::{Body, Bytes, HttpBody};
use axum::extract::rejection::{BytesRejection, JsonRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json};
use futures_util::StreamExt;
use serde::Serialize;

/// The file operation routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/files/listing", get(listing), Action::ReadFiles),
        ("/files/download", get(download), Action::ReadFiles),
        ("/files/details", get(details), Action::ReadFiles),
        ("/files/history", get(history), Action::ReadFiles),
        ("/files/versions", get(versions), Action::ReadFiles),
        (
            "/files/content",
            get(read_content).head(read_content),
            Action::ReadFiles,
        ),
        (
            "/files/upload",
            put(upload).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
            Action::WriteFiles,
        ),
        ("/files/folders", post(create_folder), Action::WriteFiles),
        ("/files/moves", post(move_path), Action::WriteFiles),
        ("/files/removals", post(remove), Action::WriteFiles),
        ("/files/restorations", post(restore), Action::WriteFiles),
        ("/files/labels", put(set_label), Action::WriteFiles),
        ("/files/history-size", get(history_size), Action::ReadFiles),
        (
            "/files/library-history-size",
            get(library_history_size),
            Action::ReadFiles,
        ),
    ]
    .into_iter()
    .chain(super::metadata_routes::routes())
    .collect()
}

/// The installed file service, or `unavailable` until the host installs one.
pub(super) fn files(
    state: &ApiState,
) -> Result<std::sync::Arc<dyn super::FileOperations>, FileError> {
    state
        .files()
        .ok_or_else(|| FileError::unavailable("no file service is installed"))
}

/// The query, or `invalid_request` for a missing, extra or malformed parameter.
pub(super) fn query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, FileError> {
    query.map(|Query(q)| q).map_err(|_| {
        FileError::invalid("The query names unknown, missing or malformed parameters.")
    })
}

/// The JSON body, or `invalid_request` for a missing, extra or malformed field.
pub(super) fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, FileError> {
    body.map(|Json(b)| b)
        .map_err(|_| FileError::invalid("The JSON body has unknown, missing or malformed fields."))
}

/// `GET /api/v1/files/listing`: by library, object and prefix, or by navigation node.
async fn listing(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<ListingRequest>, QueryRejection>,
) -> Response {
    let outcome = async {
        let files = files(&state)?;
        let listing = resolve_listing(&state, &principal, files.clone(), query(request)?).await?;
        files.listing(listing).await
    }
    .await;
    read_answer(&state, LIST_OPERATION, &principal, outcome).await
}

/// The listing a request names. A node is resolved on a blocking thread, since the
/// navigation service and the host's scope catalog may block on storage; the path form
/// does no blocking work and is resolved inline.
async fn resolve_listing(
    state: &ApiState,
    principal: &Principal,
    files: std::sync::Arc<dyn super::FileOperations>,
    request: ListingRequest,
) -> Result<ListingQuery, FileError> {
    if request.node.is_none() {
        return listing_query(request, principal, None, files.as_ref());
    }
    let navigation = state.navigation();
    let principal = principal.clone();
    let resolve = move || listing_query(request, &principal, navigation, files.as_ref());
    tokio::task::spawn_blocking(resolve)
        .await
        .map_err(|_| FileError::internal("the listing task failed"))?
}

/// `GET /api/v1/files/download`: one whole file version, streamed as an attachment named by
/// its display name.
async fn download(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<FileQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.download(query(request)?).await }.await;
    let download = match outcome {
        Ok(download) => download,
        Err(err) => return refuse(&state, DOWNLOAD_OPERATION, principal.actor(), err).await,
    };
    let disposition = content_disposition(
        Disposition::Attachment,
        &download.file_name,
        &download.id_name,
    );
    let body = Body::from_stream(journaled_stream(state, principal, download.body));
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    if let Ok(value) = HeaderValue::from_str(&download.media_type) {
        headers.insert(CONTENT_TYPE, value);
    }
    headers.insert(CONTENT_LENGTH, HeaderValue::from(download.total));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    response
}

/// `body`, journaling the error that ends it early once as [`DOWNLOAD_OPERATION`] before
/// the transport aborts the answer.
fn journaled_stream(
    state: ApiState,
    principal: Principal,
    body: super::DownloadStream,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send {
    body.then(move |piece| {
        let state = state.clone();
        let actor = principal.actor();
        async move {
            match piece {
                Ok(bytes) => Ok(bytes),
                Err(err) => {
                    let code = err.error_code();
                    refuse(&state, DOWNLOAD_OPERATION, actor, err).await;
                    Err(std::io::Error::other(format!(
                        "download ended early: {code}"
                    )))
                }
            }
        }
    })
}

/// `GET /api/v1/files/details`.
async fn details(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<FileQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.details(query(request)?).await }.await;
    read_answer(&state, DETAILS_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/history`.
async fn history(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<HistoryQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.history(query(request)?).await }.await;
    read_answer(&state, HISTORY_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/versions`.
async fn versions(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<ObjectQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.versions(query(request)?).await }.await;
    read_answer(&state, VERSIONS_OPERATION, &principal, outcome).await
}

/// `GET` and `HEAD /api/v1/files/content`: one ranged read of one file version.
async fn read_content(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    method: Method,
    headers: HeaderMap,
    request: Result<Query<ContentQuery>, QueryRejection>,
) -> Response {
    let head_only = method == Method::HEAD;
    let mut disposition = Disposition::Inline;
    let outcome = async {
        let file = query(request)?;
        if file.download == Some(true) {
            disposition = Disposition::Attachment;
        }
        let range = RangeRequest::from_headers(&headers);
        let content = ContentRequest {
            library_id: file.library_id,
            object_id: file.object_id,
            version: file.version,
            path: file.path,
            range,
            head_only,
        };
        files(&state)?.content(content).await
    }
    .await;
    let answer = ContentAnswerKind {
        head_only,
        disposition,
    };
    answer_content(&state, principal.actor(), outcome, answer).await
}

/// `PUT /api/v1/files/upload`: upload one file as its own object, or new content for an
/// existing file; the body is its bytes.
async fn upload(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<UploadQuery>, QueryRejection>,
    bytes: Result<Bytes, BytesRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let outcome = async {
        let upload = query(request)?;
        let bytes = bytes.map_err(|_| {
            FileError::invalid(format!(
                "An upload carries at most {MAX_UPLOAD_BYTES} bytes."
            ))
        })?;
        files(&state)?.upload(upload, bytes, actor).await
    }
    .await;
    upload_answer(&state, &principal, outcome).await
}

/// Answer an upload: journal its version as [`write_answer`] does and answer the upload view,
/// which also names the file.
async fn upload_answer(
    state: &ApiState,
    principal: &Principal,
    outcome: Result<super::UploadView, FileError>,
) -> Response {
    let upload = match outcome {
        Ok(upload) => upload,
        Err(err) => return refuse(state, UPLOAD_OPERATION, principal.actor(), err).await,
    };
    let commit = super::CommitView {
        library_id: upload.library_id.clone(),
        object_id: upload.object_id.clone(),
        version: upload.version.clone(),
        created: upload.created.clone(),
    };
    let journaled = write_answer(state, UPLOAD_OPERATION, principal, Ok(commit)).await;
    match journaled.status().is_success() {
        true => Json(upload).into_response(),
        false => journaled,
    }
}

/// `POST /api/v1/files/folders`: answers whether the folder exists or stays pending; writes
/// nothing.
async fn create_folder(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FolderRequest>, JsonRejection>,
) -> Response {
    let outcome = async { files(&state)?.create_folder(body(request)?).await }.await;
    read_answer(&state, FOLDER_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/moves`.
async fn move_path(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<MoveRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let outcome = async { files(&state)?.move_path(body(request)?, actor).await }.await;
    write_answer(&state, MOVE_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/removals`.
async fn remove(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<RemoveRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let outcome = async { files(&state)?.remove(body(request)?, actor).await }.await;
    write_answer(&state, REMOVE_OPERATION, &principal, outcome).await
}

/// `POST /api/v1/files/restorations`.
async fn restore(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<RestoreRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let outcome = async { files(&state)?.restore(body(request)?, actor).await }.await;
    committed_answer(&state, RESTORE_OPERATION, &principal, outcome).await
}

/// `PUT /api/v1/files/labels`: set or clear the label of one version after commit.
async fn set_label(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<LabelRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let outcome = async { files(&state)?.set_label(body(request)?, actor).await }.await;
    committed_answer(&state, LABEL_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/history-size`.
async fn history_size(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<HistoryQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.history_size(query(request)?).await }.await;
    read_answer(&state, HISTORY_SIZE_OPERATION, &principal, outcome).await
}

/// `GET /api/v1/files/library-history-size`.
async fn library_history_size(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<LibraryQuery>, QueryRejection>,
) -> Response {
    let outcome = async { files(&state)?.library_history_size(query(request)?).await }.await;
    read_answer(&state, LIBRARY_HISTORY_SIZE_OPERATION, &principal, outcome).await
}

/// Answer a write that may carry its commit's journal entries: with provenance installed
/// they are journaled (each once, under its pre-allocated ID) instead of the route's own
/// entry; without, the write is journaled as [`write_answer`] does.
async fn committed_answer(
    state: &ApiState,
    operation: &str,
    principal: &Principal,
    outcome: Result<Committed, FileError>,
) -> Response {
    let committed = match outcome {
        Ok(committed) => committed,
        Err(err) => return refuse(state, operation, principal.actor(), err).await,
    };
    if committed.entries.is_empty() {
        return write_answer(state, operation, principal, Ok(committed.view)).await;
    }
    for entry in committed.entries {
        journal_entry(state, entry).await;
    }
    Json(committed.view).into_response()
}

/// Answer a read: the JSON view, or the typed error after journaling the refusal.
pub(super) async fn read_answer<T: Serialize>(
    state: &ApiState,
    operation: &str,
    principal: &Principal,
    outcome: Result<T, FileError>,
) -> Response {
    match outcome {
        Ok(view) => Json(view).into_response(),
        Err(err) => refuse(state, operation, principal.actor(), err).await,
    }
}

/// Answer a write: journal the published version with opaque IDs only and its
/// `<object-id>@vN` object reference (ADR 033 Decision 7), or the refusal.
async fn write_answer(
    state: &ApiState,
    operation: &str,
    principal: &Principal,
    outcome: Result<super::CommitView, FileError>,
) -> Response {
    let commit = match outcome {
        Ok(commit) => commit,
        Err(err) => return refuse(state, operation, principal.actor(), err).await,
    };
    let entry = JournalEntry::new(operation, Outcome::Succeeded)
        .actor(principal.actor())
        .target(commit.library_id.as_str())
        .target(commit.object_id.as_str())
        .target(commit.version.as_str());
    let entry = match object_ref(&commit) {
        Some(object_ref) => entry.object_ref(object_ref),
        None => entry,
    };
    journal_entry(state, entry).await;
    Json(commit).into_response()
}

/// The `<object-id>@vN` reference of the version a write published, as the journal keeps it.
/// An object ID that no version reference can name (one containing `@` or whitespace, or one
/// the journal would not keep opaque) cannot be linked by PROV-O either; the write still
/// succeeds and is journaled with its opaque targets, and the missing link is logged at WARN
/// so it is never silent.
fn object_ref(commit: &super::CommitView) -> Option<String> {
    let reference =
        filesquid_core::VersionRef::new(commit.object_id.as_str(), commit.version.as_str())
            .ok()
            .map(|reference| reference.object_ref())
            .filter(|object_ref| crate::journal::is_object_ref(object_ref));
    if reference.is_none() {
        tracing::warn!(
            target: "filesquid::files",
            "a published version was journaled without an object reference"
        );
    }
    reference
}

/// Journal a refused operation once, with its code, and answer the typed error.
pub(crate) async fn refuse(
    state: &ApiState,
    operation: &str,
    actor: &str,
    err: FileError,
) -> Response {
    let level = match err.code() {
        ErrorCode::Internal | ErrorCode::Unavailable => Level::Error,
        _ => Level::Warn,
    };
    let entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    err.into_api_error(operation).into_response()
}

/// How a content answer is given: with or without its body, and inline or as a download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentAnswerKind {
    /// True for `HEAD`: the headers with an empty body.
    pub head_only: bool,
    /// How `Content-Disposition` offers the file.
    pub disposition: Disposition,
}

/// Answer one content read: `200` with the whole file, `206` with `Content-Range`, `416`
/// with `Content-Range: bytes */<length>`, or the typed error; `HEAD` answers the headers
/// with an empty body. Every success carries `Accept-Ranges: bytes`, the media type and a
/// `Content-Disposition` that names the file by its display name. A refusal, a `416`
/// included, is journaled under [`READ_OPERATION`].
pub(crate) async fn answer_content(
    state: &ApiState,
    actor: &str,
    outcome: Result<ContentAnswer, FileError>,
    kind: ContentAnswerKind,
) -> Response {
    let answer = match outcome {
        Ok(answer) => answer,
        Err(err) => return without_body(refuse(state, READ_OPERATION, actor, err).await, kind),
    };
    let total = answer.total;
    let (status, range, len) = match answer.plan {
        RangePlan::Whole { len } => (StatusCode::OK, None, len),
        RangePlan::Part { start, len } => {
            let range = format!("bytes {start}-{}/{total}", start + len - 1);
            (StatusCode::PARTIAL_CONTENT, Some(range), len)
        }
        RangePlan::Unsatisfiable => {
            return without_body(unsatisfiable(state, actor, total).await, kind)
        }
    };
    let body = match kind.head_only {
        true => Body::empty(),
        false => Body::from(answer.bytes),
    };
    let disposition = content_disposition(kind.disposition, &answer.file_name, &answer.id_name);
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    if let Ok(value) = HeaderValue::from_str(&answer.media_type) {
        headers.insert(CONTENT_TYPE, value);
    }
    headers.insert(CONTENT_LENGTH, HeaderValue::from(len));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    if let Some(range) = range.and_then(|r| HeaderValue::from_str(&r).ok()) {
        headers.insert(CONTENT_RANGE, range);
    }
    response
}

/// `response` without its body for a `HEAD` request, keeping the length its body had, so a
/// `HEAD` refusal has the status and headers of the `GET` one and no body.
fn without_body(response: Response, kind: ContentAnswerKind) -> Response {
    if !kind.head_only {
        return response;
    }
    let length = response.body().size_hint().exact();
    let (mut parts, _) = response.into_parts();
    if let Some(length) = length {
        parts
            .headers
            .insert(CONTENT_LENGTH, HeaderValue::from(length));
    }
    Response::from_parts(parts, Body::empty())
}

/// The `416` answer for a file of `total` bytes, journaled as a refusal.
async fn unsatisfiable(state: &ApiState, actor: &str, total: u64) -> Response {
    let message = "The requested range starts past the end of the file.";
    let err = FileError::new(ErrorCode::RangeNotSatisfiable, message);
    let mut response = refuse(state, READ_OPERATION, actor, err).await;
    if let Ok(value) = HeaderValue::from_str(&format!("bytes */{total}")) {
        response.headers_mut().insert(CONTENT_RANGE, value);
    }
    response
}
