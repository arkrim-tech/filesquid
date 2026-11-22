//! The files and bytes beneath the folder rows of one page (Task API-F2.21; API-S-011 **Row
//! facts**): for each collection or virtual-collection row, the ordinary files it holds that
//! the caller may read and the bytes of their originals at the head, so a client can say how
//! many files a selected folder holds.
//!
//! The count is made for the rows of the answered page only, after paging, so it never
//! shapes a cursor. It stays within the bounds of one view: all the folder rows of one
//! answer together count at most [`MAX_VIEW_FILES`] files. A row whose count a bound stopped,
//! or that holds a file whose object or record could not be read, says so
//! (`count_complete: false`), and its numbers are then lower bounds. [`folder_files`] reads
//! the host's catalog, which may block, so the route calls it on a blocking thread; the file
//! service then reads each file's object.

use super::drive::{FolderRow, ViewRow, MAX_VIEW_FILES};
use super::drive_scope::{file_scope, read_bounded};
use super::error::FileError;
use crate::auth::Principal;
use crate::library::{FileListing, FileScope, VisibilityFilter};
use crate::navigation::{NavigationService, NodeRef};
use crate::ErrorCode;
use filesquid_core::{FileId, LibraryId};

/// The files beneath one folder row, as the catalog lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderFiles {
    /// The index of the folder row among the rows of the page.
    pub row: usize,
    /// The library of the folder.
    pub library_id: String,
    /// Its ordinary files that the caller may read, in catalog order.
    pub files: Vec<FileId>,
    /// False when the bound of one view left files out, or the folder could not be listed.
    pub complete: bool,
}

/// What was counted beneath one folder row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FolderCount {
    /// The files that were read.
    pub files: u64,
    /// The bytes of their originals at the head.
    pub bytes: u64,
    /// False when a bound or an unreadable file stopped the count.
    pub complete: bool,
}

/// The nodes of the folder rows of `rows`, each with its index among the rows. Pure.
pub fn folder_nodes(rows: &[ViewRow]) -> Vec<(usize, String)> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| match row {
            ViewRow::Folder(folder) => Some((index, folder.node.clone())),
            ViewRow::File(_) => None,
        })
        .collect()
}

/// The ordinary files beneath each of `nodes` that `principal` may read, at most
/// [`MAX_VIEW_FILES`] in all: a folder the bound leaves no room for is answered with no files
/// and an incomplete count. Reads the catalog; may block.
///
/// # Errors
/// `unavailable` or `internal` when the catalog fails.
pub fn folder_files(
    navigation: &NavigationService,
    principal: &Principal,
    nodes: &[(usize, String)],
) -> Result<Vec<FolderFiles>, FileError> {
    let mut room = MAX_VIEW_FILES;
    let mut folders = Vec::with_capacity(nodes.len());
    for (row, handle) in nodes {
        let listed = list_folder(navigation, principal, handle, room)?;
        room = room.saturating_sub(listed.files.len());
        folders.push(FolderFiles {
            row: *row,
            ..listed
        });
    }
    Ok(folders)
}

/// The files of the folder `handle` names, at most `room`. A node that is no collection or
/// virtual collection, or that no longer exists, has no files and an incomplete count.
fn list_folder(
    navigation: &NavigationService,
    principal: &Principal,
    handle: &str,
    room: usize,
) -> Result<FolderFiles, FileError> {
    let unlisted = FolderFiles {
        row: 0,
        library_id: String::new(),
        files: Vec::new(),
        complete: false,
    };
    let catalog = navigation.catalog().as_ref();
    let Some(node) = NodeRef::from_handle(handle) else {
        return Ok(unlisted);
    };
    let Some((library_id, scope)) = listable(file_scope(catalog, &node))? else {
        return Ok(unlisted);
    };
    let listing = FileListing {
        library_id: library_id.clone(),
        scope,
        visibility: VisibilityFilter::Ordinary,
    };
    let read = |after: Option<&_>, limit| catalog.files(&listing, after, limit);
    let mut entries = read_bounded(read, room + 1)?;
    let complete = entries.len() <= room;
    entries.truncate(room);
    let files = entries
        .into_iter()
        .map(|keyed| keyed.item.file)
        .filter(|file| {
            let home = NodeRef::Collection(file.home_collection_id().clone());
            navigation.may_read(principal, &home)
        })
        .map(|file| file.id().clone())
        .collect();
    Ok(FolderFiles {
        library_id: library_id.as_str().to_string(),
        files,
        complete,
        ..unlisted
    })
}

/// The library and scope of a folder row, `None` for a node that no longer exists or is no
/// collection or virtual collection (its count is then incomplete), or the catalog failure,
/// which fails the answer as it would any other read. Pure.
fn listable(
    scope: Result<(LibraryId, FileScope), FileError>,
) -> Result<Option<(LibraryId, FileScope)>, FileError> {
    match scope {
        Ok(found) => Ok(Some(found)),
        Err(err) if matches!(err.code(), ErrorCode::NotFound | ErrorCode::InvalidRequest) => {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

/// Write `count` into the folder row at `row` of `rows`; any other row is left as it is.
/// Pure.
pub fn apply_count(rows: &mut [ViewRow], row: usize, count: FolderCount) {
    if let Some(ViewRow::Folder(folder)) = rows.get_mut(row) {
        set_count(folder, count);
    }
}

/// Write `count` into `folder`. Pure.
fn set_count(folder: &mut FolderRow, count: FolderCount) {
    folder.files = count.files;
    folder.bytes = count.bytes;
    folder.count_complete = count.complete;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::drive::{FolderKind, FolderRow};

    #[test]
    fn a_missing_or_other_node_is_unlisted_but_a_catalog_failure_fails_the_count() {
        let scope = || (LibraryId::new("lib").expect("id"), FileScope::Library);
        assert!(listable(Ok(scope())).expect("ok").is_some());
        assert!(listable(Err(FileError::not_found()))
            .expect("unlisted")
            .is_none());
        assert!(listable(Err(FileError::invalid("other kind")))
            .expect("unlisted")
            .is_none());
        let down = listable(Err(FileError::unavailable("catalog down"))).expect_err("fails");
        assert_eq!(down.code(), crate::ErrorCode::Unavailable);
        let broken = listable(Err(FileError::internal("catalog broken"))).expect_err("fails");
        assert_eq!(broken.code(), crate::ErrorCode::Internal);
    }

    #[test]
    fn only_folder_rows_are_counted_and_a_count_lands_on_its_own_row() {
        let folder = |name: &str| {
            ViewRow::Folder(FolderRow::uncounted(
                format!("node-{name}"),
                FolderKind::Collection,
                name.into(),
            ))
        };
        let mut rows = vec![folder("a"), folder("b")];
        assert_eq!(
            folder_nodes(&rows),
            [(0, "node-a".to_string()), (1, "node-b".to_string())]
        );
        let count = FolderCount {
            files: 3,
            bytes: 3_000,
            complete: true,
        };
        apply_count(&mut rows, 1, count);
        apply_count(&mut rows, 9, count);
        let ViewRow::Folder(first) = &rows[0] else {
            panic!("a folder row");
        };
        assert_eq!((first.files, first.count_complete), (0, false));
        let ViewRow::Folder(second) = &rows[1] else {
            panic!("a folder row");
        };
        assert_eq!(
            (second.files, second.bytes, second.count_complete),
            (3, 3_000, true)
        );
    }
}
