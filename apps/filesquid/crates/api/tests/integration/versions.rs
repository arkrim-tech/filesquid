//! Integration tests of version listing, labels, restore and history size (Task API-F2.5;
//! API-S-014, SRS-F-065, API-S-028) through the router, against a real OCFL storage root on
//! the local filesystem.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/retention_contract.rs"]
mod retention_contract;

use axum::http::{Method, StatusCode};
use files_fixture::{digest, query, Answer, FilesRig, LIBRARY, OBJECT};
use filesquid_api::files::{LABELS_PATH, MAX_LABELS};
use filesquid_api::history::ProvenanceContext;
use filesquid_api::journal::Outcome;
use filesquid_core::{InstallationId, MAX_LABEL_BYTES};
use filesquid_ocfl::read::Object;
use filesquid_prov::HostFacts;
use retention_contract::version_removals;
use serde_json::{json, Value};

/// Where the rig's object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// The installation of the provenance tests.
const INSTALLATION: &str = "0f3c2b1a-9d8e-4f7a-b6c5-d4e3f2a1b0c9";

/// A JSON body naming the rig's library and object plus `fields`.
fn body(fields: Value) -> Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        all[key] = value.clone();
    }
    all
}

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// `PUT /api/v1/files/labels` with the owner's token.
async fn put_label(rig: &FilesRig, version: &str, label: Value) -> Answer {
    let body = body(json!({ "version": version, "label": label }));
    rig.command_as(Method::PUT, "/api/v1/files/labels", body)
        .await
}

/// The file history of `path`.
async fn history(rig: &FilesRig, path: &str) -> Value {
    let answer = rig
        .get(&format!(
            "/api/v1/files/history?{}",
            query(&format!("path={path}"))
        ))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()
}

/// The logical paths of the object's head.
async fn head_paths(rig: &FilesRig) -> Vec<String> {
    let object = Object::open(rig.root.as_ref(), OBJECT_ROOT)
        .await
        .expect("object");
    let state = object.state(object.head()).expect("head state");
    state.paths().map(String::from).collect()
}

/// The head version's message.
async fn head_message(rig: &FilesRig) -> String {
    let object = Object::open(rig.root.as_ref(), OBJECT_ROOT)
        .await
        .expect("object");
    let head = object.head().clone();
    let block = &object.inventory().versions[&head];
    block.message.clone().unwrap_or_default()
}

/// Install provenance on the rig's file service.
fn with_provenance(rig: &FilesRig) {
    let context = ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    };
    assert!(rig.service.install_provenance(context, rig.journal.clone()));
}

// ---- Version listing ----

#[tokio::test]
async fn a_file_history_carries_size_digest_label_and_version_facts() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let page = history(&rig, "a.txt").await;
    let entries = page["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["version"]["version"], "v1");
    assert_eq!(entries[0]["size"], 5);
    assert_eq!(entries[0]["digest"], digest(b"alpha"));
    assert_eq!(entries[1]["size"], 6);
    assert_eq!(entries[1]["digest"], digest(b"bravo!"));
    for entry in entries {
        assert!(entry["version"]["created"].is_string());
        assert_eq!(entry["version"]["label"], Value::Null);
        assert!(entry["version"].get("user").is_some());
        assert!(entry["version"].get("message").is_some());
    }
}

#[tokio::test]
async fn a_deleted_version_has_no_size() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    rig.remove("a.txt").await;
    let entries = history(&rig, "a.txt").await["entries"].clone();
    assert_eq!(entries[1]["change"], "deleted");
    assert_eq!(entries[1]["size"], Value::Null);
}

#[tokio::test]
async fn an_earlier_version_is_served_with_ranged_reads() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=a.txt&version=v1"));
    let answer = rig
        .call(Method::GET, &uri, &[("range", "bytes=1-2")], Vec::new())
        .await;
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.header("content-range"), "bytes 1-2/5");
    assert_eq!(answer.body, b"lp");
}

// ---- Restore ----

#[tokio::test]
async fn restore_reinstates_chosen_files_in_exactly_one_version_with_a_label() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let request = body(json!({ "version": "v1", "paths": ["a.txt"], "label": "Original" }));
    let answer = rig.command("/api/v1/files/restorations", request).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["version"], "v3");
    assert_eq!(rig.head().await, "v3", "exactly one new version");
    let message = head_message(&rig).await;
    assert!(message.starts_with("filesquid.operation.v1 "), "{message}");
    assert!(message.contains("\"kind\":\"reinstate\""), "{message}");
    assert!(message.contains("\"label\":\"Original\""), "{message}");
    let entries = history(&rig, "a.txt").await["entries"].clone();
    assert_eq!(entries[2]["version"]["label"], "Original");
    assert_eq!(entries[2]["digest"], digest(b"alpha"));
    assert_eq!(entries[1]["digest"], digest(b"bravo!"), "v2 untouched");
}

#[tokio::test]
async fn restore_of_every_file_of_a_version_is_a_revert() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let request = body(json!({ "version": "v1", "paths": ["a.txt"] }));
    let answer = rig.command("/api/v1/files/restorations", request).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let message = head_message(&rig).await;
    assert!(message.contains("\"kind\":\"revert\""), "{message}");
}

#[tokio::test]
async fn restore_takes_a_label_not_a_free_message() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let request = body(json!({ "version": "v1", "paths": ["a.txt"], "message": "free text" }));
    let answer = rig.command("/api/v1/files/restorations", request).await;
    assert_error(&answer, 400, "invalid_request");
    assert_eq!(rig.head().await, "v2", "nothing published");
}

// ---- Later labels ----

#[tokio::test]
async fn a_later_label_is_committed_as_application_data_and_shown() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let answer = put_label(&rig, "v1", json!("First draft")).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["version"], "v3");
    assert!(head_paths(&rig).await.contains(&LABELS_PATH.to_string()));
    let message = head_message(&rig).await;
    assert!(message.contains("\"kind\":\"label\""), "{message}");
    assert!(
        !message.contains("\"label\":"),
        "the label version is not itself labelled"
    );
    let entries = history(&rig, "a.txt").await["entries"].clone();
    assert_eq!(entries[0]["version"]["label"], "First draft");
    assert_eq!(entries[1]["version"]["label"], Value::Null);
    let versions = rig
        .get(&format!("/api/v1/files/versions?{}", query("")))
        .await
        .json();
    assert_eq!(versions["versions"][0]["label"], "First draft");
    let uri = format!("/api/v1/files/details?{}", query("path=a.txt&version=v1"));
    let details = rig.get(&uri).await.json();
    assert_eq!(details["last_change"]["label"], "First draft", "{details}");
}

#[tokio::test]
async fn a_later_label_overrides_and_clears_a_commit_time_label() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let request = body(json!({ "version": "v1", "paths": ["a.txt"], "label": "Restored" }));
    rig.command("/api/v1/files/restorations", request).await;
    let renamed = put_label(&rig, "v3", json!("Renamed")).await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json());
    let entries = history(&rig, "a.txt").await["entries"].clone();
    assert_eq!(entries[2]["version"]["label"], "Renamed");
    let cleared = put_label(&rig, "v3", Value::Null).await;
    assert_eq!(cleared.status, StatusCode::OK, "{:?}", cleared.json());
    let entries = history(&rig, "a.txt").await["entries"].clone();
    assert_eq!(entries[2]["version"]["label"], Value::Null);
}

#[tokio::test]
async fn a_label_that_changes_nothing_is_refused_without_a_version() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    assert_eq!(
        put_label(&rig, "v1", json!("One")).await.status,
        StatusCode::OK
    );
    let head = rig.head().await;
    let again = put_label(&rig, "v1", json!("One")).await;
    assert_error(&again, 409, "conflict");
    let clear_nothing = put_label(&rig, "v2", Value::Null).await;
    assert_error(&clear_nothing, 409, "conflict");
    assert_eq!(rig.head().await, head, "nothing published");
}

#[tokio::test]
async fn labels_are_bounded_and_checked() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let long = "x".repeat(MAX_LABEL_BYTES + 1);
    for bad in [json!(""), json!("   "), json!("tab\there"), json!(long)] {
        assert_error(&put_label(&rig, "v1", bad).await, 400, "invalid_request");
    }
    assert_error(
        &put_label(&rig, "v9", json!("Later")).await,
        404,
        "not_found",
    );
    assert_eq!(rig.head().await, "v1", "nothing published");
    assert_eq!(MAX_LABELS, 1_000);
}

#[tokio::test]
async fn the_application_data_folder_is_reserved() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let request = body(json!({ "from": "a.txt", "to": ".filesquid/a.txt" }));
    let moved = rig.command("/api/v1/files/moves", request).await;
    assert_error(&moved, 400, "invalid_request");
    assert_eq!(rig.head().await, "v1");
}

// ---- History size ----

#[tokio::test]
async fn history_size_counts_each_kept_content_of_a_file_once() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let request = body(json!({ "version": "v1", "paths": ["a.txt"] }));
    rig.command("/api/v1/files/restorations", request).await;
    let uri = format!("/api/v1/files/history-size?{}", query("path=a.txt"));
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let size = answer.json();
    assert_eq!(size["versions"], 3);
    assert_eq!(size["contents"], 2);
    assert_eq!(size["bytes"], 11);
}

#[tokio::test]
async fn library_history_size_counts_every_kept_content_once() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let uri = format!("/api/v1/files/library-history-size?library_id={LIBRARY}");
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let size = answer.json();
    assert_eq!(size["library_id"], LIBRARY);
    assert_eq!(size["objects"], 1);
    assert_eq!(size["contents"], 2);
    assert_eq!(size["bytes"], 11);
    assert_eq!(size["truncated"], false);
}

#[tokio::test]
async fn an_object_that_cannot_be_opened_marks_the_library_size_truncated() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let broken = rig.dir.join("root").join("objects").join("broken");
    std::fs::create_dir_all(&broken).expect("broken object folder");
    std::fs::write(broken.join("0=ocfl_object_1.1"), "ocfl_object_1.1\n").expect("namaste");
    let uri = format!("/api/v1/files/library-history-size?library_id={LIBRARY}");
    let size = rig.get(&uri).await.json();
    assert_eq!(size["objects"], 1, "{size}");
    assert_eq!(size["bytes"], 5);
    assert_eq!(
        size["truncated"], true,
        "an unreadable object makes the total partial"
    );
}

#[tokio::test]
async fn a_folder_restore_brings_back_earlier_files_and_keeps_later_ones() {
    let rig = FilesRig::with_files(&[("docs/a.txt", b"alpha")]).await;
    rig.commit(&[("docs/a.txt", b"bravo!"), ("docs/new.txt", b"new")])
        .await;
    let request = body(json!({ "version": "v1", "paths": ["docs"] }));
    let answer = rig.command("/api/v1/files/restorations", request).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let paths = head_paths(&rig).await;
    assert!(paths.contains(&"docs/a.txt".to_string()));
    assert!(
        paths.contains(&"docs/new.txt".to_string()),
        "a restore never removes a file: {paths:?}"
    );
    let uri = format!("/api/v1/files/content?{}", query("path=docs%2Fa.txt"));
    assert_eq!(rig.get(&uri).await.body, b"alpha");
}

// ---- Retention ----

#[test]
fn the_contract_offers_no_version_deletion_expiry_or_thinning() {
    let doc: Value = serde_json::from_str(filesquid_api::CONTRACT).expect("contract");
    assert_eq!(version_removals(&doc), Vec::<String>::new());
}

#[test]
fn the_retention_check_catches_a_removal_under_any_method() {
    let doc = json!({ "paths": {
        "/api/v1/files/version-cleanups": { "post": {
            "operationId": "cleanUp", "summary": "Discard old versions of a file"
        }},
        "/api/v1/files/old": { "put": { "operationId": "removeVersion", "summary": "Tidy" }},
        "/api/v1/files/removals": { "post": {
            "operationId": "removeFiles", "summary": "Remove one file or folder"
        }}
    }});
    assert_eq!(
        version_removals(&doc),
        [
            "put /api/v1/files/old",
            "post /api/v1/files/version-cleanups"
        ]
    );
}

// ---- Provenance and journal ----

#[tokio::test]
async fn a_restore_commits_its_prov_o_record_and_journals_once() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    with_provenance(&rig);
    let request = body(json!({ "version": "v1", "paths": ["a.txt"], "label": "Back" }));
    let answer = rig.command("/api/v1/files/restorations", request).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let paths = head_paths(&rig).await;
    assert!(
        paths.contains(&"provenance/v3.jsonld".to_string()),
        "{paths:?}"
    );
    assert!(paths.contains(&"provenance/v3.journal.jsonl".to_string()));
    let object_ref = format!("{OBJECT}@v3");
    let commits = rig.journaled("ocfl.commit");
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_eq!(commits[0].object_ref.as_deref(), Some(object_ref.as_str()));
    let restores = rig.journaled("files.restore");
    assert_eq!(restores.len(), 1, "{restores:?}");
    assert_eq!(restores[0].outcome, Outcome::Succeeded);
    assert_eq!(restores[0].operation_id, commits[0].operation_id);
}

#[tokio::test]
async fn a_later_label_commits_its_prov_o_record() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    with_provenance(&rig);
    let answer = put_label(&rig, "v1", json!("Kept")).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert!(head_paths(&rig)
        .await
        .contains(&"provenance/v2.jsonld".to_string()));
    assert_eq!(rig.journaled("files.label").len(), 1);
    assert_eq!(rig.journaled("ocfl.commit").len(), 1);
}
