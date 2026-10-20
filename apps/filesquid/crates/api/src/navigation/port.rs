//! The two seams the navigation listing reads through (API-S-006, API-S-021): the folders
//! the files of a scope imply ([`FolderIndex`]) and who may read which node
//! ([`NavigationAccess`]). The host injects both; this module does no storage I/O.

use super::error::NavigationError;
use super::node::{FolderScope, NodeRef};
use crate::auth::Principal;

/// The folders implied by the logical paths of the ordinary files of a scope: the folder
/// listing of the file routes by logical-path prefix (API-F1.3, OCFL-S-012), and the catalog's
/// hide and delete state, seen through one port so the navigation keeps no copy of either.
///
/// A folder exists only when a committed logical path implies it. A path of a hidden or
/// deleted file implies none (the ordinary view of API-S-019), and a folder the UI holds as
/// pending does not exist here. Every method may block on storage, so the service calls it
/// from a blocking thread.
pub trait FolderIndex: Send + Sync {
    /// The names of the folders directly below `parent` (the empty string for the scope's
    /// root) in code-point order, those after `after` when given, at most `limit`.
    ///
    /// # Errors
    ///
    /// `unavailable` when the storage cannot be reached now, `internal` otherwise. No name or
    /// path goes into the error.
    fn subfolders(
        &self,
        scope: &FolderScope,
        parent: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, NavigationError>;

    /// Whether committed ordinary files imply the folder at `path`.
    ///
    /// # Errors
    ///
    /// As [`FolderIndex::subfolders`].
    fn folder_exists(&self, scope: &FolderScope, path: &str) -> Result<bool, NavigationError>;
}

/// Decides which nodes a caller may read. A node is listed, counted and named in an event
/// only when this says so, and its descendants are readable only when every node above them is
/// (the service checks the chain). API-F1.9 adds per-collection grants behind this seam.
pub trait NavigationAccess: Send + Sync {
    /// True when `principal` may read `node`.
    fn may_read(&self, principal: &Principal, node: &NodeRef) -> bool;
}

/// The access of this release: the principals the host policy already admits to the host
/// (the desktop owner and a server administrator) read every node. Adding a principal is a
/// compile error here until what it may read is decided.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostAccess;

impl NavigationAccess for HostAccess {
    fn may_read(&self, principal: &Principal, _node: &NodeRef) -> bool {
        match principal {
            Principal::Owner | Principal::Administrator => true,
        }
    }
}
