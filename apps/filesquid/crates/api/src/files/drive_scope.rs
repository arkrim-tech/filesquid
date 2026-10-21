//! What one view covers (Task API-F2.2; API-S-011, API-S-019, SRS-F-062): the files and
//! folder rows of a node, from the host's [`LibraryCatalog`], and the times the caller last
//! changed and opened each file, from the caller's own journal entries.
//!
//! Both read host ports that may block, so the view route calls them on a blocking thread.
//! Each read is bounded: at most [`MAX_VIEW_FILES`] files (one more is read to learn that the
//! scope is larger), [`MAX_VIEW_FOLDERS`] folder rows and [`MAX_JOURNAL_PAGES`] journal pages.
//! The journal is filtered by actor before anything is kept, so another user's activity
//! never shapes the caller's order (SRS-F-062).

use super::drive::{FolderKind, FolderRow, MyTimes, ViewKind, MAX_VIEW_FILES};
use super::error::FileError;
use super::nodes::file_error;
use super::MAX_SIZE_OBJECTS;
use super::{
    COPY_OPERATION, METADATA_REMOVE_OPERATION, METADATA_SET_OPERATION, MOVE_OPERATION,
    REMOVE_OPERATION, RENAME_OPERATION, RESTORE_OPERATION, UPLOAD_OPERATION,
};
use crate::auth::Principal;
use crate::journal::{JournalQuery, JournalStore, Outcome, MAX_PAGE};
use crate::library::{
    CatalogError, FileListing, FileScope, Keyed, LibraryCatalog, PageKey, VisibilityFilter,
};
use crate::navigation::{NavigationService, NodeRef};
use filesquid_core::{FileId, LibraryId};
use std::collections::HashMap;

/// Most folder rows (collections and virtual collections) one library view shows.
pub const MAX_VIEW_FOLDERS: usize = 1_000;

/// Most collection and virtual-collection rows one library view examines for its folder rows.
pub const MAX_FOLDER_SCAN: usize = 50_000;

/// Most journal pages read for the caller's own times.
pub const MAX_JOURNAL_PAGES: usize = 20;

/// The journal operation of a single-file download (API-S-028).
pub const FILE_DOWNLOAD_OPERATION: &str = "file.download";

/// Operations that change a file: they set "modified by me".
const MODIFYING: [&str; 8] = [
    UPLOAD_OPERATION,
    MOVE_OPERATION,
    REMOVE_OPERATION,
    RESTORE_OPERATION,
    RENAME_OPERATION,
    METADATA_SET_OPERATION,
    METADATA_REMOVE_OPERATION,
    COPY_OPERATION,
];

/// Operations that open a file: they set "opened by me".
const OPENING: [&str; 2] = [crate::content::ISSUED_OPERATION, FILE_DOWNLOAD_OPERATION];

/// The files and folder rows one view covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewScope {
    /// The library the node belongs to.
    pub library_id: String,
    /// The files, in catalog order.
    pub files: Vec<FileId>,
    /// The folder rows.
    pub folders: Vec<FolderRow>,
    /// False when the node held more files or folders than the bounds.
    pub complete: bool,
}

/// The files and folder rows `view` of `node` covers for `principal`. May block.
///
/// # Errors
/// `not_found` for a node that is unknown or unreadable; `invalid_request` for a node that is
/// not a library, collection or virtual collection; `unavailable` or `internal` when the
/// catalog fails.
pub fn view_scope(
    navigation: &NavigationService,
    principal: &Principal,
    node: &NodeRef,
    view: ViewKind,
) -> Result<ViewScope, FileError> {
    navigation
        .check_readable(principal, node)
        .map_err(file_error)?;
    let catalog = navigation.catalog().as_ref();
    let (library_id, scope) = file_scope(catalog, node, view)?;
    let visibility = match view {
        ViewKind::Deleted => VisibilityFilter::Deleted,
        _ => VisibilityFilter::Ordinary,
    };
    let listing = FileListing {
        library_id: library_id.clone(),
        scope,
        visibility,
    };
    let (files, files_complete) = list_files(catalog, &listing)?;
    let library_root = matches!(node, NodeRef::Library(_)) && view == ViewKind::All;
    let (folders, folders_complete) = match library_root {
        true => folder_rows(navigation, principal, &library_id)?,
        false => (Vec::new(), true),
    };
    Ok(ViewScope {
        library_id: library_id.as_str().to_string(),
        files,
        folders,
        complete: files_complete && folders_complete,
    })
}

/// The files whose home is the collection `node`, hidden and deleted ones included, at most
/// [`MAX_SIZE_OBJECTS`], with the collection's library and whether that was all of them.
/// May block.
///
/// # Errors
/// `not_found` for a collection that is unknown or unreadable; `unavailable` or `internal`
/// when the catalog fails.
pub fn collection_files(
    navigation: &NavigationService,
    principal: &Principal,
    node: &NodeRef,
) -> Result<(String, Vec<FileId>, bool), FileError> {
    navigation
        .check_readable(principal, node)
        .map_err(file_error)?;
    let catalog = navigation.catalog().as_ref();
    let (library_id, scope) = file_scope(catalog, node, ViewKind::All)?;
    let mut files = Vec::new();
    let mut complete = true;
    for visibility in [
        VisibilityFilter::Ordinary,
        VisibilityFilter::Hidden,
        VisibilityFilter::Deleted,
    ] {
        let listing = FileListing {
            library_id: library_id.clone(),
            scope: scope.clone(),
            visibility,
        };
        let room = MAX_SIZE_OBJECTS + 1 - files.len();
        let read = |after: Option<&PageKey>, limit| catalog.files(&listing, after, limit);
        let entries = read_bounded(read, room)?;
        files.extend(
            entries
                .into_iter()
                .map(|keyed| keyed.item.file.id().clone()),
        );
        if files.len() > MAX_SIZE_OBJECTS {
            files.truncate(MAX_SIZE_OBJECTS);
            complete = false;
            break;
        }
    }
    Ok((library_id.as_str().to_string(), files, complete))
}

/// The latest time `actor` changed and opened each file of `library_id`, by object ID, from
/// at most [`MAX_JOURNAL_PAGES`] pages of the actor's own succeeded entries. May block.
///
/// # Errors
/// `unavailable` when the journal cannot be read.
pub fn my_times(
    store: &dyn JournalStore,
    actor: &str,
    library_id: &str,
) -> Result<HashMap<String, MyTimes>, FileError> {
    let mut query = JournalQuery {
        actor: Some(actor.to_string()),
        library_id: Some(library_id.to_string()),
        outcome: Some(Outcome::Succeeded),
        descending: true,
        limit: MAX_PAGE,
        ..JournalQuery::default()
    };
    let mut times: HashMap<String, MyTimes> = HashMap::new();
    for _ in 0..MAX_JOURNAL_PAGES {
        let page = store
            .query(&query)
            .map_err(|_| FileError::unavailable("the journal cannot be read"))?;
        for entry in &page.entries {
            record_time(
                &mut times,
                &entry.operation,
                &entry.target_ids,
                entry.time_ms,
            );
        }
        match page.next_cursor {
            Some(next) => query.after_id = Some(next),
            None => break,
        }
    }
    Ok(times)
}

/// Keep `time` as the caller's latest change or opening of the file `targets` names. Pure.
fn record_time(
    times: &mut HashMap<String, MyTimes>,
    operation: &str,
    targets: &[String],
    time: i64,
) {
    let modifying = MODIFYING.contains(&operation);
    let opening = OPENING.contains(&operation);
    let object = targets.iter().find(|t| t.starts_with("urn:uuid:"));
    let (Some(object), true) = (object, modifying || opening) else {
        return;
    };
    let mine = times.entry(object.clone()).or_default();
    let slot = match modifying {
        true => &mut mine.modified_ms,
        false => &mut mine.opened_ms,
    };
    *slot = Some(slot.map_or(time, |kept| kept.max(time)));
}

/// The library of `node` and the catalog scope `view` lists there.
fn file_scope(
    catalog: &dyn LibraryCatalog,
    node: &NodeRef,
    view: ViewKind,
) -> Result<(LibraryId, FileScope), FileError> {
    match node {
        NodeRef::Library(id) => {
            let scope = match view {
                ViewKind::All => FileScope::Unfiled,
                _ => FileScope::Library,
            };
            Ok((id.clone(), scope))
        }
        NodeRef::Collection(id) => {
            let collection = catalog
                .collection(id)
                .map_err(catalog_error)?
                .ok_or_else(FileError::not_found)?;
            let scope = FileScope::Collection {
                collection_id: id.clone(),
            };
            Ok((collection.library_id().clone(), scope))
        }
        NodeRef::VirtualCollection(id) => {
            let virtual_collection = catalog
                .virtual_collection(id)
                .map_err(catalog_error)?
                .ok_or_else(FileError::not_found)?;
            let scope = FileScope::VirtualCollection {
                virtual_collection_id: id.clone(),
            };
            Ok((virtual_collection.library_id().clone(), scope))
        }
        _ => Err(FileError::invalid(
            "A view lists a library, collection or virtual collection node.",
        )),
    }
}

/// The files of `listing`, at most [`MAX_VIEW_FILES`], and whether that was all of them.
fn list_files(
    catalog: &dyn LibraryCatalog,
    listing: &FileListing,
) -> Result<(Vec<FileId>, bool), FileError> {
    let read = |after: Option<&PageKey>, limit| catalog.files(listing, after, limit);
    let entries = read_bounded(read, MAX_VIEW_FILES + 1)?;
    let complete = entries.len() <= MAX_VIEW_FILES;
    let files = entries
        .into_iter()
        .take(MAX_VIEW_FILES)
        .map(|keyed| keyed.item.file.id().clone())
        .collect();
    Ok((files, complete))
}

/// The collections and virtual collections of `library_id` that `principal` may read, as
/// folder rows, at most [`MAX_VIEW_FOLDERS`], and whether that was all of them. Each listing
/// is paged until the readable rows pass the bound or the listing ends, examining at most
/// [`MAX_FOLDER_SCAN`] rows; stopping early for either bound reports the rows incomplete.
pub(super) fn folder_rows(
    navigation: &NavigationService,
    principal: &Principal,
    library_id: &LibraryId,
) -> Result<(Vec<FolderRow>, bool), FileError> {
    let catalog = navigation.catalog().as_ref();
    let mut rows = Vec::new();
    let collections = |after: Option<&PageKey>| {
        let page = catalog.collections(library_id, after, MAX_PAGE)?;
        Ok(page
            .into_iter()
            .map(|c| {
                let node = NodeRef::Collection(c.item.id().clone());
                let row = (node, FolderKind::Collection, c.item.title().to_string());
                (c.key, row)
            })
            .collect())
    };
    let virtuals = |after: Option<&PageKey>| {
        let page = catalog.virtual_collections(library_id, after, MAX_PAGE)?;
        Ok(page
            .into_iter()
            .map(|v| {
                let node = NodeRef::VirtualCollection(v.item.id().clone());
                let row = (
                    node,
                    FolderKind::VirtualCollection,
                    v.item.title().to_string(),
                );
                (v.key, row)
            })
            .collect())
    };
    let readable = |node: &NodeRef| navigation.may_read(principal, node);
    let complete_collections = collect_folders(collections, readable, &mut rows)?;
    let complete_virtuals = complete_collections && collect_folders(virtuals, readable, &mut rows)?;
    let complete = complete_virtuals && rows.len() <= MAX_VIEW_FOLDERS;
    rows.truncate(MAX_VIEW_FOLDERS);
    Ok((rows, complete))
}

/// One candidate folder row: its node, kind and name.
type FolderNode = (NodeRef, FolderKind, String);

/// Add the readable rows of one listing read through `read` to `rows`, until `rows` passes
/// [`MAX_VIEW_FOLDERS`] or the listing ends; false when a bound stopped it first.
fn collect_folders(
    read: impl Fn(Option<&PageKey>) -> Result<Vec<(PageKey, FolderNode)>, CatalogError>,
    readable: impl Fn(&NodeRef) -> bool,
    rows: &mut Vec<FolderRow>,
) -> Result<bool, FileError> {
    let mut after: Option<PageKey> = None;
    for _ in 0..MAX_FOLDER_SCAN / MAX_PAGE {
        let page = read(after.as_ref()).map_err(catalog_error)?;
        let full = page.len() == MAX_PAGE;
        for (key, (node, node_kind, name)) in page {
            after = Some(key);
            let handle = readable(&node).then(|| node.to_handle()).flatten();
            if let Some(handle) = handle {
                rows.push(FolderRow {
                    node: handle,
                    node_kind,
                    name,
                });
            }
        }
        if rows.len() > MAX_VIEW_FOLDERS {
            return Ok(false);
        }
        if !full {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Up to `bound` items of a listing read page by page through `read`, which takes the
/// position to start after and the most items to answer: each page asks for no more than
/// the items still within `bound`, at most [`MAX_PAGE`].
pub(super) fn read_bounded<T>(
    read: impl Fn(Option<&PageKey>, usize) -> Result<Vec<Keyed<T>>, CatalogError>,
    bound: usize,
) -> Result<Vec<Keyed<T>>, FileError> {
    let mut items: Vec<Keyed<T>> = Vec::new();
    for _ in 0..=bound / MAX_PAGE {
        let wanted = bound.saturating_sub(items.len()).min(MAX_PAGE);
        if wanted == 0 {
            break;
        }
        let after = items.last().map(|keyed| keyed.key.clone());
        let page = read(after.as_ref(), wanted).map_err(catalog_error)?;
        let full = page.len() >= wanted;
        items.extend(page);
        if !full {
            break;
        }
    }
    items.truncate(bound);
    Ok(items)
}

/// The file error of a catalog failure.
pub(super) fn catalog_error(err: CatalogError) -> FileError {
    match err {
        CatalogError::Unavailable(_) => {
            FileError::unavailable("the library catalog is unavailable")
        }
        CatalogError::Conflict | CatalogError::Failed(_) => {
            FileError::internal("the library catalog failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_page_asks_only_for_what_the_bound_leaves() {
        let asked = std::cell::RefCell::new(Vec::new());
        let read = |after: Option<&PageKey>, limit: usize| {
            asked.borrow_mut().push(limit);
            let start = after.map_or(0, |key| key.order + 1);
            Ok((start..start + limit as i64)
                .map(|order| Keyed {
                    key: PageKey {
                        order,
                        id: order.to_string(),
                    },
                    item: (),
                })
                .collect())
        };
        let items = read_bounded(read, MAX_PAGE + 1).expect("items");
        assert_eq!(items.len(), MAX_PAGE + 1);
        assert_eq!(
            *asked.borrow(),
            [MAX_PAGE, 1],
            "the last page asks for one entry"
        );
    }

    #[test]
    fn only_changes_and_openings_count_and_the_latest_wins() {
        let mut times = HashMap::new();
        let file = vec![
            "lib".to_string(),
            "urn:uuid:a".to_string(),
            "v2".to_string(),
        ];
        record_time(&mut times, RENAME_OPERATION, &file, 5);
        record_time(&mut times, RENAME_OPERATION, &file, 3);
        record_time(&mut times, FILE_DOWNLOAD_OPERATION, &file, 4);
        record_time(&mut times, "files.label", &file, 9);
        record_time(&mut times, UPLOAD_OPERATION, &["lib".to_string()], 9);
        assert_eq!(
            times.get("urn:uuid:a"),
            Some(&MyTimes {
                modified_ms: Some(5),
                opened_ms: Some(4)
            })
        );
        assert_eq!(times.len(), 1);
    }
}
