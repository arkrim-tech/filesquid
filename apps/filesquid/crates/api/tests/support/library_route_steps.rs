//! Step definitions for `library_and_repair_routes_API-S-025.feature` (Task API-F1.12):
//! the library and repair routes through the router, on the desktop and the server mount,
//! over real OCFL storage roots on the local filesystem.

use crate::library_routes_fixture::{Answer, LibraryRig, Mount};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::journal::Outcome;
use serde_json::{json, Value};

/// The complete object's root and identifier.
const COMPLETE: (&str, &str) = ("obj-a", "urn:uuid:a");

/// The W010 object's root and identifier.
const W010: (&str, &str) = ("obj-b", "urn:uuid:b");

/// Per-scenario state of the library route steps.
#[derive(Default)]
pub struct LibraryRouteWorld {
    rig: Option<LibraryRig>,
    library: String,
    location: String,
    files: Vec<String>,
    failures: usize,
    answer: Option<Answer>,
    job: String,
}

impl std::fmt::Debug for LibraryRouteWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryRouteWorld")
            .field("library", &self.library)
            .finish_non_exhaustive()
    }
}

impl LibraryRouteWorld {
    fn rig(&self) -> &LibraryRig {
        self.rig.as_ref().expect("rig")
    }

    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("answer")
    }

    /// Remember the failed journal entries so far.
    fn mark(&mut self) {
        self.failures = failed(self.rig()).len();
    }

    /// Install a rig with the library holding the two objects.
    async fn with_library(&mut self, mount: Mount) {
        self.with_rig(LibraryRig::new(mount)).await;
    }

    /// Install `rig` with the library holding the two objects.
    async fn with_rig(&mut self, rig: LibraryRig) {
        let (library, location) = rig.create("lib").await;
        rig.object(&location, COMPLETE.0, COMPLETE.1).await;
        rig.w010_object(&location, W010.0, W010.1).await;
        self.files = rig.files(&location);
        self.library = library;
        self.location = location;
        self.rig = Some(rig);
        self.mark();
    }
}

/// The failed journal entries of `rig`.
fn failed(rig: &LibraryRig) -> Vec<filesquid_api::journal::JournalEntry> {
    rig.journal()
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect()
}

#[given(expr = "the desktop owner's host with an empty folder registered")]
fn empty_folder(w: &mut MonitorWorld) {
    let rig = LibraryRig::new(Mount::Desktop);
    w.routes.location = rig.locations.register("new");
    w.routes.rig = Some(rig);
    w.routes.mark();
}

#[given(expr = "the desktop owner's host with a registered folder that holds a file")]
fn full_folder(w: &mut MonitorWorld) {
    empty_folder(w);
    let folder = w.routes.rig().locations.folder(&w.routes.location);
    std::fs::write(folder.join("notes.txt"), b"mine").expect("file");
}

#[given(
    expr = "the desktop owner's library with a complete object and an object without a version inventory"
)]
async fn desktop_library(w: &mut MonitorWorld) {
    w.routes.with_library(Mount::Desktop).await;
}

#[given(
    expr = "the server administrator's library with a complete object and an object without a version inventory"
)]
async fn server_library(w: &mut MonitorWorld) {
    w.routes.with_library(Mount::Server).await;
}

#[when(expr = "the owner creates a library in that folder through the API")]
async fn create(w: &mut MonitorWorld) {
    let body = json!({ "location_id": w.routes.location });
    w.routes.answer = Some(w.routes.rig().post("/api/v1/libraries", body).await);
}

#[when(expr = "the owner opens the library's folder through the API")]
async fn open(w: &mut MonitorWorld) {
    let body = json!({ "location_id": w.routes.location });
    let rig = w.routes.rig();
    w.routes.answer = Some(rig.post("/api/v1/libraries/openings", body).await);
}

#[then(expr = "the answer should name the same library with {int} objects")]
fn same_library(w: &mut MonitorWorld, objects: u64) {
    let body = w.routes.answer().json();
    assert_eq!(body["library_id"], w.routes.library.as_str(), "{body}");
    assert_eq!(body["objects"], objects, "{body}");
}

#[when(
    expr = "the owner copies a completed snapshot of the library to an empty folder through the API"
)]
async fn copy(w: &mut MonitorWorld) {
    let dest = w.routes.rig().locations.register("copy");
    let body = json!({ "library_id": w.routes.library, "destination_location_id": dest });
    let rig = w.routes.rig();
    w.routes.answer = Some(rig.post("/api/v1/libraries/snapshot-copies", body).await);
}

#[given(
    expr = "the desktop owner's library with a complete object and an object without a version inventory, on a host that runs no rebuild yet"
)]
async fn library_without_runner(w: &mut MonitorWorld) {
    let rig = LibraryRig::without_rebuild_runner(Mount::Desktop);
    w.routes.with_rig(rig).await;
}

#[when(expr = "the owner starts a rebuild of the library from its files through the API")]
async fn start_rebuild(w: &mut MonitorWorld) {
    let body = json!({ "library_id": w.routes.library });
    let rig = w.routes.rig();
    let answer = rig.post("/api/v1/libraries/rebuilds", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    w.routes.job = answer.json()["job_id"].as_str().expect("job").to_string();
    w.routes.answer = Some(answer);
}

#[when(expr = "the owner rebuilds the library from its files through the API")]
async fn rebuild(w: &mut MonitorWorld) {
    start_rebuild(w).await;
    rebuild_finishes(w).await;
}

#[when(expr = "the rebuild job finishes")]
async fn rebuild_finishes(w: &mut MonitorWorld) {
    let rig = w.routes.rig();
    rig.finished_job(&w.routes.job).await;
    w.routes.answer = Some(rig.get(&format!("/api/v1/jobs/{}", w.routes.job)).await);
}

#[when(expr = "the owner {word} the rebuild job through the API")]
async fn control_rebuild(w: &mut MonitorWorld, control: String) {
    let control = match control.as_str() {
        "pauses" => "pause",
        "resumes" => "resume",
        "cancels" => "cancel",
        other => panic!("unknown rebuild job control: {other}"),
    };
    let rig = w.routes.rig();
    let uri = format!("/api/v1/jobs/{}/{control}", w.routes.job);
    let bearer = format!("Bearer {}", rig.token);
    let answer = rig
        .send(axum::http::Method::POST, &uri, Some(&bearer), None)
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    w.routes.answer = Some(answer);
}

#[then(expr = "the answer should be a pending library.rebuild job without a result")]
fn pending_rebuild(w: &mut MonitorWorld) {
    let body = w.routes.answer().json();
    assert_eq!(body["outcome"], "pending", "{body}");
    assert_eq!(body["job"]["kind"], "library.rebuild", "{body}");
    assert_eq!(body["job"]["state"], "pending", "{body}");
    assert_eq!(body["job"]["result"], Value::Null, "{body}");
}

#[then(expr = "the rebuild job should report {int} of {int} objects done")]
fn rebuild_progress(w: &mut MonitorWorld, done: u64, total: u64) {
    let job = w.routes.answer().json();
    assert_eq!(job["done"].as_u64(), Some(done), "{job}");
    assert_eq!(job["total"].as_u64(), Some(total), "{job}");
}

#[then(expr = "the rebuild job should be {word}")]
fn rebuild_state(w: &mut MonitorWorld, state: String) {
    let job = w.routes.answer().json()["job"].clone();
    match state.as_str() {
        "paused" => {
            assert_eq!(job["state"], "pending", "{job}");
            assert_eq!(job["paused"], true, "{job}");
        }
        _ => assert_eq!(job["state"], state.as_str(), "{job}"),
    }
    assert_eq!(job["result"], Value::Null, "no result: {job}");
}

#[then(expr = "{string} should be journaled once for the rebuild job")]
fn job_journaled_once(w: &mut MonitorWorld, operation: String) {
    let entries = w.routes.rig().journaled(&operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].target_ids, std::slice::from_ref(&w.routes.job));
}

#[when(expr = "the server administrator rebuilds the library from its files through the API")]
async fn admin_rebuild(w: &mut MonitorWorld) {
    rebuild(w).await;
}

#[when(expr = "the owner asks the API which objects need a version-inventory repair")]
async fn candidates(w: &mut MonitorWorld) {
    let uri = format!(
        "/api/v1/libraries/repair-candidates?library_id={}",
        w.routes.library
    );
    w.routes.answer = Some(w.routes.rig().get(&uri).await);
}

/// Post the consented repair of `object` with the rig's credential.
async fn repair(w: &mut MonitorWorld, object: &str) {
    let body = LibraryRig::consented(&w.routes.library, &[object]);
    let rig = w.routes.rig();
    w.routes.answer = Some(rig.post("/api/v1/libraries/repairs", body).await);
}

#[when(
    expr = "the owner repairs the object without a version inventory with consent through the API"
)]
async fn repair_w010(w: &mut MonitorWorld) {
    repair(w, W010.1).await;
}

#[when(
    expr = "the server administrator repairs the object without a version inventory with consent through the API"
)]
async fn admin_repair_w010(w: &mut MonitorWorld) {
    repair(w, W010.1).await;
}

#[when(expr = "the owner repairs the complete object with consent through the API")]
async fn repair_complete(w: &mut MonitorWorld) {
    repair(w, COMPLETE.1).await;
}

#[when(
    expr = "the owner asks the API to repair the object without a version inventory without consent"
)]
async fn repair_unconsented(w: &mut MonitorWorld) {
    let mut body = LibraryRig::consented(&w.routes.library, &[W010.1]);
    body["consent"] = Value::Null;
    let rig = w.routes.rig();
    w.routes.answer = Some(rig.post("/api/v1/libraries/repairs", body).await);
}

#[when(
    expr = "a caller without a credential asks the API to repair the object without a version inventory"
)]
async fn repair_anonymous(w: &mut MonitorWorld) {
    let body = LibraryRig::consented(&w.routes.library, &[W010.1]);
    let rig = w.routes.rig();
    let answer = rig
        .send(
            axum::http::Method::POST,
            "/api/v1/libraries/repairs",
            None,
            Some(body),
        )
        .await;
    w.routes.answer = Some(answer);
}

#[then(expr = "the answer should name an opaque library ID and no filesystem path")]
fn opaque_answer(w: &mut MonitorWorld) {
    let answer = w.routes.answer();
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let library = answer.json()["library_id"].as_str().map(str::len);
    assert_eq!(library, Some(36));
    assert!(!answer.text().contains(&w.routes.rig().dir_text()));
}

#[then(expr = "the registered folder should hold only the OCFL 1.1 storage-root declaration")]
fn declaration_only(w: &mut MonitorWorld) {
    assert_eq!(w.routes.rig().files(&w.routes.location), ["0=ocfl_1.1"]);
}

#[then(expr = "the answer should be the error {string}")]
fn error(w: &mut MonitorWorld, code: String) {
    let answer = w.routes.answer();
    assert_eq!(answer.json()["code"], code.as_str(), "{}", answer.text());
}

#[then(expr = "exactly one refusal {string} should be journaled for {string}")]
fn one_refusal(w: &mut MonitorWorld, code: String, operation: String) {
    let failures = failed(w.routes.rig());
    assert_eq!(failures.len(), w.routes.failures + 1, "{failures:?}");
    let last = failures.last().expect("refusal");
    assert_eq!(last.operation, operation);
    assert_eq!(last.error_code.as_deref(), Some(code.as_str()));
    let text = serde_json::to_string(last).expect("json");
    assert!(!text.contains(&w.routes.rig().dir_text()), "{text}");
}

#[then(expr = "the registered folder should still hold only that file")]
fn only_that_file(w: &mut MonitorWorld) {
    assert_eq!(w.routes.rig().files(&w.routes.location), ["notes.txt"]);
}

#[then(expr = "the copy should report {int} objects and no invalid object")]
fn copy_report(w: &mut MonitorWorld, objects: u64) {
    let report = w.routes.answer().json();
    assert_eq!(report["objects"], objects, "{report}");
    assert_eq!(report["invalid_objects"], json!([]));
}

#[then(expr = "the library folder should be unchanged")]
fn unchanged(w: &mut MonitorWorld) {
    assert_eq!(w.routes.rig().files(&w.routes.location), w.routes.files);
}

#[then(expr = "the rebuild should flag only the object without a version inventory")]
fn rebuild_flags(w: &mut MonitorWorld) {
    let job = w.routes.answer().json();
    assert_eq!(job["state"], "completed", "{job}");
    let report = &job["result"];
    let flagged: Vec<&str> = report["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .filter(|o| o["missing_version_inventory"] == true)
        .filter_map(|o| o["object_id"].as_str())
        .collect();
    assert_eq!(flagged, [W010.1], "{report}");
}

#[then(expr = "only the object without a version inventory should be offered")]
fn offered(w: &mut MonitorWorld) {
    let body = w.routes.answer().json();
    let offered: Vec<&str> = body["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .filter_map(|c| c["object_id"].as_str())
        .collect();
    assert_eq!(offered, [W010.1], "{body}");
}

#[then(expr = "the object should be answered as repaired")]
fn repaired(w: &mut MonitorWorld) {
    let body = w.routes.answer().json();
    assert_eq!(body["objects"][0]["result"], "repaired", "{body}");
}

#[then(expr = "the object should be answered as refused with {string}")]
fn refused(w: &mut MonitorWorld, code: String) {
    let body = w.routes.answer().json();
    assert_eq!(body["objects"][0]["result"], "refused", "{body}");
    assert_eq!(body["objects"][0]["error_code"], code.as_str(), "{body}");
}

#[then(expr = "the consent and one outcome row per object should be recorded as {string}")]
fn recorded(w: &mut MonitorWorld, operation: String) {
    let rig = w.routes.rig();
    let consents = rig.ledger.consents();
    let outcomes = rig.ledger.outcomes();
    assert_eq!(consents.len(), 1);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(consents[0].operation, operation);
    assert_eq!(outcomes[0].journal.operation, operation);
}

#[then(expr = "the object's outcome row should be failed with {string}")]
fn outcome_row_failed(w: &mut MonitorWorld, code: String) {
    let outcomes = w.routes.rig().ledger.outcomes();
    let row = &outcomes.last().expect("outcome row").journal;
    assert_eq!(row.outcome, Outcome::Failed);
    assert_eq!(row.error_code.as_deref(), Some(code.as_str()));
    assert!(outcomes.last().expect("outcome row").receipt.is_none());
}

#[then(expr = "the repaired object should accept a new version")]
async fn committable(w: &mut MonitorWorld) {
    let rig = w.routes.rig();
    rig.clock.advance(120_000);
    let committed = rig.commit_v3(&w.routes.location, W010.0, W010.1).await;
    assert!(committed.is_ok(), "{committed:?}");
}

#[when(expr = "the owner rebuilds a library that is not open through the API")]
async fn rebuild_not_open(w: &mut MonitorWorld) {
    let body = json!({ "library_id": crate::library_routes_fixture::LIBRARY });
    let rig = w.routes.rig();
    let answer = rig.post("/api/v1/libraries/rebuilds", body).await;
    assert!(
        rig.journaled("job.created").is_empty(),
        "no job is submitted"
    );
    w.routes.answer = Some(answer);
}
