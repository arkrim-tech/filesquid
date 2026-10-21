//! What one search covers (Task API-F2.3; API-S-012, SRS-F-063, SRS-NFR-004): the libraries,
//! files and folder rows a caller may read, from the host's [`LibraryCatalog`], each file
//! with its home collection's title and its delete state.
//!
//! Permission filtering comes first: a library, collection or virtual collection the caller
//! may not read, and every file whose home collection (or, when unfiled, library) the caller
//! may not read, is left out here, before any file is read or ranked, so neither results nor
//! scores reveal it. Every read is bounded: at most [`MAX_SEARCH_LIBRARIES`] libraries and
//! [`MAX_SEARCH_FILES`] files in all (one more is read to learn that the scope is larger).
//! Reads the catalog, which may block; call it on a blocking thread.

use super::drive::FolderRow;
use super::drive_scope::{catalog_error, folder_rows, read_bounded};
use super::error::FileError;
use super::nodes::file_error;
use crate::auth::Principal;
use crate::journal::MAX_PAGE;
use crate::library::{FileEntry, FileListing, FileScope, Keyed, LibraryCatalog, VisibilityFilter};
use crate::navigation::{NavigationService, NodeRef};
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::collections::HashMap;

/// Most files one search examines.
pub const MAX_SEARCH_FILES: usize = 10_000;

/// Most libraries one search covers.
pub const MAX_SEARCH_LIBRARIES: usize = 1_000;

/// One file a search examines, as the catalog places it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedFile {
    /// The file.
    pub file_id: FileId,
    /// Its home collection's title, or `None` when it is unfiled.
    pub collection: Option<String>,
    /// True when it is recoverably deleted.
    pub deleted: bool,
}

/// The files and folder rows of one library a search covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFiles {
    /// The library.
    pub library_id: String,
    /// Its files the caller may read.
    pub files: Vec<ScopedFile>,
    /// Its collections and virtual collections the caller may read.
    pub folders: Vec<FolderRow>,
}

/// Everything one search covers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchScope {
    /// The libraries, in catalog order.
    pub libraries: Vec<LibraryFiles>,
    /// False when a bound left libraries, files or folder rows out.
    pub complete: bool,
}

/// Where one search looks: a library's files in one catalog scope, with or without its
/// folder rows.
struct Target {
    library_id: LibraryId,
    scope: FileScope,
    folders: bool,
}

/// What `node` (the repository when `None`) lets `principal` search, in its ordinary or, with
/// `deleted`, its deleted files.
///
/// # Errors
/// `not_found` for a node that is unknown or unreadable; `invalid_request` for a folder
/// node; `unavailable` or `internal` when the catalog fails.
pub fn search_scope(
    navigation: &NavigationService,
    principal: &Principal,
    node: Option<&NodeRef>,
    deleted: bool,
) -> Result<SearchScope, FileError> {
    if let Some(node) = node {
        navigation
            .check_readable(principal, node)
            .map_err(file_error)?;
    }
    let catalog = navigation.catalog().as_ref();
    let (targets, mut complete) = targets(navigation, principal, node, deleted)?;
    let visibility = match deleted {
        true => VisibilityFilter::Deleted,
        false => VisibilityFilter::Ordinary,
    };
    let mut scope = SearchScope::default();
    let mut room = MAX_SEARCH_FILES + 1;
    for target in targets {
        let listing = FileListing {
            library_id: target.library_id.clone(),
            scope: target.scope,
            visibility,
        };
        let entries = read_bounded(|after| catalog.files(&listing, after, MAX_PAGE), room)?;
        room = room.saturating_sub(entries.len());
        let files = readable_files(navigation, principal, &target.library_id, entries, deleted)?;
        let (folders, folders_complete) = match target.folders {
            true => folder_rows(navigation, principal, &target.library_id)?,
            false => (Vec::new(), true),
        };
        complete &= folders_complete;
        scope.libraries.push(LibraryFiles {
            library_id: target.library_id.as_str().to_string(),
            files,
            folders,
        });
        if room == 0 {
            complete = false;
            break;
        }
    }
    let examined: usize = scope.libraries.iter().map(|l| l.files.len()).sum();
    if examined > MAX_SEARCH_FILES {
        trim_files(&mut scope);
        complete = false;
    }
    scope.complete = complete;
    Ok(scope)
}

/// The library scopes `node` names for `principal`, and whether every readable library was
/// taken.
fn targets(
    navigation: &NavigationService,
    principal: &Principal,
    node: Option<&NodeRef>,
    deleted: bool,
) -> Result<(Vec<Target>, bool), FileError> {
    let catalog = navigation.catalog().as_ref();
    let whole = |library_id: LibraryId| Target {
        library_id,
        scope: FileScope::Library,
        folders: !deleted,
    };
    match node {
        None | Some(NodeRef::Repository(_)) => {
            let repository = navigation.repository_id();
            let read = |after: Option<&_>| catalog.libraries(repository, after, MAX_PAGE);
            let libraries = read_bounded(read, MAX_SEARCH_LIBRARIES + 1)?;
            let complete = libraries.len() <= MAX_SEARCH_LIBRARIES;
            let targets = libraries
                .into_iter()
                .take(MAX_SEARCH_LIBRARIES)
                .map(|keyed| keyed.item.library.id().clone())
                .filter(|id| navigation.may_read(principal, &NodeRef::Library(id.clone())))
                .map(whole)
                .collect();
            Ok((targets, complete))
        }
        Some(NodeRef::Library(id)) => Ok((vec![whole(id.clone())], true)),
        Some(NodeRef::Collection(id)) => {
            let collection = catalog
                .collection(id)
                .map_err(catalog_error)?
                .ok_or_else(FileError::not_found)?;
            let target = Target {
                library_id: collection.library_id().clone(),
                scope: FileScope::Collection {
                    collection_id: id.clone(),
                },
                folders: false,
            };
            Ok((vec![target], true))
        }
        Some(NodeRef::VirtualCollection(id)) => {
            let virtual_collection = catalog
                .virtual_collection(id)
                .map_err(catalog_error)?
                .ok_or_else(FileError::not_found)?;
            let target = Target {
                library_id: virtual_collection.library_id().clone(),
                scope: FileScope::VirtualCollection {
                    virtual_collection_id: id.clone(),
                },
                folders: false,
            };
            Ok((vec![target], true))
        }
        Some(NodeRef::Folder { .. }) => Err(FileError::invalid(
            "A search covers the repository, a library, a collection or a virtual collection.",
        )),
    }
}

/// The files of `entries` that `principal` may read, each with its home collection's title.
fn readable_files(
    navigation: &NavigationService,
    principal: &Principal,
    library_id: &LibraryId,
    entries: Vec<Keyed<FileEntry>>,
    deleted: bool,
) -> Result<Vec<ScopedFile>, FileError> {
    let catalog = navigation.catalog().as_ref();
    let mut titles: HashMap<CollectionId, Option<String>> = HashMap::new();
    let mut files = Vec::with_capacity(entries.len());
    for keyed in entries {
        let file = keyed.item.file;
        let home = file.home_collection_id().clone();
        let title = match titles.get(&home) {
            Some(title) => title.clone(),
            None => {
                let title = collection_title(catalog, &home, library_id)?;
                titles.insert(home.clone(), title.clone());
                title
            }
        };
        let guard = match title {
            Some(_) => NodeRef::Collection(home),
            None => NodeRef::Library(library_id.clone()),
        };
        if navigation.may_read(principal, &guard) {
            files.push(ScopedFile {
                file_id: file.id().clone(),
                collection: title.map(|t| super::search_query::fold(&t)),
                deleted,
            });
        }
    }
    Ok(files)
}

/// The title of collection `id` of `library_id`, or `None` when it no longer exists there
/// (the file is unfiled).
fn collection_title(
    catalog: &dyn LibraryCatalog,
    id: &CollectionId,
    library_id: &LibraryId,
) -> Result<Option<String>, FileError> {
    let collection = catalog.collection(id).map_err(catalog_error)?;
    Ok(collection
        .filter(|c| c.library_id() == library_id)
        .map(|c| c.title().to_string()))
}

/// Keep the first [`MAX_SEARCH_FILES`] files of `scope`, in library order.
fn trim_files(scope: &mut SearchScope) {
    let mut room = MAX_SEARCH_FILES;
    for library in &mut scope.libraries {
        library.files.truncate(room);
        room -= library.files.len();
    }
}
