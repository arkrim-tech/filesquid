//! The folder listing by navigation node (Task API-F2.2; API-S-011, API-S-021, UI-F2.1b):
//! `GET /api/v1/files/listing?node=<handle>` lists a collection or folder node: from the
//! host's index of registered files, one object per file (Task DE-1.7,
//! [`super::scope_listing`]), or, where the host keeps no index for the scope, in the API-F1.3
//! path model, where one OCFL object holds the files of one folder scope.
//!
//! The host answers both through [`ScopeObjects`]. The node must
//! be readable to the caller through the navigation service, so an unknown, malformed or
//! unreadable node is `not_found` (one answer for all three). Library, virtual collection
//! and repository nodes are refused as `invalid_request`: their files come from the view
//! operation, not from one object's folder.

use super::error::FileError;
use super::scope_listing::{
    decode_cursor, listing_context, scope_page_size, ScopeAfter, ScopeListing, ScopePage,
};
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

/// Which OCFL object holds the files of each folder scope, or, where every file has its own
/// object, the host's index of registered files (Task DE-1.7). Calls may block on the host's
/// catalog; the listing route calls them on a blocking thread.
pub trait ScopeObjects: Send + Sync {
    /// The object that holds the files of `scope`, or `None` when no object does.
    fn object_of(&self, scope: &FolderScope) -> Option<ScopeObject>;

    /// One page of the folder `folder` (a collection logical path, `""` for the root) of
    /// `scope` from the host's index: its subfolders, then its registered ordinary files,
    /// at most `limit` entries after `after`; `None` when the host keeps no index for `scope`
    /// (the listing then falls back to [`ScopeObjects::object_of`]). The default keeps none.
    ///
    /// # Errors
    /// `unavailable` or `internal` when the index cannot be read.
    fn entries(
        &self,
        scope: &FolderScope,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, FileError> {
        let _ = (scope, folder, after, limit);
        Ok(None)
    }
}

/// What a listing request resolves to: one object's folder, or a scope's folder from the
/// host's index.
#[derive(Clone, Debug)]
pub enum Listing {
    /// A folder of one OCFL object.
    Object(ListingQuery),
    /// A folder of a collection scope, from the host's index.
    Scope(ScopeListing),
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
) -> Result<Listing, FileError> {
    let by_object = request.library_id.is_some() || request.object_id.is_some();
    let Some(handle) = request.node.clone() else {
        return by_path(request).map(Listing::Object);
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
    if let Some(listing) = indexed(&request, &scope, &prefix, files)? {
        return Ok(Listing::Scope(listing));
    }
    let object = files
        .scope_object(&scope)
        .ok_or_else(FileError::not_found)?;
    Ok(Listing::Object(ListingQuery {
        library_id: object.library_id,
        object_id: object.object_id,
        version: request.version,
        prefix: (!prefix.is_empty()).then_some(prefix),
        cursor: request.cursor,
        limit: request.limit,
    }))
}

/// The key of `scope` a cursor is bound to. Pure.
fn scope_key(scope: &FolderScope) -> String {
    match scope {
        FolderScope::Collection(id) => format!("collection:{}", id.as_str()),
    }
}

/// The page of `prefix` of `scope` from the host's index, or `None` when the host keeps no
/// index for it. One more entry than the page size is asked for, to learn whether a next page
/// exists. May block.
///
/// # Errors
/// `invalid_request` for a page size outside 1 to 1,000, a version (an index lists the heads),
/// or a cursor that is not a scope cursor; the index's own errors.
fn indexed(
    request: &ListingRequest,
    scope: &FolderScope,
    prefix: &str,
    files: &dyn FileOperations,
) -> Result<Option<ScopeListing>, FileError> {
    let limit = scope_page_size(request.limit)?;
    let scope_key = scope_key(scope);
    let context = listing_context(&scope_key, prefix);
    let after = request
        .cursor
        .as_deref()
        .map(|cursor| decode_cursor(&context, cursor));
    let after_ref = after.as_ref().and_then(Option::as_ref);
    let Some(page) = files.scope_page(scope, prefix, after_ref, limit + 1)? else {
        return Ok(None);
    };
    if matches!(after, Some(None)) {
        return Err(FileError::invalid(
            "The cursor does not belong to this listing.",
        ));
    }
    if request.version.is_some() {
        return Err(FileError::invalid(
            "A collection or folder node lists the current files; it takes no version.",
        ));
    }
    Ok(Some(ScopeListing::of(&scope_key, prefix, page, limit)))
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
pub(super) fn file_error(err: NavigationError) -> FileError {
    match err.api_code() {
        ErrorCode::NotFound => FileError::not_found(),
        ErrorCode::Unavailable => FileError::unavailable("the navigation service is unavailable"),
        ErrorCode::InvalidRequest => FileError::invalid(err.message()),
        _ => FileError::internal("the navigation service failed"),
    }
}
