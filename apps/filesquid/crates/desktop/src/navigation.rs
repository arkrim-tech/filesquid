//! The desktop's navigation listing (API-S-021; Task API-F1.11 host wiring, carried by DE-1.1):
//! `filesquid-api`'s [`NavigationService`] over the library catalogs, installed with
//! [`ApiState::install_navigation`], and the per-subscriber filter of the event channel.
//!
//! The repository is this installation: a repository is "one installation"
//! ([`RepositoryId`]), so its ID is the installation UUID and it has no configured name. The
//! owner reads every node ([`HostAccess`]).

use filesquid_api::auth::Principal;
use filesquid_api::events::EventFilter;
use filesquid_api::files::{FileError, ScopeAfter, ScopeObject, ScopeObjects, ScopePage};
use filesquid_api::library::CatalogError;
use filesquid_api::navigation::{
    FolderIndex, FolderScope, HostAccess, NavigationError, NavigationService, RepositoryInfo,
};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, RepositoryId};
use filesquid_native::SqliteCatalog;
use std::sync::Arc;

/// The folders the desktop catalog's committed file paths imply (Task DE-1.7; DE-S-018,
/// API-S-021): the folders below a collection or folder node, answered from the library's
/// index of registered files, where a folder exists only when the collection logical path of
/// an ordinary file (neither hidden nor deleted) implies it. A file has exactly one home
/// collection, so a scope of removed collections (`Unfiled`) holds no folder.
#[derive(Clone, Debug)]
pub struct CatalogFolders(pub Arc<SqliteCatalog>);

impl FolderIndex for CatalogFolders {
    fn subfolders(
        &self,
        scope: &FolderScope,
        parent: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, NavigationError> {
        match scope {
            FolderScope::Collection(collection) => self
                .0
                .subfolders(collection, parent, after, limit)
                .map_err(navigation_error),
            FolderScope::Unfiled(_) => Ok(Vec::new()),
        }
    }

    fn folder_exists(&self, scope: &FolderScope, path: &str) -> Result<bool, NavigationError> {
        match scope {
            FolderScope::Collection(collection) => self
                .0
                .folder_exists(collection, path)
                .map_err(navigation_error),
            FolderScope::Unfiled(_) => Ok(false),
        }
    }
}

/// The navigation error of a catalog failure, with no path or detail.
fn navigation_error(err: CatalogError) -> NavigationError {
    match err {
        CatalogError::Unavailable(_) => NavigationError::Unavailable,
        CatalogError::Conflict | CatalogError::Failed(_) => NavigationError::Internal,
    }
}

/// What the listing by navigation node lists (Task DE-1.7; DE-S-018, API-S-011): every file
/// has its own OCFL object, so no single object holds a scope's files; a collection or folder
/// node lists its subfolders and its registered ordinary files from the library's index
/// ([`SqliteCatalog::scope_entries`]), each from its own object.
#[derive(Clone, Debug)]
pub struct CatalogScopes(pub Arc<SqliteCatalog>);

impl ScopeObjects for CatalogScopes {
    fn object_of(&self, _scope: &FolderScope) -> Option<ScopeObject> {
        None
    }

    fn entries(
        &self,
        scope: &FolderScope,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, FileError> {
        let FolderScope::Collection(collection) = scope else {
            return Ok(None);
        };
        self.0
            .scope_entries(collection, folder, after, limit)
            .map_err(|err| match err {
                CatalogError::Unavailable(_) => FileError::unavailable("the library index"),
                _ => FileError::internal("the library index failed"),
            })
    }
}

/// The navigation service of this desktop over `catalog`, for the repository of
/// `installation`. Pure.
///
/// # Errors
/// The installation UUID is not a valid repository ID (never, for a parsed UUID).
pub fn navigation_service(
    catalog: Arc<SqliteCatalog>,
    installation: &InstallationId,
) -> Result<NavigationService, String> {
    let repository = RepositoryId::new(installation.as_str())
        .map_err(|_| "the installation is not a repository ID".to_string())?;
    Ok(NavigationService::new(
        catalog.clone(),
        Arc::new(CatalogFolders(catalog)),
        Arc::new(HostAccess),
        RepositoryInfo::new(repository, None),
    ))
}

/// Install the navigation service of [`navigation_service`] in `state`, so
/// `GET /api/v1/navigation` answers instead of `503 unavailable`.
///
/// # Errors
/// A navigation service is already installed; the first one is kept.
pub fn install_navigation(
    state: &ApiState,
    catalog: Arc<SqliteCatalog>,
    installation: &InstallationId,
) -> Result<(), String> {
    let service = Arc::new(navigation_service(catalog, installation)?);
    if !state.install_navigation(service) {
        return Err("a navigation service was already installed".to_string());
    }
    Ok(())
}

/// The filter of the webview's event stream: `navigation.change` events reduced to what the
/// desktop owner may read, every other event unchanged; `None` while no navigation service
/// is installed (every event passes then).
pub fn owner_event_filter(state: &ApiState) -> Option<EventFilter> {
    state
        .navigation()
        .map(|service| service.event_filter(Principal::Owner))
}
