//! The view operation (Task API-F2.2; API-S-011, SRS-F-062): one paged, stably ordered list
//! of the files a library, collection or virtual-collection node holds, at
//! `GET /api/v1/files/view`.
//!
//! A request names the node, the view (`all` or `deleted` for now), the sort key (`name`,
//! `size`, `modified`, `modified_by_me`, `opened_by_me`), the direction, the folder placement
//! (`on_top` or `mixed`), an optional view-local filter on the name and a cursor. A library
//! node's `all` view shows its collections and virtual collections as folder rows beside its
//! unfiled files, since a file of the per-file model has no folder of its own; a collection
//! node shows its files, a virtual-collection node its members, and a `deleted` view the
//! recoverably deleted files of the node with no folder rows.
//!
//! Every page sorts the whole scope again and starts after the cursor's row key. The cursor
//! also carries a digest of every row key of the scope, so when the rows or their order
//! changed between pages the cursor is refused as `conflict` instead of letting a row repeat
//! or be skipped, and the client starts again. The key ends with the row's identifier, so
//! no two rows tie. A missing value (a folder's size, a file the caller never opened) sorts
//! after every present value in both directions, and folders on top stay on top in both. The
//! cursor carries the last row's key and a digest of the request's other parameters, so a
//! cursor of one request is refused by another. Everything here is pure; the scope comes from
//! [`super::drive_scope`] and the file facts from the file service.

use super::error::FileError;
use crate::journal::format_rfc3339_ms;
use crate::preferences::{FolderPlacement, SortDirection, SortKey};
use filesquid_ocfl::DigestAlgorithm;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Rows in a page when the request names no limit.
pub const DEFAULT_VIEW_LIMIT: usize = 100;

/// Most rows one page holds.
pub const MAX_VIEW_LIMIT: usize = 1_000;

/// Most files one view sorts; a larger scope answers its first files in catalog order and
/// says it is incomplete.
pub const MAX_VIEW_FILES: usize = 10_000;

/// Longest view-local filter, in bytes.
pub const MAX_FILTER_BYTES: usize = 256;

/// Longest cursor accepted, in bytes.
const MAX_CURSOR_BYTES: usize = 8_192;

/// Prefix of a version 1 view cursor.
const CURSOR_PREFIX: &str = "v1.";

/// Which files a view shows (SRS-F-062).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    /// The Home view (not available yet).
    Home,
    /// Every ordinary file of the node.
    #[default]
    All,
    /// The personal library (not available yet).
    Personal,
    /// Recent files (not available yet).
    Recent,
    /// Favorites (not available yet).
    Favorites,
    /// Shares (not available yet).
    Shares,
    /// Tags (not available yet).
    Tags,
    /// The recoverably deleted files of the node.
    Deleted,
}

impl ViewKind {
    /// The name of this view in requests and answers.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::All => "all",
            Self::Personal => "personal",
            Self::Recent => "recent",
            Self::Favorites => "favorites",
            Self::Shares => "shares",
            Self::Tags => "tags",
            Self::Deleted => "deleted",
        }
    }
}

/// Query of `GET /api/v1/files/view`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRequest {
    /// The library, collection or virtual-collection node handle.
    pub node: String,
    /// The view; `all` when absent.
    #[serde(default)]
    pub view: ViewKind,
    /// The sort key; `name` when absent.
    #[serde(default)]
    pub sort: SortKey,
    /// The direction; `ascending` when absent.
    #[serde(default)]
    pub direction: SortDirection,
    /// Where folder rows go; `on_top` when absent.
    #[serde(default)]
    pub folders: FolderPlacement,
    /// Only rows whose name contains this text, compared without case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// The cursor of the previous page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Rows per page, 1 to [`MAX_VIEW_LIMIT`]; [`DEFAULT_VIEW_LIMIT`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// The kind of node a folder row opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderKind {
    /// A collection.
    Collection,
    /// A virtual collection.
    VirtualCollection,
}

/// A folder row: a node the view can open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FolderRow {
    /// The node's opaque handle.
    pub node: String,
    /// What the node is.
    pub node_kind: FolderKind,
    /// Its name.
    pub name: String,
}

/// A file row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileRow {
    /// The file.
    pub file_id: String,
    /// Its object, `urn:uuid:<file-id>`.
    pub object_id: String,
    /// The name users see (API-S-027).
    pub display_name: String,
    /// The media type of the original.
    pub media_type: String,
    /// The original's size at the head, in bytes.
    pub size: u64,
    /// When the head version was created (RFC 3339 UTC).
    pub modified: String,
    /// When the caller last changed it, from the caller's own journal entries only.
    pub modified_by_me: Option<String>,
    /// When the caller last opened it, from the caller's own journal entries only.
    pub opened_by_me: Option<String>,
}

/// One row of a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewRow {
    /// A collection or virtual collection.
    Folder(FolderRow),
    /// A file.
    File(FileRow),
}

/// One page of a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ViewPage {
    /// The node.
    pub node: String,
    /// The view.
    pub view: ViewKind,
    /// The sort key.
    pub sort: SortKey,
    /// The direction.
    pub direction: SortDirection,
    /// The folder placement.
    pub folders: FolderPlacement,
    /// The rows, in order.
    pub rows: Vec<ViewRow>,
    /// The cursor of the next page, or `null` on the last page.
    pub next_cursor: Option<String>,
    /// False when the scope held more than [`MAX_VIEW_FILES`] files or a file could not be
    /// read, so some files are not in the view.
    pub complete: bool,
}

/// What the file service knows about one file at its head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFacts {
    /// The file.
    pub file_id: String,
    /// Its object.
    pub object_id: String,
    /// Its display name.
    pub display_name: String,
    /// Its media type.
    pub media_type: String,
    /// The original's size at the head.
    pub size: u64,
    /// When the head version was created, in Unix milliseconds.
    pub modified_ms: i64,
}

/// When the caller last changed and last opened one file, in Unix milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MyTimes {
    /// Last change by the caller.
    pub modified_ms: Option<i64>,
    /// Last opening by the caller.
    pub opened_ms: Option<i64>,
}

/// A row with its sort key.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// The row.
    pub row: ViewRow,
    /// Where it sorts.
    key: RowKey,
}

/// The value a row sorts by.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
enum SortValue {
    /// A size or a time.
    Number(i64),
    /// A name folded to lower case.
    Text(String),
}

/// Where a row sorts: its group, its value, its name and its identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RowKey {
    /// True for a folder row.
    folder: bool,
    /// The sort value; `None` sorts after every value.
    value: Option<SortValue>,
    /// The exact name, the first tie-break.
    name: String,
    /// The node handle or file ID, the last tie-break.
    id: String,
}

/// A cursor's content: the last row's key and the digest of the request's parameters.
#[derive(Serialize, Deserialize)]
struct CursorBody {
    /// The last row's key.
    k: RowKey,
    /// The parameter digest.
    p: String,
    /// The digest of every row key of the scope, in order, when the cursor was made.
    s: String,
}

/// The folder row `folder` as a candidate under `sort`.
pub fn folder_candidate(folder: FolderRow, sort: SortKey) -> Candidate {
    let value = match sort {
        SortKey::Name => Some(SortValue::Text(folder.name.to_lowercase())),
        _ => None,
    };
    let key = RowKey {
        folder: true,
        value,
        name: folder.name.clone(),
        id: folder.node.clone(),
    };
    Candidate {
        row: ViewRow::Folder(folder),
        key,
    }
}

/// The file row of `facts` and the caller's `mine` as a candidate under `sort`.
pub fn file_candidate(facts: FileFacts, mine: MyTimes, sort: SortKey) -> Candidate {
    let value = match sort {
        SortKey::Name => Some(SortValue::Text(facts.display_name.to_lowercase())),
        SortKey::Size => Some(SortValue::Number(
            i64::try_from(facts.size).unwrap_or(i64::MAX),
        )),
        SortKey::Modified => Some(SortValue::Number(facts.modified_ms)),
        SortKey::ModifiedByMe => mine.modified_ms.map(SortValue::Number),
        SortKey::OpenedByMe => mine.opened_ms.map(SortValue::Number),
    };
    let key = RowKey {
        folder: false,
        value,
        name: facts.display_name.clone(),
        id: facts.file_id.clone(),
    };
    let row = FileRow {
        file_id: facts.file_id,
        object_id: facts.object_id,
        display_name: facts.display_name,
        media_type: facts.media_type,
        size: facts.size,
        modified: format_rfc3339_ms(facts.modified_ms),
        modified_by_me: mine.modified_ms.map(format_rfc3339_ms),
        opened_by_me: mine.opened_ms.map(format_rfc3339_ms),
    };
    Candidate {
        row: ViewRow::File(row),
        key,
    }
}

/// The checked view-local filter: `None` for none, else the text folded to lower case.
///
/// # Errors
/// `invalid_request` for a filter over [`MAX_FILTER_BYTES`].
pub fn check_filter(filter: Option<&str>) -> Result<Option<String>, FileError> {
    match filter {
        Some(text) if text.len() > MAX_FILTER_BYTES => Err(FileError::invalid(format!(
            "A view filter is at most {MAX_FILTER_BYTES} bytes."
        ))),
        Some(text) if !text.trim().is_empty() => Ok(Some(text.to_lowercase())),
        _ => Ok(None),
    }
}

/// The checked page size.
///
/// # Errors
/// `invalid_request` for a limit outside 1 to [`MAX_VIEW_LIMIT`].
pub fn check_limit(limit: Option<usize>) -> Result<usize, FileError> {
    match limit.unwrap_or(DEFAULT_VIEW_LIMIT) {
        n @ 1..=MAX_VIEW_LIMIT => Ok(n),
        _ => Err(FileError::invalid(format!(
            "A view page holds 1 to {MAX_VIEW_LIMIT} rows."
        ))),
    }
}

/// `invalid_request` unless this release serves `view`.
///
/// # Errors
/// `invalid_request` naming the view for every view but `all` and `deleted`.
pub fn check_view(view: ViewKind) -> Result<(), FileError> {
    match view {
        ViewKind::All | ViewKind::Deleted => Ok(()),
        other => Err(FileError::invalid(format!(
            "The {} view is not available yet; use all or deleted.",
            other.as_str()
        ))),
    }
}

/// True when `row`'s name contains the folded `filter`.
pub fn matches_filter(candidate: &Candidate, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| candidate.key.name.to_lowercase().contains(filter))
}

/// One page of `candidates` for `request`, after its cursor.
///
/// # Errors
/// `invalid_request` for a cursor that is malformed or was made for other parameters.
pub fn page(
    request: &ViewRequest,
    mut candidates: Vec<Candidate>,
    complete: bool,
) -> Result<ViewPage, FileError> {
    let limit = check_limit(request.limit)?;
    let fingerprint = fingerprint(request);
    let order = |a: &RowKey, b: &RowKey| compare(a, b, request.direction, request.folders);
    candidates.sort_by(|a, b| order(&a.key, &b.key));
    let scope = scope_digest(&candidates)?;
    let after = request
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(cursor, &fingerprint, &scope))
        .transpose()?;
    let mut rest = candidates
        .into_iter()
        .skip_while(|c| {
            after
                .as_ref()
                .is_some_and(|k| order(&c.key, k) != Ordering::Greater)
        })
        .peekable();
    let mut rows = Vec::with_capacity(limit);
    let mut last = None;
    while rows.len() < limit {
        let Some(candidate) = rest.next() else {
            break;
        };
        last = Some(candidate.key);
        rows.push(candidate.row);
    }
    let next_cursor = match (rest.peek(), last) {
        (Some(_), Some(key)) => Some(encode_cursor(key, fingerprint, scope)?),
        _ => None,
    };
    Ok(ViewPage {
        node: request.node.clone(),
        view: request.view,
        sort: request.sort,
        direction: request.direction,
        folders: request.folders,
        rows,
        next_cursor,
        complete,
    })
}

/// The order of two row keys: folders first when on top, then present values before missing
/// ones, values in `direction`, then the exact name and the identifier ascending.
fn compare(a: &RowKey, b: &RowKey, direction: SortDirection, folders: FolderPlacement) -> Ordering {
    let group = match folders {
        FolderPlacement::OnTop => b.folder.cmp(&a.folder),
        FolderPlacement::Mixed => Ordering::Equal,
    };
    let values = match (&a.value, &b.value) {
        (Some(x), Some(y)) => match direction {
            SortDirection::Ascending => x.cmp(y),
            SortDirection::Descending => y.cmp(x),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    group
        .then(values)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.id.cmp(&b.id))
}

/// The digest of every parameter but the cursor and the limit.
fn fingerprint(request: &ViewRequest) -> String {
    let text = format!(
        "{}\n{}\n{:?}\n{:?}\n{:?}\n{}",
        request.node,
        request.view.as_str(),
        request.sort,
        request.direction,
        request.folders,
        request.filter.as_deref().unwrap_or_default(),
    );
    DigestAlgorithm::Sha256.digest_hex(text.as_bytes())
}

/// The cursor after `key` for the parameters `fingerprint`.
fn encode_cursor(key: RowKey, fingerprint: String, scope: String) -> Result<String, FileError> {
    let body = CursorBody {
        k: key,
        p: fingerprint,
        s: scope,
    };
    let json = serde_json::to_vec(&body)
        .map_err(|_| FileError::internal("a view cursor could not be encoded"))?;
    let hex: String = json.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{CURSOR_PREFIX}{hex}"))
}

/// The row key `cursor` carries, when it was made for the parameters `fingerprint` over the
/// same rows (`scope`).
///
/// # Errors
/// `invalid_request` for a cursor that is malformed or was made for other parameters;
/// `conflict` when the view's rows or their order changed since the cursor was made, so a
/// row could otherwise repeat or be skipped.
fn decode_cursor(cursor: &str, fingerprint: &str, scope: &str) -> Result<RowKey, FileError> {
    let refused = || FileError::invalid("The cursor is not one this view answered.");
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(refused());
    }
    let hex = cursor.strip_prefix(CURSOR_PREFIX).ok_or_else(refused)?;
    let bytes = decode_hex(hex).ok_or_else(refused)?;
    let body: CursorBody = serde_json::from_slice(&bytes).map_err(|_| refused())?;
    if body.p != fingerprint {
        return Err(FileError::invalid(
            "The cursor belongs to a view with other parameters.",
        ));
    }
    match body.s == scope {
        true => Ok(body.k),
        false => Err(FileError::conflict(
            "The view changed since its first page; start again without a cursor.",
        )),
    }
}

/// The digest of the row keys of `sorted`, in order.
fn scope_digest(sorted: &[Candidate]) -> Result<String, FileError> {
    let keys: Vec<&RowKey> = sorted.iter().map(|c| &c.key).collect();
    let json = serde_json::to_vec(&keys)
        .map_err(|_| FileError::internal("a view's rows could not be digested"))?;
    Ok(DigestAlgorithm::Sha256.digest_hex(&json))
}

/// The bytes of lowercase or uppercase hex `text`, or `None`.
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file candidate named `name` of `size` bytes, modified at `ms`.
    fn file(id: &str, name: &str, size: u64, ms: i64, sort: SortKey) -> Candidate {
        let facts = FileFacts {
            file_id: id.into(),
            object_id: format!("urn:uuid:{id}"),
            display_name: name.into(),
            media_type: "image/jpeg".into(),
            size,
            modified_ms: ms,
        };
        file_candidate(facts, MyTimes::default(), sort)
    }

    /// A folder candidate named `name`.
    fn folder(node: &str, name: &str, sort: SortKey) -> Candidate {
        let row = FolderRow {
            node: node.into(),
            node_kind: FolderKind::Collection,
            name: name.into(),
        };
        folder_candidate(row, sort)
    }

    /// A request over `sort`, `direction` and `folders` with `limit` rows.
    fn request(sort: SortKey, direction: SortDirection, folders: FolderPlacement) -> ViewRequest {
        ViewRequest {
            node: "n".into(),
            view: ViewKind::All,
            sort,
            direction,
            folders,
            filter: None,
            cursor: None,
            limit: Some(2),
        }
    }

    /// The names of the rows of `page`.
    fn names(page: &ViewPage) -> Vec<String> {
        page.rows
            .iter()
            .map(|row| match row {
                ViewRow::Folder(f) => f.name.clone(),
                ViewRow::File(f) => f.display_name.clone(),
            })
            .collect()
    }

    /// Every row of the view, page by page.
    fn all_pages(mut request: ViewRequest, make: impl Fn() -> Vec<Candidate>) -> Vec<String> {
        let mut seen = Vec::new();
        for _ in 0..10 {
            let page = page(&request, make(), true).expect("page");
            seen.extend(names(&page));
            match page.next_cursor {
                Some(cursor) => request.cursor = Some(cursor),
                None => return seen,
            }
        }
        panic!("more pages than rows");
    }

    #[test]
    fn folders_on_top_stay_on_top_in_both_directions_and_mixed_interleaves() {
        let make = || {
            vec![
                file("3", "beta.jpg", 1, 1, SortKey::Name),
                folder("f", "Gamma", SortKey::Name),
                file("1", "alpha.jpg", 1, 1, SortKey::Name),
            ]
        };
        let up = request(
            SortKey::Name,
            SortDirection::Ascending,
            FolderPlacement::OnTop,
        );
        assert_eq!(all_pages(up, make), ["Gamma", "alpha.jpg", "beta.jpg"]);
        let down = request(
            SortKey::Name,
            SortDirection::Descending,
            FolderPlacement::OnTop,
        );
        assert_eq!(all_pages(down, make), ["Gamma", "beta.jpg", "alpha.jpg"]);
        let mixed = request(
            SortKey::Name,
            SortDirection::Ascending,
            FolderPlacement::Mixed,
        );
        assert_eq!(all_pages(mixed, make), ["alpha.jpg", "beta.jpg", "Gamma"]);
    }

    #[test]
    fn missing_values_sort_last_and_ties_break_by_name_then_id() {
        let make = || {
            vec![
                file("2", "same.jpg", 5, 1, SortKey::Size),
                folder("f", "Folder", SortKey::Size),
                file("1", "same.jpg", 5, 1, SortKey::Size),
                file("3", "big.jpg", 9, 1, SortKey::Size),
            ]
        };
        let mixed = request(
            SortKey::Size,
            SortDirection::Descending,
            FolderPlacement::Mixed,
        );
        assert_eq!(
            all_pages(mixed, make),
            ["big.jpg", "same.jpg", "same.jpg", "Folder"]
        );
        let ids: Vec<String> = page(
            &request(
                SortKey::Size,
                SortDirection::Ascending,
                FolderPlacement::Mixed,
            ),
            make(),
            true,
        )
        .expect("page")
        .rows
        .iter()
        .filter_map(|row| match row {
            ViewRow::File(f) => Some(f.file_id.clone()),
            ViewRow::Folder(_) => None,
        })
        .collect();
        assert_eq!(ids, ["1", "2"], "ties break by identifier");
    }

    #[test]
    fn a_cursor_for_other_parameters_or_a_forged_one_is_refused() {
        let make = || {
            vec![
                file("1", "a", 1, 1, SortKey::Name),
                file("2", "b", 1, 1, SortKey::Name),
                file("3", "c", 1, 1, SortKey::Name),
            ]
        };
        let first = request(
            SortKey::Name,
            SortDirection::Ascending,
            FolderPlacement::OnTop,
        );
        let cursor = page(&first, make(), true).expect("page").next_cursor;
        let mut other = request(
            SortKey::Size,
            SortDirection::Ascending,
            FolderPlacement::OnTop,
        );
        other.cursor = cursor;
        assert!(page(&other, make(), true).is_err());
        for forged in ["v1.zz", "v2.00", "v1.7b7d", "x"] {
            let mut bad = first.clone();
            bad.cursor = Some(forged.into());
            assert!(page(&bad, make(), true).is_err(), "{forged}");
        }
    }

    #[test]
    fn a_view_that_changed_between_pages_refuses_its_cursor() {
        let first = request(
            SortKey::Name,
            SortDirection::Ascending,
            FolderPlacement::OnTop,
        );
        let before = || {
            vec![
                file("1", "a", 1, 1, SortKey::Name),
                file("2", "c", 1, 1, SortKey::Name),
                file("3", "e", 1, 1, SortKey::Name),
            ]
        };
        let one = page(&first, before(), true).expect("page");
        let mut next = first.clone();
        next.cursor = one.next_cursor.clone();
        let two = page(&next, before(), true).expect("same rows");
        assert_eq!(names(&one), ["a", "c"]);
        assert_eq!(names(&two), ["e"]);
        let mut renamed = before();
        renamed[0] = file("1", "z", 1, 1, SortKey::Name);
        let err = page(&next, renamed, true).expect_err("the order changed");
        assert_eq!(err.code(), crate::ErrorCode::Conflict);
    }

    #[test]
    fn filters_limits_and_views_are_bounded() {
        assert_eq!(check_filter(Some("  ")).expect("blank"), None);
        assert_eq!(check_filter(Some("JPG")).expect("ok"), Some("jpg".into()));
        assert!(check_filter(Some(&"x".repeat(MAX_FILTER_BYTES + 1))).is_err());
        assert!(check_limit(Some(0)).is_err());
        assert!(check_limit(Some(MAX_VIEW_LIMIT + 1)).is_err());
        assert_eq!(check_limit(None).expect("default"), DEFAULT_VIEW_LIMIT);
        assert!(check_view(ViewKind::All).is_ok() && check_view(ViewKind::Deleted).is_ok());
        assert!(check_view(ViewKind::Favorites).is_err());
        let row = file("1", "Holiday.JPG", 1, 1, SortKey::Name);
        assert!(matches_filter(&row, Some("day.jpg")));
        assert!(!matches_filter(&row, Some("png")));
    }
}
