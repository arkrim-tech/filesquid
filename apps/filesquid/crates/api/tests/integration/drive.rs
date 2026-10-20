//! Integration tests of the drive routes of Task API-F2.2 (API-S-011, UI-S-017, API-S-021,
//! API-S-027, NFR-003) through the router, against a real OCFL storage root on the local
//! filesystem: per-user preferences and their bounds, the streamed whole-file download, the
//! reserved folders left out of listings, and the listing by navigation node.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/library_fixture.rs"]
mod library_fixture;

#[path = "../support/navigation_fixture.rs"]
mod navigation_fixture;

#[path = "../support/drive_fixture.rs"]
mod drive_fixture;

use axum::http::StatusCode;
use drive_fixture::{
    get_preferences, grid_preferences, keep_preferences, put_preferences, OneScope,
};
use files_fixture::{encode, pattern, query, Answer, FilesRig, Switchable};
use filesquid_api::auth::Authoriser;
use filesquid_api::files::{DOWNLOAD_OPERATION, MAX_ANSWER_BYTES};
use filesquid_api::journal::Outcome;
use filesquid_api::navigation::{FolderScope, NodeRef};
use filesquid_api::preferences::{
    PreferenceStore, Preferences, MAX_MEMORY_USERS, UPDATE_OPERATION,
};
use filesquid_core::{CollectionId, LibraryId};
use navigation_fixture::NavRig;
use serde_json::{json, Value};
use std::sync::Arc;

/// A node handle the navigation service could write.
fn handle(id: &str) -> String {
    let id = CollectionId::new(id).expect("collection id");
    NodeRef::Collection(id).to_handle().expect("handle")
}

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// The paths of a listing page's entries.
fn paths(page: &Value) -> Vec<String> {
    page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["path"].as_str().expect("path").to_string())
        .collect()
}

#[tokio::test]
async fn a_user_who_saved_nothing_gets_the_default_preferences() {
    let rig = FilesRig::new().await;
    keep_preferences(&rig);
    let answer = get_preferences(&rig).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let defaults = serde_json::to_value(Preferences::default()).expect("json");
    assert_eq!(answer.json(), defaults);
    assert_eq!(answer.json()["layout"], "list");
    assert_eq!(answer.json()["navigation"]["version"], 1);
}

#[tokio::test]
async fn saved_preferences_are_restored_and_journaled_without_their_values() {
    let rig = FilesRig::new().await;
    keep_preferences(&rig);
    let body = grid_preferences(&handle("c-1"));
    let saved = put_preferences(&rig, &body).await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json());
    assert_eq!(saved.json(), body);
    assert_eq!(get_preferences(&rig).await.json(), body);
    let entries = rig.journaled(UPDATE_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].actor, "owner");
    assert!(entries[0].target_ids.is_empty(), "no value is journaled");
}

#[tokio::test]
async fn preferences_belong_to_the_user_who_saved_them() {
    let owner = FilesRig::new().await;
    let store = keep_preferences(&owner);
    let token = "administrator-token-of-the-drive-test".to_string();
    let switchable = Switchable::new(&token);
    let authoriser = Authoriser::new(switchable.clone(), switchable);
    let admin = FilesRig::with_authoriser(authoriser, token).await;
    assert!(admin.state.install_preferences(store.clone()));
    let body = grid_preferences(&handle("c-1"));
    assert_eq!(put_preferences(&owner, &body).await.status, StatusCode::OK);
    let theirs = get_preferences(&admin).await;
    assert_eq!(
        theirs.json()["layout"],
        "list",
        "the administrator saved nothing"
    );
    assert_eq!(
        store.load("owner").expect("load"),
        serde_json::from_value(body).ok()
    );
    assert_eq!(store.load("administrator").expect("load"), None);
}

#[tokio::test]
async fn every_bound_of_the_preferences_is_enforced() {
    let rig = FilesRig::new().await;
    let store = keep_preferences(&rig);
    let pin = handle("c-1");
    let pins: Vec<Value> = (0..101)
        .map(|i| json!({ "node": handle(&format!("c-{i}")), "kind": "collection" }))
        .collect();
    let collapsed: Vec<String> = (0..1001).map(|i| handle(&format!("c-{i}"))).collect();
    let sections: Vec<String> = (0..17).map(|i| format!("s{i}")).collect();
    let refused: Vec<(&str, Value)> = vec![
        ("schema", json!(2)),
        ("sort_key", json!("colour")),
        ("layout", json!("table")),
        ("navigation", json!({ "version": 2 })),
        ("navigation", json!({ "version": 1, "pinned": pins })),
        (
            "navigation",
            json!({ "version": 1, "collapsed": collapsed }),
        ),
        ("navigation", json!({ "version": 1, "order": sections })),
        (
            "navigation",
            json!({ "version": 1, "order": ["x".repeat(65)] }),
        ),
        ("navigation", json!({ "version": 1, "order": [""] })),
        (
            "navigation",
            json!({ "version": 1, "collapsed": ["photos/2025"] }),
        ),
        (
            "navigation",
            json!({ "version": 1, "pinned": [{ "node": "Holiday", "kind": "folder" }] }),
        ),
        (
            "navigation",
            json!({ "version": 1, "pinned": [{ "node": pin, "kind": "drive" }] }),
        ),
        (
            "navigation",
            json!({ "version": 1, "collapsed": [pin, pin] }),
        ),
        (
            "navigation",
            json!({ "version": 1, "pinned": [{ "node": pin, "kind": "folder" }] }),
        ),
        ("token", json!("secret")),
    ];
    let attempts = refused.len() + 1;
    for (field, value) in refused {
        let mut body = grid_preferences(&pin);
        body[field] = value;
        let answer = put_preferences(&rig, &body).await;
        assert_error(&answer, 400, "invalid_request");
    }
    let mut missing = grid_preferences(&pin);
    missing.as_object_mut().expect("object").remove("layout");
    assert_error(
        &put_preferences(&rig, &missing).await,
        400,
        "invalid_request",
    );
    assert_eq!(
        store.load("owner").expect("load"),
        None,
        "nothing was saved"
    );
    let failed = rig.journaled(UPDATE_OPERATION);
    assert_eq!(failed.len(), attempts, "each refusal is journaled once");
    assert!(failed.iter().all(|e| e.outcome == Outcome::Failed));
    assert!(failed
        .iter()
        .all(|e| e.error_code.as_deref() == Some("invalid_request")));
}

#[tokio::test]
async fn the_largest_navigation_choices_are_kept() {
    let rig = FilesRig::new().await;
    keep_preferences(&rig);
    let mut body = grid_preferences(&handle("c-1"));
    let pins: Vec<Value> = (0..100)
        .map(|i| json!({ "node": handle(&format!("c-{i}")), "kind": "collection" }))
        .collect();
    let collapsed: Vec<String> = (0..1000).map(|i| handle(&format!("c-{i}"))).collect();
    let sections: Vec<String> = (0..16).map(|i| format!("{i:0>64}")).collect();
    body["navigation"] = json!({
        "version": 1, "pinned": pins, "order": sections, "collapsed": collapsed
    });
    let saved = put_preferences(&rig, &body).await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json());
    assert_eq!(get_preferences(&rig).await.json(), body);
}

#[tokio::test]
async fn a_full_store_refuses_a_new_user_without_inviting_a_retry() {
    let rig = FilesRig::new().await;
    let store = keep_preferences(&rig);
    for user in 0..MAX_MEMORY_USERS {
        store
            .save(&format!("user-{user}"), &Preferences::default())
            .expect("room");
    }
    let body = grid_preferences(&handle("c-1"));
    let answer = put_preferences(&rig, &body).await;
    assert_error(&answer, 409, "conflict");
    assert_eq!(answer.json()["retryable"], false);
}

#[tokio::test]
async fn preferences_are_unavailable_until_the_host_keeps_them() {
    let rig = FilesRig::new().await;
    assert_error(&get_preferences(&rig).await, 503, "unavailable");
    let body = grid_preferences(&handle("c-1"));
    assert_error(&put_preferences(&rig, &body).await, 503, "unavailable");
}

#[tokio::test]
async fn a_file_larger_than_one_answer_downloads_whole_in_ranged_pieces() {
    let bytes = pattern(MAX_ANSWER_BYTES + 3);
    let rig = FilesRig::with_files(&[("films/reel.bin", &bytes)]).await;
    let reads_before = rig.root.content_reads();
    let uri = format!(
        "/api/v1/files/download?{}",
        query(&format!("path={}", encode("films/reel.bin")))
    );
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.body.len(), bytes.len());
    assert!(answer.body == bytes, "the bytes arrive whole and in order");
    assert_eq!(answer.header("content-length"), bytes.len().to_string());
    assert_eq!(
        answer.header("content-disposition"),
        "attachment; filename=\"reel.bin\"; filename*=UTF-8''reel.bin"
    );
    assert_eq!(answer.header("x-content-type-options"), "nosniff");
    let pieces = rig.root.content_reads() - reads_before;
    assert!(
        pieces >= 3,
        "read in pieces of at most 8 MiB, not at once: {pieces}"
    );
    assert!(
        rig.journaled(DOWNLOAD_OPERATION).is_empty(),
        "reads are not journaled"
    );
}

#[tokio::test]
async fn an_earlier_version_downloads_as_it_was() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let uri = format!("/api/v1/files/download?{}", query("path=a.txt&version=v1"));
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, b"alpha");
    assert_eq!(answer.header("content-type"), "text/plain");
}

#[tokio::test]
async fn a_missing_file_is_refused_before_the_download_starts() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let uri = format!("/api/v1/files/download?{}", query("path=b.txt"));
    assert_error(&rig.get(&uri).await, 404, "not_found");
    let uri = format!("/api/v1/files/download?{}", query("path=a.txt&version=v9"));
    assert_error(&rig.get(&uri).await, 404, "not_found");
    let uri = format!("/api/v1/files/download?{}", query("path=..%2Fa.txt"));
    assert_error(&rig.get(&uri).await, 400, "invalid_request");
    let refused = rig.journaled(DOWNLOAD_OPERATION);
    assert_eq!(refused.len(), 3);
    assert!(refused.iter().all(|e| e.outcome == Outcome::Failed));
}

#[tokio::test]
async fn listings_leave_out_the_reserved_folders() {
    let rig = FilesRig::with_files(&[
        ("a.txt", b"alpha"),
        ("photos/b.jpg", b"b"),
        ("provenance/v1.jsonld", b"{}"),
        (".filesquid/labels.json", b"{}"),
        ("photos/provenance/c.txt", b"c"),
    ])
    .await;
    let root = rig
        .get(&format!("/api/v1/files/listing?{}", query("")))
        .await;
    assert_eq!(root.status, StatusCode::OK, "{:?}", root.json());
    assert_eq!(paths(&root.json()), ["photos", "a.txt"]);
    let nested = rig
        .get(&format!("/api/v1/files/listing?{}", query("prefix=photos")))
        .await;
    assert_eq!(paths(&nested.json()), ["photos/provenance", "photos/b.jpg"]);
    for prefix in ["provenance", ".filesquid", "provenance%2Fdeeper"] {
        let uri = format!(
            "/api/v1/files/listing?{}",
            query(&format!("prefix={prefix}"))
        );
        assert_error(&rig.get(&uri).await, 404, "not_found");
    }
}

#[tokio::test]
async fn files_cannot_be_written_into_the_provenance_folder() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let uri = format!(
        "/api/v1/files/upload?{}",
        query(&format!("path={}", encode("provenance/v9.jsonld")))
    );
    let answer = rig
        .call(axum::http::Method::PUT, &uri, &[], b"{}".to_vec())
        .await;
    assert_error(&answer, 400, "invalid_request");
    let moved = rig
        .post(
            "/api/v1/files/moves",
            json!({
                "library_id": files_fixture::LIBRARY,
                "object_id": files_fixture::OBJECT,
                "from": "a.txt",
                "to": "provenance/a.txt"
            }),
        )
        .await;
    assert_error(&moved, 400, "invalid_request");
}

/// The navigation fixture with the Photos collection served as the rig's object.
async fn photos() -> NavRig {
    let nav = NavRig::new().await;
    let scope = FolderScope::Collection(nav.photos.clone());
    assert!(nav
        .photos_rig
        .service
        .install_scope_objects(Arc::new(OneScope(scope))));
    nav
}

/// `GET /api/v1/files/listing?<query>` through the Photos rig.
async fn list(nav: &NavRig, query: &str) -> Answer {
    nav.photos_rig
        .get(&format!("/api/v1/files/listing?{query}"))
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_collection_node_lists_the_root_of_its_files() {
    let nav = photos().await;
    let node = nav.handle(&NodeRef::Collection(nav.photos.clone()));
    let answer = list(&nav, &format!("node={}", encode(&node))).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(paths(&answer.json()), ["2025"]);
    assert_eq!(answer.json()["prefix"], "");
    assert_eq!(answer.json()["library_id"], files_fixture::LIBRARY);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_node_lists_that_folder_with_paging() {
    let nav = photos().await;
    let node = encode(&nav.photos_folder("2025"));
    let first = list(&nav, &format!("node={node}&limit=1")).await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json());
    assert_eq!(paths(&first.json()), ["2025/summer"]);
    let cursor = first.json()["next_cursor"]
        .as_str()
        .expect("cursor")
        .to_string();
    let second = list(
        &nav,
        &format!("node={node}&limit=1&cursor={}", encode(&cursor)),
    )
    .await;
    assert_eq!(paths(&second.json()), ["2025/winter"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn library_virtual_collection_and_repository_nodes_do_not_list_as_folders() {
    let nav = photos().await;
    let nodes = [
        NodeRef::Library(nav.library.clone()),
        NodeRef::VirtualCollection(nav.best_of.clone()),
        NodeRef::Repository(nav.lib.repository.clone()),
    ];
    for node in nodes {
        let answer = list(&nav, &format!("node={}", encode(&nav.handle(&node)))).await;
        assert_error(&answer, 400, "invalid_request");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_malformed_and_unmapped_nodes_are_not_found() {
    let nav = photos().await;
    let unknown = NodeRef::Collection(CollectionId::new("no-such").expect("id"));
    let missing = NodeRef::folder(FolderScope::Collection(nav.photos.clone()), "1999");
    let unfiled = NodeRef::folder(FolderScope::Unfiled(nav.library.clone()), "inbox");
    let handles = [
        nav.handle(&unknown),
        nav.handle(&missing),
        nav.handle(&unfiled),
        "n1.not-a-handle".to_string(),
    ];
    for node in handles {
        let answer = list(&nav, &format!("node={}", encode(&node))).await;
        assert_error(&answer, 404, "not_found");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_the_caller_may_not_read_is_not_found() {
    let nav = photos().await;
    let rig = FilesRig::with_files(&[("2025/summer/beach.jpg", b"beach")]).await;
    let scope = FolderScope::Collection(nav.photos.clone());
    assert!(rig.service.install_scope_objects(Arc::new(OneScope(scope))));
    let user = nav.user(|node| !matches!(node, NodeRef::Folder { .. }));
    assert!(rig.state.install_navigation(Arc::new(user)));
    let folder = encode(&nav.photos_folder("2025"));
    let hidden = rig
        .get(&format!("/api/v1/files/listing?node={folder}"))
        .await;
    assert_error(&hidden, 404, "not_found");
    let collection = encode(&nav.handle(&NodeRef::Collection(nav.photos.clone())));
    let shown = rig
        .get(&format!("/api/v1/files/listing?node={collection}"))
        .await;
    assert_eq!(shown.status, StatusCode::OK, "{:?}", shown.json());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_listing_names_exactly_one_form() {
    let nav = photos().await;
    let node = encode(&nav.photos_folder("2025"));
    let library = LibraryId::new(files_fixture::LIBRARY).expect("library");
    let forms = [
        String::new(),
        format!("library_id={}", library.as_str()),
        format!("{}&node={node}", query("")),
        format!("node={node}&prefix=2025"),
    ];
    for form in forms {
        assert_error(&list(&nav, &form).await, 400, "invalid_request");
    }
}
