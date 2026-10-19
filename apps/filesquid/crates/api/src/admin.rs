//! Admin routes of the FileSquid API: health, processes, journal (query and JSON Lines export)
//! and metrics (SRS-F-061). Only the desktop owner or a server administrator may call them:
//! every route performs [`Action::ReadMonitor`], which the [`crate::auth`] layer authorises
//! before the handler runs.
//!
//! Every failure answers with a typed [`ApiError`] (API-S-009).

use crate::auth::Action;
use crate::error::{ApiError, ErrorCode};
use crate::journal::{parse_rfc3339_ms, JournalQuery, Level, Outcome, MAX_PAGE};
use crate::monitor::Monitor;
use crate::operation::PageSize;
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Json;
use serde::Deserialize;

/// Export stops after this many pages ([`MAX_PAGE`] entries each) to bound one response.
const MAX_EXPORT_PAGES: usize = 1_000;

/// Response header set when an export stopped at the export page cap (`MAX_EXPORT_PAGES`) with more entries
/// left; pass its value as `after` to continue.
pub const NEXT_CURSOR_HEADER: &str = "x-journal-next-cursor";

/// Admin routes, relative to the API prefix.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/admin/health", get(health), Action::ReadMonitor),
        ("/admin/processes", get(processes), Action::ReadMonitor),
        ("/admin/journal", get(journal), Action::ReadMonitor),
        ("/admin/journal/export", get(export), Action::ReadMonitor),
        ("/admin/metrics", get(metrics), Action::ReadMonitor),
    ]
}

/// Resolve the installed monitor (`unavailable` before the monitor starts). The caller was
/// already authorised for [`Action::ReadMonitor`] by the route's guard.
fn installed_monitor(state: &ApiState) -> Result<Monitor, ApiError> {
    state.monitor().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "The monitor is starting. Try again in a moment.",
        )
    })
}

/// Run blocking store or filesystem work off the async executor; a panicked task becomes
/// an `internal` error for `operation`.
pub(crate) async fn blocking<T: Send + 'static>(
    operation: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::internal(operation, &e.to_string()))
}

async fn health(State(state): State<ApiState>) -> Response {
    let monitor = match installed_monitor(&state) {
        Ok(m) => m,
        Err(error) => return error.into_response(),
    };
    match blocking("api.admin.health", move || monitor.health_report()).await {
        Ok(report) => Json(report).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn processes(State(state): State<ApiState>) -> Response {
    match installed_monitor(&state) {
        Ok(m) => Json(m.registry().snapshot()).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn metrics(State(state): State<ApiState>) -> Response {
    match installed_monitor(&state) {
        Ok(m) => Json(m.metrics().snapshot()).into_response(),
        Err(error) => error.into_response(),
    }
}

/// Query-string filters for the journal routes.
#[derive(Debug, Default, Deserialize)]
struct JournalParams {
    from: Option<String>,
    to: Option<String>,
    actor: Option<String>,
    operation: Option<String>,
    outcome: Option<String>,
    level: Option<String>,
    process: Option<String>,
    after: Option<i64>,
    limit: Option<usize>,
    order: Option<String>,
}

impl JournalParams {
    /// The requested page size: absent means the journal default (`0`); an explicit `limit`
    /// MUST be a [`PageSize`] (1 to [`filesquid_core::MAX_PAGE_SIZE`]), else `Err("limit")`.
    fn page_limit(&self) -> Result<usize, &'static str> {
        let Some(limit) = self.limit else {
            return Ok(0);
        };
        u32::try_from(limit)
            .ok()
            .and_then(|n| PageSize::new(n).ok())
            .map(|size| size.get() as usize)
            .ok_or("limit")
    }

    /// Convert to a store query; `Err` names the invalid parameter.
    fn to_query(&self) -> Result<JournalQuery, &'static str> {
        let time = |v: &Option<String>, name| {
            v.as_deref()
                .map(|t| parse_rfc3339_ms(t).ok_or(name))
                .transpose()
        };
        Ok(JournalQuery {
            from_ms: time(&self.from, "from")?,
            to_ms: time(&self.to, "to")?,
            actor: self.actor.clone(),
            operation: self.operation.clone(),
            outcome: self
                .outcome
                .as_deref()
                .map(|o| Outcome::parse(o).ok_or("outcome"))
                .transpose()?,
            level: self
                .level
                .as_deref()
                .map(|l| Level::parse(l).ok_or("level"))
                .transpose()?,
            process_id: self.process.clone(),
            after_id: self.after,
            limit: self.page_limit()?,
            descending: match self.order.as_deref() {
                None | Some("asc") => false,
                Some("desc") => true,
                Some(_) => return Err("order"),
            },
        })
    }
}

/// `invalid_request` error naming the invalid query parameter.
fn bad_request(param: &str) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        format!("Invalid query parameter: {param}."),
    )
}

/// Resolve the monitor, then turn the (possibly rejected) query string into a store query.
/// The route's guard has already authorised the caller, so an anonymous caller never reaches
/// parameter validation and learns nothing about parameters.
fn monitor_query(
    state: &ApiState,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Result<(Monitor, JournalQuery), ApiError> {
    let monitor = installed_monitor(state)?;
    let Query(params) = params
        .map_err(|_| ApiError::new(ErrorCode::InvalidRequest, "The query string is malformed."))?;
    let query = params.to_query().map_err(bad_request)?;
    Ok((monitor, query))
}

async fn journal(
    State(state): State<ApiState>,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Response {
    let (monitor, query) = match monitor_query(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    let store = monitor.store();
    match blocking("api.admin.journal", move || store.query(&query)).await {
        Ok(Ok(page)) => Json(page).into_response(),
        Ok(Err(e)) => ApiError::internal("api.admin.journal", &e.to_string()).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn export(
    State(state): State<ApiState>,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Response {
    // The export does not document `limit` (it always reads full pages), so a `limit` in the
    // query string is ignored rather than validated.
    let params = params.map(|Query(p)| Query(JournalParams { limit: None, ..p }));
    let (monitor, mut query) = match monitor_query(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    query.limit = MAX_PAGE;
    query.descending = false;
    let store = monitor.store();
    let pages = move || export_pages(store.as_ref(), query, MAX_EXPORT_PAGES);
    match blocking("api.admin.journal.export", pages).await {
        Ok(Ok((text, next))) => export_response(text, next),
        Ok(Err(e)) => ApiError::internal("api.admin.journal.export", &e).into_response(),
        Err(error) => error.into_response(),
    }
}

/// Read up to `max_pages` pages as JSON Lines (store I/O). Returns the text and, when
/// entries remain after the cap, the cursor to continue from.
fn export_pages(
    store: &dyn crate::journal::JournalStore,
    mut query: JournalQuery,
    max_pages: usize,
) -> Result<(String, Option<i64>), String> {
    let mut out = String::new();
    for _ in 0..max_pages {
        let page = store.query(&query).map_err(|e| e.to_string())?;
        for entry in &page.entries {
            out.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
            out.push('\n');
        }
        let Some(cursor) = page.next_cursor else {
            return Ok((out, None));
        };
        query.after_id = Some(cursor);
    }
    Ok((out, query.after_id))
}

/// JSON Lines attachment; a truncated export carries [`NEXT_CURSOR_HEADER`].
fn export_response(text: String, next: Option<i64>) -> Response {
    let mut response = (
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"filesquid-journal.jsonl\"",
            ),
        ],
        text,
    )
        .into_response();
    if let Some(cursor) = next {
        response
            .headers_mut()
            .insert(NEXT_CURSOR_HEADER, header::HeaderValue::from(cursor));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{JournalEntry, JournalStore, MemoryJournal};

    fn store_with(n: usize) -> MemoryJournal {
        let store = MemoryJournal::new("host-a");
        let entries: Vec<_> = (0..n)
            .map(|_| JournalEntry::new("ocfl.commit", Outcome::Succeeded))
            .collect();
        store.append(&entries).expect("append");
        store
    }

    #[test]
    fn an_export_that_hits_the_page_cap_returns_a_continuation_cursor() {
        let store = store_with(5);
        let query = JournalQuery {
            limit: 2,
            ..Default::default()
        };
        let (text, next) = export_pages(&store, query, 2).expect("export");
        assert_eq!(text.lines().count(), 4);
        assert_eq!(next, Some(4));
        let rest = JournalQuery {
            limit: 2,
            after_id: next,
            ..Default::default()
        };
        let (text, next) = export_pages(&store, rest, 2).expect("export");
        assert_eq!(text.lines().count(), 1);
        assert_eq!(next, None);
    }

    #[test]
    fn a_complete_export_has_no_continuation_cursor_header() {
        let (text, next) =
            export_pages(&store_with(3), JournalQuery::default(), 1).expect("export");
        assert_eq!(text.lines().count(), 3);
        assert!(export_response(text, next)
            .headers()
            .get(NEXT_CURSOR_HEADER)
            .is_none());
        let truncated = export_response(String::new(), Some(7));
        assert_eq!(truncated.headers()[NEXT_CURSOR_HEADER], "7");
    }
}
