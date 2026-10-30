//! Integration tests of home collections for uploads and copies (Task API-F2.14; API-S-011
//! **Destination** and **Same logical path**, API-S-019, API-S-021, API-S-026, ADR 041
//! Decisions 4, 5 and 12) through the router, against a real OCFL storage root and the
//! memory home collections: a request without a collection is `destination_required` and an
//! unknown one `collection_not_found`, each before anything is staged and journaled once; an
//! upload or copy is registered at `<folder>/<name>` and answered with it; a taken path,
//! a deleted file's included, is numbered beside it; after 9,999 numbers the request is
//! `path_suffix_exhausted` with no change; a retry after a crash between the object and its
//! registration completes the same file once; each success raises `navigation.change`; the
//! journal names no path or file name.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::StatusCode;
use files_fixture::{
    command_body, library, operation_id, upload_input, Answer, FilesRig, HOME_COLLECTION, LIBRARY,
};
use filesquid_api::canonical::collection_path::MAX_SUFFIX;
use filesquid_api::events::topic;
use filesquid_api::files::NEW_FILE_REVISION;
use filesquid_api::home::{HomeCollections, REGISTER_OPERATION};
use filesquid_api::journal::Outcome;
use filesquid_core::{CollectionId, FileId};
use serde_json::{json, Value};

/// A collection of no library the rig holds.
const STRANGER: &str = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";

/// The rig's home collection.
fn home() -> CollectionId {
    CollectionId::new(HOME_COLLECTION).expect("collection")
}

/// A file ID for a file placed directly in the memory port.
fn placed_id(n: u32) -> FileId {
    FileId::new(format!("0b6f6a58-5c1d-4b5e-9f3e-{n:012}")).expect("file id")
}

/// The upload facts of `name` in `folder` (the collection's root when `None`).
fn into(name: &str, bytes: &[u8], folder: Option<&str>) -> Value {
    match folder {
        Some(folder) => upload_input(name, bytes.len(), json!({ "folder": folder })),
        None => upload_input(name, bytes.len(), json!({})),
    }
}

/// Upload `bytes` named `name` into `folder`; answer the result.
async fn uploaded(rig: &FilesRig, name: &str, bytes: &[u8], folder: Option<&str>) -> Value {
    let answer = rig.upload(into(name, bytes, folder), bytes).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.result()
}

/// Assert `answer` is the typed error `status`/`code` and the journal holds exactly one
/// `failed` entry of `operation` with `journal_code`.
fn assert_refused(
    rig: &FilesRig,
    answer: &Answer,
    (status, code): (u16, &str),
    journal_code: &str,
    operation: &str,
) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code);
    assert!(answer.json()["message"]
        .as_str()
        .is_some_and(|m| m.contains(journal_code)));
    let failed: Vec<_> = rig
        .journaled(operation)
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "journaled once");
    assert_eq!(failed[0].error_code.as_deref(), Some(journal_code));
}

/// Assert no journal entry names a file name, a folder or a path.
fn assert_redacted(rig: &FilesRig, words: &[&str]) {
    let journal = format!("{:?}", rig.journal());
    for word in words {
        assert!(!journal.contains(word), "{word} was journaled");
    }
}

#[tokio::test]
async fn an_upload_without_a_collection_is_destination_required_and_stages_nothing() {
    let rig = FilesRig::new().await;
    let mut input = into("a.txt", b"abc", None);
    input
        .as_object_mut()
        .expect("object")
        .remove("collection_id");
    let answer = rig.upload(input, b"abc").await;
    assert_refused(
        &rig,
        &answer,
        (400, "invalid_request"),
        "destination_required",
        "file.upload",
    );
    assert_eq!(rig.home.reserved(), 0, "nothing reserved");
    assert!(rig.home.journal().is_empty(), "nothing registered");
}

#[tokio::test]
async fn an_unknown_or_foreign_collection_is_collection_not_found() {
    let rig = FilesRig::new().await;
    let input = upload_input("a.txt", 3, json!({ "collection_id": STRANGER }));
    let answer = rig.upload(input, b"abc").await;
    assert_refused(
        &rig,
        &answer,
        (404, "not_found"),
        "collection_not_found",
        "file.upload",
    );
    assert_eq!(rig.home.reserved(), 0);
}

#[tokio::test]
async fn a_reserved_folder_is_invalid_path() {
    let rig = FilesRig::new().await;
    let answer = rig
        .upload(into("a.txt", b"abc", Some("provenance")), b"abc")
        .await;
    assert_refused(
        &rig,
        &answer,
        (400, "invalid_request"),
        "invalid_path",
        "file.upload",
    );
}

#[tokio::test]
async fn an_upload_is_registered_at_its_folder_and_name_and_answered_with_it() {
    let rig = FilesRig::new().await;
    let mut events = rig.state.events.subscribe();
    let upload = uploaded(&rig, "Harbour.jpg", b"jpeg", Some("Trips/2026")).await;
    assert_eq!(upload["collection_id"], HOME_COLLECTION);
    assert_eq!(upload["path"], "Trips/2026/Harbour.jpg");
    let file = FileId::new(upload["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(
        rig.home.placement(&file),
        Some((home(), "Trips/2026/Harbour.jpg".into()))
    );
    let registered = rig.home.journal();
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].operation, REGISTER_OPERATION);
    assert!(registered[0].operation_id.is_some());
    assert_eq!(rig.home.reserved(), 0, "the reservation is released");
    let topics: Vec<String> = std::iter::from_fn(|| events.try_recv().ok())
        .take(1_000)
        .map(|event| event.topic)
        .collect();
    let changes = topics
        .iter()
        .filter(|t| *t == topic::NAVIGATION_CHANGE)
        .count();
    assert_eq!(changes, 1, "one navigation.change: {topics:?}");
    let succeeded = rig.journaled("file.upload");
    assert_eq!(succeeded.len(), 1, "journaled once");
    assert_redacted(&rig, &["Harbour", "Trips"]);
}

#[tokio::test]
async fn a_taken_path_is_numbered_beside_it_and_a_deleted_file_keeps_its_place() {
    let rig = FilesRig::new().await;
    rig.home.place(&placed_id(1), &home(), "photo.jpg");
    rig.home.place(&placed_id(2), &home(), "photo-1.jpg");
    let photo = uploaded(&rig, "photo.jpg", b"one", None).await;
    assert_eq!(photo["path"], "photo-2.jpg");
    let archive = uploaded(&rig, "archive.tar.gz", b"two", None).await;
    let again = uploaded(&rig, "archive.tar.gz", b"three", None).await;
    assert_eq!(archive["path"], "archive.tar.gz");
    assert_eq!(again["path"], "archive-1.tar.gz");
    let dot = uploaded(&rig, ".env", b"four", None).await;
    let dot_again = uploaded(&rig, ".env", b"five", None).await;
    assert_eq!(
        (dot["path"].clone(), dot_again["path"].clone()),
        (json!(".env"), json!(".env-1"))
    );
    let config = uploaded(&rig, ".config.json", b"six", None).await;
    let config_again = uploaded(&rig, ".config.json", b"seven", None).await;
    assert_eq!(config["path"], ".config.json");
    assert_eq!(config_again["path"], ".config-1.json");
    let report = uploaded(&rig, "report", b"eight", None).await;
    let report_again = uploaded(&rig, "report", b"nine", None).await;
    assert_eq!(
        (report["path"].clone(), report_again["path"].clone()),
        (json!("report"), json!("report-1"))
    );
}

#[tokio::test]
async fn after_9999_numbers_the_upload_is_path_suffix_exhausted_with_no_change() {
    let rig = FilesRig::new().await;
    rig.home.place(&placed_id(0), &home(), "full.txt");
    for n in 1..=MAX_SUFFIX {
        rig.home
            .place(&placed_id(n), &home(), &format!("full-{n}.txt"));
    }
    let answer = rig.upload(into("full.txt", b"abc", None), b"abc").await;
    assert_refused(
        &rig,
        &answer,
        (409, "conflict"),
        "path_suffix_exhausted",
        "file.upload",
    );
    assert_eq!(rig.home.reserved(), 0, "nothing reserved");
    assert!(rig.home.journal().is_empty(), "nothing registered");
}

#[tokio::test]
async fn a_retry_after_a_crash_between_the_object_and_its_registration_completes_once() {
    let rig = FilesRig::new().await;
    let operation = operation_id();
    let minted = placed_id(77);
    let reserved = rig
        .home
        .reserve(&library(), &operation, &minted)
        .expect("reserved");
    let input = into("crash.txt", b"abc", None);
    let mut query: filesquid_api::files::UploadQuery =
        serde_json::from_value(input.clone()).expect("facts");
    query.reserved_file_id = Some(reserved.as_str().to_string());
    let first = rig
        .service
        .upload(query, b"abc", "owner", NEW_FILE_REVISION)
        .await;
    assert_eq!(
        first.expect("the first run committed").file_id,
        reserved.as_str()
    );
    assert_eq!(
        rig.home.placement(&reserved),
        None,
        "the crash left it unregistered"
    );
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    let retried = rig.send_upload(&envelope, &input, b"abc").await;
    assert_eq!(retried.status, StatusCode::OK, "{:?}", retried.json());
    let result = retried.result();
    assert_eq!(result["file_id"], reserved.as_str(), "the same file");
    assert_eq!(result["version"], "v1", "no second version");
    assert_eq!(
        rig.home.placement(&reserved),
        Some((home(), "crash.txt".into()))
    );
    assert_eq!(rig.home.reserved(), 0);
}

#[tokio::test]
async fn a_copy_is_registered_in_its_destination_beside_its_source() {
    let rig = FilesRig::new().await;
    let source = uploaded(&rig, "beach.jpg", b"jpeg", Some("Summer")).await;
    let body = json!({
        "library_id": LIBRARY, "file_id": source["file_id"],
        "collection_id": HOME_COLLECTION, "folder": "Summer"
    });
    let answer = rig.command("/api/v1/files/copies", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let copy = answer.result();
    assert_eq!(copy["collection_id"], HOME_COLLECTION);
    assert_eq!(copy["path"], "Summer/beach-1.jpg");
    let file = FileId::new(copy["file_id"].as_str().expect("copy")).expect("id");
    assert_eq!(
        rig.home.placement(&file),
        Some((home(), "Summer/beach-1.jpg".into()))
    );
    assert_eq!(rig.journaled("file.copy").len(), 1);
    let refused = json!({ "library_id": LIBRARY, "file_id": source["file_id"] });
    let answer = rig.command("/api/v1/files/copies", refused).await;
    assert_refused(
        &rig,
        &answer,
        (400, "invalid_request"),
        "destination_required",
        "file.copy",
    );
    assert_redacted(&rig, &["beach", "Summer"]);
}
