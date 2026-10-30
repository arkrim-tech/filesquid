//! The file operation routes: one handler per route, each parsing its request, calling the
//! installed [`FileOperations`](super::FileOperations), journaling the outcome and answering
//! JSON or bytes. Every handler runs only after the router's guard has authorised its action
//! and put the [`Principal`] in the request extensions (API-S-004).

use super::command::{self, Checked, CommandInput, Done, FileCommand};
use super::error::FileError;
use super::homing;
use super::names::{content_disposition, Disposition};
use super::nodes::{listing_query, Listing};
use super::provenance::{Caller, Committed};
use super::range::{RangePlan, RangeRequest};
use super::service::{ContentAnswer, ContentRequest};
use super::view::{
    ContentQuery, FileQuery, FolderRequest, HistoryQuery, LabelRequest, LibraryQuery,
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
use crate::home::Destination;
use crate::journal::{JournalEntry, Level, Outcome};
use crate::operation::CommandEnvelope;
use crate::routes::Route;
use crate::ApiState;
use axum::body::{Body, Bytes, HttpBody};
use axum::extract::multipart::{Field, MultipartRejection};
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Multipart, Query, State};
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
            put(upload).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES + UPLOAD_OVERHEAD)),
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
    .chain(super::drive_routes::routes())
    .collect()
}

/// Most parts an upload body has: `envelope`, `input` and `file`.
const UPLOAD_PARTS: usize = 3;

/// Bytes an upload body may carry beside its file: the multipart framing and the
/// `envelope` and `input` parts (64 KiB).
const UPLOAD_OVERHEAD: usize = 64 * 1024;

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
        match listing {
            Listing::Object(listing) => files.listing(listing).await,
            Listing::Scope(listing) => files.scope_listing(listing).await,
        }
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
) -> Result<Listing, FileError> {
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
    let request = match query(request) {
        Ok(request) => request,
        Err(err) => return refuse(&state, DOWNLOAD_OPERATION, principal.actor(), err).await,
    };
    let request_ids = (request.library_id.clone(), request.object_id.clone());
    let outcome = async { files(&state)?.download(request).await }.await;
    let download = match outcome {
        Ok(download) => download,
        Err(err) => return refuse(&state, DOWNLOAD_OPERATION, principal.actor(), err).await,
    };
    let disposition = content_disposition(
        Disposition::Attachment,
        &download.file_name,
        &download.id_name,
    );
    journal_download(&state, &principal, &request_ids, &download.version).await;
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

/// Journal one started single-file download as `file.download` (API-S-028): the caller, the
/// library, the object and the version, never the name. Journal-only: it stages no record.
async fn journal_download(
    state: &ApiState,
    principal: &Principal,
    (library_id, object_id): &(String, String),
    version: &str,
) {
    let entry = JournalEntry::new(
        super::drive_scope::FILE_DOWNLOAD_OPERATION,
        Outcome::Succeeded,
    )
    .actor(principal.actor())
    .target(library_id.as_str())
    .target(object_id.as_str())
    .target(version);
    journal_entry(state, entry).await;
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
/// existing file; the body is `multipart/form-data` with the `envelope`, `input` and `file`
/// parts ([`super::command`]).
async fn upload(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    form: Result<Multipart, MultipartRejection>,
) -> Response {
    let (command, bytes) = match upload_command(form).await {
        Ok((command, bytes)) => (Ok(command), bytes),
        Err(err) => (Err(err), Bytes::new()),
    };
    let (state, principal) = (&state, &principal);
    command::run(
        state,
        principal,
        UPLOAD_OPERATION,
        command,
        |files, c, actor| async move {
            let operation_id = c.operation_id.as_str().to_string();
            let fingerprint = match c.input.facts.file_id {
                None => Some(homing::fingerprint_of(&c.input)?),
                Some(_) => None,
            };
            let mut facts = c.input.facts;
            let destination = Destination {
                collection_id: facts.collection_id.clone(),
                folder: facts.folder.clone(),
            };
            let place = (facts.library_id.as_str(), &destination);
            let ids = (operation_id.as_str(), fingerprint.as_deref());
            let prepared = homing::prepare_home(state, principal, place, &facts.name, ids).await?;
            facts.reserved_file_id = prepared.reserved.as_ref().map(|id| id.as_str().to_string());
            let caller = Caller::new(actor).with_operation(c.operation_id);
            let uploaded = files.upload(facts, bytes, caller, c.expected).await;
            if uploaded.is_err() {
                homing::release_reservation(&prepared, &operation_id).await;
            }
            let mut upload = uploaded?;
            let (collection, path) =
                homing::settle_home(state, prepared, &operation_id, &upload.view.file_id).await?;
            upload.view.collection_id = Some(collection);
            upload.view.path = Some(path);
            Ok(recorded(resumed_entry(UPLOAD_OPERATION, principal, upload)))
        },
    )
    .await
}

/// The checked command and the bytes of an upload body: exactly the parts `envelope` and
/// `input` (JSON) and `file` (at most [`MAX_UPLOAD_BYTES`] bytes), each once.
async fn upload_command(
    form: Result<Multipart, MultipartRejection>,
) -> Result<(Checked<UploadCommand>, Bytes), FileError> {
    let mut form = form.map_err(|_| {
        FileError::invalid("An upload is multipart/form-data with envelope, input and file parts.")
    })?;
    let mut parts = UploadParts::default();
    for _ in 0..UPLOAD_PARTS {
        match form.next_field().await.map_err(|_| upload_too_large())? {
            Some(field) => parts.take(field).await?,
            None => break,
        }
    }
    if form
        .next_field()
        .await
        .map_err(|_| upload_too_large())?
        .is_some()
    {
        return Err(FileError::invalid(
            "An upload has only envelope, input and file parts.",
        ));
    }
    let (checked, file) = parts.finish()?;
    let content_sha256 = hash_upload(file.clone()).await?;
    let command = Checked {
        operation_id: checked.operation_id,
        expected: checked.expected,
        input: UploadCommand {
            facts: checked.input,
            content_sha256,
        },
    };
    Ok((command, file))
}

/// The SHA-256 of an upload's bytes as lowercase hex, computed on a blocking thread since it
/// reads up to [`MAX_UPLOAD_BYTES`].
async fn hash_upload(file: Bytes) -> Result<String, FileError> {
    let digest = move || filesquid_ocfl::DigestAlgorithm::Sha256.digest_hex(&file);
    tokio::task::spawn_blocking(digest)
        .await
        .map_err(|_| FileError::internal("the upload digest task failed"))
}

/// The input an upload's receipt is matched on: its facts and the SHA-256 of its bytes, so
/// the same operation ID with other bytes is another command, never a replay.
#[derive(Clone, Debug, Serialize)]
struct UploadCommand {
    /// The upload's facts, the `input` part.
    #[serde(flatten)]
    facts: UploadQuery,
    /// The SHA-256 of the `file` part, lowercase hex.
    content_sha256: String,
}

impl CommandInput for UploadCommand {
    fn library_id(&self) -> &str {
        &self.facts.library_id
    }
}

/// The parts of an upload body read so far.
#[derive(Default)]
struct UploadParts {
    envelope: Option<CommandEnvelope>,
    input: Option<UploadQuery>,
    file: Option<Bytes>,
}

impl UploadParts {
    /// Read one part; a part named twice, or by another name, is `invalid_request`.
    async fn take(&mut self, field: Field<'_>) -> Result<(), FileError> {
        let name = field.name().unwrap_or_default().to_string();
        let bytes = field.bytes().await.map_err(|_| upload_too_large())?;
        let fresh = match name.as_str() {
            "envelope" => self.envelope.replace(json_part(&bytes)?).is_none(),
            "input" => self.input.replace(json_part(&bytes)?).is_none(),
            "file" => self.file.replace(bytes).is_none(),
            _ => false,
        };
        match fresh {
            true => Ok(()),
            false => Err(FileError::invalid(
                "An upload has one envelope, input and file part each.",
            )),
        }
    }

    /// The checked envelope and facts, and the bytes, once every part arrived within the cap;
    /// the bytes are hashed only after the envelope is checked.
    fn finish(self) -> Result<(Checked<UploadQuery>, Bytes), FileError> {
        let (Some(envelope), Some(input), Some(file)) = (self.envelope, self.input, self.file)
        else {
            return Err(FileError::invalid(
                "An upload needs its envelope, input and file parts.",
            ));
        };
        if file.len() > MAX_UPLOAD_BYTES {
            return Err(upload_too_large());
        }
        Ok((command::check(envelope, input)?, file))
    }
}

/// A JSON part, or `invalid_request` for unknown, missing or malformed fields.
fn json_part<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, FileError> {
    serde_json::from_slice(bytes)
        .map_err(|_| FileError::invalid("An upload part has unknown, missing or malformed fields."))
}

/// The refusal of an upload body over the cap, or one that cannot be read as parts.
fn upload_too_large() -> FileError {
    FileError::invalid(format!(
        "An upload carries at most {MAX_UPLOAD_BYTES} bytes in well-formed multipart parts."
    ))
}

/// The checked command of a JSON write route.
fn json_command<T: CommandInput>(
    request: Result<Json<FileCommand<T>>, JsonRejection>,
) -> Result<Checked<T>, FileError> {
    let command = body(request)?;
    command::check(command.envelope, command.input)
}

/// `POST /api/v1/files/folders`: answers whether the folder exists or stays pending; writes
/// nothing, so it stores no receipt and is journaled only when refused.
async fn create_folder(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<FolderRequest>>, JsonRejection>,
) -> Response {
    let command = json_command(request);
    command::run(
        &state,
        &principal,
        FOLDER_OPERATION,
        command,
        |files, c, _| async move {
            let folder = files.create_folder(c.input, c.expected).await?;
            Ok(Done {
                view: folder,
                entries: Vec::new(),
                changed: false,
            })
        },
    )
    .await
}

/// `POST /api/v1/files/moves`.
async fn move_path(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<MoveRequest>>, JsonRejection>,
) -> Response {
    let command = json_command(request);
    let principal = &principal;
    command::run(
        &state,
        principal,
        MOVE_OPERATION,
        command,
        |files, c, actor| async move {
            let caller = Caller::new(actor).with_operation(c.operation_id);
            Ok(recorded(
                files.move_path(c.input, caller, c.expected).await?,
            ))
        },
    )
    .await
}

/// `POST /api/v1/files/removals`.
async fn remove(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<RemoveRequest>>, JsonRejection>,
) -> Response {
    let command = json_command(request);
    let principal = &principal;
    command::run(
        &state,
        principal,
        REMOVE_OPERATION,
        command,
        |files, c, actor| async move {
            let caller = Caller::new(actor).with_operation(c.operation_id);
            Ok(recorded(files.remove(c.input, caller, c.expected).await?))
        },
    )
    .await
}

/// `POST /api/v1/files/restorations`.
async fn restore(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<RestoreRequest>>, JsonRejection>,
) -> Response {
    let command = json_command(request);
    let principal = &principal;
    command::run(
        &state,
        principal,
        RESTORE_OPERATION,
        command,
        |files, c, actor| async move {
            let caller = Caller::new(actor).with_operation(c.operation_id);
            Ok(recorded(files.restore(c.input, caller, c.expected).await?))
        },
    )
    .await
}

/// `PUT /api/v1/files/labels`: set or clear the label of one version after commit.
async fn set_label(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<LabelRequest>>, JsonRejection>,
) -> Response {
    let command = json_command(request);
    let principal = &principal;
    command::run(
        &state,
        principal,
        LABEL_OPERATION,
        command,
        |files, c, actor| async move {
            let caller = Caller::new(actor).with_operation(c.operation_id);
            Ok(recorded(
                files.set_label(c.input, caller, c.expected).await?,
            ))
        },
    )
    .await
}

/// A published version with its commit's journal entries ([`super::Committed`]): the
/// `ocfl.commit` entry and the operation's own, each journaled once.
pub(super) fn recorded<V>(committed: Committed<V>) -> Done<V> {
    Done {
        view: committed.view,
        entries: committed.entries,
        changed: true,
    }
}

/// `committed` with the journal entry of its success when it carries none: a new file an
/// earlier, unfinished run of the same operation committed, which this run completed by
/// registering it (Task API-F2.14), is journaled once as `operation` with the library, its
/// object and its version. Pure.
pub(super) fn resumed_entry<V: FileIds>(
    operation: &'static str,
    principal: &Principal,
    mut committed: Committed<V>,
) -> Committed<V> {
    if committed.entries.is_empty() {
        let view = &committed.view;
        let entry = JournalEntry::new(operation, Outcome::Succeeded)
            .actor(principal.actor())
            .target(view.library_id())
            .target(format!("{}@{}", view.object_id(), view.version()));
        committed.entries.push(entry);
    }
    committed
}

/// The opaque IDs of a new file's committed version, for its journal entry.
pub(super) trait FileIds {
    /// The library.
    fn library_id(&self) -> &str;
    /// The file's object.
    fn object_id(&self) -> &str;
    /// The version.
    fn version(&self) -> &str;
}

impl FileIds for super::UploadView {
    fn library_id(&self) -> &str {
        &self.library_id
    }
    fn object_id(&self) -> &str {
        &self.object_id
    }
    fn version(&self) -> &str {
        &self.version
    }
}

impl FileIds for super::copy::CopyView {
    fn library_id(&self) -> &str {
        &self.library_id
    }
    fn object_id(&self) -> &str {
        &self.object_id
    }
    fn version(&self) -> &str {
        &self.version
    }
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
