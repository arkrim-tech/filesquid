//! What the navigation route accepts and answers (API-S-021).

use serde::{Deserialize, Serialize};

/// What a node stands for. The wire names are the contract's `NavigationKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// The host's repository.
    Repository,
    /// A library.
    Library,
    /// A collection.
    Collection,
    /// A virtual collection.
    VirtualCollection,
    /// A folder implied by logical paths.
    Folder,
}

/// Query of `GET /api/v1/navigation`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationRequest {
    /// The node to list the children of; the host's repository level when absent.
    pub node: Option<String>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Nodes per page, 1 to 1,000; [`super::DEFAULT_LIMIT`] when absent.
    pub limit: Option<usize>,
}

/// One node of the tree, as listed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NodeView {
    /// The opaque handle (at most 512 bytes); clients never parse it.
    pub node: String,
    /// What the node stands for.
    pub kind: NodeKind,
    /// The title of a library, collection or virtual collection, the last segment of a
    /// folder's logical path, or a repository's configured name (`null` when it has none).
    pub name: Option<String>,
    /// True only when the caller may read at least one child.
    pub has_children: bool,
}

/// One page of one level of the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NavigationPage {
    /// The nodes of this page, in the stable order of API-S-021.
    pub nodes: Vec<NodeView>,
    /// For a listed node: the nodes from the repository down to and including it, for the
    /// breadcrumb. Empty at the repository level.
    pub ancestors: Vec<NodeView>,
    /// The cursor of the next page, or `null` on the last page.
    pub next_cursor: Option<String>,
}
