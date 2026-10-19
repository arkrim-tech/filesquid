//! History routes of the FileSquid API (API-S-029; PRD F-067, F-046). Every route performs
//! [`Action::ReadHistory`], which the [`crate::auth`] layer authorises before the handler
//! runs, and reads the host's [`HistorySource`] off the async executor; before the host
//! installs one every route answers `unavailable`.
//!
//! - `GET /history`: activities of the whole history, or of one object, operation or agent,
//!   or of a time range, in pages of 1 to 1000 with an opaque cursor.
//! - `GET /files/{id}/history`: the activities of the object `urn:uuid:<id>`.
//! - `GET /files/{id}/history/export`: every record of that object as one JSON-LD array,
//!   optionally redacted for sharing (`redact=true`).
//! - `GET /files/{id}/versions/{version}/prov`: the JSON-LD record of one version.
//!
//! Every failure answers with a typed [`ApiError`] (API-S-009).

use std::sync::Arc;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use filesquid_core::{HistoryEntry, VersionRef};
use filesquid_prov::{
    to_json_ld, Cursor, HistoryIndex, HistoryQuery, PageRequest, Timestamp, MAX_PAGE_SIZE,
};
use serde::{Deserialize, Serialize};

use super::redact::Redactor;
use super::source::{entry_of, HistoryError, HistorySource, MAX_HISTORY_RECORDS};
use crate::admin::blocking;
use crate::auth::Action;
use crate::error::{ApiError, ErrorCode};
use crate::journal::{is_opaque, DEFAULT_PAGE};
use crate::routes::Route;
use crate::ApiState;

/// The media type of PROV-O records and exports.
pub const JSON_LD: &str = "application/ld+json";

/// Earliest end time of an open-ended time range.
const EARLIEST: &str = "1970-01-01T00:00:00Z";

/// End time after every record of an open-ended time range (exclusive).
const LATEST: &str = "9999-12-31T23:59:59Z";

/// One page of history entries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPage {
    /// The entries, ordered by commit time and then activity.
    pub items: Vec<HistoryEntry>,
    /// The cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// History routes, relative to the API prefix.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/history", get(history), Action::ReadHistory),
        (
            "/files/{id}/history",
            get(file_history),
            Action::ReadHistory,
        ),
        (
            "/files/{id}/history/export",
            get(export),
            Action::ReadHistory,
        ),
        (
            "/files/{id}/versions/{version}/prov",
            get(prov),
            Action::ReadHistory,
        ),
    ]
}

/// Query-string filters of `GET /history`.
#[derive(Debug, Default, Deserialize)]
struct HistoryParams {
    object: Option<String>,
    operation: Option<String>,
    agent: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}

impl HistoryParams {
    /// The one filter asked for (everything when none); `Err` names the invalid parameter.
    fn query(&self) -> Result<HistoryQuery, &'static str> {
        let by_time = self.from.is_some() || self.to.is_some();
        let named = [&self.object, &self.operation, &self.agent]
            .into_iter()
            .filter(|value| value.is_some())
            .count();
        if named + usize::from(by_time) > 1 {
            return Err("filter");
        }
        let query = match (&self.object, &self.operation, &self.agent) {
            (Some(object), _, _) => HistoryQuery::Object(object.clone()),
            (_, Some(operation), _) => HistoryQuery::Operation(operation.clone()),
            (_, _, Some(agent)) => HistoryQuery::Agent(agent.clone()),
            _ => time_range(self.from.as_deref(), self.to.as_deref())?,
        };
        Ok(query)
    }
}

/// The time range `from` (inclusive) to `to` (exclusive), open ends filled in.
fn time_range(from: Option<&str>, to: Option<&str>) -> Result<HistoryQuery, &'static str> {
    let from = Timestamp::parse(from.unwrap_or(EARLIEST)).map_err(|_| "from")?;
    let to = Timestamp::parse(to.unwrap_or(LATEST)).map_err(|_| "to")?;
    Ok(HistoryQuery::TimeRange { from, to })
}

/// Query-string paging of `GET /files/{id}/history`.
#[derive(Debug, Default, Deserialize)]
struct PageParams {
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Query string of the export.
#[derive(Debug, Default, Deserialize)]
struct ExportParams {
    redact: Option<bool>,
}

/// The page request of `limit` (default [`DEFAULT_PAGE`], 1 to 1000) after `cursor`.
fn page_request(limit: Option<usize>, cursor: Option<&str>) -> Result<PageRequest, &'static str> {
    let after = cursor
        .map(|token| Cursor::from_token(token).map_err(|_| "cursor"))
        .transpose()?;
    PageRequest::new(limit.unwrap_or(DEFAULT_PAGE), after).map_err(|_| "limit")
}

/// `invalid_request` naming the invalid parameter.
fn bad_request(param: &str) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        format!("Invalid parameter: {param}."),
    )
}

/// The installed history source (`unavailable` before the host installs one).
fn installed(state: &ApiState) -> Result<Arc<dyn HistorySource>, ApiError> {
    state.history().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "The history is starting. Try again in a moment.",
        )
    })
}

/// The answer for a [`HistoryError`].
fn history_failure(operation: &str, err: HistoryError) -> ApiError {
    match err {
        HistoryError::Unavailable => ApiError::new(
            ErrorCode::Unavailable,
            "The history cannot be read right now. Try again in a moment.",
        ),
        HistoryError::Corrupt => ApiError::internal(operation, &err.to_string()),
    }
}

/// The object ID of the file `id`: `urn:uuid:<id>`.
fn object_of(id: &str) -> Result<String, ApiError> {
    if !is_opaque(id) || id.contains(':') {
        return Err(bad_request("id"));
    }
    Ok(format!("urn:uuid:{id}"))
}

/// One page of `query` from `source` (storage I/O).
fn page_of(
    source: &dyn HistorySource,
    query: &HistoryQuery,
    request: &PageRequest,
) -> Result<HistoryPage, HistoryError> {
    let index = source.index()?;
    let page = index.query(query, request);
    let items = page
        .items()
        .iter()
        .map(entry_of)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HistoryPage {
        items,
        next_cursor: page.next().map(Cursor::to_token),
    })
}

/// Answer one page of `query` from the installed source.
async fn answer_page(state: &ApiState, query: HistoryQuery, request: PageRequest) -> Response {
    let source = match installed(state) {
        Ok(source) => source,
        Err(error) => return error.into_response(),
    };
    let read = move || page_of(source.as_ref(), &query, &request);
    match blocking("api.history.query", read).await {
        Ok(Ok(page)) => Json(page).into_response(),
        Ok(Err(err)) => history_failure("api.history.query", err).into_response(),
        Err(error) => error.into_response(),
    }
}

/// `GET /history`.
async fn history(
    State(state): State<ApiState>,
    params: Result<Query<HistoryParams>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return bad_request("query").into_response();
    };
    let checked = params.query().and_then(|query| {
        page_request(params.limit, params.cursor.as_deref()).map(|request| (query, request))
    });
    match checked {
        Ok((query, request)) => answer_page(&state, query, request).await,
        Err(param) => bad_request(param).into_response(),
    }
}

/// `GET /files/{id}/history`.
async fn file_history(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    params: Result<Query<PageParams>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return bad_request("query").into_response();
    };
    let object = match object_of(&id) {
        Ok(object) => object,
        Err(error) => return error.into_response(),
    };
    match page_request(params.limit, params.cursor.as_deref()) {
        Ok(request) => answer_page(&state, HistoryQuery::Object(object), request).await,
        Err(param) => bad_request(param).into_response(),
    }
}

/// `GET /files/{id}/versions/{version}/prov`.
async fn prov(
    State(state): State<ApiState>,
    Path((id, version)): Path<(String, String)>,
) -> Response {
    let source = match installed(&state) {
        Ok(source) => source,
        Err(error) => return error.into_response(),
    };
    let object = match object_of(&id) {
        Ok(object) => object,
        Err(error) => return error.into_response(),
    };
    if VersionRef::new(object.clone(), version.clone()).is_err() {
        return bad_request("version").into_response();
    }
    let read = move || source.record(&object, &version);
    match blocking("api.history.prov", read).await {
        Ok(Ok(Some(bytes))) => ([(header::CONTENT_TYPE, JSON_LD)], bytes).into_response(),
        Ok(Ok(None)) => not_found().into_response(),
        Ok(Err(err)) => history_failure("api.history.prov", err).into_response(),
        Err(error) => error.into_response(),
    }
}

/// `not_found` for a version without a record.
fn not_found() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "This version has no history record.")
}

/// `GET /files/{id}/history/export`.
async fn export(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    params: Result<Query<ExportParams>, QueryRejection>,
) -> Response {
    let Ok(Query(params)) = params else {
        return bad_request("query").into_response();
    };
    let source = match installed(&state) {
        Ok(source) => source,
        Err(error) => return error.into_response(),
    };
    let object = match object_of(&id) {
        Ok(object) => object,
        Err(error) => return error.into_response(),
    };
    let redact = params.redact.unwrap_or(false);
    let read = move || export_of(source.as_ref(), &object, redact);
    match blocking("api.history.export", read).await {
        Ok(Ok(body)) => export_response(body),
        Ok(Err(err)) => history_failure("api.history.export", err).into_response(),
        Err(error) => error.into_response(),
    }
}

/// Every record of `object` as one JSON-LD array, redacted when asked (storage I/O). At most
/// [`MAX_HISTORY_RECORDS`] records, walked page by page.
fn export_of(
    source: &dyn HistorySource,
    object: &str,
    redact: bool,
) -> Result<String, HistoryError> {
    let index = source.index()?;
    let query = HistoryQuery::Object(object.to_string());
    let mut redactor = Redactor::new();
    let mut records = Vec::new();
    let mut after = None;
    for _ in 0..=MAX_HISTORY_RECORDS / MAX_PAGE_SIZE {
        let request = PageRequest::new(MAX_PAGE_SIZE, after).map_err(|_| HistoryError::Corrupt)?;
        let page = index.query(&query, &request);
        for activity in page.items() {
            records.push(export_value(&index, activity.id(), redact, &mut redactor)?);
        }
        let Some(next) = page.next() else {
            break;
        };
        after = Some(next.clone());
    }
    serde_json::to_string(&records).map_err(|_| HistoryError::Corrupt)
}

/// The record of `activity_id` as JSON, redacted when asked.
fn export_value(
    index: &HistoryIndex,
    activity_id: &str,
    redact: bool,
    redactor: &mut Redactor,
) -> Result<serde_json::Value, HistoryError> {
    let record = index.record(activity_id).ok_or(HistoryError::Corrupt)?;
    let text = to_json_ld(record);
    if redact {
        return redactor
            .redact(text.as_bytes())
            .map_err(|_| HistoryError::Corrupt);
    }
    serde_json::from_str(&text).map_err(|_| HistoryError::Corrupt)
}

/// A JSON-LD attachment.
fn export_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, JSON_LD),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"filesquid-history.jsonld\"",
            ),
        ],
        body,
    )
        .into_response()
}
