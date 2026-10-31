//! The search operation (Task API-F2.3; API-S-012, SRS-F-063): `GET /api/v1/files/search`
//! answers the files and folder rows of the caller's readable scope that match a query of
//! the grammar in [`super::search_query`], in relevance order by default or by any view sort
//! key, paged by the view's cursor rules ([`super::drive::page_rows`]).
//!
//! Results are rows like the view's: files by display name (API-S-027) and collections and
//! virtual collections as folder rows. The scope is permission-filtered before anything is
//! read or ranked ([`super::search_scope`]). There is no separate index: each search reads
//! the catalog and the files' OCFL records, so the answer always follows canonical state.
//! Everything here is pure; the route reads the scope and the facts.

use super::drive::{
    check_limit, digest_parameters, file_candidate, folder_candidate, page_rows, ranked, Candidate,
    FileFacts, FolderRow, MyTimes, PageParams, ViewRow,
};
use super::error::FileError;
use super::search_match::{
    file_matches, file_score, folder_matches, folder_score, SearchFile, Searcher,
};
use super::search_query::Query;
use crate::preferences::{FolderPlacement, SortDirection, SortKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::UtcOffset;

/// What a search is ordered by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
    /// Relevance to the free text (the default), most relevant first by default.
    #[default]
    Relevance,
    /// The display name.
    Name,
    /// The size in bytes.
    Size,
    /// When anyone last changed it.
    Modified,
    /// When the caller last changed it.
    ModifiedByMe,
    /// When the caller last opened it.
    OpenedByMe,
}

impl SearchSort {
    /// The view sort key this order uses, or `None` for relevance.
    fn view_key(self) -> Option<SortKey> {
        match self {
            Self::Relevance => None,
            Self::Name => Some(SortKey::Name),
            Self::Size => Some(SortKey::Size),
            Self::Modified => Some(SortKey::Modified),
            Self::ModifiedByMe => Some(SortKey::ModifiedByMe),
            Self::OpenedByMe => Some(SortKey::OpenedByMe),
        }
    }
}

/// Query of `GET /api/v1/files/search`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// The query: free text and operators.
    pub q: String,
    /// The repository, library, collection or virtual-collection node searched; the
    /// repository when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// The order; `relevance` when absent.
    #[serde(default)]
    pub sort: SearchSort,
    /// The direction; descending for relevance and ascending otherwise when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<SortDirection>,
    /// Where folder rows go; `on_top` when absent.
    #[serde(default)]
    pub folders: FolderPlacement,
    /// The caller's zone as a fixed offset (`+02:00`, `-08:00`, `Z`) for date boundaries;
    /// UTC when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// The cursor of the previous page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Rows per page, 1 to 1,000; 100 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl SearchRequest {
    /// The direction this request sorts in.
    pub fn direction(&self) -> SortDirection {
        match (self.direction, self.sort) {
            (Some(direction), _) => direction,
            (None, SearchSort::Relevance) => SortDirection::Descending,
            (None, _) => SortDirection::Ascending,
        }
    }
}

/// One page of search results.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SearchPage {
    /// The query.
    pub q: String,
    /// The node searched, or `null` for the repository.
    pub node: Option<String>,
    /// The order.
    pub sort: SearchSort,
    /// The direction.
    pub direction: SortDirection,
    /// The folder placement.
    pub folders: FolderPlacement,
    /// The rows, in order.
    pub rows: Vec<ViewRow>,
    /// The cursor of the next page, or `null` on the last page.
    pub next_cursor: Option<String>,
    /// False when a bound left files out or a file could not be read.
    pub complete: bool,
}

/// What one search examines: its files, folder rows and the caller's own times.
#[derive(Clone, Debug, Default)]
pub struct Examined {
    /// The files, with their facts.
    pub files: Vec<SearchFile>,
    /// The folder rows.
    pub folders: Vec<FolderRow>,
    /// The caller's own times, by object ID.
    pub mine: HashMap<String, MyTimes>,
    /// False when a bound left something out.
    pub complete: bool,
}

/// The widest offset of a real zone, in hours.
const MAX_ZONE_HOURS: i8 = 14;

/// The zone `zone` names: `Z` or `±HH:MM` within ±14:00; UTC when absent.
///
/// # Errors
/// `invalid_request` for any other text.
pub fn parse_zone(zone: Option<&str>) -> Result<UtcOffset, FileError> {
    let format = time::macros::format_description!("[offset_hour sign:mandatory]:[offset_minute]");
    let refused =
        || FileError::invalid("zone is Z or an offset from -14:00 to +14:00, such as -08:00.");
    let offset = match zone {
        None | Some("Z") => return Ok(UtcOffset::UTC),
        Some(text) => UtcOffset::parse(text, &format).map_err(|_| refused())?,
    };
    match offset.whole_seconds().abs() <= i32::from(MAX_ZONE_HOURS) * 3_600 {
        true => Ok(offset),
        false => Err(refused()),
    }
}

/// One page of the results of `query` over `examined` for `request` and `searcher`.
///
/// # Errors
/// `invalid_request` for a limit out of bounds or a cursor of other parameters; `conflict`
/// when the results changed since the cursor was made.
pub fn search_page(
    request: &SearchRequest,
    query: &Query,
    searcher: Searcher,
    examined: Examined,
) -> Result<SearchPage, FileError> {
    let key = request.sort.view_key();
    let sort = key.unwrap_or(SortKey::Name);
    let mut candidates: Vec<Candidate> = Vec::new();
    for folder in examined.folders {
        if folder_matches(query, &folder, searcher) {
            let score = folder_score(query, &folder);
            candidates.push(order(folder_candidate(folder, sort), key, score));
        }
    }
    for file in examined.files {
        if file_matches(query, &file, searcher) {
            let score = file_score(query, &file);
            let mine = examined
                .mine
                .get(&file.facts.object_id)
                .copied()
                .unwrap_or_default();
            candidates.push(order(file_candidate(file.facts, mine, sort, None), key, score));
        }
    }
    let direction = request.direction();
    let params = PageParams {
        fingerprint: fingerprint(request, direction),
        direction,
        folders: request.folders,
        cursor: request.cursor.as_deref(),
        limit: check_limit(request.limit)?,
    };
    let page = page_rows(params, candidates)?;
    Ok(SearchPage {
        q: request.q.clone(),
        node: request.node.clone(),
        sort: request.sort,
        direction,
        folders: request.folders,
        rows: page.rows,
        next_cursor: page.next_cursor,
        complete: examined.complete,
    })
}

/// `candidate` ranked by `score` when the order is relevance (`key` is `None`).
fn order(candidate: Candidate, key: Option<SortKey>, score: i64) -> Candidate {
    match key {
        Some(_) => candidate,
        None => ranked(candidate, score),
    }
}

/// The digest of every parameter of `request` but the cursor and the limit.
fn fingerprint(request: &SearchRequest, direction: SortDirection) -> String {
    digest_parameters(&format!(
        "search\n{}\n{}\n{:?}\n{direction:?}\n{:?}\n{}",
        request.q,
        request.node.as_deref().unwrap_or_default(),
        request.sort,
        request.folders,
        request.zone.as_deref().unwrap_or_default(),
    ))
}

/// The search files of `facts`, placed as `scoped` says (by file ID).
pub fn search_files(
    facts: Vec<FileFacts>,
    scoped: &HashMap<String, (Option<String>, bool)>,
) -> Vec<SearchFile> {
    facts
        .into_iter()
        .filter_map(|facts| {
            let (collection, deleted) = scoped.get(&facts.file_id)?.clone();
            Some(SearchFile {
                facts,
                collection,
                deleted,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::search_query::parse;

    /// A file named `name` of `size` bytes.
    fn file(id: &str, name: &str, size: u64) -> SearchFile {
        SearchFile {
            facts: FileFacts {
                file_id: id.into(),
                object_id: format!("urn:uuid:{id}"),
                display_name: name.into(),
                media_type: "application/pdf".into(),
                size,
                modified_ms: 0,
                statements: Vec::new(),
            },
            collection: None,
            deleted: false,
        }
    }

    /// A request for `q` sorted by `sort`.
    fn request(q: &str, sort: SearchSort) -> SearchRequest {
        SearchRequest {
            q: q.into(),
            node: None,
            sort,
            direction: None,
            folders: FolderPlacement::OnTop,
            zone: None,
            cursor: None,
            limit: None,
        }
    }

    /// The names of the rows of `page`.
    fn names(page: &SearchPage) -> Vec<String> {
        page.rows
            .iter()
            .map(|row| match row {
                ViewRow::File(f) => f.display_name.clone(),
                ViewRow::Folder(f) => f.name.clone(),
            })
            .collect()
    }

    const OWNER: Searcher = Searcher {
        zone: UtcOffset::UTC,
        owns_files: true,
    };

    fn examined() -> Examined {
        Examined {
            files: vec![
                file("1", "zz deck notes.pdf", 5),
                file("2", "Deck.pdf", 9),
                file("3", "budget.pdf", 1),
            ],
            folders: Vec::new(),
            mine: HashMap::new(),
            complete: true,
        }
    }

    #[test]
    fn relevance_is_the_default_and_any_view_key_replaces_it() {
        let query = parse("deck").expect("q");
        let by_relevance = search_page(
            &request("deck", SearchSort::Relevance),
            &query,
            OWNER,
            examined(),
        )
        .expect("page");
        assert_eq!(by_relevance.direction, SortDirection::Descending);
        assert_eq!(names(&by_relevance), ["Deck.pdf", "zz deck notes.pdf"]);
        let by_size = search_page(
            &request("deck", SearchSort::Size),
            &query,
            OWNER,
            examined(),
        )
        .expect("page");
        assert_eq!(by_size.direction, SortDirection::Ascending);
        assert_eq!(names(&by_size), ["zz deck notes.pdf", "Deck.pdf"]);
    }

    #[test]
    fn zones_are_fixed_offsets() {
        assert_eq!(parse_zone(None).expect("utc"), UtcOffset::UTC);
        assert_eq!(parse_zone(Some("Z")).expect("utc"), UtcOffset::UTC);
        assert_eq!(
            parse_zone(Some("-08:00")).expect("zone"),
            UtcOffset::from_hms(-8, 0, 0).expect("zone")
        );
        for bad in ["PST", "+8", "08:00", "+25:00"] {
            assert!(parse_zone(Some(bad)).is_err(), "{bad}");
        }
    }
}
