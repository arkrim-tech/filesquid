//! Integration tests of Task API-F2.3 (API-S-012, SRS-F-063) through the router, against a
//! real OCFL storage root on the local filesystem and a seeded library catalog: free text and
//! every answered operator, unknown and unavailable operators, permission filtering before
//! ranking, relevance and view sort keys, stable paging, case folding and node scopes.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

use axum::http::{Method, StatusCode};
use drive_catalog::DriveCatalog;
use files_fixture::{encode, upload_facts, Answer, FilesRig, HOME_COLLECTION, LIBRARY};
use filesquid_api::auth::Principal;
use filesquid_api::navigation::{NavigationAccess, NodeRef};
use serde_json::{json, Value};
use std::sync::Arc;

/// A second collection, unreadable in the permission test.
const SECRET: &str = "4e2f3a4b-5c6d-4e7f-8a9b-0c1d2e3f4a5b";

/// A rig whose API state also has `drive`'s navigation service.
async fn rig_with(drive: DriveCatalog) -> (FilesRig, DriveCatalog) {
    let rig = FilesRig::new().await;
    assert!(rig.state.install_navigation(drive.navigation.clone()));
    (rig, drive)
}

/// A rig over a catalog readable to the owner.
async fn rig() -> (FilesRig, DriveCatalog) {
    rig_with(DriveCatalog::new(LIBRARY)).await
}

/// Upload `bytes` as `name` with media type `media`, place it in the collection, and answer
/// its file ID.
async fn add(
    rig: &FilesRig,
    drive: &DriveCatalog,
    name: &str,
    media: &str,
    bytes: &[u8],
) -> String {
    rig.clock.advance(60_000);
    let rest = format!("name_normalization=nfc&media_type={}", encode(media));
    let uri = upload_facts(name, bytes.len(), &rest);
    let answer = rig.upload(uri, bytes).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let id = answer.result()["file_id"].as_str().expect("id").to_string();
    drive.file_in_collection(&id);
    id
}

/// Set `curie` to `value` on `file`.
async fn describe(rig: &FilesRig, file: &str, curie: &str, value: &str) {
    let body = json!({ "library_id": LIBRARY, "file_id": file, "curie": curie, "value": value });
    let headers = [("content-type", "application/json")];
    let answer = rig
        .call(
            Method::PUT,
            "/api/v1/files/metadata/statements",
            &headers,
            body.to_string().into_bytes(),
        )
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
}

/// `GET /api/v1/files/search` of `q` with `rest`.
async fn search(rig: &FilesRig, q: &str, rest: &str) -> Answer {
    rig.get(&format!("/api/v1/files/search?q={}{rest}", encode(q)))
        .await
}

/// The row names of a successful search of `q` with `rest`.
async fn found(rig: &FilesRig, q: &str, rest: &str) -> Vec<String> {
    let answer = search(rig, q, rest).await;
    assert_eq!(answer.status, StatusCode::OK, "{q}: {:?}", answer.json());
    names(&answer.json())
}

/// The names of a page's rows.
fn names(page: &Value) -> Vec<String> {
    page["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            let name = row.get("display_name").unwrap_or(&row["name"]);
            name.as_str().expect("name").to_string()
        })
        .collect()
}

/// Assert `answer` is the typed error `code` with `status` and no results.
fn assert_refused(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
    assert!(answer.json().get("rows").is_none(), "no results");
}

/// A described library: a budget PDF, a holiday photo and a client deck.
async fn library(rig: &FilesRig, drive: &DriveCatalog) -> [String; 3] {
    let budget = add(rig, drive, "budget.pdf", "application/pdf", &[1u8; 300]).await;
    describe(rig, &budget, "dcterms:title", "Budget 2026").await;
    describe(rig, &budget, "dcterms:subject", "finance").await;
    describe(rig, &budget, "dcterms:creator", "Ana Example").await;
    let photo = add(rig, drive, "img_0001.jpg", "image/jpeg", &[2u8; 50]).await;
    describe(
        rig,
        &photo,
        "dcterms:description",
        "Sunset over the harbour",
    )
    .await;
    describe(rig, &photo, "dcterms:created", "2019-07-04").await;
    let deck = add(
        rig,
        drive,
        "Client deck.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        &[3u8; 9],
    )
    .await;
    [budget, photo, deck]
}

#[tokio::test]
async fn free_text_phrases_and_exclusions_read_names_and_dublin_core() {
    let (rig, drive) = rig().await;
    library(&rig, &drive).await;
    assert_eq!(found(&rig, "harbour", "").await, ["img_0001.jpg"]);
    assert_eq!(
        found(&rig, "\"Client deck\" -draft", "").await,
        ["Client deck.pptx"]
    );
    assert_eq!(found(&rig, "2026", "").await, ["budget.pdf"]);
    assert!(found(&rig, "pdf -budget", "").await.is_empty());
}

#[tokio::test]
async fn every_answered_operator_narrows_the_results() {
    let (rig, drive) = rig().await;
    let [budget, photo, _deck] = library(&rig, &drive).await;
    let copy = rig
        .command(
            "/api/v1/files/copies",
            json!({ "library_id": LIBRARY, "file_id": photo, "collection_id": HOME_COLLECTION }),
        )
        .await
        .result();
    drive.file_in_collection(copy["file_id"].as_str().expect("copy"));
    drive.delete(&budget);
    let cases: [(&str, &[&str]); 14] = [
        ("type:image", &["img_0001.jpg", "img_0001.jpg"]),
        ("type:document", &["Client deck.pptx"]),
        ("title:budget is:deleted", &["budget.pdf"]),
        ("creator:\"ana example\" is:deleted", &["budget.pdf"]),
        ("tag:finance is:deleted", &["budget.pdf"]),
        ("is:deleted", &["budget.pdf"]),
        ("size>100B is:deleted", &["budget.pdf"]),
        ("size<10 -type:folder", &["Client deck.pptx"]),
        (
            "in:holidays -type:folder",
            &["Client deck.pptx", "img_0001.jpg", "img_0001.jpg"],
        ),
        ("source:urn", &["img_0001.jpg"]),
        (
            "createdbefore:2020-01-01",
            &["img_0001.jpg", "img_0001.jpg"],
        ),
        ("owner:me type:document", &["Client deck.pptx"]),
        ("after:2026-01-01 type:document", &["Client deck.pptx"]),
        ("before:2000-01-01", &[]),
    ];
    for (query, expected) in cases {
        let mut got = found(&rig, query, "&sort=name").await;
        got.sort();
        assert_eq!(got, expected, "{query}");
    }
}

#[tokio::test]
async fn unfiled_files_and_folder_rows_are_searchable() {
    let (rig, drive) = rig().await;
    library(&rig, &drive).await;
    let loose = add(&rig, &drive, "loose.txt", "text/plain", b"x").await;
    drive.unfiled_file(&loose);
    assert_eq!(found(&rig, "is:unfiled", "").await, ["loose.txt"]);
    assert_eq!(found(&rig, "holi type:folder", "").await, ["Holidays"]);
    assert_eq!(
        found(&rig, "type:folder", "&sort=name").await,
        ["Favourites", "Holidays"]
    );
}

#[tokio::test]
async fn unknown_operators_are_refused_and_never_match_an_empty_filter() {
    let (rig, drive) = rig().await;
    library(&rig, &drive).await;
    for query in [
        "colour:blue",
        "generator:\"Camera App\"",
        "type:spreadsheet",
        "\"open",
    ] {
        assert_refused(&search(&rig, query, "").await, 400, "invalid_request");
    }
    let message = search(&rig, "colour:blue", "").await.json()["message"].clone();
    assert!(
        message.as_str().expect("message").contains("colour"),
        "{message}"
    );
    assert_refused(&search(&rig, "", "").await, 400, "invalid_request");
    assert!(
        !rig.journaled("files.search").is_empty(),
        "refusals are journaled"
    );
}

#[tokio::test]
async fn operators_this_host_cannot_answer_are_refused_as_unavailable() {
    let (rig, drive) = rig().await;
    library(&rig, &drive).await;
    for (query, operator) in [
        ("is:favorite", "is:favorite"),
        ("owner:ben", "owner:"),
        ("sharedwith:public", "sharedwith"),
        ("to:ben", "to"),
        ("from:ben", "from"),
        ("pendingowner:me", "pendingowner"),
    ] {
        let answer = search(&rig, query, "").await;
        assert_refused(&answer, 503, "unavailable");
        let message = answer.json()["message"]
            .as_str()
            .expect("message")
            .to_string();
        let named = format!("The search operator {operator} is not available on this host.");
        assert_eq!(message, named, "{query}");
    }
}

/// Reads everything but the secret collection.
struct NoSecret;

impl NavigationAccess for NoSecret {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        node.to_handle()
            != NodeRef::Collection(filesquid_core::CollectionId::new(SECRET).expect("id"))
                .to_handle()
    }
}

#[tokio::test]
async fn results_are_filtered_by_permission_before_ranking() {
    let (rig, drive) = rig_with(DriveCatalog::with_access(LIBRARY, Arc::new(NoSecret))).await;
    library(&rig, &drive).await;
    rig.clock.advance(60_000);
    let uri = upload_facts("Budget 2026 secret.pdf", 3, "media_type=application%2Fpdf");
    let secret = rig.upload(uri, b"abc").await.result()["file_id"]
        .as_str()
        .expect("id")
        .to_string();
    drive.file_in_new_collection(&secret, SECRET, "Secret plans");
    let results = found(&rig, "budget", "").await;
    assert_eq!(
        results,
        ["budget.pdf"],
        "the unreadable file is never returned"
    );
    assert!(
        found(&rig, "secret", "").await.is_empty(),
        "nor is its collection"
    );
    let hidden = NodeRef::Collection(filesquid_core::CollectionId::new(SECRET).expect("id"))
        .to_handle()
        .expect("handle");
    assert_refused(
        &search(&rig, "budget", &format!("&node={hidden}")).await,
        404,
        "not_found",
    );
}

#[tokio::test]
async fn an_unreadable_file_is_never_read_before_ranking() {
    let (rig, drive) = rig_with(DriveCatalog::with_access(LIBRARY, Arc::new(NoSecret))).await;
    let uri = upload_facts("Budget 2026 secret.pdf", 3, "media_type=application%2Fpdf");
    let answer = rig.upload(uri, b"abc").await;
    let secret = answer.result()["file_id"].as_str().expect("id").to_string();
    drive.file_in_new_collection(&secret, SECRET, "Secret plans");
    let before = rig.root.reads();
    assert!(found(&rig, "budget", "").await.is_empty());
    assert_eq!(
        rig.root.reads(),
        before,
        "the only matching file is unreadable, so search reads no file object at all"
    );
}

/// Reads nothing at all: not even the repository.
struct Nothing;

impl NavigationAccess for Nothing {
    fn may_read(&self, _principal: &Principal, _node: &NodeRef) -> bool {
        false
    }
}

#[tokio::test]
async fn a_caller_who_may_not_read_the_repository_cannot_search_it() {
    let (rig, drive) = rig_with(DriveCatalog::with_access(LIBRARY, Arc::new(Nothing))).await;
    let facts = upload_facts("budget.pdf", 3, "media_type=application%2Fpdf");
    let query = serde_json::from_value(facts).expect("upload facts");
    let new_file = filesquid_api::files::NEW_FILE_REVISION;
    let upload = rig.service.upload(query, b"abc", "owner", new_file).await;
    drive.file_in_collection(&upload.expect("committed directly").view.file_id);
    assert_refused(&search(&rig, "budget", "").await, 404, "not_found");
}

#[tokio::test]
async fn relevance_orders_by_default_and_any_view_key_replaces_it() {
    let (rig, drive) = rig().await;
    add(&rig, &drive, "deck notes.txt", "text/plain", &[0u8; 50]).await;
    add(&rig, &drive, "old.txt", "text/plain", &[0u8; 1]).await;
    let titled = add(&rig, &drive, "Deck.pptx", "text/plain", &[0u8; 5]).await;
    describe(&rig, &titled, "dcterms:title", "Deck of the year").await;
    let described = add(&rig, &drive, "zz.txt", "text/plain", &[0u8; 20]).await;
    describe(&rig, &described, "dcterms:description", "notes for a deck").await;
    let page = search(&rig, "deck", "").await.json();
    assert_eq!(page["sort"], "relevance");
    assert_eq!(page["direction"], "descending");
    assert_eq!(names(&page), ["Deck.pptx", "deck notes.txt", "zz.txt"]);
    assert_eq!(
        found(&rig, "deck", "&sort=size").await,
        ["Deck.pptx", "zz.txt", "deck notes.txt"],
        "the same matches in another order"
    );
}

#[tokio::test]
async fn paging_is_stable_and_a_changed_result_set_refuses_the_cursor() {
    let (rig, drive) = rig().await;
    for i in 0..5 {
        add(&rig, &drive, &format!("report {i}.txt"), "text/plain", b"r").await;
    }
    let mut seen = Vec::new();
    let mut cursor = String::new();
    for _ in 0..10 {
        let page = search(&rig, "report", &format!("&limit=2&sort=name{cursor}")).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.json());
        let page = page.json();
        seen.extend(names(&page));
        match page["next_cursor"].as_str() {
            Some(next) => cursor = format!("&cursor={next}"),
            None => break,
        }
    }
    assert_eq!(
        seen,
        (0..5)
            .map(|i| format!("report {i}.txt"))
            .collect::<Vec<_>>()
    );
    let first = search(&rig, "report", "&limit=2&sort=name").await.json();
    let next = first["next_cursor"].as_str().expect("cursor").to_string();
    add(&rig, &drive, "report 00.txt", "text/plain", b"r").await;
    let changed = search(&rig, "report", &format!("&limit=2&sort=name&cursor={next}")).await;
    assert_refused(&changed, 409, "conflict");
    let other = search(
        &rig,
        "reports",
        &format!("&limit=2&sort=name&cursor={next}"),
    )
    .await;
    assert_refused(&other, 400, "invalid_request");
}

#[tokio::test]
async fn nfc_and_case_folding_are_declared_and_diacritics_are_not_folded() {
    let (rig, drive) = rig().await;
    let cafe = add(&rig, &drive, "notes.txt", "text/plain", b"c").await;
    describe(&rig, &cafe, "dcterms:title", "Café Résumé").await;
    assert_eq!(found(&rig, "CAFÉ RÉSUMÉ", "").await, ["notes.txt"]);
    assert!(found(&rig, "cafe resume", "").await.is_empty());
    rig.clock.advance(60_000);
    let decomposed = "Re\u{301}sume\u{301}.txt";
    let uri = upload_facts(
        decomposed,
        1,
        "name_normalization=nfd&media_type=text%2Fplain",
    );
    let answer = rig.upload(uri, b"r").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    drive.file_in_collection(answer.result()["file_id"].as_str().expect("id"));
    assert_eq!(
        found(&rig, "R\u{c9}SUM\u{c9}", "").await,
        [decomposed, "notes.txt"],
        "an NFD name matches an NFC query, as the NFC title does"
    );
}

#[tokio::test]
async fn a_node_limits_the_search_and_dates_follow_the_callers_zone() {
    let (rig, drive) = rig().await;
    let [_budget, photo, _deck] = library(&rig, &drive).await;
    drive.add_to_virtual(&photo);
    assert_eq!(
        found(
            &rig,
            "-type:folder",
            &format!("&node={}&sort=name", drive.virtual_node())
        )
        .await,
        ["img_0001.jpg"]
    );
    describe(&rig, &photo, "dcterms:created", "2026-03-01T00:30:00").await;
    let west = "&zone=-08:00";
    assert_eq!(
        found(&rig, "createdafter:2026-03-01", west).await,
        ["img_0001.jpg"],
        "a zoneless capture date compares by the calendar date it states"
    );
    describe(&rig, &photo, "dcterms:created", "2026-03-01T05:00:00Z").await;
    assert_eq!(
        found(&rig, "createdafter:2026-03-01", "").await,
        ["img_0001.jpg"],
        "in UTC the instant is on 1 March"
    );
    assert!(
        found(&rig, "createdafter:2026-03-01", west)
            .await
            .is_empty(),
        "at -08:00 the same instant is on 28 February"
    );
    assert_eq!(
        found(&rig, "createdbefore:2026-03-01", west).await,
        ["img_0001.jpg"]
    );
    let repository = NodeRef::Repository(
        filesquid_core::RepositoryId::new(drive_catalog::REPOSITORY).expect("repository"),
    )
    .to_handle()
    .expect("handle");
    let page = search(&rig, "harbour", &format!("&node={repository}"))
        .await
        .json();
    assert!(page["node"].is_null(), "the repository is answered as null");
    assert_refused(
        &search(&rig, "a", "&zone=PST").await,
        400,
        "invalid_request",
    );
}
