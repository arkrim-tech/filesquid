//! Integration tests of the command envelopes on the library mutation routes (Task
//! API-F2.19; API-S-002, API-S-019, API-S-025, UI-S-012) through the router, against real
//! OCFL storage roots on the local filesystem. Create, open, copy and rebuild take a
//! `CommandEnvelope` beside their input and answer a `CommandOutcome` for the same operation
//! ID; a stale expected revision is HTTP 409 with a `conflict` outcome and no change; a
//! repeated operation ID of a command that changed something answers its first answer with no
//! second change; a command that changed nothing stores no receipt; the rebuild answers its
//! durable job as the `pending` outcome; and the location-choices and repair routes keep their
//! plain bodies.

#[path = "../support/library_routes_fixture.rs"]
mod fixture;

use axum::http::StatusCode;
use filesquid_api::journal::Outcome;
use filesquid_api::local_library::{
    COPY_OPERATION, CREATE_OPERATION, OPEN_OPERATION, REBUILD_OPERATION, RENAME_OPERATION,
};
use fixture::{
    command, library_command, location_command, operation_id, Answer, LibraryRig, Mount,
};
use serde_json::{json, Value};

/// The create route.
const CREATE: &str = "/api/v1/libraries";
/// The open route.
const OPEN: &str = "/api/v1/libraries/openings";
/// The copy route.
const COPY: &str = "/api/v1/libraries/snapshot-copies";
/// The rebuild route.
const REBUILD: &str = "/api/v1/libraries/rebuilds";
/// The rename route.
const RENAME: &str = "/api/v1/libraries/renames";

/// The rename command of `library` to `name`, expecting `revision`.
fn rename_body(library: &str, operation: &str, revision: &str, name: &str) -> Value {
    let input = json!({ "library_id": library, "name": name });
    library_command(library, operation, revision, input)
}

/// Assert `answer` is HTTP 200 with a `committed` outcome of `operation` and a result.
fn assert_committed(answer: &Answer, operation: &str) -> Value {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let body = answer.json();
    assert_eq!(body["operation_id"], operation, "{body}");
    assert_eq!(body["kind"], "committed", "{body}");
    assert_eq!(body["error"], Value::Null, "{body}");
    assert!(body["result"].is_object(), "{body}");
    body["result"].clone()
}

/// Assert `answer` is HTTP 409 with a `conflict` outcome of `operation` beside the typed
/// error fields.
fn assert_conflict(answer: &Answer, operation: &str) {
    assert_eq!(answer.status, StatusCode::CONFLICT, "{}", answer.text());
    let body = answer.json();
    assert_eq!(body["operation_id"], operation, "{body}");
    assert_eq!(body["kind"], "conflict", "{body}");
    assert_eq!(body["error"], Value::Null, "{body}");
    assert_eq!(body["result"], Value::Null, "{body}");
    assert_eq!(body["code"], "conflict", "{body}");
    assert!(body["message"].is_string(), "{body}");
    assert_eq!(body["retryable"], false, "{body}");
}

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{}", answer.text());
    assert_eq!(answer.json()["code"], code, "{}", answer.text());
}

/// How many journal entries of `operation` with `outcome` the rig holds.
fn count(rig: &LibraryRig, operation: &str, outcome: Outcome) -> usize {
    rig.journaled(operation)
        .into_iter()
        .filter(|entry| entry.outcome == outcome)
        .count()
}

/// A rig with a created library holding one complete object; answers the rig, the library
/// and its location.
async fn library(mount: Mount) -> (LibraryRig, String, String) {
    let rig = LibraryRig::new(mount);
    let (library, location) = rig.create("lib").await;
    rig.object(&location, "obj-a", "urn:uuid:a").await;
    (rig, library, location)
}

#[tokio::test]
async fn every_library_mutation_route_refuses_a_body_without_an_envelope() {
    let (rig, library, _) = library(Mount::Desktop).await;
    let dest = rig.locations.register("dest");
    let plain = [
        (CREATE, CREATE_OPERATION, json!({ "location_id": dest })),
        (OPEN, OPEN_OPERATION, json!({ "location_id": dest })),
        (
            COPY,
            COPY_OPERATION,
            json!({ "library_id": library, "destination_location_id": dest }),
        ),
        (REBUILD, REBUILD_OPERATION, json!({ "library_id": library })),
    ];
    for (uri, operation, body) in plain {
        let before = count(&rig, operation, Outcome::Failed);
        let answer = rig.post(uri, body).await;
        assert_error(&answer, 400, "invalid_request");
        assert_eq!(count(&rig, operation, Outcome::Failed), before + 1, "{uri}");
    }
    assert!(rig.files(&dest).is_empty(), "nothing was written");
    assert!(
        rig.journaled("job.created").is_empty(),
        "no job was submitted"
    );
}

#[tokio::test]
async fn create_open_copy_and_rebuild_answer_an_outcome_for_the_same_operation() {
    for mount in [Mount::Desktop, Mount::Server] {
        let rig = LibraryRig::new(mount);
        let location = rig.locations.register("new");
        let created = operation_id();
        let body = location_command(&location, &created, "0", None);
        let view = assert_committed(&rig.post(CREATE, body).await, &created);
        let library = view["library_id"].as_str().expect("library").to_string();
        assert_eq!(view["revision"], "1", "{view}");
        assert_eq!(view["objects"], 0, "{view}");

        let opened = operation_id();
        let body = location_command(&location, &opened, "0", Some(&library));
        let again = assert_committed(&rig.post(OPEN, body).await, &opened);
        assert_eq!(again["library_id"], library, "{again}");
        assert_eq!(again["revision"], "1", "{again}");

        let dest = rig.locations.register("copy");
        let copied = operation_id();
        let input = json!({ "library_id": library, "destination_location_id": dest });
        let body = library_command(&library, &copied, "1", input);
        let report = assert_committed(&rig.post(COPY, body).await, &copied);
        assert_eq!(report["objects"], 0, "{report}");

        let rebuilt = operation_id();
        let body = library_command(&library, &rebuilt, "1", json!({ "library_id": library }));
        let answer = rig.post(REBUILD, body).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        let pending = answer.json();
        assert_eq!(pending["operation_id"], rebuilt, "{pending}");
        assert_eq!(pending["kind"], "pending", "{pending}");
        assert_eq!(pending["error"], Value::Null, "{pending}");
        let job = &pending["result"];
        assert_eq!(job["outcome"], "pending", "{pending}");
        assert_eq!(job["job"]["kind"], "library.rebuild", "{pending}");
        assert_eq!(job["job"]["operation_id"], rebuilt, "{pending}");
        assert!(
            job["job_id"].is_string() && job["scheduled"].is_boolean(),
            "{pending}"
        );
    }
}

#[tokio::test]
async fn a_repeated_operation_id_answers_its_first_answer_with_no_second_change() {
    let (rig, library, _) = library(Mount::Desktop).await;
    let dest = rig.locations.register("copy");
    let copied = operation_id();
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let body = library_command(&library, &copied, "1", input);
    let first = rig.post(COPY, body.clone()).await;
    assert_committed(&first, &copied);
    let files = rig.files(&dest);
    assert!(!files.is_empty(), "the copy wrote the destination");

    let second = rig.post(COPY, body).await;
    assert_eq!(second.json(), first.json(), "the first answer again");
    assert_eq!(rig.files(&dest), files, "no second write");
    assert_eq!(count(&rig, COPY_OPERATION, Outcome::Succeeded), 1);

    let rebuilt = operation_id();
    let body = library_command(&library, &rebuilt, "1", json!({ "library_id": library }));
    let first = rig.post(REBUILD, body.clone()).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let second = rig.post(REBUILD, body).await;
    assert_eq!(second.json(), first.json(), "the same job again");
    assert_eq!(rig.journaled("job.created").len(), 1, "one job");
}

#[tokio::test]
async fn a_repeated_create_answers_the_same_library_once_written() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("new");
    let created = operation_id();
    let body = location_command(&location, &created, "0", None);
    let first = rig.post(CREATE, body.clone()).await;
    let view = assert_committed(&first, &created);
    let second = rig.post(CREATE, body).await;
    assert_eq!(second.json(), first.json(), "the first answer again");
    assert_eq!(count(&rig, CREATE_OPERATION, Outcome::Succeeded), 1);
    assert_eq!(count(&rig, CREATE_OPERATION, Outcome::Failed), 0);
    let other = operation_id();
    let fresh = rig
        .post(CREATE, location_command(&location, &other, "0", None))
        .await;
    assert_conflict(&fresh, &other);
    let _ = view;
}

#[tokio::test]
async fn a_stale_expected_revision_is_a_conflict_outcome_with_no_change() {
    let (rig, library, _) = library(Mount::Desktop).await;
    let dest = rig.locations.register("copy");
    let copied = operation_id();
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let answer = rig
        .post(COPY, library_command(&library, &copied, "2", input))
        .await;
    assert_conflict(&answer, &copied);
    assert!(rig.files(&dest).is_empty(), "nothing was copied");

    let rebuilt = operation_id();
    let body = library_command(&library, &rebuilt, "0", json!({ "library_id": library }));
    assert_conflict(&rig.post(REBUILD, body).await, &rebuilt);
    assert!(
        rig.journaled("job.created").is_empty(),
        "no job was submitted"
    );

    let fresh = rig.locations.register("fresh");
    let created = operation_id();
    let answer = rig
        .post(CREATE, location_command(&fresh, &created, "1", None))
        .await;
    assert_conflict(&answer, &created);
    assert!(rig.files(&fresh).is_empty(), "no declaration was written");

    let opened = operation_id();
    let answer = rig
        .post(OPEN, location_command(&fresh, &opened, "7", None))
        .await;
    assert_conflict(&answer, &opened);
    assert_eq!(count(&rig, COPY_OPERATION, Outcome::Failed), 1);
    assert_eq!(count(&rig, REBUILD_OPERATION, Outcome::Failed), 1);
    assert_eq!(count(&rig, CREATE_OPERATION, Outcome::Failed), 1);
    assert_eq!(count(&rig, OPEN_OPERATION, Outcome::Failed), 1);
}

#[tokio::test]
async fn an_envelope_of_another_schema_or_scope_is_refused() {
    let (rig, library, location) = library(Mount::Desktop).await;
    let dest = rig.locations.register("copy");
    let mut other_schema = location_command(&dest, &operation_id(), "0", None);
    other_schema["envelope"]["schema_version"] = json!(2);
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let wrong_scope = command(
        &format!("location:{location}"),
        &operation_id(),
        "1",
        input.clone(),
    );
    let not_opaque = library_command(&library, "op with spaces", "1", input);
    let library_scope = command(
        &format!("library:{library}"),
        &operation_id(),
        "0",
        json!({ "location_id": dest }),
    );
    for (uri, operation, body) in [
        (CREATE, CREATE_OPERATION, other_schema),
        (COPY, COPY_OPERATION, wrong_scope),
        (COPY, COPY_OPERATION, not_opaque),
        (OPEN, OPEN_OPERATION, library_scope),
    ] {
        let before = count(&rig, operation, Outcome::Failed);
        let answer = rig.post(uri, body).await;
        assert_error(&answer, 400, "invalid_request");
        assert_eq!(
            count(&rig, operation, Outcome::Failed),
            before + 1,
            "{uri}: journaled once"
        );
    }
    assert!(rig.files(&dest).is_empty(), "nothing was written");
}

#[tokio::test]
async fn an_operation_id_of_another_command_is_refused() {
    let (rig, library, _) = library(Mount::Desktop).await;
    let dest = rig.locations.register("copy");
    let shared = operation_id();
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let body = library_command(&library, &shared, "1", input);
    assert_committed(&rig.post(COPY, body).await, &shared);
    let other = rig.locations.register("other");
    let before = count(&rig, CREATE_OPERATION, Outcome::Failed);
    let answer = rig
        .post(CREATE, location_command(&other, &shared, "0", None))
        .await;
    assert_error(&answer, 400, "invalid_request");
    assert_eq!(count(&rig, CREATE_OPERATION, Outcome::Failed), before + 1);
    assert!(rig.files(&other).is_empty(), "nothing was written");
}

#[tokio::test]
async fn a_repeated_open_and_a_refusal_store_no_receipt() {
    let (rig, library, location) = library(Mount::Desktop).await;
    let opened = operation_id();
    let body = location_command(&location, &opened, "1", Some(&library));
    let again = assert_committed(&rig.post(OPEN, body).await, &opened);
    assert_eq!(again["library_id"], library, "{again}");
    // The open changed nothing, so its operation ID is free for another command.
    let dest = rig.locations.register("copy");
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let body = library_command(&library, &opened, "1", input);
    assert_committed(&rig.post(COPY, body).await, &opened);

    let refused = operation_id();
    let other = rig.locations.register("other");
    let stale = location_command(&other, &refused, "3", None);
    assert_conflict(&rig.post(CREATE, stale).await, &refused);
    let retry = location_command(&other, &refused, "0", None);
    assert_committed(&rig.post(CREATE, retry).await, &refused);
}

#[tokio::test]
async fn a_rebuild_with_an_unfinished_job_answers_that_job_under_a_new_operation_id() {
    let rig = LibraryRig::without_rebuild_runner(Mount::Desktop);
    let (library, location) = rig.create("lib").await;
    rig.object(&location, "obj-a", "urn:uuid:a").await;
    let first = operation_id();
    let body = library_command(&library, &first, "1", json!({ "library_id": library }));
    let started = rig.post(REBUILD, body).await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.text());
    let job_id = started.json()["result"]["job_id"].clone();

    let second = operation_id();
    let body = library_command(&library, &second, "1", json!({ "library_id": library }));
    let answered = rig.post(REBUILD, body).await;
    assert_eq!(answered.status, StatusCode::OK, "{}", answered.text());
    let again = answered.json();
    assert_eq!(again["operation_id"], second, "{again}");
    assert_eq!(again["kind"], "pending", "{again}");
    assert_eq!(again["result"]["job_id"], job_id, "the unfinished job");
    assert_eq!(rig.journaled("job.created").len(), 1, "no second job");
    // Answering an unfinished job stored no receipt: the ID is free for another command.
    let dest = rig.locations.register("copy");
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let body = library_command(&library, &second, "1", input);
    assert_committed(&rig.post(COPY, body).await, &second);
}

#[tokio::test]
async fn a_rename_takes_an_envelope_advances_the_revision_and_replays_its_answer() {
    let (rig, library, _) = library(Mount::Desktop).await;
    let plain = json!({ "library_id": library, "name": "Plain" });
    assert_error(&rig.post(RENAME, plain).await, 400, "invalid_request");

    let renamed = operation_id();
    let body = rename_body(&library, &renamed, "1", "Family archive");
    let first = rig.post(RENAME, body.clone()).await;
    let facts = assert_committed(&first, &renamed);
    assert_eq!(facts["name"], "Family archive", "{facts}");
    assert_eq!(
        facts["revision"], "2",
        "a changed name advances it: {facts}"
    );
    let again = rig.post(RENAME, body).await;
    assert_eq!(again.json(), first.json(), "the first answer again");
    assert_eq!(count(&rig, RENAME_OPERATION, Outcome::Succeeded), 1);

    let stale = operation_id();
    let body = rename_body(&library, &stale, "1", "Stale name");
    assert_conflict(&rig.post(RENAME, body).await, &stale);
    let read = rig.get(&format!("/api/v1/libraries/{library}/facts")).await;
    assert_eq!(read.json()["name"], "Family archive", "nothing changed");
    assert_eq!(read.json()["revision"], "2", "{}", read.text());

    let same = operation_id();
    let body = rename_body(&library, &same, "2", "Family archive");
    let unchanged = assert_committed(&rig.post(RENAME, body).await, &same);
    assert_eq!(
        unchanged["revision"], "2",
        "a rename to the current name advances nothing"
    );
    // It changed nothing, so its operation ID stored no receipt and is free for another command.
    let dest = rig.locations.register("copy");
    let input = json!({ "library_id": library, "destination_location_id": dest });
    let copy = library_command(&library, &same, "2", input);
    assert_committed(&rig.post(COPY, copy).await, &same);
    assert_eq!(count(&rig, RENAME_OPERATION, Outcome::Succeeded), 1);
}

/// The open command of the folder `location` holding `library`, naming it `name`.
fn renaming_open(
    location: &str,
    library: &str,
    operation: &str,
    revision: &str,
    name: &str,
) -> Value {
    let mut body = location_command(location, operation, revision, Some(library));
    body["input"]["name"] = json!(name);
    body
}

#[tokio::test]
async fn an_open_that_renames_an_open_library_expects_its_current_revision() {
    let (rig, library, location) = library(Mount::Desktop).await;
    let stale = operation_id();
    let body = renaming_open(&location, &library, &stale, "0", "Renamed by open");
    let before = count(&rig, OPEN_OPERATION, Outcome::Failed);
    assert_conflict(&rig.post(OPEN, body).await, &stale);
    assert_eq!(count(&rig, OPEN_OPERATION, Outcome::Failed), before + 1);
    let read = rig.get(&format!("/api/v1/libraries/{library}/facts")).await;
    assert_ne!(read.json()["name"], "Renamed by open", "nothing changed");
    assert_eq!(read.json()["revision"], "1", "{}", read.text());

    let renamed = operation_id();
    let body = renaming_open(&location, &library, &renamed, "1", "Renamed by open");
    let first = rig.post(OPEN, body.clone()).await;
    let view = assert_committed(&first, &renamed);
    assert_eq!(view["revision"], "2", "the rename advances it: {view}");
    let again = rig.post(OPEN, body).await;
    assert_eq!(
        again.json(),
        first.json(),
        "the stored answer, no second change"
    );
    assert_eq!(count(&rig, RENAME_OPERATION, Outcome::Succeeded), 1);

    let unnamed = operation_id();
    let body = location_command(&location, &unnamed, "0", Some(&library));
    let plain = assert_committed(&rig.post(OPEN, body).await, &unnamed);
    assert_eq!(
        plain["revision"], "2",
        "an open that renames nothing: {plain}"
    );
}

#[tokio::test]
async fn a_rename_waits_for_a_running_open_so_its_revision_check_sees_it() {
    let (rig, library, location) = library(Mount::Desktop).await;
    let opened = operation_id();
    let open = renaming_open(&location, &library, &opened, "1", "By open");
    let renamed = operation_id();
    let rename = rename_body(&library, &renamed, "1", "By rename");
    let (open, rename) = tokio::join!(rig.post(OPEN, open), rig.post(RENAME, rename));
    let committed = [&open, &rename]
        .iter()
        .filter(|answer| answer.json()["kind"] == "committed")
        .count();
    assert_eq!(
        committed,
        1,
        "one expected revision 1 wins: {} {}",
        open.text(),
        rename.text()
    );
    let read = rig.get(&format!("/api/v1/libraries/{library}/facts")).await;
    assert_eq!(read.json()["revision"], "2", "{}", read.text());
    assert_eq!(count(&rig, RENAME_OPERATION, Outcome::Succeeded), 1);
}

#[tokio::test]
async fn the_location_choices_and_repair_routes_keep_their_bodies() {
    let (rig, library, location) = library(Mount::Desktop).await;
    rig.w010_object(&location, "obj-b", "urn:uuid:b").await;
    let repaired = rig
        .post(
            "/api/v1/libraries/repairs",
            LibraryRig::consented(&library, &["urn:uuid:b"]),
        )
        .await;
    assert_eq!(repaired.status, StatusCode::OK, "{}", repaired.text());
    assert_eq!(repaired.json()["objects"][0]["result"], "repaired");
    let choice = rig
        .post(
            "/api/v1/libraries/location-choices",
            json!({ "purpose": "create" }),
        )
        .await;
    assert_error(&choice, 503, "unavailable");
}
