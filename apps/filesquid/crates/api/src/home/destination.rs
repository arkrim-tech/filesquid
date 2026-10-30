//! The destination of an import, upload or copy (API-S-011 **Destination**, API-S-036; owner
//! decisions 2026-10-22, "Must choose", ADR 041 Decision 4): a request names `collection_id`,
//! a collection of its library, and optionally `folder`, a collection logical folder path
//! inside it. A request without a collection is refused before anything is staged or any
//! dialog is shown ([`DESTINATION_REQUIRED`]), so nothing lands at a library's top level and
//! no default collection is chosen; an unknown collection, one of another library or one the
//! caller may not read is `not_found` ([`COLLECTION_NOT_FOUND`]); a folder outside the
//! collection-path rule or inside a reserved folder is `invalid_request` (`invalid_path`). The
//! collection is looked up through the navigation service's library catalog, the one meaning
//! of API-S-019.

use crate::auth::Principal;
use crate::canonical::collection_path::check_folder;
use crate::error::ErrorCode;
use crate::home::HomeCollections;
use crate::library::CatalogError;
use crate::navigation::{NavigationError, NavigationService, NodeRef};
use crate::ApiState;
use filesquid_core::CollectionId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Log target of the destination check.
const LOG_TARGET: &str = "filesquid::home";

/// Error code of a request that names no destination collection.
pub const DESTINATION_REQUIRED: &str = "destination_required";

/// Error code of a destination collection that is unknown, of another library or unreadable
/// to the caller.
pub const COLLECTION_NOT_FOUND: &str = "collection_not_found";

/// Where a new file goes, as a request names it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    /// The destination collection, a collection of the request's library. Required: a request
    /// without it is refused ([`DESTINATION_REQUIRED`]); it is optional here only so that
    /// refusal is answered and journaled with its own code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// The destination folder inside the collection, a collection logical path; the
    /// collection's root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

/// Why a destination was refused before anything was staged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationRefusal {
    /// No `collection_id`.
    Required,
    /// A `folder` outside the collection-path rule or in a reserved folder.
    Folder,
    /// An unknown collection, one of another library, or one the caller may not read.
    NotFound,
    /// The library catalog cannot answer now.
    Unavailable,
    /// The library catalog failed in a way a retry will not fix.
    Internal,
}

impl DestinationRefusal {
    /// The refusal of a catalog failure: `unavailable` only when a retry may succeed. Pure.
    fn of_catalog(err: &CatalogError) -> Self {
        match err {
            CatalogError::Unavailable(_) => Self::Unavailable,
            CatalogError::Conflict | CatalogError::Failed(_) => Self::Internal,
        }
    }

    /// The refusal of a readability check of the collection and its ancestors. Pure.
    fn of_navigation(err: NavigationError) -> Self {
        match err {
            NavigationError::NotFound | NavigationError::Invalid(_) => Self::NotFound,
            NavigationError::Unavailable => Self::Unavailable,
            NavigationError::Internal => Self::Internal,
        }
    }

    /// The error code, the answer's status code and its message (which names the error code
    /// and never the request's values). Pure.
    pub fn answer(self) -> (&'static str, ErrorCode, &'static str) {
        match self {
            Self::Required => (
                DESTINATION_REQUIRED,
                ErrorCode::InvalidRequest,
                "Choose the collection to put the file in (destination_required); nothing goes \
                 to a library's top level.",
            ),
            Self::Folder => (
                "invalid_path",
                ErrorCode::InvalidRequest,
                "The destination folder is not a folder path inside the collection \
                 (invalid_path).",
            ),
            Self::NotFound => (
                COLLECTION_NOT_FOUND,
                ErrorCode::NotFound,
                "That collection is not in this library (collection_not_found).",
            ),
            Self::Unavailable => (
                "unavailable",
                ErrorCode::Unavailable,
                "This host cannot reach the library's collections now.",
            ),
            Self::Internal => (
                "internal",
                ErrorCode::Internal,
                "This host could not check the destination.",
            ),
        }
    }
}

/// A destination that passed [`check`]: the collection and the folder, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The destination collection.
    pub collection_id: CollectionId,
    /// The destination folder inside it.
    pub folder: Option<String>,
}

/// Check `destination` for a request of `library` by `principal`: a collection is named, the
/// folder (if any) follows the collection-path rule outside the reserved folders, and the
/// collection belongs to `library` and is readable to `principal`. Reads the library catalog
/// on a blocking thread.
///
/// # Errors
/// The [`DestinationRefusal`] of the first rule broken.
pub async fn check(
    state: &ApiState,
    principal: &Principal,
    library: &str,
    destination: &Destination,
) -> Result<Checked, DestinationRefusal> {
    let collection = destination
        .collection_id
        .clone()
        .ok_or(DestinationRefusal::Required)?;
    if let Some(folder) = &destination.folder {
        check_folder(folder).map_err(|_| DestinationRefusal::Folder)?;
    }
    let lookup = lookup_of(state)?;
    let (principal, library) = (principal.clone(), library.to_string());
    let found = tokio::task::spawn_blocking(move || match lookup {
        Lookup::Navigation(navigation) => {
            collection_in(&navigation, &principal, &library, &collection)
        }
        Lookup::Home(home) => collection_held(home.as_ref(), &library, &collection),
    })
    .await
    .unwrap_or_else(|err| {
        tracing::error!(target: LOG_TARGET, error = %err, "a collection lookup failed");
        Err(DestinationRefusal::Internal)
    })?;
    Ok(Checked {
        collection_id: found,
        folder: destination.folder.clone(),
    })
}

/// Where a host looks a destination collection up.
enum Lookup {
    /// The navigation service: the library catalog and the caller's read access (API-S-021).
    Navigation(Arc<NavigationService>),
    /// The home collections, on a host without navigation, whose single owner reads every
    /// collection.
    Home(Arc<dyn HomeCollections>),
}

/// The lookup of `state`: its navigation service, else its home collections.
///
/// # Errors
/// [`DestinationRefusal::Unavailable`] on a host with neither.
fn lookup_of(state: &ApiState) -> Result<Lookup, DestinationRefusal> {
    if let Some(navigation) = state.navigation() {
        return Ok(Lookup::Navigation(navigation));
    }
    state
        .home_collections()
        .map(Lookup::Home)
        .ok_or(DestinationRefusal::Unavailable)
}

/// `collection` when `home` holds it in `library`. Reads the catalog (may block).
fn collection_held(
    home: &dyn HomeCollections,
    library: &str,
    collection: &str,
) -> Result<CollectionId, DestinationRefusal> {
    let id = CollectionId::new(collection).map_err(|_| DestinationRefusal::NotFound)?;
    let held = home
        .collection_library(&id)
        .map_err(|err| DestinationRefusal::of_catalog(&err))?;
    match held {
        Some(found) if found.as_str() == library => Ok(id),
        _ => Err(DestinationRefusal::NotFound),
    }
}

/// `collection` when it is a collection of `library` that `principal` may read. Reads the
/// library catalog (may block).
fn collection_in(
    navigation: &NavigationService,
    principal: &Principal,
    library: &str,
    collection: &str,
) -> Result<CollectionId, DestinationRefusal> {
    let id = CollectionId::new(collection).map_err(|_| DestinationRefusal::NotFound)?;
    let found = navigation
        .catalog()
        .collection(&id)
        .map_err(|err| DestinationRefusal::of_catalog(&err))?
        .ok_or(DestinationRefusal::NotFound)?;
    if found.library_id().as_str() != library {
        return Err(DestinationRefusal::NotFound);
    }
    navigation
        .check_readable(principal, &NodeRef::Collection(id.clone()))
        .map_err(DestinationRefusal::of_navigation)?;
    Ok(id)
}
