//! The folder listing by navigation node (Task API-F2.2; API-S-011, API-S-021, UI-F2.1b):
//! `GET /api/v1/files/listing?node=<handle>` lists a collection or folder node in the
//! API-F1.3 path model, where one OCFL object holds the files of one folder scope.
//!
//! The host says which object holds a scope's files through [`ScopeObjects`]. The node must
//! be readable to the caller through the navigation service, so an unknown, malformed or
//! unreadable node is `not_found` (one answer for all three). Library, virtual collection
//! and repository nodes are refused as `invalid_request`: their files come from the view
//! operation, not from one object's folder.

use super::error::FileError;
use super::view::{ListingQuery, ListingRequest};
use super::FileOperations;
use crate::auth::Principal;
use crate::navigation::{FolderScope, NavigationError, NavigationService, NodeRef};
use crate::ErrorCode;
use std::sync::Arc;

/// The library and OCFL object that hold the files of one folder scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeObject {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
}

/// Which OCFL object holds the files of each folder scope. Calls may block on the host's
/// catalog; the listing route calls it on a blocking thread.
pub trait ScopeObjects: Send + Sync {
    /// The object that holds the files of `scope`, or `None` when no object does.
    fn object_of(&self, scope: &FolderScope) -> Option<ScopeObject>;
}

/// The listing a route request names: by library, object and prefix as given, or by `node`
/// resolved for `principal`. May block; call it from a blocking thread.
///
/// # Errors
///
/// `invalid_request` unless exactly one form is given, or for a node of another kind;
/// `not_found` for an unknown, malformed or unreadable node or one no object holds;
/// `unavailable` when no navigation service is installed or a port is down.
pub(super) fn listing_query(
    request: ListingRequest,
    principal: &Principal,
    navigation: Option<Arc<NavigationService>>,
    files: &dyn FileOperations,
) -> Result<ListingQuery, FileError> {
    let by_object = request.library_id.is_some() || request.object_id.is_some();
    let Some(handle) = request.node.clone() else {
        return by_path(request);
    };
    if by_object || request.prefix.is_some() {
        return Err(both_forms());
    }
    let node = NodeRef::from_handle(&handle).ok_or_else(FileError::not_found)?;
    let navigation = navigation.ok_or_else(|| FileError::unavailable("no navigation service"))?;
    // Readability first, so an unreadable node of any kind gets the one `not_found`.
    navigation
        .check_readable(principal, &node)
        .map_err(file_error)?;
    let (scope, prefix) = node_folder(&node)?;
    let object = files
        .scope_object(&scope)
        .ok_or_else(FileError::not_found)?;
    Ok(ListingQuery {
        library_id: object.library_id,
        object_id: object.object_id,
        version: request.version,
        prefix: (!prefix.is_empty()).then_some(prefix),
        cursor: request.cursor,
        limit: request.limit,
    })
}

/// The query of a request naming library, object and prefix.
fn by_path(request: ListingRequest) -> Result<ListingQuery, FileError> {
    let (Some(library_id), Some(object_id)) = (request.library_id, request.object_id) else {
        return Err(both_forms());
    };
    Ok(ListingQuery {
        library_id,
        object_id,
        version: request.version,
        prefix: request.prefix,
        cursor: request.cursor,
        limit: request.limit,
    })
}

/// The folder scope and logical path a collection or folder node lists (`""` for a
/// collection's root). Pure.
fn node_folder(node: &NodeRef) -> Result<(FolderScope, String), FileError> {
    match node {
        NodeRef::Collection(id) => Ok((FolderScope::Collection(id.clone()), String::new())),
        NodeRef::Folder { scope, path } => Ok((scope.clone(), path.clone())),
        _ => Err(FileError::invalid(
            "Only a collection or folder node lists as a folder; list other nodes with the view operation.",
        )),
    }
}

/// `invalid_request` for a request naming neither or both forms.
fn both_forms() -> FileError {
    FileError::invalid(
        "A listing names either library_id and object_id (with an optional prefix) or one node.",
    )
}

/// The file error of a navigation refusal, keeping its code.
fn file_error(err: NavigationError) -> FileError {
    match err.api_code() {
        ErrorCode::NotFound => FileError::not_found(),
        ErrorCode::Unavailable => FileError::unavailable("the navigation service is unavailable"),
        ErrorCode::InvalidRequest => FileError::invalid(err.message()),
        _ => FileError::internal("the navigation service failed"),
    }
}
