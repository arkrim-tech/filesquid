//! [`NavigationService`]: one level of the navigation tree per call (API-S-021).
//!
//! The service keeps no tree of its own. Libraries, collections and virtual collections come
//! from the [`LibraryCatalog`] queries of API-S-019, folders from the injected
//! [`FolderIndex`], and readability from the injected [`NavigationAccess`]. A call resolves
//! the named node and every node above it (each must exist and be readable, or the answer is
//! the one `not_found`), then lists the node's children in the fixed kind order, filtering by
//! readability before paging.
//!
//! Every loop is bounded: a call scans at most [`MAX_SCAN`] children, and when that cap is
//! reached the page ends there with a cursor that continues the scan, so a caller who can
//! read few of very many siblings still makes progress without a gap or a repeat.

use super::cursor::{Position, Rank};
use super::error::NavigationError;
use super::node::{FolderScope, NodeRef};
use super::port::{FolderIndex, NavigationAccess};
use super::view::{NavigationPage, NavigationRequest, NodeKind, NodeView};
use super::{DEFAULT_LIMIT, LOG_TARGET, MAX_LIMIT, MAX_SCAN};
use crate::auth::Principal;
use crate::library::{LibraryCatalog, PageKey};
use filesquid_core::RepositoryId;
use std::sync::Arc;

/// The host's one repository: its identifier and, when the host is configured with one, its
/// name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryInfo {
    id: RepositoryId,
    name: Option<String>,
}

impl RepositoryInfo {
    /// The repository `id` with the name its host is configured with, if any.
    pub fn new(id: RepositoryId, name: Option<String>) -> Self {
        Self { id, name }
    }
}

/// A node as the listing knows it: what it is, what it is called and where the scan that
/// found it stood.
#[derive(Clone, Debug)]
struct Child {
    node: NodeRef,
    name: String,
    position: Position,
}

/// One scanned page of children: those the caller may read, where to continue when the scan
/// stopped early, and how many children it may still examine.
struct Scan {
    children: Vec<Child>,
    next: Option<Position>,
    budget: usize,
}

/// What one scan is for: who reads, whose children, and how many are wanted.
struct Sweep<'a> {
    principal: &'a Principal,
    parent: &'a NodeRef,
    limit: usize,
}

/// Lists the navigation tree for every host.
#[derive(Clone)]
pub struct NavigationService {
    catalog: Arc<dyn LibraryCatalog>,
    folders: Arc<dyn FolderIndex>,
    access: Arc<dyn NavigationAccess>,
    repository: RepositoryInfo,
}

impl std::fmt::Debug for NavigationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavigationService").finish_non_exhaustive()
    }
}

impl NavigationService {
    /// A service over the host's `catalog`, `folders` and `access` for its `repository`.
    pub fn new(
        catalog: Arc<dyn LibraryCatalog>,
        folders: Arc<dyn FolderIndex>,
        access: Arc<dyn NavigationAccess>,
        repository: RepositoryInfo,
    ) -> Self {
        Self {
            catalog,
            folders,
            access,
            repository,
        }
    }

    /// The opaque handle of `node`, or `None` when it has none (see [`NodeRef::to_handle`]).
    pub fn handle_of(&self, node: &NodeRef) -> Option<String> {
        node.to_handle()
    }

    /// Whether `principal` may read `node` (not its ancestors).
    pub(crate) fn may_read(&self, principal: &Principal, node: &NodeRef) -> bool {
        self.access.may_read(principal, node)
    }

    /// Answer one request for `principal`. It reads the catalog and the folder index, which
    /// may block, so async callers run it on a blocking thread.
    ///
    /// # Errors
    ///
    /// `invalid_request` for a limit outside 1 to 1,000 or a cursor this service did not
    /// issue for the level; `not_found` for a node that is unknown, malformed or unreadable
    /// (one answer for all three); `unavailable` or `internal` when a port fails.
    pub fn list(
        &self,
        principal: &Principal,
        request: &NavigationRequest,
    ) -> Result<NavigationPage, NavigationError> {
        let limit = check_limit(request.limit)?;
        let after = request
            .cursor
            .as_deref()
            .map(Position::decode)
            .transpose()?;
        match request.node.as_deref() {
            None => self.list_root(principal, after),
            Some(handle) => self.list_node(principal, handle, after, limit),
        }
    }

    /// The repository level: the host's one repository.
    fn list_root(
        &self,
        principal: &Principal,
        after: Option<Position>,
    ) -> Result<NavigationPage, NavigationError> {
        if after.is_some() {
            return Err(wrong_level());
        }
        let repository = NodeRef::Repository(self.repository.id.clone());
        let has_children = self.has_children(principal, &repository)?;
        Ok(NavigationPage {
            nodes: vec![view_of(
                &repository,
                self.repository.name.clone(),
                has_children,
            )?],
            ancestors: Vec::new(),
            next_cursor: None,
        })
    }

    /// The children of the node `handle` names, with the breadcrumb down to it.
    fn list_node(
        &self,
        principal: &Principal,
        handle: &str,
        after: Option<Position>,
        limit: usize,
    ) -> Result<NavigationPage, NavigationError> {
        let node = NodeRef::from_handle(handle).ok_or(NavigationError::NotFound)?;
        let chain = self.resolve(principal, &node)?;
        let scan = self.scan(principal, &node, after.as_ref(), limit)?;
        let (listed, next) = split_page(scan, limit);
        let mut nodes = Vec::with_capacity(listed.len());
        for child in &listed {
            let has_children = self.has_children(principal, &child.node)?;
            nodes.push(view_of(
                &child.node,
                Some(child.name.clone()),
                has_children,
            )?);
        }
        let node_has_children = self.node_has_children(principal, &node, &after, &nodes, &next)?;
        let next_cursor = next.as_ref().map(Position::encode).transpose()?;
        Ok(NavigationPage {
            nodes,
            ancestors: breadcrumb(&chain, node_has_children)?,
            next_cursor,
        })
    }

    /// Whether the listed node itself has a readable child, from this page when it can tell.
    fn node_has_children(
        &self,
        principal: &Principal,
        node: &NodeRef,
        after: &Option<Position>,
        nodes: &[NodeView],
        next: &Option<Position>,
    ) -> Result<bool, NavigationError> {
        if after.is_none() && (!nodes.is_empty() || next.is_some()) {
            return Ok(true);
        }
        self.has_children(principal, node)
    }

    /// True when `principal` may read at least one child of `node`.
    fn has_children(&self, principal: &Principal, node: &NodeRef) -> Result<bool, NavigationError> {
        if matches!(node, NodeRef::VirtualCollection(_)) {
            return Ok(false);
        }
        Ok(!self.scan(principal, node, None, 1)?.children.is_empty())
    }

    /// The nodes from the repository down to `node`, each of which must exist and be readable.
    fn resolve(
        &self,
        principal: &Principal,
        node: &NodeRef,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        let mut chain = match node {
            NodeRef::Repository(id) => self.resolve_repository(id)?,
            NodeRef::Library(id) => self.resolve_library(id)?,
            NodeRef::Collection(id) => self.resolve_collection(id)?,
            NodeRef::VirtualCollection(id) => self.resolve_virtual(id)?,
            NodeRef::Folder { scope, path } => self.resolve_folder(scope, path)?,
        };
        for (step, _) in &chain {
            if !self.access.may_read(principal, step) {
                return Err(NavigationError::NotFound);
            }
        }
        chain.shrink_to_fit();
        Ok(chain)
    }

    /// `[repository]`, or `not_found` for another repository.
    fn resolve_repository(
        &self,
        id: &RepositoryId,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        if *id != self.repository.id {
            return Err(NavigationError::NotFound);
        }
        let repository = NodeRef::Repository(id.clone());
        Ok(vec![(repository, self.repository.name.clone())])
    }

    /// `[repository, library]`.
    fn resolve_library(
        &self,
        id: &filesquid_core::LibraryId,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        let state = self.catalog.library(id)?.ok_or(NavigationError::NotFound)?;
        let mut chain = self.resolve_repository(state.record.library.repository_id())?;
        chain.push((NodeRef::Library(id.clone()), Some(state.record.name)));
        Ok(chain)
    }

    /// `[repository, library, collection]`.
    fn resolve_collection(
        &self,
        id: &filesquid_core::CollectionId,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        let collection = self
            .catalog
            .collection(id)?
            .ok_or(NavigationError::NotFound)?;
        let mut chain = self.resolve_library(collection.library_id())?;
        let title = collection.title().to_string();
        chain.push((NodeRef::Collection(id.clone()), Some(title)));
        Ok(chain)
    }

    /// `[repository, library, virtual collection]`.
    fn resolve_virtual(
        &self,
        id: &filesquid_core::VirtualCollectionId,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        let found = self
            .catalog
            .virtual_collection(id)?
            .ok_or(NavigationError::NotFound)?;
        let mut chain = self.resolve_library(found.library_id())?;
        let title = found.title().to_string();
        chain.push((NodeRef::VirtualCollection(id.clone()), Some(title)));
        Ok(chain)
    }

    /// The chain down to a folder: its scope's chain, then one node per path prefix. The
    /// folder must be implied by committed ordinary files.
    fn resolve_folder(
        &self,
        scope: &FolderScope,
        path: &str,
    ) -> Result<Vec<(NodeRef, Option<String>)>, NavigationError> {
        let mut chain = match scope {
            FolderScope::Collection(id) => self.resolve_collection(id)?,
            FolderScope::Unfiled(id) => self.resolve_library(id)?,
        };
        if !self.folders.folder_exists(scope, path)? {
            return Err(NavigationError::NotFound);
        }
        let mut prefix = String::new();
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            let node = NodeRef::folder(scope.clone(), prefix.clone());
            chain.push((node, Some(segment.to_string())));
        }
        Ok(chain)
    }

    /// Scan the children of `parent` after `after`, in the fixed kind order, keeping those
    /// `principal` may read, until more than `limit` are found, the children end, or
    /// [`MAX_SCAN`] have been examined.
    fn scan(
        &self,
        principal: &Principal,
        parent: &NodeRef,
        after: Option<&Position>,
        limit: usize,
    ) -> Result<Scan, NavigationError> {
        let ranks = child_ranks(parent);
        if after.is_some_and(|after| !ranks.contains(&after.rank)) {
            return Err(wrong_level());
        }
        let sweep = Sweep {
            principal,
            parent,
            limit,
        };
        let mut scan = Scan {
            children: Vec::new(),
            next: None,
            budget: MAX_SCAN,
        };
        for rank in ranks {
            if after.is_some_and(|after| after.rank > *rank) {
                continue;
            }
            let resume = after.filter(|after| after.rank == *rank).cloned();
            self.scan_rank(&sweep, *rank, resume, &mut scan)?;
            if scan.children.len() > limit || scan.budget == 0 {
                return Ok(scan);
            }
        }
        scan.next = None;
        Ok(scan)
    }

    /// Scan the children of one kind into `scan`, spending its budget, from after `resume`.
    fn scan_rank(
        &self,
        sweep: &Sweep<'_>,
        rank: Rank,
        mut resume: Option<Position>,
        scan: &mut Scan,
    ) -> Result<(), NavigationError> {
        let batch_size = (sweep.limit + 1).clamp(16, 1_000);
        while scan.budget > 0 {
            let want = batch_size.min(scan.budget);
            let batch = self.fetch(sweep.parent, rank, resume.as_ref(), want)?;
            let fetched = batch.len();
            for child in batch {
                scan.budget -= 1;
                resume = Some(child.position.clone());
                if self.access.may_read(sweep.principal, &child.node) {
                    scan.children.push(child);
                }
                if scan.children.len() > sweep.limit || scan.budget == 0 {
                    scan.next = resume;
                    return Ok(());
                }
            }
            if fetched < want {
                return Ok(());
            }
        }
        Ok(())
    }

    /// One batch of the `rank` children of `parent` after `after`, at most `want`.
    fn fetch(
        &self,
        parent: &NodeRef,
        rank: Rank,
        after: Option<&Position>,
        want: usize,
    ) -> Result<Vec<Child>, NavigationError> {
        match (parent, rank) {
            (NodeRef::Repository(id), Rank::Library) => {
                let key = after.and_then(Position::page_key);
                let rows = self.catalog.libraries(id, key.as_ref(), want)?;
                Ok(rows
                    .into_iter()
                    .map(|row| {
                        row_child(
                            rank,
                            &row.key,
                            NodeRef::Library(row.item.library.id().clone()),
                            row.item.name,
                        )
                    })
                    .collect())
            }
            (NodeRef::Library(id), Rank::Collection) => {
                let key = after.and_then(Position::page_key);
                let rows = self.catalog.collections(id, key.as_ref(), want)?;
                Ok(rows
                    .into_iter()
                    .map(|row| {
                        let node = NodeRef::Collection(row.item.id().clone());
                        row_child(rank, &row.key, node, row.item.title().to_string())
                    })
                    .collect())
            }
            (NodeRef::Library(id), Rank::VirtualCollection) => {
                let key = after.and_then(Position::page_key);
                let rows = self.catalog.virtual_collections(id, key.as_ref(), want)?;
                Ok(rows
                    .into_iter()
                    .map(|row| {
                        let node = NodeRef::VirtualCollection(row.item.id().clone());
                        row_child(rank, &row.key, node, row.item.title().to_string())
                    })
                    .collect())
            }
            (_, Rank::Folder) => self.fetch_folders(parent, after, want),
            _ => Ok(Vec::new()),
        }
    }

    /// One batch of the folders directly below `parent`, after the position `after`.
    fn fetch_folders(
        &self,
        parent: &NodeRef,
        after: Option<&Position>,
        want: usize,
    ) -> Result<Vec<Child>, NavigationError> {
        let (scope, base) = match parent {
            NodeRef::Library(id) => (FolderScope::Unfiled(id.clone()), String::new()),
            NodeRef::Collection(id) => (FolderScope::Collection(id.clone()), String::new()),
            NodeRef::Folder { scope, path } => (scope.clone(), path.clone()),
            _ => return Ok(Vec::new()),
        };
        let after_name = after.map(|position| position.key.as_str());
        let names = self.folders.subfolders(&scope, &base, after_name, want)?;
        Ok(names
            .into_iter()
            .map(|name| folder_child(&scope, &base, name))
            .collect())
    }

    /// `change` with only the handles `principal` may read; `None` when it names none. An
    /// `all` change is unchanged: it names no handle.
    pub fn filter_change(
        &self,
        principal: &Principal,
        change: &super::NavigationChange,
    ) -> Option<super::NavigationChange> {
        let super::NavigationChange::Parents { parents } = change else {
            return Some(change.clone());
        };
        let readable: Vec<String> = parents
            .iter()
            .filter(|handle| {
                NodeRef::from_handle(handle).is_some_and(|node| self.may_read(principal, &node))
            })
            .cloned()
            .collect();
        (!readable.is_empty()).then_some(super::NavigationChange::Parents { parents: readable })
    }

    /// The filter of one subscriber's event stream: `navigation.change` events reduced to
    /// what `principal` may read (see [`Self::filter_change`]), every other event unchanged.
    pub fn event_filter(self: &Arc<Self>, principal: Principal) -> crate::events::EventFilter {
        let service = Arc::clone(self);
        Arc::new(move |event| {
            if event.topic != crate::events::topic::NAVIGATION_CHANGE {
                return Some(event);
            }
            let change: super::NavigationChange =
                serde_json::from_value(event.data.clone()).ok()?;
            service.filter_change(&principal, &change)?.to_event()
        })
    }
}

/// The kinds of child a `parent` lists, in the fixed order of the listing.
fn child_ranks(parent: &NodeRef) -> &'static [Rank] {
    match parent {
        NodeRef::Repository(_) => &[Rank::Library],
        NodeRef::Library(_) => &[Rank::Collection, Rank::VirtualCollection, Rank::Folder],
        NodeRef::Collection(_) | NodeRef::Folder { .. } => &[Rank::Folder],
        NodeRef::VirtualCollection(_) => &[],
    }
}

/// A catalog row as a child.
fn row_child(rank: Rank, key: &PageKey, node: NodeRef, name: String) -> Child {
    Child {
        node,
        name,
        position: Position::of_row(rank, key),
    }
}

/// The folder `name` below `base` in `scope` as a child.
fn folder_child(scope: &FolderScope, base: &str, name: String) -> Child {
    let path = if base.is_empty() {
        name.clone()
    } else {
        format!("{base}/{name}")
    };
    Child {
        position: Position::of_folder(&name),
        node: NodeRef::folder(scope.clone(), path),
        name,
    }
}

/// Keep at most `limit` children and the position the next page continues from.
fn split_page(scan: Scan, limit: usize) -> (Vec<Child>, Option<Position>) {
    let Scan {
        mut children, next, ..
    } = scan;
    if children.len() <= limit {
        return (children, next);
    }
    children.truncate(limit);
    let last = children.last().map(|child| child.position.clone());
    (children, last)
}

/// The view of `node`; a node with no handle cannot be shown.
fn view_of(
    node: &NodeRef,
    name: Option<String>,
    has_children: bool,
) -> Result<NodeView, NavigationError> {
    let Some(handle) = node.to_handle() else {
        tracing::info!(target: LOG_TARGET, "a navigation node has no handle");
        return Err(NavigationError::internal());
    };
    Ok(NodeView {
        node: handle,
        kind: kind_of(node),
        name,
        has_children,
    })
}

/// The wire kind of `node`.
fn kind_of(node: &NodeRef) -> NodeKind {
    match node {
        NodeRef::Repository(_) => NodeKind::Repository,
        NodeRef::Library(_) => NodeKind::Library,
        NodeRef::Collection(_) => NodeKind::Collection,
        NodeRef::VirtualCollection(_) => NodeKind::VirtualCollection,
        NodeRef::Folder { .. } => NodeKind::Folder,
    }
}

/// The breadcrumb of a resolved `chain`: every node above the last has the last below it, so
/// it has children; the last has them when `last_has_children`.
fn breadcrumb(
    chain: &[(NodeRef, Option<String>)],
    last_has_children: bool,
) -> Result<Vec<NodeView>, NavigationError> {
    let mut views = Vec::with_capacity(chain.len());
    for (index, (node, name)) in chain.iter().enumerate() {
        let is_last = index + 1 == chain.len();
        views.push(view_of(node, name.clone(), !is_last || last_has_children)?);
    }
    Ok(views)
}

/// The refusal of a cursor issued for another level (or any cursor at the repository level).
fn wrong_level() -> NavigationError {
    NavigationError::invalid("The paging position does not belong to this level. Start again.")
}

/// A limit in range, or `invalid_request`.
fn check_limit(limit: Option<usize>) -> Result<usize, NavigationError> {
    match limit {
        None => Ok(DEFAULT_LIMIT),
        Some(n) if (1..=MAX_LIMIT).contains(&n) => Ok(n),
        Some(_) => Err(NavigationError::invalid(
            "The page size must be a whole number from 1 to 1,000.",
        )),
    }
}
