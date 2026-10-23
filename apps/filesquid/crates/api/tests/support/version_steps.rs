//! Step definitions for `version_history_labels_restore_API-S-014.feature` (Task API-F2.5):
//! version listing, ranged reads of an earlier version, restore, labels and history size,
//! through the router over a real OCFL storage root on the local filesystem.

use crate::files_fixture::{query, Answer, FilesRig, LIBRARY, OBJECT};
use crate::retention_contract::version_removals;
use crate::MonitorWorld;
use axum::http::Method;
use cucumber::{given, then, when};
use filesquid_api::files::LABELS_PATH;
use filesquid_api::history::ProvenanceContext;
use filesquid_core::InstallationId;
use filesquid_ocfl::read::Object;
use filesquid_prov::HostFacts;
use serde_json::{json, Value};

/// Where the rig's object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// Per-scenario state of the version steps.
#[derive(Default)]
pub struct VersionWorld {
    rig: Option<FilesRig>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for VersionWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VersionWorld").finish_non_exhaustive()
    }
}

impl VersionWorld {
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("rig")
    }

    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("answer")
    }
}

/// A JSON body naming the rig's library and object plus `fields`.
fn body(fields: Value) -> Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        all[key] = value.clone();
    }
    all
}

/// `PUT /api/v1/files/labels`.
async fn put_label(rig: &FilesRig, version: &str, label: Value) -> Answer {
    let body = body(json!({ "version": version, "label": label }));
    rig.command_as(Method::PUT, "/api/v1/files/labels", body)
        .await
}

/// The history entry of `version` of `a.txt`, or of the object's versions when the file
/// did not change in it.
async fn version_entry(rig: &FilesRig, version: &str) -> Value {
    let versions = rig
        .get(&format!("/api/v1/files/versions?{}", query("")))
        .await
        .json();
    versions["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .find(|v| v["version"] == version)
        .cloned()
        .unwrap_or(Value::Null)
}

/// The logical paths of the object's head.
async fn head_paths(rig: &FilesRig) -> Vec<String> {
    let object = Object::open(rig.root.as_ref(), OBJECT_ROOT)
        .await
        .expect("object");
    let state = object.state(object.head()).expect("state");
    state.paths().map(String::from).collect()
}

/// The text of `path` at `version`.
async fn text_at(rig: &FilesRig, path: &str, version: &str) -> String {
    let uri = format!(
        "/api/v1/files/content?{}",
        query(&format!("path={path}&version={version}"))
    );
    String::from_utf8_lossy(&rig.get(&uri).await.body).into_owned()
}

#[given(expr = "the owner's file {string} was saved as {string} and then as {string}")]
async fn saved_twice(w: &mut MonitorWorld, path: String, first: String, second: String) {
    let rig = FilesRig::with_files(&[(path.as_str(), first.as_bytes())]).await;
    rig.commit(&[(path.as_str(), second.as_bytes())]).await;
    w.versions.rig = Some(rig);
}

#[given(expr = "the host records provenance")]
fn records_provenance(w: &mut MonitorWorld) {
    let rig = w.versions.rig();
    let context = ProvenanceContext {
        installation: InstallationId::parse("0f3c2b1a-9d8e-4f7a-b6c5-d4e3f2a1b0c9")
            .expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    };
    assert!(rig.service.install_provenance(context, rig.journal.clone()));
}

#[given(expr = "the owner labelled version {string} as {string}")]
async fn labelled(w: &mut MonitorWorld, version: String, text: String) {
    let answer = put_label(w.versions.rig(), &version, json!(text)).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
}

#[when(expr = "the owner lists the versions of {string}")]
async fn list_versions(w: &mut MonitorWorld, path: String) {
    let uri = format!("/api/v1/files/history?{}", query(&format!("path={path}")));
    w.versions.answer = Some(w.versions.rig().get(&uri).await);
}

#[when(expr = "the owner reads bytes {int} to {int} of {string} as it was in {string}")]
async fn read_range(w: &mut MonitorWorld, start: u64, end: u64, path: String, version: String) {
    let uri = format!(
        "/api/v1/files/content?{}",
        query(&format!("path={path}&version={version}"))
    );
    let range = format!("bytes={start}-{end}");
    let rig = w.versions.rig();
    let answer = rig
        .call(Method::GET, &uri, &[("range", range.as_str())], Vec::new())
        .await;
    w.versions.answer = Some(answer);
}

#[when(expr = "the owner restores {string} from {string} with the label {string}")]
async fn restore(w: &mut MonitorWorld, path: String, version: String, text: String) {
    let request = body(json!({ "version": version, "paths": [path], "label": text }));
    let rig = w.versions.rig();
    w.versions.answer = Some(rig.command("/api/v1/files/restorations", request).await);
}

#[when(expr = "the owner labels version {string} as {string}")]
async fn set_label(w: &mut MonitorWorld, version: String, text: String) {
    w.versions.answer = Some(put_label(w.versions.rig(), &version, json!(text)).await);
}

#[when(expr = "the owner clears the label of version {string}")]
async fn clear(w: &mut MonitorWorld, version: String) {
    w.versions.answer = Some(put_label(w.versions.rig(), &version, Value::Null).await);
}

#[when(expr = "the owner asks how much history {string} keeps")]
async fn ask_size(w: &mut MonitorWorld, path: String) {
    let uri = format!(
        "/api/v1/files/history-size?{}",
        query(&format!("path={path}"))
    );
    w.versions.answer = Some(w.versions.rig().get(&uri).await);
}

#[then(expr = "the listing should show {int} versions with sizes {int} and {int}")]
fn listing_sizes(w: &mut MonitorWorld, count: usize, first: u64, second: u64) {
    let page = w.versions.answer().json();
    let entries = page["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), count, "{page}");
    assert_eq!(entries[0]["size"], first);
    assert_eq!(entries[1]["size"], second);
}

#[then(expr = "each version should show its digest, creation time and label")]
fn listing_facts(w: &mut MonitorWorld) {
    let page = w.versions.answer().json();
    for entry in page["entries"].as_array().expect("entries") {
        assert!(entry["digest"].is_string(), "{entry}");
        assert!(entry["version"]["created"].is_string(), "{entry}");
        assert!(entry["version"].get("label").is_some(), "{entry}");
    }
}

#[then(expr = "the answer should be the partial content {string}")]
fn partial(w: &mut MonitorWorld, expected: String) {
    let answer = w.versions.answer();
    assert_eq!(answer.status.as_u16(), 206);
    assert_eq!(answer.body, expected.as_bytes());
}

#[then(expr = "the object should have {int} versions")]
async fn version_count(w: &mut MonitorWorld, count: usize) {
    let versions = w
        .versions
        .rig()
        .get(&format!("/api/v1/files/versions?{}", query("")))
        .await
        .json();
    assert_eq!(versions["versions"].as_array().map(Vec::len), Some(count));
}

#[then(expr = "{string} should hold {string} again")]
async fn holds_again(w: &mut MonitorWorld, path: String, text: String) {
    assert_eq!(text_at(w.versions.rig(), &path, "head").await, text);
}

#[then(expr = "version {string} should still hold {string}")]
async fn still_holds(w: &mut MonitorWorld, version: String, text: String) {
    assert_eq!(text_at(w.versions.rig(), "a.txt", &version).await, text);
}

#[then(expr = "version {string} should be labelled {string}")]
async fn labelled_as(w: &mut MonitorWorld, version: String, text: String) {
    let entry = version_entry(w.versions.rig(), &version).await;
    assert_eq!(entry["label"], text.as_str(), "{entry}");
}

#[then(expr = "version {string} should have no label")]
async fn no_label(w: &mut MonitorWorld, version: String) {
    let entry = version_entry(w.versions.rig(), &version).await;
    assert_eq!(entry["label"], Value::Null, "{entry}");
}

#[then(expr = "the object should keep its labels in its own files")]
async fn labels_in_object(w: &mut MonitorWorld) {
    let paths = head_paths(w.versions.rig()).await;
    assert!(paths.contains(&LABELS_PATH.to_string()), "{paths:?}");
}

#[then(expr = "the answer should be the version error {string}")]
fn version_error(w: &mut MonitorWorld, code: String) {
    let answer = w.versions.answer();
    assert_eq!(answer.json()["code"], code.as_str(), "{:?}", answer.json());
}

#[then(expr = "the file's history should keep {int} contents in {int} bytes")]
fn file_size(w: &mut MonitorWorld, contents: usize, bytes: u64) {
    let size = w.versions.answer().json();
    assert_eq!(size["contents"], contents, "{size}");
    assert_eq!(size["bytes"], bytes, "{size}");
}

#[then(expr = "the library's history should keep {int} contents in {int} bytes")]
async fn library_size(w: &mut MonitorWorld, contents: usize, bytes: u64) {
    let uri = format!("/api/v1/files/library-history-size?library_id={LIBRARY}");
    let size = w.versions.rig().get(&uri).await.json();
    assert_eq!(size["contents"], contents, "{size}");
    assert_eq!(size["bytes"], bytes, "{size}");
    assert_eq!(size["truncated"], false);
}

#[then(expr = "the contract should offer no version deletion, expiry or thinning")]
fn no_deletion(_w: &mut MonitorWorld) {
    let doc: Value = serde_json::from_str(filesquid_api::CONTRACT).expect("contract");
    assert_eq!(version_removals(&doc), Vec::<String>::new());
}

#[then(expr = "version {string} should hold its PROV-O record")]
async fn prov_record(w: &mut MonitorWorld, version: String) {
    let rig = w.versions.rig();
    let path = format!("provenance%2F{version}.jsonld");
    let uri = format!(
        "/api/v1/files/content?{}",
        query(&format!("path={path}&version={version}"))
    );
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status.as_u16(), 200, "the record is in {version}");
    let record: Value = serde_json::from_slice(&answer.body).expect("a JSON-LD record");
    assert!(record.get("@context").is_some(), "{record}");
    let restores = rig.journaled("files.restore");
    let operation = restores[0].operation_id.clone().expect("operation id");
    assert!(
        record.to_string().contains(&operation),
        "the record names the restore's operation {operation}"
    );
}

#[then(expr = "the restore should be journaled once with its commit")]
fn journaled_once(w: &mut MonitorWorld) {
    let rig = w.versions.rig();
    let restores = rig.journaled("files.restore");
    let commits = rig.journaled("ocfl.commit");
    assert_eq!(restores.len(), 1, "{restores:?}");
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_eq!(restores[0].operation_id, commits[0].operation_id);
}
