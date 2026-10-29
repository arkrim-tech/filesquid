//! Integration tests of the library and repair routes (Task API-F1.12; API-S-025, API-S-023,
//! API-S-004, API-S-005, API-S-017, OCFL-S-020) through the router, on the desktop and the
//! server mount, against real OCFL storage roots on the local filesystem injected as
//! `StorageBackend`s: create, open, copy a completed snapshot, rebuild from files, the
//! repair-candidate query and the consented version-inventory repair.

#[path = "../support/library_routes_fixture.rs"]
mod fixture;

use axum::http::{Method, StatusCode};
use filesquid_api::auth::DENIED_OPERATION;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::local_library::{
    CANDIDATES_OPERATION, COPY_OPERATION, CREATE_OPERATION, MAX_OPEN_LIBRARIES, OPEN_OPERATION,
    REBUILD_OPERATION, REPAIR_OPERATION,
};
use fixture::{
    library_command, location_command, operation_id, Answer, LibraryRig, Mount, LIBRARY,
};
use serde_json::{json, Value};

/// Both mounts, desktop first.
const MOUNTS: [Mount; 2] = [Mount::Desktop, Mount::Server];

/// The create or open command of `location` under a fresh operation ID, expecting `0`,
/// naming `library` when given.
fn bind_body(location: &str, library: Option<&str>) -> Value {
    location_command(location, &operation_id(), "0", library)
}

/// The copy command of the open library `library` to `dest`, expecting revision `1`.
fn copy_body(library: &str, dest: &str) -> Value {
    let input = json!({ "library_id": library, "destination_location_id": dest });
    library_command(library, &operation_id(), "1", input)
}

/// The rebuild command of the open library `library`, expecting revision `1`.
fn rebuild_body(library: &str) -> Value {
    library_command(
        library,
        &operation_id(),
        "1",
        json!({ "library_id": library }),
    )
}

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{}", answer.text());
    assert_eq!(answer.json()["code"], code, "{}", answer.text());
}

/// Failed journal entries so far.
fn failures(rig: &LibraryRig) -> Vec<JournalEntry> {
    rig.journal()
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect()
}

/// Assert exactly one more failed entry than `before`, for `operation` with `error_code`,
/// and answer it.
fn assert_one_refusal(
    rig: &LibraryRig,
    before: usize,
    operation: &str,
    error_code: &str,
) -> JournalEntry {
    let failed = failures(rig);
    assert_eq!(failed.len(), before + 1, "exactly one refusal: {failed:?}");
    let entry = failed.last().expect("refusal").clone();
    assert_eq!(entry.operation, operation);
    assert_eq!(entry.error_code.as_deref(), Some(error_code));
    entry
}

/// Assert no answer body or journal entry carries the rig directory, an object root or
/// content.
fn assert_redacted(rig: &LibraryRig, answers: &[&Answer]) {
    let dir = rig.dir_text();
    let journal = serde_json::to_string(&rig.journal()).expect("journal json");
    for text in answers.iter().map(|a| a.text()).chain([journal]) {
        assert!(!text.contains(&dir), "a path leaked: {text}");
        for secret in [
            "obj-a",
            "obj-b",
            "inventory.json",
            "alpha",
            "bravo",
            "0=ocfl",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
    }
}

// ---- Create and open ----

#[tokio::test]
async fn create_writes_only_the_declaration_and_answers_an_opaque_library_id() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let location = rig.locations.register("new");
        let answer = rig
            .post("/api/v1/libraries", bind_body(&location, None))
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        let body = answer.result();
        let library = body["library_id"].as_str().expect("library id");
        assert_eq!(library.len(), 36, "a minted UUID: {library}");
        assert_eq!(body["spec_version"], "1.1");
        assert_eq!(body["objects"], 0);
        assert_eq!(rig.files(&location), ["0=ocfl_1.1"]);
        let created = rig.journaled(CREATE_OPERATION);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].outcome, Outcome::Succeeded);
        assert_eq!(created[0].target_ids, [library]);
        assert_redacted(&rig, &[&answer]);
    }
}

#[tokio::test]
async fn create_keeps_a_library_id_the_caller_names() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("named");
    let body = bind_body(&location, Some(LIBRARY));
    let answer = rig.post("/api/v1/libraries", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    assert_eq!(answer.result()["library_id"], LIBRARY);
}

#[tokio::test]
async fn create_in_a_folder_that_holds_a_file_is_refused_and_writes_nothing() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("full");
    std::fs::write(rig.locations.folder(&location).join("notes.txt"), b"mine").expect("file");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries", bind_body(&location, None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_eq!(rig.files(&location), ["notes.txt"]);
    assert_one_refusal(&rig, before, CREATE_OPERATION, "root_not_empty");
    assert_redacted(&rig, &[&answer]);
}

#[tokio::test]
async fn an_unknown_location_is_not_found_and_journaled_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries", bind_body("loc-missing", None))
        .await;
    assert_error(&answer, 404, "not_found");
    assert_one_refusal(&rig, before, CREATE_OPERATION, "location_not_found");
}

#[tokio::test]
async fn a_malformed_body_is_invalid_and_journaled_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries", json!({ "path": "/home/someone" }))
        .await;
    assert_error(&answer, 400, "invalid_request");
    assert_one_refusal(&rig, before, CREATE_OPERATION, "invalid_request");
}

#[tokio::test]
async fn open_answers_the_library_and_its_objects() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let (_, location) = rig.create("source").await;
        rig.object(&location, "obj-a", "urn:uuid:a").await;
        let other = LibraryRig::new(mount);
        let reopened = other.locations.register("source");
        std::fs::remove_dir_all(other.locations.folder(&reopened)).expect("clear");
        copy_dir(
            &rig.locations.folder(&location),
            &other.locations.folder(&reopened),
        );
        let body = bind_body(&reopened, Some(LIBRARY));
        let answer = other.post("/api/v1/libraries/openings", body).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        assert_eq!(answer.result()["library_id"], LIBRARY);
        assert_eq!(answer.result()["objects"], 1);
        assert!(
            other.journaled(OPEN_OPERATION).is_empty(),
            "reads succeed silently"
        );
        assert_redacted(&other, &[&answer]);
    }
}

#[tokio::test]
async fn opening_a_folder_that_is_not_a_library_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("plain");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/openings", bind_body(&location, None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "not_a_storage_root");
}

#[tokio::test]
async fn opening_an_open_location_answers_its_library_again() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, location) = rig.create("lib").await;
    let answer = rig
        .post("/api/v1/libraries/openings", bind_body(&location, None))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    assert_eq!(answer.result()["library_id"], library.as_str());
    let other = bind_body(&location, Some(LIBRARY));
    let before = failures(&rig).len();
    let refused = rig.post("/api/v1/libraries/openings", other).await;
    assert_error(&refused, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "location_in_use");
}

#[tokio::test]
async fn an_open_library_folder_under_a_second_location_id_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (_, location) = rig.create("lib").await;
    rig.locations.alias(&location, "loc-again");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/openings", bind_body("loc-again", None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "location_in_use");
}

#[tokio::test]
async fn opening_a_folder_answers_the_library_its_snapshot_names() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (_, location) = rig.create("named").await;
    let snapshot = format!("urn:uuid:{LIBRARY}");
    let mine = LibraryRig::new(Mount::Desktop);
    let copy = mine.locations.register("named");
    std::fs::remove_dir_all(mine.locations.folder(&copy)).expect("clear");
    rig.object(&location, "snapshot", &snapshot).await;
    copy_dir(
        &rig.locations.folder(&location),
        &mine.locations.folder(&copy),
    );
    let answer = mine
        .post("/api/v1/libraries/openings", bind_body(&copy, None))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    assert_eq!(
        answer.result()["library_id"],
        LIBRARY,
        "not a newly minted ID"
    );
}

#[tokio::test]
async fn opening_a_folder_under_another_library_id_than_its_snapshot_s_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("named");
    std::fs::remove_dir_all(rig.locations.folder(&location)).expect("clear");
    let source = LibraryRig::new(Mount::Desktop);
    let (_, origin) = source.create("named").await;
    source
        .object(&origin, "snapshot", &format!("urn:uuid:{LIBRARY}"))
        .await;
    copy_dir(
        &source.locations.folder(&origin),
        &rig.locations.folder(&location),
    );
    let other = "0e1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
    let before = failures(&rig).len();
    let answer = rig
        .post(
            "/api/v1/libraries/openings",
            bind_body(&location, Some(other)),
        )
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "library_mismatch");
}

#[tokio::test]
async fn opening_a_folder_whose_snapshot_cannot_be_read_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let location = rig.locations.register("damaged");
    std::fs::remove_dir_all(rig.locations.folder(&location)).expect("clear");
    let source = LibraryRig::new(Mount::Desktop);
    let (_, origin) = source.create("damaged").await;
    source
        .object(&origin, "snapshot", &format!("urn:uuid:{LIBRARY}"))
        .await;
    copy_dir(
        &source.locations.folder(&origin),
        &rig.locations.folder(&location),
    );
    let inventory = rig
        .locations
        .folder(&location)
        .join("snapshot")
        .join("inventory.json");
    std::fs::write(&inventory, b"{ not an inventory").expect("damage the snapshot");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/openings", bind_body(&location, None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "snapshot_unreadable");
}

#[tokio::test]
async fn a_second_folder_of_an_open_library_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let first = rig.locations.register("first");
    let body = bind_body(&first, Some(LIBRARY));
    let created = rig.post("/api/v1/libraries", body).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    rig.object(&first, "snapshot", &format!("urn:uuid:{LIBRARY}"))
        .await;
    let second = rig.locations.register("second");
    std::fs::remove_dir_all(rig.locations.folder(&second)).expect("clear");
    copy_dir(
        &rig.locations.folder(&first),
        &rig.locations.folder(&second),
    );
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/openings", bind_body(&second, None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, OPEN_OPERATION, "library_already_open");
}

#[tokio::test]
async fn open_libraries_are_bounded() {
    let rig = LibraryRig::new(Mount::Desktop);
    for n in 0..MAX_OPEN_LIBRARIES {
        rig.create(&format!("lib-{n}")).await;
    }
    let location = rig.locations.register("one-too-many");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries", bind_body(&location, None))
        .await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, CREATE_OPERATION, "too_many_libraries");
    assert!(rig.files(&location).is_empty(), "nothing written");
}

// ---- Copy and rebuild ----

#[tokio::test]
async fn a_completed_snapshot_is_copied_to_an_empty_location_and_validates() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let (library, location) = rig.create("source").await;
        rig.object(&location, "obj-a", "urn:uuid:a").await;
        rig.w010_object(&location, "obj-b", "urn:uuid:b").await;
        let source_files = rig.files(&location);
        let dest = rig.locations.register("copy");
        let body = copy_body(&library, &dest);
        let answer = rig.post("/api/v1/libraries/snapshot-copies", body).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        let report = answer.result();
        assert_eq!(report["objects"], 2);
        assert_eq!(report["invalid_objects"], json!([]));
        assert!(report["files"].as_u64().expect("files") > 0);
        assert_eq!(
            rig.files(&location),
            source_files,
            "the source is untouched"
        );
        assert!(rig
            .files(&dest)
            .contains(&"obj-a/v2/inventory.json".to_string()));
        assert!(!rig
            .files(&dest)
            .contains(&"obj-b/v2/inventory.json".to_string()));
        let copied = rig.journaled(COPY_OPERATION);
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].outcome, Outcome::Succeeded);
        assert_redacted(&rig, &[&answer]);
    }
}

#[tokio::test]
async fn a_copy_into_an_open_library_or_of_an_unknown_library_is_refused() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, location) = rig.create("source").await;
    let before = failures(&rig).len();
    let body = copy_body(&library, &location);
    let answer = rig.post("/api/v1/libraries/snapshot-copies", body).await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, COPY_OPERATION, "destination_overlaps_library");
    let (_, other) = rig.create("other").await;
    let before = failures(&rig).len();
    let body = copy_body(&library, &other);
    let answer = rig.post("/api/v1/libraries/snapshot-copies", body).await;
    assert_error(&answer, 409, "conflict");
    assert_one_refusal(&rig, before, COPY_OPERATION, "destination_overlaps_library");
    assert_eq!(
        rig.files(&other),
        ["0=ocfl_1.1"],
        "another open library is untouched"
    );
    let dest = rig.locations.register("copy");
    let before = failures(&rig).len();
    let body = copy_body(&LIBRARY, &dest);
    let answer = rig.post("/api/v1/libraries/snapshot-copies", body).await;
    assert_error(&answer, 404, "not_found");
    assert_one_refusal(&rig, before, COPY_OPERATION, "library_not_open");
    assert!(rig.files(&dest).is_empty(), "nothing written");
}

#[tokio::test]
async fn the_rebuild_answers_pending_with_a_library_rebuild_job_whose_result_is_the_report() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let (library, location) = rig.create("lib").await;
        rig.object(&location, "obj-a", "urn:uuid:a").await;
        rig.w010_object(&location, "obj-b", "urn:uuid:b").await;
        let files = rig.files(&location);
        let answer = rig
            .post("/api/v1/libraries/rebuilds", rebuild_body(&library))
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        assert_eq!(answer.json()["kind"], "pending", "{}", answer.text());
        let body = answer.result();
        assert_eq!(body["outcome"], "pending", "{body}");
        assert_eq!(body["job"]["kind"], "library.rebuild");
        assert_eq!(body["job"]["id"], body["job_id"]);
        assert_eq!(body["job"]["result"], Value::Null, "no result yet: {body}");
        assert_eq!(body["scheduled"], true, "handed to the host's runner");
        let job = rig
            .finished_job(body["job_id"].as_str().expect("job id"))
            .await;
        assert_eq!(job["state"], "completed", "{job}");
        assert_eq!(
            (job["done"].as_u64(), job["total"].as_u64()),
            (Some(2), Some(2))
        );
        let objects = job["result"]["objects"]
            .as_array()
            .expect("objects")
            .clone();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0]["object_id"], "urn:uuid:a");
        assert_eq!(objects[0]["missing_version_inventory"], false);
        assert_eq!(objects[1]["object_id"], "urn:uuid:b");
        assert_eq!(objects[1]["missing_version_inventory"], true);
        assert_eq!(rig.files(&location), files, "nothing written");
        let rebuilt = rig.journaled(REBUILD_OPERATION);
        assert_eq!(rebuilt.len(), 1, "the finished rebuild is journaled once");
        assert_eq!(rebuilt[0].outcome, Outcome::Succeeded);
        assert_eq!(rig.journaled("job.created").len(), 1);
        assert_eq!(rig.journaled("job.completed").len(), 1);
        let view = rig
            .get(&format!(
                "/api/v1/jobs/{}",
                body["job_id"].as_str().unwrap()
            ))
            .await;
        let listed = rig.get("/api/v1/jobs?kind=library.rebuild").await;
        assert_eq!(listed.json()["jobs"][0]["state"], "completed");
        assert_eq!(
            listed.json()["jobs"][0]["result"],
            Value::Null,
            "the listing carries no report; the job route does"
        );
        assert_redacted(&rig, &[&answer, &view]);
    }
}

#[tokio::test]
async fn a_repeated_rebuild_answers_the_unfinished_job_and_cancel_changes_no_file() {
    let rig = LibraryRig::without_rebuild_runner(Mount::Desktop);
    let (library, location) = rig.create("lib").await;
    rig.object(&location, "obj-a", "urn:uuid:a").await;
    let files = rig.files(&location);
    let body = rebuild_body(&library);
    let first = rig.post("/api/v1/libraries/rebuilds", body.clone()).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(first.result()["scheduled"], false, "no runner: it waits");
    let second = rig.post("/api/v1/libraries/rebuilds", body).await;
    assert_eq!(
        second.result()["job_id"],
        first.result()["job_id"],
        "one job"
    );
    assert_eq!(rig.journaled("job.created").len(), 1);
    let id = first.result()["job_id"].as_str().expect("id").to_string();
    let cancel = rig
        .send(
            Method::POST,
            &format!("/api/v1/jobs/{id}/cancel"),
            Some(&format!("Bearer {}", rig.token)),
            None,
        )
        .await;
    assert_eq!(cancel.status, StatusCode::OK, "{}", cancel.text());
    assert_eq!(cancel.json()["job"]["state"], "cancelled");
    assert_eq!(cancel.json()["job"]["result"], Value::Null);
    assert_eq!(rig.files(&location), files, "cancel changed no file");
    assert_eq!(rig.journaled("job.cancelled").len(), 1);
    assert!(rig.journaled(REBUILD_OPERATION).is_empty(), "it never ran");
}

#[tokio::test]
async fn a_rebuild_of_a_library_that_is_not_open_submits_no_job() {
    let rig = LibraryRig::new(Mount::Desktop);
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/rebuilds", rebuild_body(&LIBRARY))
        .await;
    assert_error(&answer, 404, "not_found");
    assert_one_refusal(&rig, before, REBUILD_OPERATION, "library_not_open");
    assert!(rig.journaled("job.created").is_empty(), "no job");
    let before = failures(&rig).len();
    let answer = rig
        .post("/api/v1/libraries/rebuilds", json!({ "library": LIBRARY }))
        .await;
    assert_error(&answer, 400, "invalid_request");
    assert_one_refusal(&rig, before, REBUILD_OPERATION, "invalid_request");
    assert!(rig.journaled("job.created").is_empty(), "no job");
}

// ---- Repair candidates and repair ----

/// A rig on `mount` with a library holding a complete object (`urn:uuid:a`) and a W010
/// object (`urn:uuid:b`); answers the rig, library ID and location ID.
async fn library_with_w010(mount: Mount) -> (LibraryRig, String, String) {
    let rig = LibraryRig::new(mount);
    let (library, location) = rig.create("lib").await;
    rig.object(&location, "obj-a", "urn:uuid:a").await;
    rig.w010_object(&location, "obj-b", "urn:uuid:b").await;
    (rig, library, location)
}

#[tokio::test]
async fn the_candidate_query_lists_only_the_object_without_a_version_inventory() {
    for mount in MOUNTS {
        let (rig, library, _) = library_with_w010(mount).await;
        let uri = format!("/api/v1/libraries/repair-candidates?library_id={library}");
        let answer = rig.get(&uri).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        let body = answer.json();
        assert_eq!(body["truncated"], false);
        let candidates = body["candidates"].as_array().expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0]["object_id"], "urn:uuid:b");
        assert_eq!(candidates[0]["head"], "v2");
        assert_eq!(candidates[0]["committable_after_repair"], true);
        assert!(rig.journaled(CANDIDATES_OPERATION).is_empty());
        assert_redacted(&rig, &[&answer]);
    }
}

#[tokio::test]
async fn the_candidate_query_for_a_library_that_is_not_open_is_not_found() {
    let rig = LibraryRig::new(Mount::Desktop);
    let before = failures(&rig).len();
    let uri = format!("/api/v1/libraries/repair-candidates?library_id={LIBRARY}");
    let answer = rig.get(&uri).await;
    assert_error(&answer, 404, "not_found");
    assert_one_refusal(&rig, before, CANDIDATES_OPERATION, "library_not_open");
}

#[tokio::test]
async fn a_consented_repair_by_the_desktop_owner_makes_a_sha256_object_committable() {
    let (rig, library, location) = library_with_w010(Mount::Desktop).await;
    let refused = rig.commit_v3(&location, "obj-b", "urn:uuid:b").await;
    let refused = refused.expect_err("a W010 object stays closed to commits");
    assert!(
        refused.contains("version inventory is incomplete"),
        "{refused}"
    );
    let body = LibraryRig::consented(&library, &["urn:uuid:b"]);
    let answer = rig.post("/api/v1/libraries/repairs", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let body = answer.json();
    assert_eq!(body["library_id"], library.as_str());
    let one = &body["objects"][0];
    assert_eq!(one["object_id"], "urn:uuid:b");
    assert_eq!(one["result"], "repaired");
    assert_eq!(one["version"], "v2");
    assert_eq!(one["error_code"], Value::Null);
    let files = rig.files(&location);
    assert!(files.contains(&"obj-b/v2/inventory.json".to_string()));
    assert!(files.contains(&"obj-b/v2/inventory.json.sha256".to_string()));
    rig.clock.advance(120_000);
    rig.commit_v3(&location, "obj-b", "urn:uuid:b")
        .await
        .expect("the repaired object is committable");
    assert_redacted(&rig, &[&answer]);
}

#[tokio::test]
async fn consent_and_one_outcome_row_per_object_are_recorded_without_paths() {
    let (rig, library, _) = library_with_w010(Mount::Desktop).await;
    let body = LibraryRig::consented(&library, &["urn:uuid:b", "urn:uuid:a"]);
    let answer = rig.post("/api/v1/libraries/repairs", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let consents = rig.ledger.consents();
    assert_eq!(consents.len(), 1);
    assert_eq!(consents[0].operation, REPAIR_OPERATION);
    assert_eq!(
        consents[0].target_ids,
        [library.as_str(), "urn:uuid:b", "urn:uuid:a"]
    );
    let outcomes = rig.ledger.outcomes();
    assert_eq!(outcomes.len(), 2, "one row per object");
    assert!(outcomes
        .iter()
        .all(|o| o.journal.operation == REPAIR_OPERATION));
    assert_eq!(outcomes[0].journal.outcome, Outcome::Succeeded);
    assert_eq!(outcomes[1].journal.outcome, Outcome::Failed);
    let rows: Vec<&JournalEntry> = consents
        .iter()
        .chain(outcomes.iter().map(|o| &o.journal))
        .collect();
    let text = serde_json::to_string(&rows).expect("json");
    assert!(
        !text.contains(&rig.dir_text()) && !text.contains("obj-"),
        "{text}"
    );
    assert!(!text.contains("inventory.json"), "{text}");
    assert_redacted(&rig, &[&answer]);
}

#[tokio::test]
async fn a_head_with_both_files_is_refused_with_nothing_to_repair_and_nothing_written() {
    let (rig, library, location) = library_with_w010(Mount::Desktop).await;
    let files = rig.files(&location);
    let body = LibraryRig::consented(&library, &["urn:uuid:a"]);
    let answer = rig.post("/api/v1/libraries/repairs", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let one = &answer.json()["objects"][0];
    assert_eq!(one["result"], "refused");
    assert_eq!(one["error_code"], "nothing_to_repair");
    assert_eq!(rig.files(&location), files, "nothing written");
}

#[tokio::test]
async fn a_repair_without_consent_is_refused_once_and_writes_nothing() {
    let (rig, library, location) = library_with_w010(Mount::Desktop).await;
    let files = rig.files(&location);
    let mut unconsented = LibraryRig::consented(&library, &["urn:uuid:b"]);
    unconsented["consent"] = Value::Null;
    let mut mismatched = LibraryRig::consented(&library, &["urn:uuid:b"]);
    mismatched["consent"]["object_ids"] = json!(["urn:uuid:a"]);
    let cases = [
        (unconsented, "consent_required"),
        (mismatched, "consent_mismatch"),
    ];
    for (body, code) in cases {
        let before = failures(&rig).len();
        let answer = rig.post("/api/v1/libraries/repairs", body).await;
        assert_error(&answer, 409, "conflict");
        assert_one_refusal(&rig, before, REPAIR_OPERATION, code);
    }
    assert_eq!(rig.files(&location), files, "nothing written");
    assert!(rig.ledger.consents().is_empty() && rig.ledger.outcomes().is_empty());
}

#[tokio::test]
async fn a_repair_with_a_missing_or_wrong_credential_is_denied_once() {
    let (rig, library, location) = library_with_w010(Mount::Desktop).await;
    let files = rig.files(&location);
    let body = LibraryRig::consented(&library, &["urn:uuid:b"]);
    let cases = [
        (None, 401, "unauthorized", "missing_credentials"),
        (
            Some("Bearer wrong"),
            403,
            "forbidden",
            "invalid_credentials",
        ),
    ];
    for (authorization, status, code, reason) in cases {
        let before = failures(&rig).len();
        let answer = rig
            .send(
                Method::POST,
                "/api/v1/libraries/repairs",
                authorization,
                Some(body.clone()),
            )
            .await;
        assert_error(&answer, status, code);
        assert_eq!(answer.json()["reason"], reason);
        let entry = assert_one_refusal(&rig, before, DENIED_OPERATION, reason);
        assert_eq!(entry.target_ids, ["library.repair"]);
    }
    assert_eq!(rig.files(&location), files, "nothing written");
    assert!(rig.journaled(REPAIR_OPERATION).is_empty());
    assert!(rig.ledger.consents().is_empty());
}

#[tokio::test]
async fn every_server_caller_including_the_administrator_is_refused_the_repair() {
    let (rig, library, location) = library_with_w010(Mount::Server).await;
    let files = rig.files(&location);
    let before = failures(&rig).len();
    let body = LibraryRig::consented(&library, &["urn:uuid:b"]);
    let answer = rig.post("/api/v1/libraries/repairs", body).await;
    assert_error(&answer, 403, "forbidden");
    assert_eq!(answer.json()["reason"], "not_permitted");
    let entry = assert_one_refusal(&rig, before, DENIED_OPERATION, "not_permitted");
    assert_eq!(entry.actor, "administrator");
    assert_eq!(rig.files(&location), files, "nothing written");
    assert!(rig.journaled(REPAIR_OPERATION).is_empty());
    assert!(rig.ledger.consents().is_empty());
}

#[tokio::test]
async fn a_repair_of_a_library_that_is_not_open_is_not_found_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let before = failures(&rig).len();
    let body = LibraryRig::consented(LIBRARY, &["urn:uuid:b"]);
    let answer = rig.post("/api/v1/libraries/repairs", body).await;
    assert_error(&answer, 404, "not_found");
    assert_one_refusal(&rig, before, REPAIR_OPERATION, "library_not_open");
}

#[tokio::test]
async fn the_routes_answer_unavailable_once_until_the_host_installs_its_commands() {
    let rig = LibraryRig::new(Mount::Desktop);
    let state = filesquid_api::ApiState::new();
    let token = filesquid_api::auth::LaunchToken::generate().expect("token");
    state.install_authoriser(filesquid_api::auth::Authoriser::desktop_owner(&token));
    state.install_monitor(rig.monitor.clone());
    let before = failures(&rig).len();
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/v1/libraries/rebuilds")
        .header("authorization", format!("Bearer {}", token.secret()))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(rebuild_body(&LIBRARY).to_string()))
        .expect("request");
    let response = tower::ServiceExt::oneshot(filesquid_api::router(state), request)
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_one_refusal(&rig, before, REBUILD_OPERATION, "unavailable");
}

#[tokio::test]
async fn a_rebuild_on_a_host_without_a_job_service_is_unavailable_and_journaled_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, _) = rig.create("lib").await;
    let state = filesquid_api::ApiState::new();
    let token = filesquid_api::auth::LaunchToken::generate().expect("token");
    state.install_authoriser(filesquid_api::auth::Authoriser::desktop_owner(&token));
    state.install_monitor(rig.monitor.clone());
    assert!(state.install_libraries(rig.commands.clone()));
    let before = failures(&rig).len();
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri("/api/v1/libraries/rebuilds")
        .header("authorization", format!("Bearer {}", token.secret()))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(rebuild_body(&library).to_string()))
        .expect("request");
    let response = tower::ServiceExt::oneshot(filesquid_api::router(state), request)
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_one_refusal(&rig, before, REBUILD_OPERATION, "unavailable");
}

/// Copy the directory tree `from` into `to`.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("destination");
    for file in fixture::files_under(from) {
        let target = to.join(&file);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("parent dir");
        std::fs::copy(from.join(&file), target).expect("copy file");
    }
}
