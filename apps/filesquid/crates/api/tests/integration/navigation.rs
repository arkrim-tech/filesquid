//! Integration tests of the navigation listing (Task API-F1.11; API-S-021, API-S-005,
//! API-S-017, API-S-019, OCFL-S-012) through the router, against the in-memory catalog and a
//! real OCFL storage root: every level of the tree, folders implied by nested logical paths,
//! hidden and deleted files leaving no folder, ancestors for a deep folder, stable paging
//! across 1,001 siblings, the cursor and limit refusals, identical `not_found` answers,
//! per-user readability, and `navigation.change` events.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/library_fixture.rs"]
mod library_fixture;

#[path = "../support/navigation_fixture.rs"]
mod navigation_fixture;

use axum::http::StatusCode;
use filesquid_api::auth::Principal;
use filesquid_api::events::topic;
use filesquid_api::library::LibraryCommand;
use filesquid_api::navigation::{
    FolderScope, MemoryFolderIndex, NavigationChange, NavigationPublisher, NavigationRequest,
    NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_api::ApiEvent;
use navigation_fixture::NavRig;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

/// The `nodes` of an answer as (kind, name) pairs.
fn kinds(page: &Value) -> Vec<(String, Option<String>)> {
    page["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|n| {
            (
                n["kind"].as_str().expect("kind").to_string(),
                n["name"].as_str().map(str::to_string),
            )
        })
        .collect()
}

/// Assert `answer` is the typed error `code`.
fn assert_error(answer: &files_fixture::Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

// ---- Levels ----

#[tokio::test(flavor = "multi_thread")]
async fn the_root_level_is_the_hosts_one_repository() {
    let rig = NavRig::new().await;
    let answer = rig.get("").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let page = answer.json();
    assert_eq!(kinds(&page), vec![("repository".to_string(), None)]);
    assert_eq!(page["nodes"][0]["has_children"], true);
    assert_eq!(page["ancestors"], json!([]));
    assert_eq!(page["next_cursor"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_repository_lists_its_libraries_with_the_repository_as_ancestor() {
    let rig = NavRig::new().await;
    let root = rig.get("").await.json();
    let repository = root["nodes"][0]["node"]
        .as_str()
        .expect("handle")
        .to_string();
    let page = rig.get(&format!("node={repository}")).await.json();
    assert_eq!(
        kinds(&page),
        vec![("library".to_string(), Some("Family archive".to_string()))]
    );
    assert_eq!(page["nodes"][0]["has_children"], true);
    assert_eq!(page["ancestors"].as_array().expect("ancestors").len(), 1);
    assert_eq!(page["ancestors"][0]["kind"], "repository");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_library_lists_collections_then_virtual_collections_then_unfiled_folders() {
    let rig = NavRig::new().await;
    let library = rig.handle(&NodeRef::Library(rig.library.clone()));
    let page = rig.get(&format!("node={library}")).await.json();
    assert_eq!(
        kinds(&page),
        vec![
            ("collection".to_string(), Some("Photos".to_string())),
            (
                "virtual_collection".to_string(),
                Some("Best of 2025".to_string())
            ),
            ("folder".to_string(), Some("inbox".to_string())),
        ]
    );
    let flags: Vec<bool> = page["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|n| n["has_children"].as_bool().expect("flag"))
        .collect();
    assert_eq!(flags, vec![true, false, false], "{page}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_collection_lists_the_folders_its_files_imply_and_never_a_file() {
    let rig = NavRig::new().await;
    let collection = rig.handle(&NodeRef::Collection(rig.photos.clone()));
    let top = rig.get(&format!("node={collection}")).await.json();
    assert_eq!(
        kinds(&top),
        vec![("folder".to_string(), Some("2025".to_string()))]
    );
    let year = rig.photos_folder("2025");
    let page = rig.get(&format!("node={year}")).await.json();
    assert_eq!(
        kinds(&page),
        vec![
            ("folder".to_string(), Some("summer".to_string())),
            ("folder".to_string(), Some("winter".to_string())),
        ]
    );
    let leaf = rig.photos_folder("2025/summer");
    let page = rig.get(&format!("node={leaf}")).await.json();
    assert_eq!(
        page["nodes"],
        json!([]),
        "a folder of files only has no children"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn one_call_fills_the_breadcrumb_for_a_deep_folder() {
    let rig = NavRig::new().await;
    let leaf = rig.photos_folder("2025/summer");
    let page = rig.get(&format!("node={leaf}")).await.json();
    let trail: Vec<(String, Option<String>)> = page["ancestors"]
        .as_array()
        .expect("ancestors")
        .iter()
        .map(|n| {
            (
                n["kind"].as_str().expect("kind").to_string(),
                n["name"].as_str().map(str::to_string),
            )
        })
        .collect();
    assert_eq!(
        trail,
        vec![
            ("repository".to_string(), None),
            ("library".to_string(), Some("Family archive".to_string())),
            ("collection".to_string(), Some("Photos".to_string())),
            ("folder".to_string(), Some("2025".to_string())),
            ("folder".to_string(), Some("summer".to_string())),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_virtual_collection_has_no_children_here() {
    let rig = NavRig::new().await;
    let handle = rig.handle(&NodeRef::VirtualCollection(rig.best_of.clone()));
    let page = rig.get(&format!("node={handle}")).await.json();
    assert_eq!(page["nodes"], json!([]));
    assert_eq!(page["ancestors"].as_array().expect("ancestors").len(), 3);
    assert_eq!(page["ancestors"][2]["has_children"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed() {
    let rig = NavRig::new().await;
    rig.folders
        .hide(&FolderScope::Collection(rig.photos.clone()), "2025/winter");
    let year = rig.photos_folder("2025");
    let page = rig.get(&format!("node={year}")).await.json();
    assert_eq!(
        kinds(&page),
        vec![("folder".to_string(), Some("summer".to_string()))]
    );
    let winter = rig.photos_folder("2025/winter");
    let gone = rig.get(&format!("node={winter}")).await;
    assert_error(&gone, 404, "not_found");
}

// ---- Paging ----

/// A service over an in-memory index holding `count` top-level folders of collection
/// "Photos", named so that code-point order differs from numeric and insertion order.
fn many_folders(rig: &NavRig, count: usize) -> Arc<NavigationService> {
    let index = MemoryFolderIndex::new();
    let scope = FolderScope::Collection(rig.photos.clone());
    for n in (0..count).rev() {
        index.add_file(&scope, &format!("folder-{n}/file.txt"));
    }
    index.add_file(&scope, "É/file.txt");
    index.add_file(&scope, "a/file.txt");
    Arc::new(NavigationService::new(
        rig.lib.catalog.clone(),
        Arc::new(index),
        Arc::new(filesquid_api::navigation::HostAccess),
        RepositoryInfo::new(rig.lib.repository.clone(), None),
    ))
}

#[tokio::test(flavor = "multi_thread")]
async fn paging_across_1001_siblings_repeats_nothing_and_leaves_no_gap() {
    let rig = NavRig::new().await;
    let service = many_folders(&rig, 1001);
    let node = rig.handle(&NodeRef::Collection(rig.photos.clone()));
    let mut names = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    for _ in 0..20 {
        let request = NavigationRequest {
            node: Some(node.clone()),
            cursor: cursor.clone(),
            limit: Some(200),
        };
        let page = NavRig::list(service.clone(), request).await.expect("page");
        names.extend(page.nodes.iter().map(|n| n.name.clone().expect("name")));
        pages += 1;
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(pages, 6);
    assert_eq!(names.len(), 1003);
    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), 1003, "no repeat");
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "Unicode code-point order");
    assert_eq!(names.last().map(String::as_str), Some("É"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_page_holds_200_and_the_largest_1000() {
    let rig = NavRig::new().await;
    let service = many_folders(&rig, 1001);
    let node = rig.handle(&NodeRef::Collection(rig.photos.clone()));
    let first = NavigationRequest {
        node: Some(node.clone()),
        ..NavigationRequest::default()
    };
    let page = NavRig::list(service.clone(), first).await.expect("page");
    assert_eq!(page.nodes.len(), 200);
    let widest = NavigationRequest {
        node: Some(node),
        limit: Some(1000),
        ..NavigationRequest::default()
    };
    let page = NavRig::list(service, widest).await.expect("page");
    assert_eq!(page.nodes.len(), 1000);
    assert!(page.next_cursor.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_through_the_router_carries_an_opaque_cursor_that_continues_the_listing() {
    let rig = NavRig::new().await;
    let library = rig.handle(&NodeRef::Library(rig.library.clone()));
    let first = rig.get(&format!("node={library}&limit=2")).await.json();
    assert_eq!(first["nodes"].as_array().expect("nodes").len(), 2);
    let cursor = first["next_cursor"].as_str().expect("cursor").to_string();
    let second = rig
        .get(&format!("node={library}&limit=2&cursor={cursor}"))
        .await
        .json();
    assert_eq!(
        kinds(&second),
        vec![("folder".to_string(), Some("inbox".to_string()))]
    );
    assert_eq!(second["next_cursor"], Value::Null);
}

// ---- Refusals ----

#[tokio::test(flavor = "multi_thread")]
async fn an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once() {
    let rig = NavRig::new().await;
    let library = rig.handle(&NodeRef::Library(rig.library.clone()));
    for bad in [
        "cursor=not-a-cursor".to_string(),
        "cursor=c1.zz".to_string(),
        "limit=0".to_string(),
        "limit=1001".to_string(),
        "limit=abc".to_string(),
        "limit=-1".to_string(),
        "extra=1".to_string(),
    ] {
        let answer = rig.get(&format!("node={library}&{bad}")).await;
        assert_error(&answer, 400, "invalid_request");
    }
    rig.photos_rig.monitor.journal().flush();
    let journal = rig.photos_rig.journal();
    let refusals: Vec<Value> = journal
        .iter()
        .map(|e| serde_json::to_value(e).expect("entry"))
        .filter(|e| e["operation"] == "navigation.list")
        .collect();
    assert_eq!(
        refusals.len(),
        7,
        "one journal entry per refusal: {refusals:?}"
    );
    assert!(refusals
        .iter()
        .all(|e| e["outcome"] == "failed" && e["error_code"] == "invalid_request"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cursor_of_the_wrong_level_is_refused() {
    let rig = NavRig::new().await;
    let library = rig.handle(&NodeRef::Library(rig.library.clone()));
    let first = rig.get(&format!("node={library}&limit=1")).await.json();
    let cursor = first["next_cursor"].as_str().expect("cursor").to_string();
    let collection = rig.handle(&NodeRef::Collection(rig.photos.clone()));
    let answer = rig.get(&format!("node={collection}&cursor={cursor}")).await;
    assert_error(&answer, 400, "invalid_request");
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_unreadable_and_malformed_nodes_get_the_same_answer() {
    let rig = NavRig::new().await;
    let photos = NodeRef::Collection(rig.photos.clone());
    let reader = Arc::new(rig.user({
        let photos = photos.clone();
        move |node| !matches!(node, NodeRef::Collection(_)) || *node != photos
    }));
    let unreadable = rig.handle(&photos);
    let absent = rig.handle(&NodeRef::Collection(
        filesquid_core::CollectionId::new("no-such-collection").expect("id"),
    ));
    let mut bodies = Vec::new();
    for node in [
        unreadable,
        absent,
        "n1.not-a-handle".to_string(),
        "zzz".to_string(),
    ] {
        let request = NavigationRequest {
            node: Some(node),
            ..NavigationRequest::default()
        };
        let err = NavRig::list(reader.clone(), request)
            .await
            .expect_err("not found");
        bodies.push(serde_json::to_value(filesquid_api::ApiError::from(err)).expect("body"));
    }
    assert!(
        bodies.windows(2).all(|pair| pair[0] == pair[1]),
        "{bodies:?}"
    );
    assert_eq!(bodies[0]["code"], "not_found");
}

// ---- Readability ----

#[tokio::test(flavor = "multi_thread")]
async fn only_readable_children_are_listed_and_counted() {
    let rig = NavRig::new().await;
    let best_of = NodeRef::VirtualCollection(rig.best_of.clone());
    let library = NodeRef::Library(rig.library.clone());
    let second = Arc::new(rig.user(move |node| {
        matches!(node, NodeRef::Repository(_)) || *node == library || *node == best_of
    }));
    let root = NavRig::list(second.clone(), NavigationRequest::default())
        .await
        .expect("root");
    let repository = root.nodes[0].node.clone();
    let libraries = NavRig::list(
        second.clone(),
        NavigationRequest {
            node: Some(repository),
            ..NavigationRequest::default()
        },
    )
    .await
    .expect("libraries");
    assert!(libraries.nodes[0].has_children);
    let children = NavRig::list(
        second,
        NavigationRequest {
            node: Some(libraries.nodes[0].node.clone()),
            ..NavigationRequest::default()
        },
    )
    .await
    .expect("children");
    let listed: Vec<&str> = children
        .nodes
        .iter()
        .map(|n| n.name.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(listed, vec!["Best of 2025"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_library_with_no_readable_child_reports_none() {
    let rig = NavRig::new().await;
    let library = NodeRef::Library(rig.library.clone());
    let third =
        Arc::new(rig.user(move |node| matches!(node, NodeRef::Repository(_)) || *node == library));
    let root = NavRig::list(third.clone(), NavigationRequest::default())
        .await
        .expect("root");
    let libraries = NavRig::list(
        third.clone(),
        NavigationRequest {
            node: Some(root.nodes[0].node.clone()),
            ..NavigationRequest::default()
        },
    )
    .await
    .expect("libraries");
    assert!(!libraries.nodes[0].has_children);
    let children = NavRig::list(
        third,
        NavigationRequest {
            node: Some(libraries.nodes[0].node.clone()),
            ..NavigationRequest::default()
        },
    )
    .await
    .expect("children");
    assert!(children.nodes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_under_an_unreadable_ancestor_is_not_found() {
    let rig = NavRig::new().await;
    let library = NodeRef::Library(rig.library.clone());
    let user = Arc::new(rig.user(move |node| *node != library));
    let request = NavigationRequest {
        node: Some(rig.photos_folder("2025")),
        ..NavigationRequest::default()
    };
    let err = NavRig::list(user, request)
        .await
        .expect_err("hidden by its library");
    assert_eq!(err.error_code(), "not_found");
}

// ---- Opacity ----

#[tokio::test(flavor = "multi_thread")]
async fn node_handles_are_opaque_and_at_most_512_bytes() {
    let rig = NavRig::new().await;
    let year = rig.photos_folder("2025/summer");
    let page = rig.get(&format!("node={year}")).await;
    let text = String::from_utf8_lossy(&page.body).to_string();
    for needle in [
        "objects/",
        "/v1/",
        "content/",
        "2025/summer",
        ".jpg",
        "SELECT",
        &rig.photos_rig.token,
    ] {
        assert!(!text.contains(needle), "{needle} in {text}");
    }
    let handles = page.json();
    for node in handles["ancestors"].as_array().expect("ancestors") {
        let handle = node["node"].as_str().expect("handle");
        assert!(handle.len() <= 512, "{handle}");
        assert!(!handle.contains('/'), "{handle}");
    }
}

// ---- Authorisation ----

#[tokio::test(flavor = "multi_thread")]
async fn the_route_is_authenticated_and_declares_its_action() {
    let rig = NavRig::new().await;
    let anonymous = files_fixture::send(
        &rig.photos_rig.state,
        axum::http::Method::GET,
        "/api/v1/navigation",
        &[],
        Vec::new(),
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let declared = filesquid_api::route_actions()
        .into_iter()
        .find(|(path, _)| path == "/api/v1/navigation")
        .expect("routed");
    assert_eq!(declared.1.as_str(), "navigation.read");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_installed_service_the_route_answers_unavailable() {
    let rig = files_fixture::FilesRig::new().await;
    let answer = rig.get("/api/v1/navigation").await;
    assert_error(&answer, 503, "unavailable");
}

// ---- Events ----

/// The next navigation change on `subscription`, skipping the journal and process events that
/// share the bus, or `None` after a short wait (at most 50 events are looked at).
async fn next_change(subscription: &mut filesquid_api::EventSubscription) -> Option<ApiEvent> {
    let wait = std::time::Duration::from_millis(300);
    for _ in 0..50 {
        match tokio::time::timeout(wait, subscription.next()).await {
            Ok(Some(event)) if event.topic == topic::NAVIGATION_CHANGE => return Some(event),
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => return None,
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread")]
async fn renaming_a_collection_publishes_one_change_naming_its_library() {
    let mut rig = NavRig::new().await;
    let bus = rig.photos_rig.state.events.clone();
    rig.lib.service = rig
        .lib
        .service
        .clone()
        .with_navigation(NavigationPublisher::new(bus.clone()));
    let mut subscription = bus.subscription().expect("slot");
    rig.lib.run(LibraryCommand::RenameCollection {
        library_id: rig.library.clone(),
        collection_id: rig.photos.clone(),
        title: "Pictures".to_string(),
    });
    let event = next_change(&mut subscription).await.expect("one event");
    assert_eq!(event.topic, "navigation.change");
    let library = rig.handle(&NodeRef::Library(rig.library.clone()));
    assert_eq!(event.data, json!({ "parents": [library] }));
    assert!(
        next_change(&mut subscription).await.is_none(),
        "exactly one"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_reaches_each_subscriber_only_with_the_parents_it_may_read() {
    let rig = NavRig::new().await;
    let letters = filesquid_core::CollectionId::new("letters").expect("id");
    let photos = rig.photos.clone();
    let ana = rig.user(|_| true);
    let ben = rig.user({
        let letters = letters.clone();
        move |node| *node != NodeRef::Collection(letters.clone())
    });
    let cy = rig.user(move |node| matches!(node, NodeRef::Repository(_)));
    let change = NavigationChange::of(&[
        NodeRef::Collection(letters.clone()),
        NodeRef::Collection(photos.clone()),
    ]);
    let both = vec![
        rig.handle(&NodeRef::Collection(letters.clone())),
        rig.handle(&NodeRef::Collection(photos.clone())),
    ];
    let only_photos = vec![rig.handle(&NodeRef::Collection(photos))];
    let for_ana = ana.filter_change(&Principal::Owner, &change).expect("ana");
    assert_eq!(
        serde_json::to_value(&for_ana).expect("json"),
        json!({ "parents": both })
    );
    let for_ben = ben.filter_change(&Principal::Owner, &change).expect("ben");
    assert_eq!(
        serde_json::to_value(&for_ben).expect("json"),
        json!({ "parents": only_photos })
    );
    assert!(cy.filter_change(&Principal::Owner, &change).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_filtered_subscription_delivers_the_filtered_event() {
    let rig = NavRig::new().await;
    let bus = rig.photos_rig.state.events.clone();
    let photos = NodeRef::Collection(rig.photos.clone());
    let library = NodeRef::Library(rig.library.clone());
    let reader = Arc::new(rig.user({
        let photos = photos.clone();
        move |node| *node != photos
    }));
    let mut subscription = bus
        .subscription_filtered(reader.event_filter(Principal::Owner))
        .expect("slot");
    let publisher = NavigationPublisher::new(bus.clone());
    publisher.publish(&[photos]);
    publisher.publish(std::slice::from_ref(&library));
    let event = next_change(&mut subscription)
        .await
        .expect("the readable change");
    assert_eq!(event.data, json!({ "parents": [rig.handle(&library)] }));
}

#[tokio::test(flavor = "multi_thread")]
async fn more_than_64_changed_parents_become_all() {
    let rig = NavRig::new().await;
    let bus = rig.photos_rig.state.events.clone();
    let mut subscription = bus.subscription().expect("slot");
    let parents: Vec<NodeRef> = (0..65)
        .map(|n| NodeRef::Library(filesquid_core::LibraryId::new(format!("lib-{n}")).expect("id")))
        .collect();
    NavigationPublisher::new(bus).publish(&parents);
    let event = next_change(&mut subscription).await.expect("event");
    assert_eq!(event.data, json!({ "all": true }));
}

#[test]
fn the_topic_is_documented() {
    assert!(topic::ALL.contains(&"navigation.change"));
}
