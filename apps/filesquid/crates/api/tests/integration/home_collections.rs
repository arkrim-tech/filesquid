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

use axum::http::{Method, StatusCode};
use files_fixture::{
    command_body, operation_id, upload_input, Answer, FilesRig, HOME_COLLECTION, LIBRARY,
};
use filesquid_api::canonical::collection_path::MAX_SUFFIX;
use filesquid_api::events::topic;
use filesquid_api::files::NEW_FILE_REVISION;
use filesquid_api::home::{PLACE_OPERATION, REGISTER_OPERATION};
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
        rig.home.placed(&file),
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
    let input = into("crash.txt", b"abc", None);
    let operation = operation_id();
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    rig.home.fail_next_registration();
    let crashed = rig.send_upload(&envelope, &input, b"abc").await;
    assert_eq!(
        crashed.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{:?}",
        crashed.json()
    );
    assert_eq!(rig.home.reserved(), 1, "the reservation survives the crash");
    let retried = rig.send_upload(&envelope, &input, b"abc").await;
    assert_eq!(retried.status, StatusCode::OK, "{:?}", retried.json());
    let result = retried.result();
    assert_eq!(result["version"], "v1", "no second version");
    let file = FileId::new(result["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(rig.home.placed(&file), Some((home(), "crash.txt".into())));
    assert_eq!(rig.home.journal().len(), 1, "registered once");
    assert_eq!(rig.home.reserved(), 0);
    let succeeded = rig
        .journaled("file.upload")
        .into_iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .count();
    assert_eq!(succeeded, 1, "journaled once as a success");
}

#[tokio::test]
async fn the_same_operation_id_with_other_bytes_after_a_crash_is_refused() {
    let rig = FilesRig::new().await;
    let input = into("crash.txt", b"abc", None);
    let operation = operation_id();
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    rig.home.fail_next_registration();
    let crashed = rig.send_upload(&envelope, &input, b"abc").await;
    assert_eq!(crashed.status, StatusCode::SERVICE_UNAVAILABLE);
    let other = rig.send_upload(&envelope, &input, b"xyz").await;
    assert_eq!(other.status, StatusCode::BAD_REQUEST, "{:?}", other.json());
    assert_eq!(rig.home.journal().len(), 0, "nothing registered");
}

#[tokio::test]
async fn a_failed_upload_releases_its_reservation() {
    let rig = FilesRig::new().await;
    let input = upload_input("short.txt", 9, json!({}));
    let answer = rig.upload(input, b"abc").await;
    assert_eq!(
        answer.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        answer.json()
    );
    assert_eq!(rig.home.reserved(), 0, "released");
}

#[tokio::test]
async fn new_content_of_a_file_answers_its_own_home() {
    let rig = FilesRig::new().await;
    let first = uploaded(&rig, "notes.txt", b"one", Some("Work")).await;
    let file = first["file_id"].as_str().expect("file").to_string();
    let input = upload_input(
        "notes.txt",
        3,
        json!({ "file_id": file, "folder": "Elsewhere" }),
    );
    let answer = rig.upload(input, b"two").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["collection_id"], HOME_COLLECTION);
    assert_eq!(
        answer.result()["path"],
        "Work/notes.txt",
        "it keeps its home"
    );
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
        rig.home.placed(&file),
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

/// Rename the file `file_id` to `name`; answer the answer.
async fn renamed(rig: &FilesRig, file_id: &Value, name: &str) -> Answer {
    let body = json!({ "library_id": LIBRARY, "file_id": file_id, "display_name": name });
    rig.command("/api/v1/files/renames", body).await
}

/// The topics of the events `events` received so far.
fn topics(
    events: &mut tokio::sync::broadcast::Receiver<filesquid_api::events::ApiEvent>,
) -> Vec<String> {
    std::iter::from_fn(|| events.try_recv().ok())
        .take(1_000)
        .map(|event| event.topic)
        .collect()
}

#[tokio::test]
async fn a_rename_gives_a_registered_file_its_new_name_in_its_folder_numbered_when_taken() {
    let rig = FilesRig::new().await;
    let upload = uploaded(&rig, "IMG_0001.jpg", b"jpeg", Some("Summer")).await;
    rig.home.place(&placed_id(1), &home(), "Summer/Harbour.jpg");
    let mut events = rig.state.events.subscribe();
    let answer = renamed(&rig, &upload["file_id"], "Harbour.jpg").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["path"], "Summer/Harbour-1.jpg");
    let file = FileId::new(upload["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(
        rig.home.placed(&file),
        Some((home(), "Summer/Harbour-1.jpg".into()))
    );
    let placed = rig.home.journal();
    assert_eq!(placed.last().expect("relocated").operation, PLACE_OPERATION);
    let seen = topics(&mut events);
    assert!(
        seen.iter().any(|t| t == topic::NAVIGATION_CHANGE),
        "{seen:?}"
    );
    assert_redacted(&rig, &["Harbour", "Summer"]);
}

#[tokio::test]
async fn a_rename_that_cannot_be_placed_is_refused_before_the_name_changes() {
    let rig = FilesRig::new().await;
    let upload = uploaded(&rig, "a.txt", b"abc", None).await;
    rig.home.place(&placed_id(0), &home(), "full.txt");
    for n in 1..=MAX_SUFFIX {
        rig.home
            .place(&placed_id(n), &home(), &format!("full-{n}.txt"));
    }
    let answer = renamed(&rig, &upload["file_id"], "full.txt").await;
    assert_refused(
        &rig,
        &answer,
        (409, "conflict"),
        "path_suffix_exhausted",
        "files.rename",
    );
    let file = FileId::new(upload["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(
        rig.home.placed(&file),
        Some((home(), "a.txt".into())),
        "nothing moved"
    );
}

#[tokio::test]
async fn a_rename_whose_relocation_failed_is_completed_by_its_retry() {
    let rig = FilesRig::new().await;
    let upload = uploaded(&rig, "IMG_0001.jpg", b"jpeg", Some("Summer")).await;
    let input = json!({
        "library_id": LIBRARY, "file_id": upload["file_id"], "display_name": "Harbour.jpg"
    });
    let body = command_body(&input, &operation_id(), &rig.revision_of(&input).await);
    rig.home.fail_next_relocation();
    let crashed = rig
        .send_command(Method::POST, "/api/v1/files/renames", &body)
        .await;
    assert_eq!(crashed.status, StatusCode::SERVICE_UNAVAILABLE);
    let file = FileId::new(upload["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(
        rig.home.placed(&file),
        Some((home(), "Summer/IMG_0001.jpg".into())),
        "the name was committed, the path not yet"
    );
    let retry = rig
        .send_command(Method::POST, "/api/v1/files/renames", &body)
        .await;
    assert_eq!(retry.status, StatusCode::CONFLICT, "{:?}", retry.json());
    assert_eq!(
        rig.home.placed(&file),
        Some((home(), "Summer/Harbour.jpg".into())),
        "the retry completed the relocation"
    );
    let fresh = renamed(&rig, &upload["file_id"], "Harbour.jpg").await;
    assert_eq!(
        fresh.status,
        StatusCode::CONFLICT,
        "nothing is left to change"
    );
    let places = rig
        .home
        .journal()
        .into_iter()
        .filter(|e| e.operation == PLACE_OPERATION)
        .count();
    assert_eq!(places, 1, "relocated once");
}

/// The bytes and head version of the stored original of the file `file_id`, read through the
/// listing and content routes.
async fn original_of(rig: &FilesRig, file_id: &Value) -> (Vec<u8>, String) {
    let object = format!("urn:uuid:{}", file_id.as_str().expect("file"));
    let object = files_fixture::encode(&object);
    let listing = rig
        .get(&format!(
            "/api/v1/files/listing?library_id={LIBRARY}&object_id={object}&prefix=original"
        ))
        .await;
    assert_eq!(listing.status, StatusCode::OK, "{:?}", listing.json());
    let path = listing.json()["entries"][0]["path"]
        .as_str()
        .expect("the original")
        .to_string();
    let path = files_fixture::encode(&path);
    let content = rig
        .get(&format!(
            "/api/v1/files/content?library_id={LIBRARY}&object_id={object}&path={path}"
        ))
        .await;
    assert_eq!(content.status, StatusCode::OK);
    let head = rig.revision_of(&json!({ "file_id": file_id })).await;
    (content.body, head)
}

#[tokio::test]
async fn a_collection_move_gives_a_file_another_folder_keeping_its_name_and_its_bytes() {
    let rig = FilesRig::new().await;
    let upload = uploaded(&rig, "beach.jpg", b"jpeg", Some("Summer")).await;
    let before = original_of(&rig, &upload["file_id"]).await;
    rig.home.place(&placed_id(1), &home(), "Archive/beach.jpg");
    let mut events = rig.state.events.subscribe();
    let body = json!({
        "library_id": LIBRARY, "file_id": upload["file_id"],
        "collection_id": HOME_COLLECTION, "folder": "Archive"
    });
    let answer = rig.command("/api/v1/files/moves", body.clone()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["path"], "Archive/beach-1.jpg");
    assert_eq!(answer.result()["collection_id"], HOME_COLLECTION);
    let file = FileId::new(upload["file_id"].as_str().expect("file")).expect("id");
    assert_eq!(
        rig.home.placed(&file),
        Some((home(), "Archive/beach-1.jpg".into()))
    );
    let changes = topics(&mut events)
        .iter()
        .filter(|t| *t == topic::NAVIGATION_CHANGE)
        .count();
    assert_eq!(changes, 2, "the folder it left and the folder it entered");
    let after = original_of(&rig, &upload["file_id"]).await;
    assert_eq!(after, before, "no byte and no file version changed");
    assert_eq!(after.0, b"jpeg");
    let moves: Vec<_> = rig
        .journaled("file.move")
        .into_iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .collect();
    assert_eq!(moves.len(), 1, "journaled once");
    let mut nowhere = body;
    nowhere
        .as_object_mut()
        .expect("object")
        .remove("collection_id");
    let refused = rig.command("/api/v1/files/moves", nowhere).await;
    assert_refused(
        &rig,
        &refused,
        (400, "invalid_request"),
        "destination_required",
        "file.move",
    );
    assert_redacted(&rig, &["beach", "Archive", "Summer"]);
}

/// Ask for the collection folder `folder` with the extra fields of `extra`; answer the answer.
async fn foldered(rig: &FilesRig, folder: &str, extra: Value) -> Answer {
    let mut body = json!({
        "library_id": LIBRARY, "collection_id": HOME_COLLECTION, "folder": folder
    });
    let fields = body.as_object_mut().expect("object");
    for (key, value) in extra.as_object().expect("extra fields") {
        match value {
            Value::Null => fields.remove(key),
            _ => fields.insert(key.clone(), value.clone()),
        };
    }
    rig.command("/api/v1/files/folders", body).await
}

#[tokio::test]
async fn a_new_folder_at_a_taken_path_is_numbered_beside_it_and_nothing_is_written() {
    let rig = FilesRig::new().await;
    rig.home
        .place(&placed_id(1), &home(), "filepathX/directoryY/a.jpg");
    let answer = foldered(&rig, "filepathX/directoryY", json!({})).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(
        answer.result(),
        json!({
            "library_id": LIBRARY, "collection_id": HOME_COLLECTION,
            "path": "filepathX/directoryY-1", "pending": true
        })
    );
    rig.home
        .place(&placed_id(2), &home(), "filepathX/directoryY-1/b.jpg");
    let again = foldered(&rig, "filepathX/directoryY", json!({})).await;
    assert_eq!(again.result()["path"], "filepathX/directoryY-2");
    let fresh = foldered(&rig, "filepathX/Fresh", json!({})).await;
    assert_eq!(fresh.result()["path"], "filepathX/Fresh");
    assert!(rig.home.journal().is_empty(), "no snapshot version");
    assert_eq!(
        rig.home.placed(&placed_id(1)),
        Some((home(), "filepathX/directoryY/a.jpg".into())),
        "nothing moved"
    );
    assert_redacted(&rig, &["filepathX", "directoryY", "Fresh"]);
}

#[tokio::test]
async fn a_new_folder_at_a_file_path_is_numbered_beside_the_file() {
    let rig = FilesRig::new().await;
    rig.home.place(&placed_id(1), &home(), "Summer/notes");
    let answer = foldered(&rig, "Summer/notes", json!({})).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["path"], "Summer/notes-1");
}

#[tokio::test]
async fn a_new_folder_names_its_collection_or_is_refused_with_the_rule_code() {
    let cases = [
        (
            json!({ "collection_id": null }),
            (400, "invalid_request"),
            "destination_required",
            "Summer",
        ),
        (
            json!({ "collection_id": STRANGER }),
            (404, "not_found"),
            "collection_not_found",
            "Summer",
        ),
        (
            json!({}),
            (400, "invalid_request"),
            "invalid_path",
            "provenance",
        ),
        (
            json!({}),
            (400, "invalid_request"),
            "invalid_path",
            ".filesquid/notes",
        ),
    ];
    for (extra, status, journal_code, folder) in cases {
        let rig = FilesRig::new().await;
        let answer = foldered(&rig, folder, extra).await;
        assert_refused(&rig, &answer, status, journal_code, "files.create_folder");
        assert_redacted(&rig, &["Summer"]);
    }
}

#[tokio::test]
async fn after_9999_numbers_a_new_folder_is_path_suffix_exhausted() {
    let rig = FilesRig::new().await;
    rig.home.place(&placed_id(0), &home(), "full/a.txt");
    for n in 1..=MAX_SUFFIX {
        rig.home
            .place(&placed_id(n), &home(), &format!("full-{n}/a.txt"));
    }
    let answer = foldered(&rig, "full", json!({})).await;
    assert_refused(
        &rig,
        &answer,
        (409, "conflict"),
        "path_suffix_exhausted",
        "files.create_folder",
    );
    assert!(rig.home.journal().is_empty(), "no snapshot version");
}

#[tokio::test]
async fn a_numbered_folder_over_255_bytes_is_name_too_long() {
    let rig = FilesRig::new().await;
    let long = "d".repeat(255);
    rig.home
        .place(&placed_id(1), &home(), &format!("{long}/a.txt"));
    let answer = foldered(&rig, &long, json!({})).await;
    assert_refused(
        &rig,
        &answer,
        (400, "invalid_request"),
        "name_too_long",
        "files.create_folder",
    );
}
