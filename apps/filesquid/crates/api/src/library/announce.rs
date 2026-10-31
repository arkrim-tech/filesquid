//! Which navigation parents a committed command changed (API-S-021): the nodes whose
//! children it added, removed or renamed. Pure: it reads the planned changes and nothing else.

use super::catalog::CatalogChange;
use crate::navigation::NodeRef;
use filesquid_core::LibraryId;

/// What a command changed in the navigation tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NavigationParents {
    /// These nodes' children changed.
    Nodes(Vec<NodeRef>),
    /// A file left or entered the ordinary view, which may add or remove folders anywhere
    /// below the library, so a client rereads its whole visible tree.
    Everything,
}

/// The navigation parents `changes` of a command on `library_id` change, or `None` when they
/// change nothing navigation shows (an empty command, or a virtual-collection membership).
pub(crate) fn navigation_parents(
    library_id: &LibraryId,
    changes: &[CatalogChange],
) -> Option<NavigationParents> {
    let mut nodes: Vec<NodeRef> = Vec::new();
    for change in changes {
        let parent = match change {
            CatalogChange::CreateLibrary(record) => {
                NodeRef::Repository(record.library.repository_id().clone())
            }
            CatalogChange::PutCollection(_)
            | CatalogChange::RemoveCollection(_)
            | CatalogChange::PutVirtualCollection(_)
            | CatalogChange::RemoveVirtualCollection(_) => NodeRef::Library(library_id.clone()),
            CatalogChange::SetFileState { .. }
            | CatalogChange::PurgeFile(_)
            | CatalogChange::DeleteCollectionFiles { .. }
            | CatalogChange::RehomeFile { .. } => {
                return Some(NavigationParents::Everything);
            }
            CatalogChange::AddVirtualMember(_)
            | CatalogChange::RemoveVirtualMember { .. }
            | CatalogChange::SetRetention(_) => {
                continue;
            }
        };
        if !nodes.contains(&parent) {
            nodes.push(parent);
        }
    }
    (!nodes.is_empty()).then_some(NavigationParents::Nodes(nodes))
}
