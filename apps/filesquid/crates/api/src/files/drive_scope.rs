//! What one view covers (Task API-F2.2; API-S-011, API-S-019, SRS-F-062): the files and
//! folder rows of a node, from the host's [`LibraryCatalog`], and the times the caller last
//! changed and opened each file, from the caller's own journal entries.
//!
//! Both read host ports that may block, so the view route calls them on a blocking thread.
//! Each read is bounded: at most [`MAX_VIEW_FILES`] files (one more is read to learn that the
//! scope is larger), [`MAX_VIEW_FOLDERS`] folder rows and [`MAX_JOURNAL_PAGES`] journal pages.
//! The journal is filtered by actor before anything is kept, so another user's activity
//! never shapes the caller's order (SRS-F-062).

use super::drive::{DeletedRow, FolderKind, FolderRow, MyTimes, ViewKind, MAX_VIEW_FILES};
use super::error::FileError;
use super::nodes::file_error;
use super::search_scope::MAX_SEARCH_LIBRARIES;
use super::MAX_SIZE_OBJECTS;
use super::{
    COPY_OPERATION, METADATA_REMOVE_OPERATION, METADATA_SET_OPERATION, MOVE_OPERATION,
    REMOVE_OPERATION, RENAME_OPERATION, RESTORE_OPERATION, UPLOAD_OPERATION,
};
use crate::auth::Principal;
use crate::home::HomeCollections;
use crate::journal::format_rfc3339_ms;
use crate::journal::{JournalQuery, JournalStore, Outcome, MAX_PAGE};
use crate::library::{
    CatalogError, FileEntry, FileListing, FileScope, Keyed, LibraryCatalog, PageKey,
    VisibilityFilter,
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

/// The files of one library a view covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFiles {
    /// The library.
    pub library_id: String,
    /// The files, in catalog order.
    pub files: Vec<FileId>,
    /// For a deleted view, what Recently deleted shows of each deleted file, by file ID.
    pub deleted: HashMap<String, DeletedRow>,
}

/// The files and folder rows one view covers: one library's files for a library, collection
/// or virtual-collection node, or the deleted files of every readable library for the
/// repository's Recently deleted node (ADR 041 Decision 8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewScope {
    /// The files, grouped by library, in catalog order.
    pub libraries: Vec<LibraryFiles>,
    /// The folder rows.
    pub folders: Vec<FolderRow>,
    /// False when the node held more files or folders than the bounds.
    pub complete: bool,
    /// True for the Recently deleted node, whose files come from many libraries: a library
    /// the file service does not serve makes the view incomplete instead of failing it.
    pub across_libraries: bool,
}

/// The files and folder rows `view` of `node` covers for `principal`. `home`, when the host
/// keeps home collections, names each deleted file's folder. May block.
///
/// # Errors
/// `not_found` for a node that is unknown or unreadable; `invalid_request` for a node that is
/// not a library, collection, virtual collection or the Recently deleted node;
/// `unavailable` or `internal` when the catalog fails.
pub fn view_scope(
    navigation: &NavigationService,
    principal: &Principal,
    node: &NodeRef,
    view: ViewKind,
    home: Option<&dyn HomeCollections>,
) -> Result<ViewScope, FileError> {
    navigation
        .check_readable(principal, node)
        .map_err(file_error)?;
    if let NodeRef::RecentlyDeleted(_) = node {
        return recently_deleted_scope(navigation, principal, home);
    }
    let catalog = navigation.catalog().as_ref();
    let (library_id, scope) = file_scope(catalog, node)?;
    let library_root = matches!(node, NodeRef::Library(_)) && view == ViewKind::All;
    let (folders, folders_complete) = match library_root {
        true => folder_rows(navigation, principal, &library_id)?,
        false => (Vec::new(), true),
    };
    let deleted_view = view == ViewKind::Deleted;
    let visibility = match deleted_view {
        true => VisibilityFilter::Deleted,
        false => VisibilityFilter::Ordinary,
    };
    let listing = FileListing {
        library_id: library_id.clone(),
        scope,
        visibility,
    };
    // A library node's own files are its collections' files: its `all` view shows only its
    // collections and virtual collections as folder rows (no file is ever unfiled).
    let (entries, files_complete) = match library_root {
        true => (Vec::new(), true),
        false => list_files(catalog, &listing, MAX_VIEW_FILES)?,
    };
    let library = library_files(catalog, &library_id, entries, deleted_view, home)?;
    Ok(ViewScope {
        libraries: vec![library],
        folders,
        complete: files_complete && folders_complete,
        across_libraries: false,
    })
}

/// The deleted files of every library of the repository that `principal` may read, at most
/// [`MAX_VIEW_FILES`] in all and [`MAX_SEARCH_LIBRARIES`] libraries (Recently deleted, ADR 041
/// Decision 8 as amended 2026-10-23). May block.
fn recently_deleted_scope(
    navigation: &NavigationService,
    principal: &Principal,
    home: Option<&dyn HomeCollections>,
) -> Result<ViewScope, FileError> {
    let catalog = navigation.catalog().as_ref();
    let repository = navigation.repository_id();
    let read = |after: Option<&PageKey>, limit| catalog.libraries(repository, after, limit);
    let rows = read_bounded(read, MAX_SEARCH_LIBRARIES + 1)?;
    let mut complete = rows.len() <= MAX_SEARCH_LIBRARIES;
    let mut libraries = Vec::new();
    let mut room = MAX_VIEW_FILES;
    for row in rows.into_iter().take(MAX_SEARCH_LIBRARIES) {
        let library_id = row.item.library.id().clone();
        if !navigation.may_read(principal, &NodeRef::Library(library_id.clone())) {
            continue;
        }
        let listing = FileListing {
            library_id: library_id.clone(),
            scope: FileScope::Library,
            visibility: VisibilityFilter::Deleted,
        };
        let (entries, all) = list_files(catalog, &listing, room)?;
        complete &= all;
        room = room.saturating_sub(entries.len());
        let readable = readable_entries(navigation, principal, entries);
        libraries.push(library_files(catalog, &library_id, readable, true, home)?);
        if room == 0 {
            complete = false;
            break;
        }
    }
    Ok(ViewScope {
        libraries,
        folders: Vec::new(),
        complete,
        across_libraries: true,
    })
}

/// The entries whose home collection `principal` may read; a file whose home collection was
/// deleted is guarded by its library, which the caller already may read.
fn readable_entries(
    navigation: &NavigationService,
    principal: &Principal,
    entries: Vec<FileEntry>,
) -> Vec<FileEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            entry.removed_home_title.is_some()
                || navigation.may_read(
                    principal,
                    &NodeRef::Collection(entry.file.home_collection_id().clone()),
                )
        })
        .collect()
}

/// The files of `entries` of `library_id`, with the Recently deleted facts of each deleted
/// one when `deleted_view`. Reads the library's name and each home collection's title.
fn library_files(
    catalog: &dyn LibraryCatalog,
    library_id: &LibraryId,
    entries: Vec<FileEntry>,
    deleted_view: bool,
    home: Option<&dyn HomeCollections>,
) -> Result<LibraryFiles, FileError> {
    let mut deleted = HashMap::new();
    if deleted_view && !entries.is_empty() {
        let context = DeletedContext::read(catalog, library_id, home)?;
        for entry in &entries {
            if let Some(row) = context.row(entry)? {
                deleted.insert(entry.file.id().as_str().to_string(), row);
            }
        }
    }
    Ok(LibraryFiles {
        library_id: library_id.as_str().to_string(),
        files: entries.into_iter().map(|e| e.file.id().clone()).collect(),
        deleted,
    })
}

/// What the Recently deleted facts of one library's files are read from.
struct DeletedContext<'a> {
    /// The catalog, for home collection titles.
    catalog: &'a dyn LibraryCatalog,
    /// The library.
    library_id: LibraryId,
    /// Its name.
    library_name: String,
    /// The host's home collections, for each file's folder.
    home: Option<&'a dyn HomeCollections>,
    /// Day count from now, for days left.
    now_ms: i64,
}

impl<'a> DeletedContext<'a> {
    /// The context of `library_id`. Reads the library record.
    fn read(
        catalog: &'a dyn LibraryCatalog,
        library_id: &LibraryId,
        home: Option<&'a dyn HomeCollections>,
    ) -> Result<Self, FileError> {
        let state = catalog
            .library(library_id)
            .map_err(catalog_error)?
            .ok_or_else(FileError::not_found)?;
        Ok(Self {
            catalog,
            library_id: library_id.clone(),
            library_name: state.record.name,
            home,
            now_ms: crate::journal::now_ms(),
        })
    }

    /// The Recently deleted row facts of `entry`, or `None` when it is not deleted.
    fn row(&self, entry: &FileEntry) -> Result<Option<DeletedRow>, FileError> {
        let Some(deletion) = entry.state.deletion else {
            return Ok(None);
        };
        let home_id = entry.file.home_collection_id();
        let live = self
            .catalog
            .collection(home_id)
            .map_err(catalog_error)?
            .filter(|c| c.library_id() == &self.library_id)
            .map(|c| c.title().to_string());
        let folder = match self.home {
            Some(home) => home
                .placement(&self.library_id, entry.file.id())
                .map_err(catalog_error)?
                .and_then(|(_, path)| folder_of(&path)),
            None => None,
        };
        Ok(Some(DeletedRow {
            library_id: self.library_id.as_str().to_string(),
            library_name: self.library_name.clone(),
            deleted_at: format_rfc3339_ms(deletion.deleted_at_ms),
            purge_after: format_rfc3339_ms(deletion.purge_after_ms),
            days_left: days_left(deletion.purge_after_ms, self.now_ms),
            cause: deletion.cause,
            home_collection_id: home_id.as_str().to_string(),
            home_collection_deleted: live.is_none(),
            home_collection_title: live.or_else(|| entry.removed_home_title.clone()),
            folder,
        }))
    }
}

/// The folder part of a collection logical path, `None` at the collection's root. Pure.
fn folder_of(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(folder, _)| folder.to_string())
}

/// Whole days left from `now_ms` until `purge_after_ms`, counting a part day as a day; `0`
/// once it is due (a file of a library that was closed at its time is expired by the next
/// run after the library is opened). Pure.
pub fn days_left(purge_after_ms: i64, now_ms: i64) -> i64 {
    const DAY_MS: i64 = 86_400_000;
    let left = purge_after_ms.saturating_sub(now_ms);
    match left > 0 {
        true => left.saturating_add(DAY_MS - 1) / DAY_MS,
        false => 0,
    }
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
    let (library_id, scope) = file_scope(catalog, node)?;
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

/// The library of `node` and the catalog scope it lists.
fn file_scope(
    catalog: &dyn LibraryCatalog,
    node: &NodeRef,
) -> Result<(LibraryId, FileScope), FileError> {
    match node {
        NodeRef::Library(id) => Ok((id.clone(), FileScope::Library)),
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

/// The entries of `listing`, at most `bound`, and whether that was all of them.
fn list_files(
    catalog: &dyn LibraryCatalog,
    listing: &FileListing,
    bound: usize,
) -> Result<(Vec<FileEntry>, bool), FileError> {
    let read = |after: Option<&PageKey>, limit| catalog.files(listing, after, limit);
    let entries = read_bounded(read, bound + 1)?;
    let complete = entries.len() <= bound;
    let files = entries
        .into_iter()
        .take(bound)
        .map(|keyed| keyed.item)
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
