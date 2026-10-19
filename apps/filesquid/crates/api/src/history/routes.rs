//! History routes of the FileSquid API (API-S-029; PRD F-067, F-046). Every route performs
//! [`Action::ReadHistory`], which the [`crate::auth`] layer authorises before the handler
//! runs, and reads the host's [`HistorySource`] off the async executor; before the host
//! installs one every route answers `unavailable`.
//!
//! - `GET /history`: activities of the whole history, or of one object, operation or agent,
//!   or of a time range, in pages of 1 to 1000 with an opaque cursor.
//! - `GET /files/{id}/history`: the activities of the object `urn:uuid:<id>`.
//! - `GET /files/{id}/history/export`: every record of that object as one JSON-LD array,
//!   optionally redacted for sharing (`redact=true`), written record by record into one
//!   body of at most [`MAX_EXPORT_BYTES`]; a partial export carries [`TRUNCATED_HEADER`].
//! - `GET /files/{id}/versions/{version}/prov`: the JSON-LD record of one version.
//!
//! A history cut short by its record or object cap is never answered as complete: pages
//! carry `truncated: true` and the export carries [`TRUNCATED_HEADER`].
//!
//! Every failure answers with a typed [`ApiError`] (API-S-009).

use std::sync::Arc;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use filesquid_core::{HistoryEntry, VersionRef};
use filesquid_prov::{to_json_ld, Cursor, HistoryQuery, PageRequest, Timestamp, MAX_PAGE_SIZE};
use serde::{Deserialize, Serialize};

use super::redact::Redactor;
use super::source::{entry_of, HistoryError, HistorySnapshot, HistorySource, MAX_HISTORY_RECORDS};
use crate::admin::blocking;
use crate::auth::Action;
use crate::error::{ApiError, ErrorCode};
use crate::journal::{is_opaque, DEFAULT_PAGE};
use crate::routes::Route;
use crate::ApiState;

/// The media type of PROV-O records and exports.
pub const JSON_LD: &str = "application/ld+json";

/// Response header of an export that left records out (value `true`): the history index
/// reached its cap or the body reached [`MAX_EXPORT_BYTES`].
pub const TRUNCATED_HEADER: &str = "x-history-truncated";

/// Largest export body in bytes; records past it are left out and the export is flagged.
pub const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

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
    /// True when the history index stopped at its record or object cap, so this page may
    /// miss activities.
    pub truncated: bool,
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
    let snapshot = source.index()?;
    let page = snapshot.index.query(query, request);
    let items = page
        .items()
        .iter()
        .map(entry_of)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HistoryPage {
        items,
        next_cursor: page.next().map(Cursor::to_token),
        truncated: snapshot.truncated,
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
    let read = move || export_of(source.as_ref(), &object, redact, MAX_EXPORT_BYTES);
    match blocking("api.history.export", read).await {
        Ok(Ok(body)) => export_response(body),
        Ok(Err(err)) => history_failure("api.history.export", err).into_response(),
        Err(error) => error.into_response(),
    }
}

/// A JSON-LD array written record by record, refusing records past a byte budget.
#[derive(Debug)]
struct ExportBody {
    /// The array so far, without its closing bracket.
    text: String,
    /// Records written.
    count: usize,
    /// Largest body, closing bracket included.
    max_bytes: usize,
    /// True when records were left out.
    truncated: bool,
}

impl ExportBody {
    /// An empty array of at most `max_bytes`, already flagged when the index was cut short.
    fn new(max_bytes: usize, truncated: bool) -> Self {
        Self {
            text: String::from("["),
            count: 0,
            max_bytes,
            truncated,
        }
    }

    /// Append the JSON text of one record, or flag the body truncated and return `false`
    /// when it would not fit within the budget.
    fn push(&mut self, record: &str) -> bool {
        let separator = usize::from(self.count > 0);
        if self.text.len() + separator + record.len() + 1 > self.max_bytes {
            self.truncated = true;
            return false;
        }
        if separator == 1 {
            self.text.push(',');
        }
        self.text.push_str(record);
        self.count += 1;
        true
    }

    /// The closed array and whether records were left out.
    fn finish(mut self) -> (String, bool) {
        self.text.push(']');
        (self.text, self.truncated)
    }
}

/// Every record of `object` as one JSON-LD array of at most `max_bytes`, redacted when
/// asked (storage I/O), and whether records were left out. At most [`MAX_HISTORY_RECORDS`]
/// records, walked page by page and written straight into the body, so no second copy of
/// the history is held.
fn export_of(
    source: &dyn HistorySource,
    object: &str,
    redact: bool,
    max_bytes: usize,
) -> Result<(String, bool), HistoryError> {
    let snapshot = source.index()?;
    let query = HistoryQuery::Object(object.to_string());
    let mut redactor = Redactor::new();
    let mut body = ExportBody::new(max_bytes, snapshot.truncated);
    let mut after = None;
    for _ in 0..=MAX_HISTORY_RECORDS / MAX_PAGE_SIZE {
        let request = PageRequest::new(MAX_PAGE_SIZE, after).map_err(|_| HistoryError::Corrupt)?;
        let page = snapshot.index.query(&query, &request);
        for activity in page.items() {
            let text = export_text(&snapshot, activity.id(), redact, &mut redactor)?;
            if !body.push(&text) {
                return Ok(body.finish());
            }
        }
        let Some(next) = page.next() else {
            break;
        };
        after = Some(next.clone());
    }
    Ok(body.finish())
}

/// The record of `activity_id` as JSON text, redacted when asked.
fn export_text(
    snapshot: &HistorySnapshot,
    activity_id: &str,
    redact: bool,
    redactor: &mut Redactor,
) -> Result<String, HistoryError> {
    let record = snapshot
        .index
        .record(activity_id)
        .ok_or(HistoryError::Corrupt)?;
    let text = to_json_ld(record);
    if !redact {
        return Ok(text);
    }
    let value = redactor
        .redact(text.as_bytes())
        .map_err(|_| HistoryError::Corrupt)?;
    Ok(value.to_string())
}

/// A JSON-LD attachment, carrying [`TRUNCATED_HEADER`] when records were left out.
fn export_response((body, truncated): (String, bool)) -> Response {
    let mut response = (
        [
            (header::CONTENT_TYPE, JSON_LD),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"filesquid-history.jsonld\"",
            ),
        ],
        body,
    )
        .into_response();
    if truncated {
        response.headers_mut().insert(
            HeaderName::from_static(TRUNCATED_HEADER),
            HeaderValue::from_static("true"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::MemoryHistory;
    use filesquid_prov::{build_record, VersionFacts};

    const OBJECT: &str = "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

    /// A history of `count` versions of [`OBJECT`].
    fn history_of(count: u64) -> MemoryHistory {
        let history = MemoryHistory::new();
        for n in 1..=count {
            let facts = VersionFacts {
                object_id: OBJECT.into(),
                version: format!("v{n}"),
                base_version: (n > 1).then(|| format!("v{}", n - 1)),
                created: format!("2026-10-19T10:00:0{n}Z"),
                started: format!("2026-10-19T10:00:0{n}Z"),
                operation_id: format!("op-{n}"),
                kind: "import".into(),
                new_state: [(format!("f{n}.txt"), "c".repeat(64))].into(),
                journal_entries: vec![n.to_string()],
                ..VersionFacts::default()
            };
            history.insert(build_record(&facts).unwrap()).unwrap();
        }
        history
    }

    #[test]
    fn an_export_past_its_byte_budget_stops_at_a_whole_record_and_is_flagged() {
        let history = history_of(3);
        let (full, truncated) = export_of(&history, OBJECT, false, MAX_EXPORT_BYTES).unwrap();
        assert!(!truncated);
        let all: Vec<serde_json::Value> = serde_json::from_str(&full).unwrap();
        assert_eq!(all.len(), 3);
        let (part, truncated) = export_of(&history, OBJECT, false, full.len() - 1).unwrap();
        assert!(truncated);
        let some: Vec<serde_json::Value> = serde_json::from_str(&part).unwrap();
        assert_eq!(
            some.len(),
            2,
            "a whole record is left out, the array stays valid"
        );
        assert!(part.len() < full.len());
    }
}
