//! The home-collection steps of the upload and copy routes (Task API-F2.14; API-S-011
//! **Destination** and **Same logical path**, API-S-021, API-S-026): check the destination
//! before anything is staged, find the path the new file takes, reserve its file ID under the
//! operation, register it after its object is committed, and announce the change. Each step
//! that reads or writes the host's catalog runs on a blocking thread and is named for what it
//! does; the refusals carry the rule's own journal code.

use super::error::FileError;
use super::view::{
    CollectionFolderRequest, CollectionFolderView, CollectionMoveRequest, CollectionMoveView,
};
use crate::auth::Principal;
use crate::canonical::collection_path::{join, safe_segment, ItemKind, PathError};
use crate::error::ErrorCode;
use crate::home::destination::{self, Checked, Destination};
use crate::home::{place_entry, register_entry, HomeCollections, HomeRequest, Placed, Reserved};
use crate::library::CatalogError;
use crate::navigation::{FolderScope, NavigationPublisher, NodeRef};
use crate::ApiState;
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::sync::Arc;

/// Where a new file goes and the path it asks for, after the destination check.
#[derive(Clone, Debug)]
pub(super) struct Homing {
    /// The library.
    pub library_id: LibraryId,
    /// The checked destination.
    pub destination: Checked,
    /// The collection logical path asked for: `<folder>/<safe name>`.
    pub path: String,
}

/// Check the destination of a new file named `name` in `library` for `principal` and compose
/// the path it asks for. Reads the library catalog.
///
/// # Errors
/// The destination's refusal (`destination_required`, `collection_not_found`,
/// `invalid_path`), or `invalid_path` for a name the safe transformation refuses.
pub(super) async fn check_destination(
    state: &ApiState,
    principal: &Principal,
    library: &str,
    destination: &Destination,
    name: &str,
) -> Result<Homing, FileError> {
    let checked = destination::check(state, principal, library, destination)
        .await
        .map_err(|refusal| {
            let (specific, code, message) = refusal.answer();
            FileError::coded(code, specific, message)
        })?;
    let library_id = LibraryId::new(library).map_err(|_| FileError::not_found())?;
    let segment = safe_segment(name).map_err(path_error)?;
    let path = join(checked.folder.as_deref(), &segment);
    Ok(Homing {
        library_id,
        destination: checked,
        path,
    })
}

/// The host's home collections, or `unavailable` on a host that has none.
///
/// # Errors
/// `unavailable` when no port is installed.
pub(super) fn installed(state: &ApiState) -> Result<Arc<dyn HomeCollections>, FileError> {
    state
        .home_collections()
        .ok_or_else(|| FileError::unavailable("this host keeps no home collections"))
}

/// The first free path for the new file of `homing`, numbered beside a taken one; read from
/// the host's catalog on a blocking thread. Changes nothing.
///
/// # Errors
/// `path_suffix_exhausted` (`conflict`), `name_too_long` or `invalid_path`
/// (`invalid_request`), or a store failure.
pub(super) async fn read_free_path(
    home: Arc<dyn HomeCollections>,
    homing: &Homing,
) -> Result<String, FileError> {
    let (library, collection) = (
        homing.library_id.clone(),
        homing.destination.collection_id.clone(),
    );
    let path = homing.path.clone();
    let found = tokio::task::spawn_blocking(move || {
        home.free_path(&library, &collection, &path, ItemKind::File)
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?;
    found.map_err(catalog_error)?.map_err(path_error)
}

/// Reserve the file ID of the new file `operation_id` makes in `library` for the command
/// whose input has `fingerprint`: an earlier, unfinished run's, else a freshly minted one.
/// Writes the reservation on a blocking thread.
///
/// # Errors
/// `invalid_request` when the operation ID already reserved a file for another command;
/// `unavailable` when no ID can be minted; a store failure.
pub(super) async fn reserve_file_id(
    home: Arc<dyn HomeCollections>,
    library: &LibraryId,
    (operation_id, fingerprint): (&str, &str),
) -> Result<FileId, FileError> {
    let minted = super::upload::mint_file_id()?;
    let (library, operation) = (library.clone(), operation_id.to_string());
    let fingerprint = fingerprint.to_string();
    let reserved = tokio::task::spawn_blocking(move || {
        home.reserve(&library, &operation, &fingerprint, &minted)
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?
    .map_err(catalog_error)?;
    match reserved {
        Reserved::File(file_id) => Ok(file_id),
        Reserved::Reused => Err(FileError::invalid(
            "That operation_id already names another command; send a new one.",
        )),
    }
}

/// Release the reservation of `operation_id` after its command failed before writing
/// anything; a failure to release is logged, since the command's own refusal is what the
/// caller is answered.
pub(super) async fn release_reservation(prepared: &Prepared, operation_id: &str) {
    if prepared.reserved.is_none() {
        return;
    }
    let (home, library) = (prepared.home.clone(), prepared.homing.library_id.clone());
    let operation = operation_id.to_string();
    let released = tokio::task::spawn_blocking(move || home.release(&library, &operation)).await;
    if !matches!(released, Ok(Ok(()))) {
        tracing::warn!(target: super::LOG_TARGET, "a reservation could not be released");
    }
}

/// The fingerprint of a command's input: the SHA-256 of its JSON, lowercase hex. Pure.
///
/// # Errors
/// `internal` when the input does not encode.
pub(super) fn fingerprint_of<T: serde::Serialize>(input: &T) -> Result<String, FileError> {
    let bytes = serde_json::to_vec(input)
        .map_err(|_| FileError::internal("a command input did not encode"))?;
    Ok(filesquid_ocfl::DigestAlgorithm::Sha256.digest_hex(&bytes))
}

/// Register the committed file `file_id` of `homing` in its home collection under
/// `operation_id`, publishing one snapshot version; answer the home collection and the path
/// it got (a file that already has a home keeps it). Writes the host's catalog on a blocking
/// thread.
///
/// # Errors
/// The registration's refusal (`collection_not_found`, `path_suffix_exhausted`,
/// `name_too_long`, `invalid_path`), or a store failure.
pub(super) async fn register_file(
    home: Arc<dyn HomeCollections>,
    homing: &Homing,
    operation_id: &str,
    file_id: FileId,
) -> Result<(CollectionId, String), FileError> {
    let request = HomeRequest {
        file_id,
        collection_id: homing.destination.collection_id.clone(),
        path: homing.path.clone(),
    };
    let (library, operation) = (homing.library_id.clone(), operation_id.to_string());
    let placed = tokio::task::spawn_blocking(move || {
        home.register(&library, &operation, &request, register_entry(&operation))
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?
    .map_err(catalog_error)?;
    match placed {
        Placed::Registered { path } => Ok((homing.destination.collection_id.clone(), path)),
        Placed::AlreadyRegistered {
            collection_id,
            path,
        } => Ok((collection_id, path)),
        Placed::Refused(code) => Err(refused(code)),
    }
}

/// Announce on the host's event bus that the folder holding `path` in `collection` changed
/// (`navigation.change`, API-S-021): the collection itself for a file at its root.
pub(super) fn publish_change(state: &ApiState, collection: &CollectionId, path: &str) {
    let parent = match path.rsplit_once('/') {
        Some((folder, _)) => NodeRef::folder(FolderScope::Collection(collection.clone()), folder),
        None => NodeRef::Collection(collection.clone()),
    };
    NavigationPublisher::new(state.events.clone()).publish(&[parent]);
}

/// A new file's home, ready before anything is staged: its checked destination, the
/// host's port, and the file ID its operation reserved (none for new content of an existing
/// file, which keeps its home).
pub(super) struct Prepared {
    /// The destination and the path asked for.
    pub homing: Homing,
    /// The host's home collections.
    pub home: Arc<dyn HomeCollections>,
    /// The reserved file ID of a new file.
    pub reserved: Option<FileId>,
}

/// Prepare the home of a file named `name` that `operation_id` writes in `library`: check
/// the destination, then, for a new file (whose command's input has the fingerprint given),
/// refuse a path the numbering rule cannot place and reserve its file ID. Reads the catalog
/// and writes the reservation.
///
/// # Errors
/// As [`check_destination`], [`installed`], [`read_free_path`] and [`reserve_file_id`].
pub(super) async fn prepare_home(
    state: &ApiState,
    principal: &Principal,
    (library, destination): (&str, &Destination),
    name: &str,
    (operation_id, fingerprint): (&str, Option<&str>),
) -> Result<Prepared, FileError> {
    let homing = check_destination(state, principal, library, destination, name).await?;
    let home = installed(state)?;
    let reserved = match fingerprint {
        Some(fingerprint) => {
            read_free_path(home.clone(), &homing).await?;
            let ids = (operation_id, fingerprint);
            Some(reserve_file_id(home.clone(), &homing.library_id, ids).await?)
        }
        None => None,
    };
    Ok(Prepared {
        homing,
        home,
        reserved,
    })
}

/// Register the committed file `file_id` of `prepared` under `operation_id` and announce the
/// change; answer its home collection and the path it got. Writes the catalog.
///
/// # Errors
/// `internal` for a committed file ID that is not one; as [`register_file`].
pub(super) async fn settle_home(
    state: &ApiState,
    prepared: Prepared,
    operation_id: &str,
    file_id: &str,
) -> Result<(String, String), FileError> {
    let file_id =
        FileId::new(file_id).map_err(|_| FileError::internal("a committed file ID was refused"))?;
    let (collection, path) =
        register_file(prepared.home, &prepared.homing, operation_id, file_id).await?;
    publish_change(state, &collection, &path);
    Ok((collection.as_str().to_string(), path))
}

/// A rename's new place, checked before the new name is committed: the registered file, its
/// collection and the path its new name asks for.
pub(super) struct Renaming {
    /// The host's home collections.
    home: Arc<dyn HomeCollections>,
    /// The file's library.
    library_id: LibraryId,
    /// The new place asked for.
    request: HomeRequest,
}

/// The new place of the file `file_id` of `library` renamed to `name`: its folder and the
/// safe new name, after checking that the numbering rule can place it; `None` on a host
/// without home collections, for a file no home collection registers, or when the path does
/// not change. Reads the catalog.
///
/// # Errors
/// `invalid_request` for a malformed ID; the path's refusal (`path_suffix_exhausted`,
/// `name_too_long`, `invalid_path`); a store failure.
pub(super) async fn prepare_rename(
    state: &ApiState,
    library: &str,
    file_id: &str,
    name: &str,
) -> Result<Option<Renaming>, FileError> {
    let Some(home) = state.home_collections() else {
        return Ok(None);
    };
    let library_id = LibraryId::new(library).map_err(|_| FileError::not_found())?;
    let file_id =
        FileId::new(file_id).map_err(|_| FileError::invalid("A file_id is a UUID v4."))?;
    let Some((collection, current)) = read_placement(home.clone(), &library_id, &file_id).await?
    else {
        return Ok(None);
    };
    let folder = current
        .rsplit_once('/')
        .map(|(folder, _)| folder.to_string());
    let path = join(folder.as_deref(), &safe_segment(name).map_err(path_error)?);
    if path == current {
        return Ok(None);
    }
    let homing = Homing {
        library_id: library_id.clone(),
        destination: Checked {
            collection_id: collection.clone(),
            folder,
        },
        path: path.clone(),
    };
    read_free_path(home.clone(), &homing).await?;
    let request = HomeRequest {
        file_id,
        collection_id: collection,
        path,
    };
    Ok(Some(Renaming {
        home,
        library_id,
        request,
    }))
}

/// Give the renamed file of `renaming` its new path under `operation_id`, publishing one
/// snapshot version, and announce the change; answer the path it got. Writes the catalog.
///
/// # Errors
/// The relocation's refusal, or a store failure.
pub(super) async fn settle_rename(
    state: &ApiState,
    renaming: Renaming,
    operation_id: &str,
) -> Result<String, FileError> {
    let Renaming {
        home,
        library_id,
        request,
    } = renaming;
    let collection = request.collection_id.clone();
    let operation = operation_id.to_string();
    let placed = tokio::task::spawn_blocking(move || {
        home.relocate(&library_id, &request, place_entry(&operation))
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?
    .map_err(catalog_error)?;
    let path = match placed {
        Placed::Registered { path } | Placed::AlreadyRegistered { path, .. } => path,
        Placed::Refused(code) => return Err(refused(code)),
    };
    publish_change(state, &collection, &path);
    Ok(path)
}

/// The home collection and logical path of `file_id`, read from the host's catalog on a
/// blocking thread.
///
/// # Errors
/// A store failure.
async fn read_placement(
    home: Arc<dyn HomeCollections>,
    library: &LibraryId,
    file_id: &FileId,
) -> Result<Option<(CollectionId, String)>, FileError> {
    let (library, file_id) = (library.clone(), file_id.clone());
    tokio::task::spawn_blocking(move || home.placement(&library, &file_id))
        .await
        .map_err(|_| FileError::internal("the home-collection task failed"))?
        .map_err(catalog_error)
}

/// Move the registered file of `request` to its destination for `principal` under
/// `operation_id`: check the destination, keep the file's name, give it the first free path
/// there in one snapshot version, and announce both folders. Reads and writes the catalog.
///
/// # Errors
/// The destination's refusal; `not_found` for a file no home collection registers; the
/// path's refusal; a store failure.
pub(super) async fn move_to_destination(
    state: &ApiState,
    principal: &Principal,
    request: CollectionMoveRequest,
    operation_id: &str,
) -> Result<(CollectionMoveView, bool), FileError> {
    let destination = Destination {
        collection_id: request.collection_id.clone(),
        folder: request.folder.clone(),
    };
    let checked = destination::check(state, principal, &request.library_id, &destination)
        .await
        .map_err(|refusal| {
            let (specific, code, message) = refusal.answer();
            FileError::coded(code, specific, message)
        })?;
    let home = installed(state)?;
    let library_id = LibraryId::new(&request.library_id).map_err(|_| FileError::not_found())?;
    let file_id =
        FileId::new(&request.file_id).map_err(|_| FileError::invalid("A file_id is a UUID v4."))?;
    let (from, current) = read_placement(home.clone(), &library_id, &file_id)
        .await?
        .ok_or_else(FileError::not_found)?;
    let name = current.rsplit('/').next().unwrap_or(&current).to_string();
    let path = join(checked.folder.as_deref(), &name);
    let to = HomeRequest {
        file_id,
        collection_id: checked.collection_id.clone(),
        path,
    };
    let operation = operation_id.to_string();
    let placed = tokio::task::spawn_blocking(move || {
        home.relocate(&library_id, &to, place_entry(&operation))
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?
    .map_err(catalog_error)?;
    let (path, changed) = match placed {
        Placed::Registered { path } => (path, true),
        Placed::AlreadyRegistered { path, .. } => (path, false),
        Placed::Refused(code) => return Err(refused(code)),
    };
    if changed {
        publish_change(state, &from, &current);
        publish_change(state, &checked.collection_id, &path);
    }
    let view = CollectionMoveView {
        library_id: request.library_id,
        file_id: request.file_id,
        collection_id: checked.collection_id.as_str().to_string(),
        path,
    };
    Ok((view, changed))
}

/// Plan the collection folder of `request` for `principal`: check the collection and the
/// folder path, then find the first free folder path beside a taken one; nothing is written,
/// since a folder exists once a file is placed in it (ADR 041 Decision 3). Reads the catalog.
///
/// # Errors
/// The destination's refusal; the path's refusal (`path_suffix_exhausted`, `name_too_long`,
/// `invalid_path`); a store failure.
pub(super) async fn plan_folder(
    state: &ApiState,
    principal: &Principal,
    request: CollectionFolderRequest,
) -> Result<CollectionFolderView, FileError> {
    let destination = Destination {
        collection_id: request.collection_id.clone(),
        folder: Some(request.folder.clone()),
    };
    let checked = destination::check(state, principal, &request.library_id, &destination)
        .await
        .map_err(|refusal| {
            let (specific, code, message) = refusal.answer();
            FileError::coded(code, specific, message)
        })?;
    let home = installed(state)?;
    let library = LibraryId::new(&request.library_id).map_err(|_| FileError::not_found())?;
    let collection = checked.collection_id.clone();
    let folder = request.folder.clone();
    let found = tokio::task::spawn_blocking(move || {
        home.free_path(&library, &collection, &folder, ItemKind::Folder)
    })
    .await
    .map_err(|_| FileError::internal("the home-collection task failed"))?;
    let path = found.map_err(catalog_error)?.map_err(path_error)?;
    Ok(CollectionFolderView {
        library_id: request.library_id,
        collection_id: checked.collection_id.as_str().to_string(),
        path,
        pending: true,
    })
}

/// The refusal of a path the rule of API-S-011 refuses. Pure.
fn path_error(err: PathError) -> FileError {
    refused(err.error_code())
}

/// The refusal of a registration code. Pure.
fn refused(code: &'static str) -> FileError {
    match code {
        "path_suffix_exhausted" => FileError::coded(
            ErrorCode::Conflict,
            code,
            "Every name from -1 to -9999 beside that one is taken (path_suffix_exhausted).",
        ),
        "not_found" => FileError::not_found(),
        "collection_not_found" => FileError::coded(
            ErrorCode::NotFound,
            code,
            "That collection is not in this library (collection_not_found).",
        ),
        "name_too_long" => FileError::coded(
            ErrorCode::InvalidRequest,
            code,
            "The name is longer than 255 bytes (name_too_long).",
        ),
        _ => FileError::coded(
            ErrorCode::InvalidRequest,
            "invalid_path",
            "The name or folder is not a valid path inside the collection (invalid_path).",
        ),
    }
}

/// The answer of a catalog failure: `unavailable` only when a retry may succeed. Pure.
fn catalog_error(err: CatalogError) -> FileError {
    match err {
        CatalogError::Unavailable(detail) => FileError::unavailable(detail),
        CatalogError::Conflict => {
            FileError::conflict("The library changed while the file was registered; try again.")
        }
        CatalogError::Failed(detail) => FileError::internal(detail),
    }
}
