//! The journal routes (SRS-F-061, API-S-031; ADR 033 Decision 7):
//!
//! - `GET /admin/journal`: one page of entries, filtered by time range, actor, operation,
//!   outcome, level, process, library, object ID or file, and operation ID;
//! - `GET /admin/journal/entries/{entry}`: one entry by ID;
//! - `GET /admin/journal/export`: every matching entry as JSON Lines or CSV (RFC 4180),
//!   journaled as [`EXPORT_OPERATION`].
//!
//! Pages and single entries are [`JournalEntryView`]s: the stored entry, its
//! [`ProvenanceLink`] (the PROV-O activity of a committed operation, or `journal_only`), and,
//! only when asked with `names=true` and only through the host's [`JournalNames`], the
//! current display name of the file its object ID names. Names are resolved at read time for
//! the caller, never stored and never exported.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use filesquid_core::VersionRef;
use serde::{Deserialize, Serialize};

use super::{blocking, installed_monitor};
use crate::auth::{journal_entry, Principal};
use crate::client_events::schema::is_uuid;
use crate::error::{ApiError, ErrorCode};
use crate::files::NameFuture;
use crate::journal::csv::{csv_row, CSV_HEADER, CSV_LINE_END};
use crate::journal::link::{provenance_of, ProvenanceLink};
use crate::journal::{
    is_opaque, parse_rfc3339_ms, JournalEntry, JournalPage, JournalQuery, JournalStore, Level,
    Outcome, MAX_PAGE,
};
use crate::monitor::Monitor;
use crate::operation::PageSize;
use crate::ApiState;

/// Journal operation of every export.
pub const EXPORT_OPERATION: &str = "journal.export";

/// Export stops after this many pages ([`MAX_PAGE`] entries each, so at most 50,000
/// entries) to bound the memory of one response; [`NEXT_CURSOR_HEADER`] continues it.
pub const MAX_EXPORT_PAGES: usize = 50;

/// Most distinct objects whose names one page resolves; entries of further objects carry
/// no name (names are for display only).
pub const MAX_NAMED_OBJECTS: usize = 200;

/// Response header set when an export stopped at the export page cap (`MAX_EXPORT_PAGES`)
/// with more entries left; pass its value as `after` to continue.
pub const NEXT_CURSOR_HEADER: &str = "x-journal-next-cursor";

/// Prefix of FileSquid object IDs; `file=<uuid>` filters by `urn:uuid:<uuid>`.
const URN_UUID: &str = "urn:uuid:";

/// The host's lookup from an object ID to the current display name of the file it names,
/// for the caller (API-S-031). An implementation answers `None` for an object the
/// `principal` may not read, and for one it cannot name; it never fails the page.
pub trait JournalNames: Send + Sync {
    /// The display name of the file `object_id` names, as `principal` may see it.
    fn display_name<'a>(&'a self, principal: &'a Principal, object_id: &'a str) -> NameFuture<'a>;
}

/// A journal entry as the journal routes answer it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalEntryView {
    /// The stored entry.
    #[serde(flatten)]
    pub entry: JournalEntry,
    /// The PROV-O activity of the entry's operation, or that it has none.
    pub provenance: ProvenanceLink,
    /// The current display name of the file the entry's object ID names (display only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// One page of [`JournalEntryView`]s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalViewPage {
    /// The entries, in the requested order.
    pub entries: Vec<JournalEntryView>,
    /// Cursor for the next page, when more entries match.
    pub next_cursor: Option<i64>,
}

/// The export formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExportFormat {
    /// One JSON object per line.
    JsonLines,
    /// RFC 4180 CSV with a header row.
    Csv,
}

impl ExportFormat {
    fn parse(text: Option<&str>) -> Result<Self, &'static str> {
        match text {
            None | Some("jsonl") => Ok(Self::JsonLines),
            Some("csv") => Ok(Self::Csv),
            Some(_) => Err("format"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::JsonLines => "jsonl",
            Self::Csv => "csv",
        }
    }
}

/// Query-string filters for the journal routes.
#[derive(Debug, Default, Deserialize)]
pub(super) struct JournalParams {
    from: Option<String>,
    to: Option<String>,
    actor: Option<String>,
    operation: Option<String>,
    outcome: Option<String>,
    level: Option<String>,
    process: Option<String>,
    library: Option<String>,
    object: Option<String>,
    file: Option<String>,
    operation_id: Option<String>,
    after: Option<i64>,
    limit: Option<usize>,
    order: Option<String>,
    names: Option<bool>,
    format: Option<String>,
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

    /// The object filter: `object` as given, or `urn:uuid:<file>`; both is an error.
    fn object_id(&self) -> Result<Option<String>, &'static str> {
        match (&self.object, &self.file) {
            (Some(_), Some(_)) => Err("file"),
            (Some(object), None) => opaque(object, "object").map(Some),
            (None, Some(file)) if is_uuid(file) => {
                Ok(Some(format!("{URN_UUID}{}", file.to_ascii_lowercase())))
            }
            (None, Some(_)) => Err("file"),
            (None, None) => Ok(None),
        }
    }

    /// Convert to a store query; `Err` names the invalid parameter.
    fn to_query(&self) -> Result<JournalQuery, &'static str> {
        let time = |v: &Option<String>, name| {
            v.as_deref()
                .map(|t| parse_rfc3339_ms(t).ok_or(name))
                .transpose()
        };
        let optional = |v: &Option<String>, name| v.as_deref().map(|t| opaque(t, name)).transpose();
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
            library_id: optional(&self.library, "library")?,
            object_id: self.object_id()?,
            operation_id: optional(&self.operation_id, "operation_id")?,
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

/// `value` when it is an opaque token, else `Err(name)`.
fn opaque(value: &str, name: &'static str) -> Result<String, &'static str> {
    if is_opaque(value) {
        Ok(value.to_string())
    } else {
        Err(name)
    }
}

/// Query string of `GET /admin/journal/entries/{entry}`.
#[derive(Debug, Default, Deserialize)]
pub(super) struct EntryParams {
    names: Option<bool>,
}

/// `invalid_request` error naming the invalid query parameter.
fn bad_request(param: &str) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        format!("Invalid query parameter: {param}."),
    )
}

/// Resolve the monitor, then turn the (possibly rejected) query string into the parameters.
/// The route's guard has already authorised the caller, so an anonymous caller never reaches
/// parameter validation and learns nothing about parameters.
fn monitor_params<P: Default>(
    state: &ApiState,
    params: Result<Query<P>, QueryRejection>,
) -> Result<(Monitor, P), ApiError> {
    let monitor = installed_monitor(state)?;
    let Query(params) = params
        .map_err(|_| ApiError::new(ErrorCode::InvalidRequest, "The query string is malformed."))?;
    Ok((monitor, params))
}

/// `GET /admin/journal`: one page of entry views.
pub(super) async fn page(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Response {
    let (monitor, params) = match monitor_params(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    let query = match params.to_query() {
        Ok(query) => query,
        Err(param) => return bad_request(param).into_response(),
    };
    let store = monitor.store();
    let page = match blocking("api.admin.journal", move || store.query(&query)).await {
        Ok(Ok(page)) => page,
        Ok(Err(e)) => {
            return ApiError::internal("api.admin.journal", &e.to_string()).into_response()
        }
        Err(error) => return error.into_response(),
    };
    let names = wanted_names(&state, params.names);
    Json(view_page(page, names, &principal).await).into_response()
}

/// `GET /admin/journal/entries/{entry}`: one entry view, or `not_found`.
pub(super) async fn one(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(entry): Path<String>,
    params: Result<Query<EntryParams>, QueryRejection>,
) -> Response {
    let (monitor, params) = match monitor_params(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    let Ok(id) = entry.parse::<i64>() else {
        return ApiError::new(ErrorCode::InvalidRequest, "The entry ID must be a number.")
            .into_response();
    };
    let store = monitor.store();
    let found = match blocking("api.admin.journal.entry", move || store.entry(id)).await {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => {
            return ApiError::internal("api.admin.journal.entry", &e.to_string()).into_response()
        }
        Err(error) => return error.into_response(),
    };
    let Some(entry) = found else {
        return ApiError::new(ErrorCode::NotFound, "No journal entry has this ID.").into_response();
    };
    let names = wanted_names(&state, params.names);
    let mut named = resolve_names(names.as_deref(), &principal, std::slice::from_ref(&entry)).await;
    Json(view(entry, &mut named)).into_response()
}

/// The host's name lookup, when the caller asked for names and the host installed one.
fn wanted_names(state: &ApiState, asked: Option<bool>) -> Option<std::sync::Arc<dyn JournalNames>> {
    if asked == Some(true) {
        state.journal_names()
    } else {
        None
    }
}

/// `page` as views, with names when `names` is given.
async fn view_page(
    page: JournalPage,
    names: Option<std::sync::Arc<dyn JournalNames>>,
    principal: &Principal,
) -> JournalViewPage {
    let mut named = resolve_names(names.as_deref(), principal, &page.entries).await;
    JournalViewPage {
        entries: page
            .entries
            .into_iter()
            .map(|entry| view(entry, &mut named))
            .collect(),
        next_cursor: page.next_cursor,
    }
}

/// `entry` as a view, its name taken from `named`. Pure.
fn view(entry: JournalEntry, named: &mut HashMap<String, Option<String>>) -> JournalEntryView {
    let display_name = object_of(&entry).and_then(|o| named.get(&o).cloned().flatten());
    JournalEntryView {
        provenance: provenance_of(&entry),
        display_name,
        entry,
    }
}

/// The object an entry is about: its object reference's object, else a target that is a
/// FileSquid object ID. Pure.
fn object_of(entry: &JournalEntry) -> Option<String> {
    let by_ref = entry
        .object_ref
        .as_deref()
        .and_then(|r| VersionRef::parse_object_ref(r).ok())
        .map(|r| r.object_id().to_string());
    by_ref.or_else(|| {
        entry
            .target_ids
            .iter()
            .find(|t| t.starts_with(URN_UUID))
            .cloned()
    })
}

/// The display names of the objects `entries` name (at most [`MAX_NAMED_OBJECTS`]), looked
/// up for `principal` through `names`. Empty without a lookup. Host I/O through the lookup.
async fn resolve_names(
    names: Option<&dyn JournalNames>,
    principal: &Principal,
    entries: &[JournalEntry],
) -> HashMap<String, Option<String>> {
    let mut named = HashMap::new();
    let Some(names) = names else {
        return named;
    };
    for object in entries.iter().filter_map(object_of) {
        if named.len() >= MAX_NAMED_OBJECTS {
            break;
        }
        if let Entry::Vacant(slot) = named.entry(object) {
            let name = names.display_name(principal, slot.key()).await;
            slot.insert(name);
        }
    }
    named
}

/// `GET /admin/journal/export`: every matching entry as JSON Lines or CSV, journaled.
pub(super) async fn export(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Response {
    let (monitor, params) = match monitor_params(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    // The export does not document `limit` (it always reads full pages) or names (it never
    // carries them), so those parameters are ignored rather than validated.
    let params = JournalParams {
        limit: None,
        ..params
    };
    let (mut query, format) = match params
        .to_query()
        .and_then(|q| ExportFormat::parse(params.format.as_deref()).map(|f| (q, f)))
    {
        Ok(found) => found,
        Err(param) => return bad_request(param).into_response(),
    };
    query.limit = MAX_PAGE;
    query.descending = false;
    let store = monitor.store();
    let pages = move || export_pages(store.as_ref(), query, MAX_EXPORT_PAGES, format);
    let exported = blocking("api.admin.journal.export", pages).await;
    journal_entry(&state, export_entry(&principal, format, &exported)).await;
    match exported {
        Ok(Ok(export)) => export_response(export, format),
        Ok(Err(e)) => ApiError::internal("api.admin.journal.export", &e).into_response(),
        Err(error) => error.into_response(),
    }
}

/// One export's text, entry count and continuation cursor.
#[derive(Debug)]
struct Export {
    text: String,
    entries: usize,
    next: Option<i64>,
}

/// The `journal.export` entry of one export by `principal`: format and entry count, or the
/// failure. Pure.
fn export_entry(
    principal: &Principal,
    format: ExportFormat,
    exported: &Result<Result<Export, String>, ApiError>,
) -> JournalEntry {
    let entry = |outcome| {
        JournalEntry::new(EXPORT_OPERATION, outcome)
            .actor(principal.actor())
            .target(format!("format:{}", format.as_str()))
    };
    match exported {
        Ok(Ok(export)) => entry(Outcome::Succeeded).target(format!("entries:{}", export.entries)),
        _ => entry(Outcome::Failed).error_code(ErrorCode::Internal.as_str()),
    }
}

/// Read up to `max_pages` pages in `format` (store I/O). Returns the text, the number of
/// entries and, when entries remain after the cap, the cursor to continue from.
fn export_pages(
    store: &dyn JournalStore,
    mut query: JournalQuery,
    max_pages: usize,
    format: ExportFormat,
) -> Result<Export, String> {
    let mut export = Export {
        text: String::new(),
        entries: 0,
        next: None,
    };
    if format == ExportFormat::Csv {
        export.text.push_str(CSV_HEADER);
        export.text.push_str(CSV_LINE_END);
    }
    for _ in 0..max_pages {
        let page = store.query(&query).map_err(|e| e.to_string())?;
        for entry in &page.entries {
            push_line(&mut export.text, entry, format)?;
        }
        export.entries += page.entries.len();
        let Some(cursor) = page.next_cursor else {
            return Ok(export);
        };
        query.after_id = Some(cursor);
    }
    export.next = query.after_id;
    Ok(export)
}

/// Append `entry` as one line of `format`.
fn push_line(out: &mut String, entry: &JournalEntry, format: ExportFormat) -> Result<(), String> {
    match format {
        ExportFormat::JsonLines => {
            out.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
            out.push('\n');
        }
        ExportFormat::Csv => {
            out.push_str(&csv_row(entry));
            out.push_str(CSV_LINE_END);
        }
    }
    Ok(())
}

/// The export attachment; a truncated export carries [`NEXT_CURSOR_HEADER`].
fn export_response(export: Export, format: ExportFormat) -> Response {
    let (content_type, disposition) = match format {
        ExportFormat::JsonLines => (
            "application/x-ndjson",
            "attachment; filename=\"filesquid-journal.jsonl\"",
        ),
        ExportFormat::Csv => (
            "text/csv; charset=utf-8",
            "attachment; filename=\"filesquid-journal.csv\"",
        ),
    };
    let mut response = (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        export.text,
    )
        .into_response();
    if let Some(cursor) = export.next {
        response
            .headers_mut()
            .insert(NEXT_CURSOR_HEADER, header::HeaderValue::from(cursor));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::MemoryJournal;

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
        let export = export_pages(&store, query, 2, ExportFormat::JsonLines).expect("export");
        assert_eq!(export.text.lines().count(), 4);
        assert_eq!(export.entries, 4);
        assert_eq!(export.next, Some(4));
        let rest = JournalQuery {
            limit: 2,
            after_id: export.next,
            ..Default::default()
        };
        let export = export_pages(&store, rest, 2, ExportFormat::JsonLines).expect("export");
        assert_eq!(export.text.lines().count(), 1);
        assert_eq!(export.next, None);
    }

    #[test]
    fn a_complete_export_has_no_continuation_cursor_header() {
        let export = export_pages(
            &store_with(3),
            JournalQuery::default(),
            1,
            ExportFormat::Csv,
        )
        .expect("export");
        assert_eq!(export.text.split_terminator("\r\n").count(), 4);
        let response = export_response(export, ExportFormat::Csv);
        assert!(response.headers().get(NEXT_CURSOR_HEADER).is_none());
        let truncated = Export {
            text: String::new(),
            entries: 0,
            next: Some(7),
        };
        let response = export_response(truncated, ExportFormat::JsonLines);
        assert_eq!(response.headers()[NEXT_CURSOR_HEADER], "7");
    }
}
