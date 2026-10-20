//! The navigation listing (Task API-F1.11; API-S-021, F-004): one level of the tree a UI
//! shows as its navigation and breadcrumb, through `GET /api/v1/navigation`.
//!
//! Without a `node` the answer is the host's one repository; with one it is that node's
//! children (a repository's libraries; a library's collections, virtual collections and the
//! folders its unfiled files imply; a collection's folders; a folder's subfolders; nothing
//! under a virtual collection; never a file), each as an opaque [`NodeRef`] handle with its
//! `kind`, `name` and `has_children`, and the `ancestors` for a breadcrumb. Paging is by
//! cursor in a stable order: the fixed kind order, then the catalog's order for libraries,
//! collections and virtual collections, folders by name in Unicode code-point order. Only
//! nodes the caller may read are listed or counted, and an unknown node and an unreadable one
//! get the same `not_found`.
//!
//! The service keeps no copy of the catalog or the folders: libraries, collections and
//! virtual collections come from the [`crate::library::LibraryCatalog`] queries and folders
//! from the host's [`FolderIndex`] (the folder listing by logical-path prefix over the
//! ordinary files). After a committed change a [`NavigationPublisher`] publishes one
//! `navigation.change` event ([`NavigationChange`]), which a host filters per subscriber with
//! [`NavigationService::event_filter`]. This module does no storage I/O (API-S-006).

/// `tracing` target of this module's stdout-only diagnostics, outside `filesquid::activity`
/// so the journal records each refusal once, through the route.
const LOG_TARGET: &str = "filesquid::navigation";

/// Journal operation of a refused navigation listing.
pub const NAVIGATE_OPERATION: &str = "navigation.list";

/// Nodes in a page when the request names no limit.
pub const DEFAULT_LIMIT: usize = 200;

/// Most nodes one page holds.
pub const MAX_LIMIT: usize = 1_000;

/// Most children one call examines before it ends the page with a continuing cursor.
pub const MAX_SCAN: usize = 5_000;

/// Most scans one `has_children` check makes (each examines up to [`MAX_SCAN`] children).
pub const MAX_HAS_CHILDREN_SCANS: usize = 20;

mod change;
mod cursor;
mod error;
mod memory;
mod node;
mod port;
mod routes;
mod service;
mod view;

pub use change::{AlwaysTrue, NavigationChange, NavigationPublisher, MAX_CHANGE_PARENTS};
pub use error::NavigationError;
pub use memory::MemoryFolderIndex;
pub use node::{FolderScope, NodeRef, MAX_HANDLE_BYTES};
pub use port::{FolderIndex, HostAccess, NavigationAccess};
pub(crate) use routes::routes;
pub use service::{NavigationService, RepositoryInfo};
pub use view::{NavigationPage, NavigationRequest, NodeKind, NodeView};
