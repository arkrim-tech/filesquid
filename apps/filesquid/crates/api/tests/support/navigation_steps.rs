//! Step definitions for `navigation_listing_API-S-021.feature` (Task API-F1.11): the
//! navigation listing driven through `NavigationService` over the reference `MemoryCatalog`
//! and `MemoryFolderIndex`, with readers whose access is a fixed set of nodes. The routes and a
//! real OCFL root are covered by `tests/integration/navigation.rs`.

use crate::library_fixture::Fixture;
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::auth::Principal;
use filesquid_api::library::{LibraryCatalog, LibraryCommand};
use filesquid_api::navigation::{
    FolderIndex, FolderScope, MemoryFolderIndex, NavigationAccess, NavigationChange,
    NavigationError, NavigationPage, NavigationPublisher, NavigationRequest, NavigationService,
    NodeRef, NodeView, RepositoryInfo, MAX_HANDLE_BYTES,
};
use filesquid_api::{ApiEvent, EventBus};
use filesquid_core::{CollectionId, LibraryId, VirtualCollectionId};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// Most pages one paging step reads (AGENTS.md section 22).
const MAX_PAGES: usize = 50;

/// Access granted by a fixed set of nodes.
struct ReadSet(HashSet<NodeRef>);

impl NavigationAccess for ReadSet {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        self.0.contains(node)
    }
}

/// Per-scenario state of the navigation steps.
#[derive(Default)]
pub struct NavigationWorld {
    fx: Option<Fixture>,
    library: Option<LibraryId>,
    collections: BTreeMap<String, CollectionId>,
    virtuals: BTreeMap<String, VirtualCollectionId>,
    index: Option<Arc<MemoryFolderIndex>>,
    photo_paths: Vec<String>,
    owner: Option<Arc<NavigationService>>,
    users: BTreeMap<String, Arc<NavigationService>>,
    current_user: Option<String>,
    page: Option<Result<NavigationPage, NavigationError>>,
    pages: Vec<NavigationPage>,
    errors: Vec<NavigationError>,
    bus: Option<EventBus>,
    receiver: Option<tokio::sync::broadcast::Receiver<ApiEvent>>,
    change: Option<NavigationChange>,
}

impl std::fmt::Debug for NavigationWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavigationWorld").finish_non_exhaustive()
    }
}

impl NavigationWorld {
    fn fx(&self) -> &Fixture {
        self.fx.as_ref().expect("fixture")
    }

    fn library(&self) -> LibraryId {
        self.library.clone().expect("library")
    }

    fn index(&self) -> Arc<MemoryFolderIndex> {
        self.index.clone().expect("index")
    }

    /// A service over the fixture whose callers read `nodes`.
    fn reader(&self, nodes: HashSet<NodeRef>) -> Arc<NavigationService> {
        let fx = self.fx();
        Arc::new(NavigationService::new(
            fx.catalog.clone() as Arc<dyn LibraryCatalog>,
            self.index() as Arc<dyn FolderIndex>,
            Arc::new(ReadSet(nodes)),
            RepositoryInfo::new(fx.repository.clone(), None),
        ))
    }

    /// The nodes of the fixture a reader of the whole tree may read: everything created so
    /// far plus every folder its files imply.
    fn everything(&self) -> Arc<NavigationService> {
        let fx = self.fx();
        Arc::new(NavigationService::new(
            fx.catalog.clone() as Arc<dyn LibraryCatalog>,
            self.index() as Arc<dyn FolderIndex>,
            Arc::new(filesquid_api::navigation::HostAccess),
            RepositoryInfo::new(fx.repository.clone(), None),
        ))
    }

    fn node(&self, kind: &str, name: &str) -> NodeRef {
        match kind {
            "library" => NodeRef::Library(self.library()),
            "collection" => NodeRef::Collection(self.collections[name].clone()),
            "virtual collection" => NodeRef::VirtualCollection(self.virtuals[name].clone()),
            other => panic!("unknown kind {other}"),
        }
    }

    /// List `node` (the repository level when `None`) with `request`'s paging for `user`
    /// (the owner when `None`).
    fn list(
        &self,
        user: Option<&str>,
        node: Option<&NodeRef>,
        cursor: Option<String>,
        limit: Option<usize>,
    ) -> Result<NavigationPage, NavigationError> {
        let service = match user {
            Some(name) => self.users[name].clone(),
            None => self.owner.clone().expect("owner service"),
        };
        let request = NavigationRequest {
            node: node.and_then(NodeRef::to_handle),
            cursor,
            limit,
        };
        service.list(&Principal::Owner, &request)
    }

    fn answer(&self) -> &NavigationPage {
        self.page
            .as_ref()
            .expect("an answer")
            .as_ref()
            .expect("a successful answer")
    }
}

/// The names of `nodes`, in order.
fn names(nodes: &[NodeView]) -> Vec<String> {
    nodes
        .iter()
        .map(|n| n.name.clone().unwrap_or_default())
        .collect()
}

/// The wire names of the kinds of `nodes`, in order.
fn kinds(nodes: &[NodeView]) -> Vec<String> {
    nodes
        .iter()
        .map(|n| {
            serde_json::to_value(n.kind)
                .expect("kind")
                .as_str()
                .expect("text")
                .to_string()
        })
        .collect()
}

#[given(
    expr = "a navigation fixture with library {string} holding collection {string} and virtual collection {string}"
)]
fn fixture(w: &mut MonitorWorld, library: String, collection: String, virtual_name: String) {
    let nav = &mut w.navigation;
    let fx = Fixture::new();
    let created = fx.create_library(&library);
    let library_id = LibraryId::new(created.created_id.expect("id")).expect("library id");
    let made = fx.run_in(
        &library_id,
        LibraryCommand::CreateCollection {
            library_id: library_id.clone(),
            title: collection.clone(),
            folder_id: None,
            sort: Default::default(),
        },
    );
    nav.collections.insert(
        collection,
        CollectionId::new(made.created_id.expect("id")).expect("id"),
    );
    let made = fx.run_in(
        &library_id,
        LibraryCommand::CreateVirtualCollection {
            library_id: library_id.clone(),
            title: virtual_name.clone(),
            expression: None,
        },
    );
    nav.virtuals.insert(
        virtual_name,
        VirtualCollectionId::new(made.created_id.expect("id")).expect("id"),
    );
    nav.library = Some(library_id);
    nav.fx = Some(fx);
    nav.index = Some(Arc::new(MemoryFolderIndex::new()));
    nav.owner = Some(nav.everything());
}

#[given(expr = "collection {string} holds files at logical paths {string} and {string}")]
fn collection_files(w: &mut MonitorWorld, collection: String, first: String, second: String) {
    let nav = &mut w.navigation;
    let scope = FolderScope::Collection(nav.collections[&collection].clone());
    for path in [first, second] {
        nav.index().add_file(&scope, &path);
        nav.photo_paths.push(path);
    }
}


#[when(expr = "the owner lists the navigation tree without a node")]
fn list_root(w: &mut MonitorWorld) {
    let nav = &mut w.navigation;
    nav.page = Some(nav.list(None, None, None, None));
}

#[when(expr = "the owner lists the children of library {string}")]
fn list_library(w: &mut MonitorWorld, name: String) {
    let nav = &mut w.navigation;
    let node = nav.node("library", &name);
    nav.page = Some(nav.list(None, Some(&node), None, None));
}

#[when(expr = "the owner lists the children of folder {string} in collection {string}")]
fn list_folder(w: &mut MonitorWorld, path: String, collection: String) {
    let nav = &mut w.navigation;
    let scope = FolderScope::Collection(nav.collections[&collection].clone());
    let node = NodeRef::folder(scope, path);
    nav.page = Some(nav.list(None, Some(&node), None, None));
}

#[when(expr = "the owner lists the children of virtual collection {string}")]
fn list_virtual(w: &mut MonitorWorld, name: String) {
    let nav = &mut w.navigation;
    let node = nav.node("virtual collection", &name);
    nav.page = Some(nav.list(None, Some(&node), None, None));
}

#[given(expr = "every file under {string} in collection {string} is {word}")]
fn files_out_of_view(w: &mut MonitorWorld, prefix: String, collection: String, _state: String) {
    let nav = &w.navigation;
    let scope = FolderScope::Collection(nav.collections[&collection].clone());
    for path in nav
        .photo_paths
        .iter()
        .filter(|p| p.starts_with(&format!("{prefix}/")))
    {
        nav.index().set_ordinary(&scope, path, false);
    }
}

#[given(expr = "collection {string} holds {int} top-level folders")]
fn many_folders(w: &mut MonitorWorld, collection: String, count: usize) {
    let nav = &w.navigation;
    let scope = FolderScope::Collection(nav.collections[&collection].clone());
    for n in (0..count).rev() {
        nav.index()
            .add_file(&scope, &format!("folder-{n:04}/file.txt"));
    }
}

#[when(expr = "the owner pages through the children of collection {string} {int} at a time")]
fn page_through(w: &mut MonitorWorld, collection: String, size: usize) {
    let nav = &mut w.navigation;
    let node = nav.node("collection", &collection);
    nav.pages.clear();
    let mut cursor = None;
    for _ in 0..MAX_PAGES {
        let page = nav
            .list(None, Some(&node), cursor.clone(), Some(size))
            .expect("page");
        cursor = page.next_cursor.clone();
        nav.pages.push(page);
        if cursor.is_none() {
            return;
        }
    }
    panic!("paging did not end within {MAX_PAGES} pages");
}

#[when(expr = "the owner lists the children of library {string} with {}")]
fn list_with(w: &mut MonitorWorld, name: String, request: String) {
    let nav = &mut w.navigation;
    let node = nav.node("library", &name);
    let (cursor, limit) = match request.as_str() {
        "a malformed cursor" => (Some("not-a-cursor".to_string()), None),
        "a limit of 0" => (None, Some(0)),
        "a limit of 1001" => (None, Some(1001)),
        other => panic!("unknown request {other}"),
    };
    nav.page = Some(nav.list(None, Some(&node), cursor, limit));
}

#[then(expr = "the answer should hold exactly one node of kind {string}")]
fn one_node_of_kind(w: &mut MonitorWorld, kind: String) {
    let page = w.navigation.answer();
    assert_eq!(kinds(&page.nodes), vec![kind]);
}

#[then(expr = "the kinds should be {string} and {string} in that order")]
fn two_kinds_in_order(w: &mut MonitorWorld, a: String, b: String) {
    assert_eq!(kinds(&w.navigation.answer().nodes), vec![a, b]);
}

#[then(expr = "the last node should be the one {string} node, with no name and no children")]
fn last_is_recently_deleted(w: &mut MonitorWorld, kind: String) {
    let page = w.navigation.answer();
    let found = kinds(&page.nodes);
    assert_eq!(found.iter().filter(|k| **k == kind).count(), 1, "{found:?}");
    let last = page.nodes.last().expect("a node");
    assert_eq!(found.last(), Some(&kind));
    assert_eq!(last.name, None);
    assert!(!last.has_children);
}

#[then(expr = "the kinds should be {string}, {string} and {string} in that order")]
fn kinds_in_order(w: &mut MonitorWorld, a: String, b: String, c: String) {
    assert_eq!(kinds(&w.navigation.answer().nodes), vec![a, b, c]);
}

#[then(expr = "the folder should be named {string}")]
fn folder_named(w: &mut MonitorWorld, name: String) {
    let page = w.navigation.answer();
    let folder = page
        .nodes
        .iter()
        .find(|n| kinds(std::slice::from_ref(n))[0] == "folder");
    assert_eq!(folder.and_then(|n| n.name.clone()), Some(name));
}

#[then(expr = "the answer should hold folders {string} and {string}")]
fn holds_folders(w: &mut MonitorWorld, a: String, b: String) {
    let page = w.navigation.answer();
    assert_eq!(names(&page.nodes), vec![a, b]);
    assert!(kinds(&page.nodes).iter().all(|k| k == "folder"));
}

#[then(expr = "no file should be listed")]
fn no_file(w: &mut MonitorWorld) {
    let page = w.navigation.answer();
    assert!(
        names(&page.nodes).iter().all(|n| !n.contains('.')),
        "{page:?}"
    );
}

#[then(
    expr = "the ancestors should be the repository, {string}, {string}, {string} and {string} in that order"
)]
fn ancestors_are(w: &mut MonitorWorld, a: String, b: String, c: String, d: String) {
    let page = w.navigation.answer();
    assert_eq!(kinds(&page.ancestors)[0], "repository");
    assert_eq!(names(&page.ancestors)[1..], [a, b, c, d]);
}

#[then(expr = "the answer should be empty")]
fn answer_empty(w: &mut MonitorWorld) {
    assert!(w.navigation.answer().nodes.is_empty());
}

#[then(expr = "the virtual collection should report no children")]
fn virtual_no_children(w: &mut MonitorWorld) {
    let page = w.navigation.answer();
    let last = page.ancestors.last().expect("the node itself");
    assert!(!last.has_children);
}

#[then(expr = "the answer should hold only folder {string}")]
fn only_folder(w: &mut MonitorWorld, name: String) {
    assert_eq!(names(&w.navigation.answer().nodes), vec![name]);
}

#[then(expr = "every folder should appear exactly once in a stable order")]
fn exactly_once(w: &mut MonitorWorld) {
    let all: Vec<String> = w
        .navigation
        .pages
        .iter()
        .flat_map(|p| names(&p.nodes))
        .collect();
    let mut sorted = all.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(all, sorted, "no repeat, in code-point order");
    assert_eq!(
        all.len(),
        1002,
        "1,001 folders and the fixture's own `2025`"
    );
    assert_eq!(w.navigation.pages.len(), 6);
}

#[then(
    expr = "no node handle should contain a filesystem path, storage key, OCFL content path, SQL or token"
)]
fn handles_opaque(w: &mut MonitorWorld) {
    let page = w.navigation.answer();
    let handles: Vec<&String> = page
        .nodes
        .iter()
        .chain(&page.ancestors)
        .map(|n| &n.node)
        .collect();
    for handle in handles {
        for needle in ["/", "objects", "content", "SELECT", "2025", "token"] {
            assert!(!handle.contains(needle), "{needle} in {handle}");
        }
    }
}

#[then(expr = "no node handle should be longer than 512 bytes")]
fn handles_short(w: &mut MonitorWorld) {
    let page = w.navigation.answer();
    assert!(page
        .nodes
        .iter()
        .chain(&page.ancestors)
        .all(|n| n.node.len() <= MAX_HANDLE_BYTES));
}

#[when(expr = "the owner renames collection {string} to {string}")]
fn rename_collection(w: &mut MonitorWorld, from: String, to: String) {
    let nav = &mut w.navigation;
    let bus = nav.bus.clone().expect("a subscribed client");
    let fx = nav.fx.as_mut().expect("fixture");
    fx.service = fx
        .service
        .clone()
        .with_navigation(NavigationPublisher::new(bus));
    let collection = nav.collections[&from].clone();
    let library = nav.library();
    nav.fx().run_in(
        &library,
        LibraryCommand::RenameCollection {
            library_id: library.clone(),
            collection_id: collection,
            title: to,
        },
    );
}

#[given(expr = "a client subscribed to the event channel")]
fn subscribed(w: &mut MonitorWorld) {
    let bus = EventBus::new();
    w.navigation.receiver = Some(bus.subscribe());
    w.navigation.bus = Some(bus);
}

/// The navigation changes the subscribed client has received so far.
fn received(nav: &mut NavigationWorld) -> Vec<NavigationChange> {
    let receiver = nav.receiver.as_mut().expect("subscribed");
    let mut changes = Vec::new();
    for _ in 0..MAX_PAGES {
        match receiver.try_recv() {
            Ok(event) if event.topic == "navigation.change" => {
                changes.push(serde_json::from_value(event.data).expect("change"));
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    changes
}

#[then(expr = "the client should receive one {string} event naming the handle of library {string}")]
fn one_event_for_library(w: &mut MonitorWorld, _topic: String, _library: String) {
    let nav = &mut w.navigation;
    let handle = NodeRef::Library(nav.library()).to_handle().expect("handle");
    let changes = received(nav);
    assert_eq!(
        changes,
        vec![NavigationChange::Parents {
            parents: vec![handle]
        }]
    );
}

#[given(expr = "library {string} also holds collection {string}")]
fn also_holds(w: &mut MonitorWorld, _library: String, title: String) {
    let nav = &mut w.navigation;
    let library = nav.library();
    let made = nav.fx().run_in(
        &library,
        LibraryCommand::CreateCollection {
            library_id: library.clone(),
            title: title.clone(),
            folder_id: None,
            sort: Default::default(),
        },
    );
    nav.collections.insert(
        title,
        CollectionId::new(made.created_id.expect("id")).expect("id"),
    );
}

/// The nodes a subscriber named by `collections` may read, with the repository and library.
fn readable(nav: &NavigationWorld, library: bool, collections: &[&str]) -> HashSet<NodeRef> {
    let mut nodes = HashSet::new();
    nodes.insert(NodeRef::Repository(nav.fx().repository.clone()));
    if library {
        nodes.insert(NodeRef::Library(nav.library()));
    }
    for title in collections {
        nodes.insert(NodeRef::Collection(nav.collections[*title].clone()));
    }
    nodes
}

#[given(
    expr = "subscriber {string} may read library {string}, collection {string} and collection {string}"
)]
fn subscriber_all(w: &mut MonitorWorld, name: String, _library: String, a: String, b: String) {
    let nav = &mut w.navigation;
    let nodes = readable(nav, true, &[&a, &b]);
    nav.users.insert(name, nav.reader(nodes));
}

#[given(
    expr = "subscriber {string} may read library {string} and collection {string} but not collection {string}"
)]
fn subscriber_some(w: &mut MonitorWorld, name: String, _library: String, yes: String, _no: String) {
    let nav = &mut w.navigation;
    let nodes = readable(nav, true, &[&yes]);
    nav.users.insert(name, nav.reader(nodes));
}

#[given(expr = "subscriber {string} may read nothing in library {string}")]
fn subscriber_none(w: &mut MonitorWorld, name: String, _library: String) {
    let nav = &mut w.navigation;
    let nodes = readable(nav, false, &[]);
    nav.users.insert(name, nav.reader(nodes));
}

#[when(
    expr = "the API announces one committed move of a folder from collection {string} into collection {string}"
)]
fn committed_move(w: &mut MonitorWorld, from: String, into: String) {
    let nav = &mut w.navigation;
    let bus = EventBus::new();
    let mut receiver = bus.subscribe();
    let parents = [
        NodeRef::Collection(nav.collections[&from].clone()),
        NodeRef::Collection(nav.collections[&into].clone()),
    ];
    NavigationPublisher::new(bus).publish(&parents);
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    assert_eq!(events.len(), 1, "one event for one committed move");
    nav.change = Some(serde_json::from_value(events.remove(0).data).expect("change"));
}

/// The handles `user` is told about, or `None` when it receives no event.
fn told(nav: &NavigationWorld, user: &str) -> Option<Vec<String>> {
    let change = nav.change.as_ref().expect("a change");
    match nav.users[user].filter_change(&Principal::Owner, change)? {
        NavigationChange::Parents { parents } => Some(parents),
        NavigationChange::All { .. } => panic!("unexpected all"),
    }
}

fn handles_of(nav: &NavigationWorld, titles: &[&str]) -> Vec<String> {
    titles
        .iter()
        .map(|t| {
            NodeRef::Collection(nav.collections[*t].clone())
                .to_handle()
                .expect("handle")
        })
        .collect()
}

#[then(
    expr = "{string} should receive one {string} event naming the handles of collections {string} and {string}"
)]
fn receives_both(w: &mut MonitorWorld, user: String, _topic: String, a: String, b: String) {
    let nav = &w.navigation;
    assert_eq!(told(nav, &user), Some(handles_of(nav, &[&a, &b])));
}

#[then(
    expr = "{string} should receive one {string} event naming only the handle of collection {string}"
)]
fn receives_only(w: &mut MonitorWorld, user: String, _topic: String, only: String) {
    let nav = &w.navigation;
    assert_eq!(told(nav, &user), Some(handles_of(nav, &[&only])));
}

#[then(expr = "{string} should receive no {string} event")]
fn receives_none(w: &mut MonitorWorld, user: String, _topic: String) {
    assert_eq!(told(&w.navigation, &user), None);
}

#[then(expr = "the request should be refused as {string}")]
fn refused_as(w: &mut MonitorWorld, code: String) {
    let err = w
        .navigation
        .page
        .as_ref()
        .expect("an answer")
        .as_ref()
        .expect_err("refused");
    assert_eq!(err.error_code(), code);
}

#[given(
    expr = "a second user who may read library {string} and virtual collection {string} but not collection {string}"
)]
fn second_user(w: &mut MonitorWorld, _library: String, virtual_name: String, _collection: String) {
    let nav = &mut w.navigation;
    let mut nodes = readable(nav, true, &[]);
    nodes.insert(NodeRef::VirtualCollection(
        nav.virtuals[&virtual_name].clone(),
    ));
    nav.users.insert("second".into(), nav.reader(nodes));
    nav.current_user = Some("second".into());
}

#[given(expr = "a third user who may read library {string} and nothing inside it")]
fn third_user(w: &mut MonitorWorld, _library: String) {
    let nav = &mut w.navigation;
    let nodes = readable(nav, true, &[]);
    nav.users.insert("third".into(), nav.reader(nodes));
    nav.current_user = Some("third".into());
}

#[when(
    expr = "that user lists the children of collection {string} and of a node that does not exist"
)]
fn list_unreadable_and_absent(w: &mut MonitorWorld, collection: String) {
    let nav = &mut w.navigation;
    let user = nav.current_user.clone().expect("user");
    let hidden = nav.node("collection", &collection);
    let absent = NodeRef::Collection(CollectionId::new("no-such-collection").expect("id"));
    nav.errors = [hidden, absent]
        .iter()
        .map(|node| {
            nav.list(Some(&user), Some(node), None, None)
                .expect_err("not found")
        })
        .collect();
}

#[then(expr = "both answers should be the same {string} error")]
fn same_error(w: &mut MonitorWorld, code: String) {
    let errors = &w.navigation.errors;
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0], errors[1]);
    assert_eq!(errors[0].error_code(), code);
}

#[when(expr = "that user lists the children of the repository")]
fn user_lists_repository(w: &mut MonitorWorld) {
    let nav = &mut w.navigation;
    let user = nav.current_user.clone().expect("user");
    let root = nav.list(Some(&user), None, None, None).expect("root");
    let repository = NodeRef::from_handle(&root.nodes[0].node).expect("repository");
    nav.page = Some(nav.list(Some(&user), Some(&repository), None, None));
}

#[when(expr = "that user lists the children of library {string}")]
fn user_lists_library(w: &mut MonitorWorld, name: String) {
    let nav = &mut w.navigation;
    let user = nav.current_user.clone().expect("user");
    let node = nav.node("library", &name);
    nav.page = Some(nav.list(Some(&user), Some(&node), None, None));
}

#[then(expr = "library {string} should report has_children {word}")]
fn library_has_children(w: &mut MonitorWorld, _library: String, expected: String) {
    let page = w.navigation.answer();
    assert_eq!(page.nodes[0].has_children, expected == "true");
}

#[then(expr = "the answer should hold only virtual collection {string}")]
fn only_virtual(w: &mut MonitorWorld, name: String) {
    let page = w.navigation.answer();
    assert_eq!(kinds(&page.nodes), vec!["virtual_collection".to_string()]);
    assert_eq!(names(&page.nodes), vec![name]);
}
