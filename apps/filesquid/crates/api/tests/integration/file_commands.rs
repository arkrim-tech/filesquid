//! Integration tests of the command envelopes on the file write routes (Task API-F2.13;
//! API-S-002, API-S-019, UI-S-012) through the router, against a real OCFL storage root on
//! the local filesystem. Every write route (upload, create folder, rename, move, copy,
//! remove, restore, version labels) takes a `CommandEnvelope` and answers a `CommandOutcome`
//! for the same operation ID; a stale expected revision is HTTP 409 with a `conflict`
//! outcome; a repeated operation ID answers its first outcome with no second version; and a
//! command that changed nothing stores no receipt.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{
    command_body, operation_id, upload_body, upload_input, Answer, FilesRig, LIBRARY, MULTIPART,
    OBJECT, UPLOAD_URI,
};
use filesquid_api::journal::Outcome;
use serde_json::{json, Value};

/// One file write route and a valid input for it on a prepared rig.
struct Case {
    /// The route's journal operation.
    operation: &'static str,
    /// The HTTP method.
    method: Method,
    /// The route.
    uri: &'static str,
    /// A valid input.
    input: Value,
    /// The object whose head the command changes, or `None` when it makes a new file.
    changes: Option<String>,
}

/// Every write route that publishes a version, each prepared on its own rig.
const ROUTES: [&str; 7] = [
    "upload",
    "renames",
    "moves",
    "copies",
    "removals",
    "restorations",
    "labels",
];

/// The rig's object input plus `fields`.
fn object_input(fields: Value) -> Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        all[key] = value.clone();
    }
    all
}

/// Upload a new file `name` and answer its file ID.
async fn uploaded(rig: &FilesRig, name: &str) -> String {
    let input = upload_input(name, 3, json!({ "name_normalization": "nfc" }));
    let answer = rig.upload(input, b"abc").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.result()["file_id"]
        .as_str()
        .expect("file id")
        .to_string()
}

/// A rig and a valid command of `route` on it.
async fn prepare(route: &str) -> (FilesRig, Case) {
    let rig = FilesRig::with_files(&[("a.txt", b"a"), ("b.txt", b"b")]).await;
    let object = Some(OBJECT.to_string());
    let case = match route {
        "upload" => Case {
            operation: "files.upload",
            method: Method::PUT,
            uri: UPLOAD_URI,
            input: upload_input("new.txt", 3, json!({})),
            changes: None,
        },
        "renames" => {
            let file_id = uploaded(&rig, "old.txt").await;
            Case {
                operation: "files.rename",
                method: Method::POST,
                uri: "/api/v1/files/renames",
                input: json!({ "library_id": LIBRARY, "file_id": file_id, "display_name": "New.txt" }),
                changes: Some(format!("urn:uuid:{file_id}")),
            }
        }
        "copies" => {
            let file_id = uploaded(&rig, "source.txt").await;
            Case {
                operation: "files.copy",
                method: Method::POST,
                uri: "/api/v1/files/copies",
                input: json!({ "library_id": LIBRARY, "file_id": file_id }),
                changes: None,
            }
        }
        "moves" => Case {
            operation: "files.move",
            method: Method::POST,
            uri: "/api/v1/files/moves",
            input: object_input(json!({ "from": "a.txt", "to": "c.txt" })),
            changes: object,
        },
        "removals" => Case {
            operation: "files.remove",
            method: Method::POST,
            uri: "/api/v1/files/removals",
            input: object_input(json!({ "path": "a.txt" })),
            changes: object,
        },
        "restorations" => {
            rig.remove("a.txt").await;
            Case {
                operation: "files.restore",
                method: Method::POST,
                uri: "/api/v1/files/restorations",
                input: object_input(json!({ "version": "v1", "paths": ["a.txt"] })),
                changes: object,
            }
        }
        "labels" => Case {
            operation: "files.label",
            method: Method::PUT,
            uri: "/api/v1/files/labels",
            input: object_input(json!({ "version": "v1", "label": "First" })),
            changes: object,
        },
        other => panic!("no case for {other}"),
    };
    (rig, case)
}

/// Send `case` with `operation` and `revision`.
async fn send(rig: &FilesRig, case: &Case, operation: &str, revision: &str) -> Answer {
    let body = command_body(&case.input, operation, revision);
    if case.uri == UPLOAD_URI {
        return rig
            .send_upload(&body["envelope"], &case.input, b"abc")
            .await;
    }
    rig.send_command(case.method.clone(), case.uri, &body).await
}

/// The revision `case` names now.
async fn revision(rig: &FilesRig, case: &Case) -> String {
    match case.uri {
        "/api/v1/files/copies" => rig.revision_of(&case.input).await,
        _ => match &case.changes {
            Some(object) => rig.head_of(object).await,
            None => "0".to_string(),
        },
    }
}

/// The succeeded journal entries of `operation`.
fn successes(rig: &FilesRig, operation: &str) -> usize {
    rig.journaled(operation)
        .iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .count()
}

/// Assert `answer` is a `committed` outcome of `operation` carrying a result.
fn assert_committed(answer: &Answer, operation: &str) {
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let json = answer.json();
    assert_eq!(json["operation_id"], operation, "{json:?}");
    assert_eq!(json["kind"], "committed", "{json:?}");
    assert_eq!(json["error"], Value::Null, "{json:?}");
    assert!(json["result"].is_object(), "{json:?}");
}

/// Assert `answer` is HTTP 409 with a `conflict` outcome of `operation`.
fn assert_conflict(answer: &Answer, operation: &str) {
    assert_eq!(answer.status, StatusCode::CONFLICT, "{:?}", answer.json());
    let json = answer.json();
    assert_eq!(json["operation_id"], operation, "{json:?}");
    assert_eq!(json["kind"], "conflict", "{json:?}");
    assert_eq!(json["error"], Value::Null, "{json:?}");
    assert_eq!(json["result"], Value::Null, "{json:?}");
    assert_eq!(json["code"], "conflict", "{json:?}");
}

/// Assert `answer` is the typed error `invalid_request`.
fn assert_invalid(answer: &Answer) {
    assert_eq!(
        answer.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        answer.json()
    );
    assert_eq!(
        answer.json()["code"],
        "invalid_request",
        "{:?}",
        answer.json()
    );
}

#[tokio::test]
async fn every_write_route_answers_a_committed_outcome_for_the_same_operation() {
    for route in ROUTES {
        let (rig, case) = prepare(route).await;
        let before = successes(&rig, case.operation);
        let operation = operation_id();
        let answer = send(&rig, &case, &operation, &revision(&rig, &case).await).await;
        assert_committed(&answer, &operation);
        assert_eq!(successes(&rig, case.operation), before + 1, "{route}");
        let entry = rig.journaled(case.operation).pop().expect("entry");
        assert_eq!(
            entry.operation_id.as_deref(),
            Some(operation.as_str()),
            "{route}"
        );
    }
}

#[tokio::test]
async fn every_write_route_refuses_a_body_without_an_envelope() {
    for route in ROUTES {
        let (rig, case) = prepare(route).await;
        let before = successes(&rig, case.operation);
        let answer = match case.uri {
            UPLOAD_URI => {
                let body = upload_body(None, Some(&case.input), Some(b"abc"));
                let headers = [("content-type", MULTIPART)];
                rig.call(Method::PUT, UPLOAD_URI, &headers, body).await
            }
            uri => {
                let headers = [("content-type", "application/json")];
                let body = case.input.to_string().into_bytes();
                rig.call(case.method.clone(), uri, &headers, body).await
            }
        };
        assert_invalid(&answer);
        assert_eq!(successes(&rig, case.operation), before, "{route}");
        let refused = rig.journaled(case.operation);
        let last = refused.last().expect("refusal journaled");
        assert_eq!(last.outcome, Outcome::Failed, "{route}");
    }
}

#[tokio::test]
async fn a_stale_expected_revision_is_a_conflict_outcome_with_no_version() {
    for route in ROUTES {
        let (rig, case) = prepare(route).await;
        let before = successes(&rig, case.operation);
        let head = match &case.changes {
            Some(object) => Some(rig.head_of(object).await),
            None => None,
        };
        let operation = operation_id();
        let answer = send(&rig, &case, &operation, "v99").await;
        assert_conflict(&answer, &operation);
        assert_eq!(successes(&rig, case.operation), before, "{route}");
        if let (Some(object), Some(head)) = (&case.changes, head) {
            assert_eq!(rig.head_of(object).await, head, "{route}: no version");
        }
    }
}

#[tokio::test]
async fn a_repeated_operation_answers_its_first_outcome_with_no_second_version() {
    for route in ROUTES {
        let (rig, case) = prepare(route).await;
        let before = successes(&rig, case.operation);
        let operation = operation_id();
        let revision = revision(&rig, &case).await;
        let first = send(&rig, &case, &operation, &revision).await;
        assert_committed(&first, &operation);
        let again = send(&rig, &case, &operation, &revision).await;
        assert_eq!(again.status, StatusCode::OK, "{route}: {:?}", again.json());
        assert_eq!(again.json(), first.json(), "{route}: the first outcome");
        assert_eq!(successes(&rig, case.operation), before + 1, "{route}");
    }
}

#[tokio::test]
async fn a_repeated_operation_is_answered_before_its_revision_is_checked() {
    let (rig, case) = prepare("moves").await;
    let operation = operation_id();
    let first = send(&rig, &case, &operation, "v1").await;
    assert_committed(&first, &operation);
    // The head is now v2, so v1 is stale, yet the stored receipt answers first (API-S-019).
    let again = send(&rig, &case, &operation, "v1").await;
    assert_eq!(again.json(), first.json());
    assert_eq!(rig.head_of(OBJECT).await, "v2");
}

#[tokio::test]
async fn a_refused_command_stores_no_receipt_so_its_retry_is_evaluated_again() {
    let (rig, case) = prepare("restorations").await;
    let operation = operation_id();
    // b.txt is the same at v2 and at the head v2, so restoring it changes nothing.
    let noop = object_input(json!({ "version": "v2", "paths": ["b.txt"] }));
    let body = command_body(&noop, &operation, "v2");
    let refused = rig.send_command(Method::POST, case.uri, &body).await;
    assert_conflict(&refused, &operation);
    rig.remove("b.txt").await;
    let body = command_body(&noop, &operation, "v3");
    let retried = rig.send_command(Method::POST, case.uri, &body).await;
    assert_committed(&retried, &operation);
    assert_eq!(rig.head_of(OBJECT).await, "v4");
}

#[tokio::test]
async fn creating_a_folder_answers_committed_and_stores_no_receipt() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let operation = operation_id();
    let input = object_input(json!({ "path": "albums" }));
    let body = command_body(&input, &operation, "v1");
    let pending = rig
        .send_command(Method::POST, "/api/v1/files/folders", &body)
        .await;
    assert_committed(&pending, &operation);
    assert_eq!(pending.result()["pending"], true);
    rig.commit(&[("albums/b.txt", b"b")]).await;
    let body = command_body(&input, &operation, "v2");
    let existing = rig
        .send_command(Method::POST, "/api/v1/files/folders", &body)
        .await;
    assert_committed(&existing, &operation);
    assert_eq!(existing.result()["pending"], false, "evaluated again");
    let stale = command_body(&input, &operation_id(), "v1");
    let answer = rig
        .send_command(Method::POST, "/api/v1/files/folders", &stale)
        .await;
    assert_conflict(
        &answer,
        stale["envelope"]["operation_id"].as_str().expect("id"),
    );
}

#[tokio::test]
async fn an_operation_id_of_another_command_is_refused() {
    let (rig, case) = prepare("moves").await;
    let operation = operation_id();
    assert_committed(&send(&rig, &case, &operation, "v1").await, &operation);
    let removal = object_input(json!({ "path": "b.txt" }));
    let body = command_body(&removal, &operation, "v2");
    let other_route = rig
        .send_command(Method::POST, "/api/v1/files/removals", &body)
        .await;
    assert_invalid(&other_route);
    let other_input = object_input(json!({ "from": "b.txt", "to": "d.txt" }));
    let body = command_body(&other_input, &operation, "v2");
    let other_body = rig.send_command(Method::POST, case.uri, &body).await;
    assert_invalid(&other_body);
    assert_eq!(rig.head_of(OBJECT).await, "v2", "nothing else changed");
}

#[tokio::test]
async fn an_envelope_of_another_schema_or_scope_is_refused() {
    let (rig, case) = prepare("removals").await;
    let mut wrong_schema = command_body(&case.input, &operation_id(), "v1");
    wrong_schema["envelope"]["schema_version"] = json!(2);
    let mut wrong_scope = command_body(&case.input, &operation_id(), "v1");
    wrong_scope["envelope"]["scope"] = json!("library:another");
    let mut wrong_id = command_body(&case.input, &operation_id(), "v1");
    wrong_id["envelope"]["operation_id"] = json!("not opaque/../");
    let mut extra = command_body(&case.input, &operation_id(), "v1");
    extra["force"] = json!(true);
    for body in [wrong_schema, wrong_scope, wrong_id, extra] {
        let answer = rig.send_command(Method::POST, case.uri, &body).await;
        assert_invalid(&answer);
    }
    assert_eq!(rig.head_of(OBJECT).await, "v1");
}

#[tokio::test]
async fn a_new_upload_names_revision_zero_and_a_copy_its_source_head() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let input = upload_input("x.txt", 3, json!({}));
    let operation = operation_id();
    let body = command_body(&input, &operation, "v1");
    let stale = rig.send_upload(&body["envelope"], &input, b"abc").await;
    assert_conflict(&stale, &operation);
    let file_id = uploaded(&rig, "source.txt").await;
    let rename = json!({ "library_id": LIBRARY, "file_id": file_id, "display_name": "S.txt" });
    let renamed = rig.command("/api/v1/files/renames", rename).await;
    assert_eq!(renamed.json()["kind"], "committed", "{:?}", renamed.json());
    let copy = json!({ "library_id": LIBRARY, "file_id": file_id });
    let operation = operation_id();
    let body = command_body(&copy, &operation, "v1");
    let answer = rig
        .send_command(Method::POST, "/api/v1/files/copies", &body)
        .await;
    assert_conflict(&answer, &operation);
    let body = command_body(&copy, &operation, "v2");
    let answer = rig
        .send_command(Method::POST, "/api/v1/files/copies", &body)
        .await;
    assert_committed(&answer, &operation);
}

#[tokio::test]
async fn an_upload_without_its_file_or_input_part_is_refused() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let input = upload_input("x.txt", 3, json!({}));
    let envelope = command_body(&input, &operation_id(), "0")["envelope"].clone();
    let headers = [("content-type", MULTIPART)];
    for body in [
        upload_body(Some(&envelope), Some(&input), None),
        upload_body(Some(&envelope), None, Some(b"abc")),
    ] {
        let answer = rig.call(Method::PUT, UPLOAD_URI, &headers, body).await;
        assert_invalid(&answer);
    }
    let raw = format!("{UPLOAD_URI}?library_id={LIBRARY}&name=x.txt&size=3");
    let answer = rig.call(Method::PUT, &raw, &[], b"abc".to_vec()).await;
    assert_invalid(&answer);
    assert_eq!(successes(&rig, "files.upload"), 0);
}
