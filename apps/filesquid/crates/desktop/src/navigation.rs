//! The desktop's navigation listing (API-S-021; Task API-F1.11 host wiring, carried by DE-1.1):
//! `filesquid-api`'s [`NavigationService`] over the library catalogs, installed with
//! [`ApiState::install_navigation`], and the per-subscriber filter of the event channel.
//!
//! The repository is this installation: a repository is "one installation"
//! ([`RepositoryId`]), so its ID is the installation UUID and it has no configured name. The
//! owner reads every node ([`HostAccess`]).

use filesquid_api::auth::Principal;
use filesquid_api::events::EventFilter;
use filesquid_api::files::{ScopeObject, ScopeObjects};
use filesquid_api::library::LibraryCatalog;
use filesquid_api::navigation::{
    FolderIndex, FolderScope, HostAccess, NavigationError, NavigationService, RepositoryInfo,
};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, RepositoryId};
use std::sync::Arc;

/// The folders the desktop catalog's committed file paths imply. The catalog records no
/// logical path for its files yet (the import of Task DE-1.2 adds them), so no committed path
/// implies a folder and every scope has none. Pure.
#[derive(Clone, Copy, Debug, Default)]
pub struct CatalogFolders;

impl FolderIndex for CatalogFolders {
    fn subfolders(
        &self,
        _scope: &FolderScope,
        _parent: &str,
        _after: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<String>, NavigationError> {
        Ok(Vec::new())
    }

    fn folder_exists(&self, _scope: &FolderScope, _path: &str) -> Result<bool, NavigationError> {
        Ok(false)
    }
}

/// Which OCFL object holds the files of each folder scope, for the file listing by
/// navigation node (API-F2.2). The catalog records no object for a collection's files yet
/// (the import of Task DE-1.2 adds them), so no object holds any scope and every node
/// listing answers `not_found`. Pure.
#[derive(Clone, Copy, Debug, Default)]
pub struct CatalogScopes;

impl ScopeObjects for CatalogScopes {
    fn object_of(&self, _scope: &FolderScope) -> Option<ScopeObject> {
        None
    }
}

/// The navigation service of this desktop over `catalog`, for the repository of
/// `installation`. Pure.
///
/// # Errors
/// The installation UUID is not a valid repository ID (never, for a parsed UUID).
pub fn navigation_service(
    catalog: Arc<dyn LibraryCatalog>,
    installation: &InstallationId,
) -> Result<NavigationService, String> {
    let repository = RepositoryId::new(installation.as_str())
        .map_err(|_| "the installation is not a repository ID".to_string())?;
    Ok(NavigationService::new(
        catalog,
        Arc::new(CatalogFolders),
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
    catalog: Arc<dyn LibraryCatalog>,
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
