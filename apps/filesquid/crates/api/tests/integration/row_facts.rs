//! Integration tests of Task API-F2.21 (API-S-011 **Row facts** and **Recent**, API-S-012)
//! through the router, against a real OCFL storage root on the local filesystem and a seeded
//! library catalog: the library, head version and home collection on every file row of the
//! view and the search, the files and bytes beneath every folder row, the collection ID on
//! collection rows, and the Recent view from the caller's own journal entries.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

use axum::http::StatusCode;
use drive_catalog::{DriveCatalog, COLLECTION, COLLECTION_TITLE, REPOSITORY, VIRTUAL_TITLE};
use files_fixture::{encode, upload_facts, Answer, FilesRig, LIBRARY};
use filesquid_api::journal::{JournalEntry, JournalStore, Outcome};
use filesquid_api::navigation::NodeRef;
use filesquid_core::RepositoryId;
use serde_json::{json, Value};

/// A rig whose API state also has the seeded catalog's navigation service.
async fn rig() -> (FilesRig, DriveCatalog) {
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::new(LIBRARY);
    assert!(rig.state.install_navigation(drive.navigation.clone()));
    (rig, drive)
}

/// Upload `bytes` as a new file named `name`, a minute after the last one, and answer its
/// file ID. The file is not in the catalog until the test places it.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8]) -> String {
    rig.clock.advance(60_000);
    let uri = upload_facts(name, bytes.len(), "name_normalization=nfc");
    let answer = rig.upload(uri, bytes).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.result()["file_id"]
        .as_str()
        .expect("file id")
        .to_string()
}

/// Upload a file and place it in the seeded collection.
async fn in_collection(rig: &FilesRig, drive: &DriveCatalog, name: &str, bytes: &[u8]) -> String {
    let id = upload(rig, name, bytes).await;
    drive.file_in_collection(&id);
    id
}

/// Rename `file` to `name`, a minute after the last change.
async fn rename(rig: &FilesRig, file: &str, name: &str) {
    rig.clock.advance(60_000);
    let input = json!({ "library_id": LIBRARY, "file_id": file, "display_name": name });
    let answer = rig.command("/api/v1/files/renames", input).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
}

/// Download `file` whole, a minute after the last change.
async fn open(rig: &FilesRig, file: &str, extension: &str) {
    rig.clock.advance(60_000);
    let path = encode(&format!("original/{file}.{extension}"));
    let object = encode(&format!("urn:uuid:{file}"));
    let uri = format!("/api/v1/files/download?library_id={LIBRARY}&object_id={object}&path={path}");
    assert_eq!(rig.get(&uri).await.status, StatusCode::OK);
}

/// `GET /api/v1/files/view` of `node` with `rest`.
async fn view(rig: &FilesRig, node: &str, rest: &str) -> Answer {
    rig.get(&format!("/api/v1/files/view?node={node}{rest}"))
        .await
}

/// The rows of a successful answer.
fn rows(answer: &Answer) -> Vec<Value> {
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()["rows"].as_array().expect("rows").clone()
}

/// The row of `rows` shown as `name` (a file's display name or a folder's name).
fn row<'a>(rows: &'a [Value], name: &str) -> &'a Value {
    rows.iter()
        .find(|row| row["display_name"] == name || row["name"] == name)
        .unwrap_or_else(|| panic!("no row {name}: {rows:?}"))
}

/// The handle of the repository node.
fn repository_node() -> String {
    NodeRef::Repository(RepositoryId::new(REPOSITORY).expect("repository"))
        .to_handle()
        .expect("handle")
}

/// The Recent view of `node` as (display name, action) pairs, in order.
async fn recent(rig: &FilesRig, node: &str) -> Vec<(String, String)> {
    rig.monitor.journal().flush();
    rows(&view(rig, node, "&view=recent").await)
        .iter()
        .map(|row| {
            (
                row["display_name"].as_str().expect("name").to_string(),
                row["recent"]["action"]
                    .as_str()
                    .expect("action")
                    .to_string(),
            )
        })
        .collect()
}

/// A pair of owned strings, for comparing with [`recent`].
fn pair(name: &str, action: &str) -> (String, String) {
    (name.to_string(), action.to_string())
}

/// Journal that another user did `operation` to `file`, later than anything the owner did.
fn another_user_does(rig: &FilesRig, operation: &str, file: &str) {
    let mut entry = JournalEntry::new(operation, Outcome::Succeeded)
        .actor("user:ben")
        .target(LIBRARY)
        .target(format!("urn:uuid:{file}").as_str())
        .target("v9");
    entry.time_ms = i64::MAX / 2;
    rig.journal.append(&[entry]).expect("append");
}

// ---- Row facts on file rows ----

#[tokio::test]
async fn a_view_file_row_names_its_library_head_version_and_home_collection() {
    let (rig, drive) = rig().await;
    let beach = in_collection(&rig, &drive, "beach.jpg", b"1234").await;
    in_collection(&rig, &drive, "notes.md", b"12").await;
    rename(&rig, &beach, "Beach.jpg").await;
    let listed = rows(&view(&rig, &drive.collection_node(), "").await);
    let moved = row(&listed, "Beach.jpg");
    assert_eq!(moved["library_id"], LIBRARY);
    assert_eq!(moved["version"], "v2", "the rename made the head v2");
    assert_eq!(moved["collection_id"], COLLECTION);
    assert_eq!(moved["collection_title"], COLLECTION_TITLE);
    let untouched = row(&listed, "notes.md");
    assert_eq!(untouched["version"], "v1");
    assert_eq!(untouched["library_id"], LIBRARY);
    assert!(untouched.get("recent").is_none(), "only Recent rows say so");
}

#[tokio::test]
async fn a_search_file_row_carries_the_same_facts() {
    let (rig, drive) = rig().await;
    let beach = in_collection(&rig, &drive, "beach.jpg", b"1234").await;
    rename(&rig, &beach, "Beach day.jpg").await;
    let found = rows(&rig.get("/api/v1/files/search?q=beach").await);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["library_id"], LIBRARY);
    assert_eq!(found[0]["version"], "v2");
    assert_eq!(found[0]["collection_id"], COLLECTION);
    assert_eq!(found[0]["collection_title"], COLLECTION_TITLE);
}

#[tokio::test]
async fn a_file_registered_in_no_collection_carries_null_collection_facts() {
    let (rig, drive) = rig().await;
    let loose = upload(&rig, "loose.bin", b"123").await;
    drive.outside_file(&loose);
    drive.add_to_virtual(&loose);
    let found = rows(&rig.get("/api/v1/files/search?q=loose").await);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["collection_id"], Value::Null);
    assert_eq!(found[0]["collection_title"], Value::Null);
    assert!(found[0].get("collection_id").is_some(), "always a member");
    assert_eq!(found[0]["library_id"], LIBRARY);
    let members = rows(&view(&rig, &drive.virtual_node(), "").await);
    assert_eq!(members[0]["collection_id"], Value::Null);
    assert_eq!(members[0]["collection_title"], Value::Null);
    assert_eq!(members[0]["version"], "v1");
}

#[tokio::test]
async fn a_deleted_row_keeps_its_library_and_version() {
    let (rig, drive) = rig().await;
    let gone = in_collection(&rig, &drive, "gone.jpg", b"1234").await;
    drive.delete(&gone);
    let deleted = rows(&view(&rig, &drive.library_node(), "&view=deleted").await);
    assert_eq!(deleted[0]["library_id"], LIBRARY);
    assert_eq!(deleted[0]["version"], "v1");
    assert_eq!(deleted[0]["collection_id"], COLLECTION);
    assert_eq!(deleted[0]["deleted"]["home_collection_id"], COLLECTION);
}

// ---- Folder rows ----

#[tokio::test]
async fn a_folder_row_counts_the_files_and_bytes_beneath_it() {
    let (rig, drive) = rig().await;
    let first = in_collection(&rig, &drive, "a.jpg", &[1; 1000]).await;
    in_collection(&rig, &drive, "b.jpg", &[2; 1500]).await;
    in_collection(&rig, &drive, "c.jpg", &[3; 500]).await;
    drive.add_to_virtual(&first);
    let hidden = in_collection(&rig, &drive, "deleted.jpg", &[4; 700]).await;
    drive.delete(&hidden);
    let listed = rows(&view(&rig, &drive.library_node(), "").await);
    let collection = row(&listed, COLLECTION_TITLE);
    assert_eq!(collection["kind"], "folder");
    assert_eq!(collection["files"], 3, "a deleted file is not beneath it");
    assert_eq!(collection["bytes"], 3000);
    assert_eq!(collection["count_complete"], true);
    assert_eq!(collection["collection_id"], COLLECTION);
    let favourites = row(&listed, VIRTUAL_TITLE);
    assert_eq!(favourites["files"], 1);
    assert_eq!(favourites["bytes"], 1000);
    assert_eq!(favourites["count_complete"], true);
    assert_eq!(
        favourites["collection_id"],
        Value::Null,
        "a virtual collection is no collection"
    );
}

#[tokio::test]
async fn a_search_folder_row_carries_the_same_counts() {
    let (rig, drive) = rig().await;
    in_collection(&rig, &drive, "a.jpg", &[1; 10]).await;
    in_collection(&rig, &drive, "b.jpg", &[2; 20]).await;
    let found = rows(&rig.get("/api/v1/files/search?q=holidays").await);
    let collection = row(&found, COLLECTION_TITLE);
    assert_eq!(collection["files"], 2);
    assert_eq!(collection["bytes"], 30);
    assert_eq!(collection["count_complete"], true);
    assert_eq!(collection["collection_id"], COLLECTION);
}

#[tokio::test]
async fn an_unreadable_file_marks_its_folder_count_incomplete() {
    let (rig, drive) = rig().await;
    in_collection(&rig, &drive, "a.jpg", &[1; 10]).await;
    // The catalog lists a file whose object is not in the storage root.
    drive.file_in_collection("0c0c0c0c-1111-4222-8333-444455556666");
    let listed = rows(&view(&rig, &drive.library_node(), "").await);
    let collection = row(&listed, COLLECTION_TITLE);
    assert_eq!(collection["files"], 1, "only the files that were read");
    assert_eq!(collection["bytes"], 10);
    assert_eq!(collection["count_complete"], false);
}

#[tokio::test]
async fn a_folder_count_stopped_by_the_view_bound_says_so() {
    let (rig, drive) = rig().await;
    // More catalog entries than one view may count: the count stops at the bound.
    for n in 0..10_001 {
        drive.file_in_collection(&format!("0c0c0c0c-1111-4222-8333-{n:012}"));
    }
    drive.add_to_virtual("0c0c0c0c-1111-4222-8333-000000000000");
    let listed = rows(&view(&rig, &drive.library_node(), "").await);
    let collection = row(&listed, COLLECTION_TITLE);
    assert_eq!(collection["count_complete"], false);
    let favourites = row(&listed, VIRTUAL_TITLE);
    assert_eq!(
        favourites["count_complete"], false,
        "the bound is one view's, so no room is left for the next folder"
    );
}

// ---- Recent ----

#[tokio::test]
async fn the_recent_view_lists_the_callers_own_actions_newest_first() {
    let (rig, drive) = rig().await;
    let beach = in_collection(&rig, &drive, "beach.jpg", b"1234").await;
    let notes = in_collection(&rig, &drive, "notes.md", b"12").await;
    in_collection(&rig, &drive, "a.txt", b"123").await;
    open(&rig, &beach, "jpg").await;
    rename(&rig, &notes, "Notes.md").await;
    let expected = [
        pair("Notes.md", "changed"),
        pair("beach.jpg", "opened"),
        pair("a.txt", "uploaded"),
    ];
    assert_eq!(recent(&rig, &repository_node()).await, expected);
    assert_eq!(recent(&rig, &drive.library_node()).await, expected);
    assert_eq!(recent(&rig, &drive.collection_node()).await, expected);
    let page = view(&rig, &repository_node(), "&view=recent").await.json();
    assert_eq!(page["view"], "recent");
    assert_eq!(page["direction"], "descending");
    assert_eq!(page["complete"], true);
    let top = &page["rows"][0];
    assert_eq!(top["kind"], "file");
    assert_eq!(top["library_id"], LIBRARY);
    assert_eq!(top["version"], "v2");
    assert_eq!(top["collection_title"], COLLECTION_TITLE);
    let at = top["recent"]["at"].as_str().expect("time");
    assert_eq!(
        at.len(),
        "2026-10-16T00:00:00.000Z".len(),
        "RFC 3339 UTC: {at}"
    );
    assert!(at.ends_with('Z'), "{at}");
    assert_eq!(top["modified_by_me"], at, "the rename is my latest change");
}

#[tokio::test]
async fn the_recent_view_never_shows_another_callers_actions() {
    let (rig, drive) = rig().await;
    let mine = in_collection(&rig, &drive, "mine.jpg", b"1234").await;
    let theirs = in_collection(&rig, &drive, "theirs.jpg", b"12").await;
    // Another user opens and renames both files, later than anything the owner did.
    another_user_does(&rig, "file.download", &theirs);
    another_user_does(&rig, "files.rename", &theirs);
    another_user_does(&rig, "file.download", &mine);
    assert_eq!(
        recent(&rig, &repository_node()).await,
        [pair("theirs.jpg", "uploaded"), pair("mine.jpg", "uploaded")],
        "only my own uploads count, in my own order"
    );
    let listed = rows(&view(&rig, &repository_node(), "&view=recent").await);
    let other_time = filesquid_api::journal::format_rfc3339_ms(i64::MAX / 2);
    for row in &listed {
        assert_ne!(row["recent"]["at"], other_time.as_str(), "{row}");
        assert_eq!(row["opened_by_me"], Value::Null, "{row}");
    }
    // A file only another user touched is not in my Recent view at all.
    let untouched = "0c0c0c0c-1111-4222-8333-444455556666";
    another_user_does(&rig, "file.upload", untouched);
    assert_eq!(recent(&rig, &repository_node()).await.len(), 2);
}

#[tokio::test]
async fn recent_leaves_out_failed_actions_deleted_files_and_other_collections() {
    let (rig, drive) = rig().await;
    let kept = in_collection(&rig, &drive, "kept.jpg", b"1234").await;
    let gone = in_collection(&rig, &drive, "gone.jpg", b"12").await;
    drive.delete(&gone);
    let elsewhere = upload(&rig, "elsewhere.jpg", b"123").await;
    drive.file_in_new_collection(&elsewhere, "4e2f3a4b-5c6d-4e7f-8a9b-0c1d2e3f4a5b", "Work");
    let mut failed = JournalEntry::new("files.rename", Outcome::Failed)
        .actor("owner")
        .target(LIBRARY)
        .target(format!("urn:uuid:{kept}").as_str());
    failed.time_ms = i64::MAX / 2;
    rig.journal.append(&[failed]).expect("append");
    assert_eq!(
        recent(&rig, &drive.library_node()).await,
        [
            pair("elsewhere.jpg", "uploaded"),
            pair("kept.jpg", "uploaded")
        ]
    );
    assert_eq!(
        recent(&rig, &drive.collection_node()).await,
        [pair("kept.jpg", "uploaded")],
        "a collection node narrows Recent to its own files"
    );
}

#[tokio::test]
async fn recent_pages_stably_and_narrows_by_the_view_filter() {
    let (rig, drive) = rig().await;
    for name in ["one.jpg", "two.jpg", "three.png"] {
        in_collection(&rig, &drive, name, b"1234").await;
    }
    rig.monitor.journal().flush();
    let node = repository_node();
    let first = view(&rig, &node, "&view=recent&limit=2").await.json();
    assert_eq!(first["rows"].as_array().expect("rows").len(), 2);
    let cursor = first["next_cursor"].as_str().expect("cursor");
    let second = view(
        &rig,
        &node,
        &format!("&view=recent&limit=2&cursor={cursor}"),
    )
    .await
    .json();
    assert_eq!(second["rows"][0]["display_name"], "one.jpg");
    assert_eq!(second["next_cursor"], Value::Null);
    let other = view(&rig, &node, &format!("&view=all&limit=2&cursor={cursor}")).await;
    assert_eq!(other.status, StatusCode::BAD_REQUEST, "{:?}", other.json());
    let filtered = rows(&view(&rig, &node, "&view=recent&filter=PNG").await);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["display_name"], "three.png");
}

#[tokio::test]
async fn recent_is_refused_for_nodes_it_does_not_cover_and_other_views_still_wait() {
    let (rig, drive) = rig().await;
    let virtual_node = view(&rig, &drive.virtual_node(), "&view=recent").await;
    assert_eq!(virtual_node.status, StatusCode::BAD_REQUEST);
    assert_eq!(virtual_node.json()["code"], "invalid_request");
    let unknown = view(&rig, "n1.not-a-handle", "&view=recent").await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    for waiting in ["home", "personal", "shares", "favorites", "tags"] {
        let answer = view(&rig, &drive.library_node(), &format!("&view={waiting}")).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{waiting}");
        assert_eq!(answer.json()["code"], "invalid_request", "{waiting}");
    }
    let all_of_repository = view(&rig, &repository_node(), "").await;
    assert_eq!(
        all_of_repository.status,
        StatusCode::BAD_REQUEST,
        "only Recent covers the repository node"
    );
}

// ---- Redaction ----

#[tokio::test]
async fn no_name_title_or_path_of_these_reads_reaches_the_journal() {
    let (rig, drive) = rig().await;
    let secret = in_collection(&rig, &drive, "Secret holiday.jpg", b"1234").await;
    open(&rig, &secret, "jpg").await;
    rows(&view(&rig, &drive.library_node(), "").await);
    rows(&view(&rig, &drive.collection_node(), "").await);
    rows(&rig.get("/api/v1/files/search?q=secret").await);
    recent(&rig, &repository_node()).await;
    view(&rig, &drive.library_node(), "&view=favorites").await;
    view(&rig, &drive.virtual_node(), "&view=recent").await;
    rig.monitor.journal().flush();
    let journal = format!("{:?}", rig.journal());
    for word in [
        "Secret",
        "holiday",
        COLLECTION_TITLE,
        VIRTUAL_TITLE,
        "original/",
    ] {
        assert!(!journal.contains(word), "{word} was journaled");
    }
    assert!(
        rig.journaled("files.view")
            .iter()
            .all(|e| e.outcome == Outcome::Failed),
        "a successful view is not journaled"
    );
}
