//! Integration tests of Task API-F2.2 part 2 (API-S-011, SRS-F-062) through the router,
//! against a real OCFL storage root on the local filesystem and a seeded library catalog:
//! copy with a new FileId and source provenance, the view operation by library, collection
//! and virtual-collection node with its sort keys, folder placement, filter and stable
//! paging, the caller-only "by me" sort keys, and storage use per collection.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

use axum::http::{Method, StatusCode};
use drive_catalog::{DriveCatalog, COLLECTION};
use files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use filesquid_api::journal::{JournalEntry, JournalStore, Outcome};
use serde_json::{json, Value};

/// A rig whose API state also has the seeded catalog's navigation service.
async fn rig() -> (FilesRig, DriveCatalog) {
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::new(LIBRARY);
    assert!(rig.state.install_navigation(drive.navigation.clone()));
    (rig, drive)
}

/// Upload `bytes` as a new file named `name` (NFC) and answer its file ID.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8]) -> String {
    let uri = upload_uri(name, bytes.len(), "name_normalization=nfc");
    let answer = rig.call(Method::PUT, &uri, &[], bytes.to_vec()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()["file_id"]
        .as_str()
        .expect("file id")
        .to_string()
}

/// `GET /api/v1/files/view` of `node` with `rest`.
async fn view(rig: &FilesRig, node: &str, rest: &str) -> Answer {
    rig.get(&format!("/api/v1/files/view?node={node}{rest}"))
        .await
}

/// The names of the rows of a view page, folders and files alike.
fn names(page: &Value) -> Vec<String> {
    page["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| match row["kind"].as_str() {
            Some("folder") => row["name"].as_str().expect("name").to_string(),
            _ => row["display_name"].as_str().expect("name").to_string(),
        })
        .collect()
}

/// Every name of a view, page by page, following `next_cursor`.
async fn every_name(rig: &FilesRig, node: &str, rest: &str) -> Vec<String> {
    let mut seen = Vec::new();
    let mut cursor = String::new();
    for _ in 0..20 {
        let page = view(rig, node, &format!("{rest}{cursor}")).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.json());
        let page = page.json();
        seen.extend(names(&page));
        match page["next_cursor"].as_str() {
            Some(next) => cursor = format!("&cursor={next}"),
            None => return seen,
        }
    }
    panic!("more pages than files");
}

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// Three files of different names, sizes and modified times, all in the collection.
async fn three_in_collection(rig: &FilesRig, drive: &DriveCatalog) -> Vec<String> {
    let mut ids = Vec::new();
    for (name, bytes) in [
        ("beta.jpg", &b"1234"[..]),
        ("Alpha.jpg", &b"123456789"[..]),
        ("gamma.png", &b"12"[..]),
    ] {
        rig.clock.advance(60_000);
        let id = upload(rig, name, bytes).await;
        drive.file_in_collection(&id);
        ids.push(id);
    }
    ids
}

// ---- Copy ----

#[tokio::test]
async fn a_copy_is_a_new_file_that_keeps_the_display_name_and_names_its_source() {
    let (rig, _drive) = rig().await;
    let source = upload(&rig, "IMG_0001.JPG", b"jpeg bytes").await;
    let rename = json!({ "library_id": LIBRARY, "file_id": source, "display_name": "Harbour.JPG" });
    assert_eq!(
        rig.post("/api/v1/files/renames", rename).await.status,
        StatusCode::OK
    );
    let body = json!({ "library_id": LIBRARY, "file_id": source });
    let answer = rig.post("/api/v1/files/copies", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let copy = answer.json();
    let copy_id = copy["file_id"].as_str().expect("copy id");
    assert_ne!(copy_id, source, "a new FileId");
    assert_eq!(copy["object_id"], format!("urn:uuid:{copy_id}"));
    assert_eq!(copy["version"], "v1");
    assert_eq!(copy["source"], format!("urn:uuid:{source}@v2"));
    let listing = rig
        .get(&format!(
            "/api/v1/files/listing?library_id={LIBRARY}&object_id={}&prefix=original",
            encode(&format!("urn:uuid:{copy_id}"))
        ))
        .await
        .json();
    assert_eq!(
        listing["entries"][0]["path"],
        format!("original/{copy_id}.JPG")
    );
    assert_eq!(listing["entries"][0]["display_name"], "Harbour.JPG");
    let statements = rig
        .get(&format!(
            "/api/v1/files/metadata?library_id={LIBRARY}&file_id={copy_id}"
        ))
        .await
        .json();
    let source_statement = statements["statements"]
        .as_array()
        .expect("statements")
        .iter()
        .find(|s| s["curie"] == "dcterms:source")
        .cloned()
        .expect("dcterms:source");
    assert_eq!(source_statement["value"], format!("urn:uuid:{source}@v2"));
    let facts = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={copy_id}"
        ))
        .await
        .json();
    assert!(facts.to_string().contains("IMG_0001.JPG"), "{facts}");
    let source_versions = rig
        .get(&format!(
            "/api/v1/files/versions?library_id={LIBRARY}&object_id={}",
            encode(&format!("urn:uuid:{source}"))
        ))
        .await
        .json();
    assert_eq!(source_versions["head"], "v2", "the source never changes");
    let journaled = rig.journaled("files.copy");
    assert_eq!(journaled.len(), 1);
    assert!(journaled[0]
        .target_ids
        .contains(&format!("urn:uuid:{copy_id}")));
}

#[tokio::test]
async fn a_copy_of_an_unknown_or_malformed_file_publishes_nothing() {
    let (rig, _drive) = rig().await;
    let unknown =
        json!({ "library_id": LIBRARY, "file_id": "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19" });
    assert_error(
        &rig.post("/api/v1/files/copies", unknown).await,
        404,
        "not_found",
    );
    let malformed = json!({ "library_id": LIBRARY, "file_id": "../x" });
    assert_error(
        &rig.post("/api/v1/files/copies", malformed).await,
        400,
        "invalid_request",
    );
    assert_eq!(rig.journaled("files.copy").len(), 2, "each refusal once");
    assert!(!rig.dir.join("root").join("files").exists());
}

// ---- The view operation ----

#[tokio::test]
async fn a_collection_view_sorts_by_each_key_in_both_directions() {
    let (rig, drive) = rig().await;
    three_in_collection(&rig, &drive).await;
    let node = drive.collection_node();
    let by = |sort: &str, direction: &str| format!("&sort={sort}&direction={direction}");
    assert_eq!(
        every_name(&rig, &node, &by("name", "ascending")).await,
        ["Alpha.jpg", "beta.jpg", "gamma.png"]
    );
    assert_eq!(
        every_name(&rig, &node, &by("name", "descending")).await,
        ["gamma.png", "beta.jpg", "Alpha.jpg"]
    );
    assert_eq!(
        every_name(&rig, &node, &by("size", "descending")).await,
        ["Alpha.jpg", "beta.jpg", "gamma.png"]
    );
    assert_eq!(
        every_name(&rig, &node, &by("modified", "descending")).await,
        ["gamma.png", "Alpha.jpg", "beta.jpg"]
    );
    let page = view(&rig, &node, "").await.json();
    assert_eq!(page["rows"][0]["media_type"], "image/jpeg");
    assert_eq!(page["rows"][0]["size"], 9);
    assert_eq!(page["complete"], true);
}

#[tokio::test]
async fn paging_is_stable_with_a_cursor_and_a_cursor_of_another_view_is_refused() {
    let (rig, drive) = rig().await;
    three_in_collection(&rig, &drive).await;
    let node = drive.collection_node();
    let paged = every_name(&rig, &node, "&limit=1&sort=size").await;
    assert_eq!(paged, ["gamma.png", "beta.jpg", "Alpha.jpg"]);
    let first = view(&rig, &node, "&limit=1&sort=size").await.json();
    let cursor = first["next_cursor"].as_str().expect("cursor");
    let other = view(&rig, &node, &format!("&limit=1&sort=name&cursor={cursor}")).await;
    assert_error(&other, 400, "invalid_request");
    let forged = view(&rig, &node, "&cursor=v1.7b7d").await;
    assert_error(&forged, 400, "invalid_request");
}

#[tokio::test]
async fn a_library_view_shows_its_collections_as_folders_beside_its_unfiled_files() {
    let (rig, drive) = rig().await;
    three_in_collection(&rig, &drive).await;
    let loose = upload(&rig, "Zebra.txt", b"z").await;
    drive.unfiled_file(&loose);
    let node = drive.library_node();
    assert_eq!(
        every_name(&rig, &node, "").await,
        ["Favourites", "Holidays", "Zebra.txt"],
        "folders on top, then the unfiled file"
    );
    assert_eq!(
        every_name(&rig, &node, "&direction=descending").await,
        ["Holidays", "Favourites", "Zebra.txt"],
        "folders stay on top in both directions"
    );
    assert_eq!(
        every_name(&rig, &node, "&folders=mixed&sort=name").await,
        ["Favourites", "Holidays", "Zebra.txt"]
    );
    assert_eq!(
        every_name(&rig, &node, "&folders=mixed&sort=size&direction=descending").await,
        ["Zebra.txt", "Favourites", "Holidays"],
        "a folder has no size, so it sorts after every file"
    );
    let page = view(&rig, &node, "").await.json();
    assert_eq!(page["rows"][1]["node_kind"], "collection");
    assert_eq!(page["rows"][1]["node"], drive.collection_node());
}

#[tokio::test]
async fn a_virtual_collection_lists_its_members_and_the_filter_narrows_any_view() {
    let (rig, drive) = rig().await;
    let ids = three_in_collection(&rig, &drive).await;
    drive.add_to_virtual(&ids[0]);
    drive.add_to_virtual(&ids[2]);
    assert_eq!(
        every_name(&rig, &drive.virtual_node(), "").await,
        ["beta.jpg", "gamma.png"]
    );
    assert_eq!(
        every_name(&rig, &drive.collection_node(), "&filter=JPG").await,
        ["Alpha.jpg", "beta.jpg"]
    );
    assert_eq!(
        every_name(&rig, &drive.library_node(), "&filter=holi").await,
        ["Holidays"]
    );
}

#[tokio::test]
async fn the_deleted_view_lists_only_deleted_files_and_other_views_wait() {
    let (rig, drive) = rig().await;
    let ids = three_in_collection(&rig, &drive).await;
    drive.delete(&ids[1]);
    assert_eq!(
        every_name(&rig, &drive.collection_node(), "").await,
        ["beta.jpg", "gamma.png"]
    );
    assert_eq!(
        every_name(&rig, &drive.library_node(), "&view=deleted").await,
        ["Alpha.jpg"]
    );
    for view_name in ["home", "recent", "favorites", "shares", "tags", "personal"] {
        let answer = view(&rig, &drive.library_node(), &format!("&view={view_name}")).await;
        assert_error(&answer, 400, "invalid_request");
    }
}

#[tokio::test]
async fn unknown_and_foreign_nodes_are_refused() {
    let (rig, drive) = rig().await;
    assert_error(&view(&rig, "not-a-handle", "").await, 404, "not_found");
    let other = DriveCatalog::new("0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19");
    assert_error(
        &view(&rig, &other.library_node(), "").await,
        404,
        "not_found",
    );
    let repository = filesquid_api::navigation::NodeRef::Repository(
        filesquid_core::RepositoryId::new(drive_catalog::REPOSITORY).expect("repository"),
    )
    .to_handle()
    .expect("handle");
    assert_error(&view(&rig, &repository, "").await, 400, "invalid_request");
    assert_error(
        &view(&rig, &drive.library_node(), "&limit=1001").await,
        400,
        "invalid_request",
    );
    assert!(
        !rig.journaled("files.view").is_empty(),
        "refusals are journaled"
    );
}

// ---- Modified by me and opened by me ----

#[tokio::test]
async fn by_me_keys_follow_only_the_callers_own_changes_and_openings() {
    let (rig, drive) = rig().await;
    let ids = three_in_collection(&rig, &drive).await;
    let (beta, alpha, gamma) = (&ids[0], &ids[1], &ids[2]);
    rig.clock.advance(60_000);
    let rename = json!({ "library_id": LIBRARY, "file_id": alpha, "display_name": "Alpha 2.jpg" });
    assert_eq!(
        rig.post("/api/v1/files/renames", rename).await.status,
        StatusCode::OK
    );
    rig.clock.advance(60_000);
    let path = encode(&format!("original/{beta}.jpg"));
    let download = rig
        .get(&format!(
            "/api/v1/files/download?library_id={LIBRARY}&object_id={}&path={path}",
            encode(&format!("urn:uuid:{beta}"))
        ))
        .await;
    assert_eq!(download.status, StatusCode::OK);
    another_user_touches(&rig, gamma);
    rig.monitor.journal().flush();
    let node = drive.collection_node();
    let modified = every_name(&rig, &node, "&sort=modified_by_me&direction=descending").await;
    assert_eq!(modified[0], "Alpha 2.jpg", "my rename is my latest change");
    let opened = every_name(&rig, &node, "&sort=opened_by_me&direction=descending").await;
    assert_eq!(opened, ["beta.jpg", "Alpha 2.jpg", "gamma.png"]);
    let page = view(&rig, &node, "&sort=opened_by_me&direction=descending")
        .await
        .json();
    assert!(page["rows"][0]["opened_by_me"].is_string());
    let gamma_row = page["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["file_id"] == gamma.as_str())
        .cloned()
        .expect("gamma");
    assert!(
        gamma_row["opened_by_me"].is_null(),
        "another user's opening is not mine"
    );
    assert!(
        gamma_row["modified_by_me"].is_string(),
        "my upload is my change"
    );
    let downloads: Vec<JournalEntry> = rig
        .journaled("file.download")
        .into_iter()
        .filter(|entry| entry.actor != "user:ben")
        .collect();
    assert_eq!(downloads.len(), 1, "my download is journaled once");
    assert_eq!(downloads[0].outcome, Outcome::Succeeded);
}

#[tokio::test]
async fn a_download_is_journaled_with_opaque_ids_only() {
    let (rig, _drive) = rig().await;
    let file = upload(&rig, "Secret holiday.jpg", b"jpeg").await;
    let path = encode(&format!("original/{file}.jpg"));
    let download = rig
        .get(&format!(
            "/api/v1/files/download?library_id={LIBRARY}&object_id={}&path={path}",
            encode(&format!("urn:uuid:{file}"))
        ))
        .await;
    assert_eq!(download.status, StatusCode::OK);
    let entries = rig.journaled("file.download");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(
        entry.target_ids,
        [
            LIBRARY.to_string(),
            format!("urn:uuid:{file}"),
            "v1".to_string()
        ]
    );
    assert!(entry
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id)));
    let text = serde_json::to_string(entry).expect("json");
    for leak in ["Secret", "holiday", "original/", ".jpg"] {
        assert!(!text.contains(leak), "{leak} in {text}");
    }
}

/// Journal a later opening and change of `file` by another actor.
fn another_user_touches(rig: &FilesRig, file: &str) {
    let object = format!("urn:uuid:{file}");
    let entries = ["file.download", "files.rename"].map(|operation| {
        let mut entry = JournalEntry::new(operation, Outcome::Succeeded)
            .actor("user:ben")
            .target(LIBRARY)
            .target(object.as_str())
            .target("v9");
        entry.time_ms = i64::MAX / 2;
        entry
    });
    rig.journal.append(&entries).expect("append");
}

// ---- Storage use per collection ----

#[tokio::test]
async fn storage_use_per_collection_counts_its_files_history_once() {
    let (rig, drive) = rig().await;
    let ids = three_in_collection(&rig, &drive).await;
    drive.delete(&ids[2]);
    let loose = upload(&rig, "outside.bin", &[0u8; 1000]).await;
    drive.unfiled_file(&loose);
    let answer = rig
        .get(&format!(
            "/api/v1/files/collection-history-size?collection_id={COLLECTION}"
        ))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let size = answer.json();
    assert_eq!(size["files"], 3, "deleted files still hold bytes");
    assert_eq!(size["contents"], 6, "each original and its record");
    let bytes = size["bytes"].as_u64().expect("bytes");
    let library = rig
        .get(&format!(
            "/api/v1/files/library-history-size?library_id={LIBRARY}"
        ))
        .await
        .json();
    let all = library["bytes"].as_u64().expect("library bytes");
    assert!(
        bytes > 15 && all - bytes > 1000,
        "only the collection's files: {bytes} of {all}"
    );
    assert_eq!(size["truncated"], false);
    let unknown = rig
        .get("/api/v1/files/collection-history-size?collection_id=0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19")
        .await;
    assert_error(&unknown, 404, "not_found");
}
