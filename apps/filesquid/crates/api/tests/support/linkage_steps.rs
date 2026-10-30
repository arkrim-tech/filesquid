//! Step definitions for `journal_and_commit_linkage_API-S-028.feature` (Task API-F2.8;
//! API-S-028, API-S-017; ADR 040 Decision 6): every commit of the file and metadata routes
//! names its operation and the installation and is journaled once as `ocfl.commit`, through
//! the router against a real OCFL root on the local filesystem.

use crate::files_fixture::{
    command_body, operation_id, upload_facts, Answer, FilesRig, HOME_COLLECTION, LIBRARY, OBJECT,
};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::history::ProvenanceContext;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_core::InstallationId;
use filesquid_ocfl::inventory::{User, VersionNum};
use filesquid_ocfl::read::Object;
use filesquid_prov::HostFacts;
use serde_json::{json, Value};

/// The installation of these scenarios.
const INSTALLATION: &str = "5b0e8c1d-2f4a-4c6b-9d8e-7a1b2c3d4e5f";

/// Where the rig's object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// A statement value that must never reach the journal.
const PRIVATE_VALUE: &str = "Aunt Rosa on the pier";

/// What the linkage scenarios keep between steps.
#[derive(Default)]
pub struct LinkageWorld {
    rig: Option<FilesRig>,
    uploaded: Value,
    operation: String,
    result: Value,
    head_before: String,
}

impl LinkageWorld {
    /// The scenario's rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    /// Keep the answer of a command sent under `operation`.
    fn keep(&mut self, operation: String, answer: &Answer) {
        assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
        self.operation = operation;
        self.result = answer
            .json()
            .get("result")
            .cloned()
            .unwrap_or(answer.json());
    }
}

/// The storage-root path of `object_id`: the rig's object, the library's snapshot or a file
/// object.
fn object_root(object_id: &str) -> String {
    let id = object_id.trim_start_matches("urn:uuid:");
    match (object_id == OBJECT, id == LIBRARY) {
        (true, _) => OBJECT_ROOT.to_string(),
        (false, true) => filesquid_api::canonical::SNAPSHOT_ROOT.to_string(),
        (false, false) => format!("files/{id}"),
    }
}

/// The user, message and logical paths of the version `result` names.
async fn version_of(rig: &FilesRig, result: &Value) -> (Option<User>, String, Vec<String>) {
    let object_id = result["object_id"].as_str().expect("object id");
    let version = VersionNum::parse(result["version"].as_str().expect("version")).expect("v");
    let object = Object::open(rig.root.as_ref(), &object_root(object_id))
        .await
        .expect("object");
    let block = &object.inventory().versions[&version];
    let paths = object
        .state(&version)
        .expect("state")
        .paths()
        .map(str::to_string)
        .collect();
    (
        block.user.clone(),
        block.message.clone().unwrap_or_default(),
        paths,
    )
}

/// The object reference `<object>@<version>` of `result`.
fn reference_of(result: &Value) -> String {
    let object = result["object_id"].as_str().expect("object id");
    format!("{object}@{}", result["version"].as_str().expect("version"))
}

/// The entries of `operation` journaled for the operation ID `id`.
fn journaled_for(rig: &FilesRig, operation: &str, id: &str) -> Vec<JournalEntry> {
    rig.journaled(operation)
        .into_iter()
        .filter(|entry| entry.operation_id.as_deref() == Some(id))
        .collect()
}

/// Send the command `input` to `uri` with `method` under a new operation ID and the current
/// revision; keep its answer.
async fn command(w: &mut MonitorWorld, method: Method, uri: &str, input: Value) {
    let id = operation_id();
    let revision = w.linkage.rig().revision_of(&input).await;
    let body = command_body(&input, &id, &revision);
    let answer = w.linkage.rig().send_command(method, uri, &body).await;
    w.linkage.keep(id, &answer);
}

/// Set `dcterms:title` of the uploaded file to `value`; keep the answer and the operation ID
/// its single `ocfl.commit` entry names (the route takes no envelope).
async fn set_title(w: &mut MonitorWorld, value: &str) {
    let body = json!({
        "library_id": LIBRARY, "file_id": w.linkage.uploaded["file_id"],
        "curie": "dcterms:title", "value": value,
    });
    let headers = [("content-type", "application/json")];
    let uri = "/api/v1/files/metadata/statements";
    let rig = w.linkage.rig();
    let answer = rig
        .call(Method::PUT, uri, &headers, body.to_string().into_bytes())
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let reference = reference_of(&answer.json());
    let commits: Vec<JournalEntry> = rig
        .journaled("ocfl.commit")
        .into_iter()
        .filter(|entry| entry.object_ref.as_deref() == Some(reference.as_str()))
        .collect();
    assert_eq!(commits.len(), 1, "{commits:?}");
    let id = commits[0].operation_id.clone().expect("operation id");
    w.linkage.keep(id, &answer);
}

#[given(expr = "an owner's library with provenance configured for a known installation")]
async fn library_with_provenance(w: &mut MonitorWorld) {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    let context = ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    };
    assert!(rig.service.install_provenance(context, rig.journal.clone()));
    w.linkage.rig = Some(rig);
}

#[given(expr = "the library holds an object with an edited file and an uploaded file")]
async fn edited_and_uploaded(w: &mut MonitorWorld) {
    let rig = w.linkage.rig();
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let answer = rig
        .upload(upload_facts("harbour.jpg", 4, ""), b"jpeg")
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let head = rig.head().await;
    w.linkage.uploaded = answer.result();
    w.linkage.head_before = head;
}

#[when(regex = r"^the owner (.+) with a new operation ID$")]
async fn operation_with_id(w: &mut MonitorWorld, operation: String) {
    let file_id = w.linkage.uploaded["file_id"].clone();
    match operation.as_str() {
        "uploads a new version of the uploaded file" => {
            let input = upload_facts(
                "harbour.jpg",
                5,
                &format!("file_id={}", file_id.as_str().expect("id")),
            );
            let id = operation_id();
            let revision = w.linkage.rig().revision_of(&input).await;
            let envelope = command_body(&input, &id, &revision)["envelope"].clone();
            let answer = w
                .linkage
                .rig()
                .send_upload(&envelope, &input, b"jpeg2")
                .await;
            w.linkage.keep(id, &answer);
        }
        "copies the uploaded file" => {
            let input = json!({
                "library_id": LIBRARY, "file_id": file_id, "collection_id": HOME_COLLECTION
            });
            command(w, Method::POST, "/api/v1/files/copies", input).await;
        }
        "renames the uploaded file" => {
            let input =
                json!({ "library_id": LIBRARY, "file_id": file_id, "display_name": "Quay" });
            command(w, Method::POST, "/api/v1/files/renames", input).await;
        }
        "moves the edited file" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
            command(w, Method::POST, "/api/v1/files/moves", input).await;
        }
        "removes the edited file" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "path": "a.txt" });
            command(w, Method::POST, "/api/v1/files/removals", input).await;
        }
        "restores the edited file's first version" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "paths": ["a.txt"] });
            command(w, Method::POST, "/api/v1/files/restorations", input).await;
        }
        "sets a statement on the uploaded file" => set_title(w, PRIVATE_VALUE).await,
        other => panic!("no such operation in this feature: {other}"),
    }
}

#[when(expr = "the owner moves the edited file with the message {string}")]
async fn move_with_message(w: &mut MonitorWorld, message: String) {
    let input = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt",
        "message": message,
    });
    command(w, Method::POST, "/api/v1/files/moves", input).await;
}

#[when(expr = "the owner moves the edited file naming a stale revision")]
async fn move_stale(w: &mut MonitorWorld) {
    let input =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let id = operation_id();
    let body = command_body(&input, &id, "v9");
    let rig = w.linkage.rig();
    let answer = rig
        .send_command(Method::POST, "/api/v1/files/moves", &body)
        .await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{:?}", answer.json());
    w.linkage.operation = id;
}

#[then(
    expr = "exactly one {string} entry should name that operation ID and the new object version"
)]
fn exactly_one_entry(w: &mut MonitorWorld, operation: String) {
    one_entry(w, operation);
}

#[then(expr = "one {string} entry should name that operation ID and the new object version")]
fn one_entry(w: &mut MonitorWorld, operation: String) {
    let entries = journaled_for(w.linkage.rig(), &operation, &w.linkage.operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let reference = reference_of(&w.linkage.result);
    assert_eq!(entries[0].object_ref.as_deref(), Some(reference.as_str()));
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
}

#[then(
    expr = "the new version's user should be the installation and its message should name the operation ID and the kind {string}"
)]
async fn user_and_message(w: &mut MonitorWorld, kind: String) {
    let (user, message, _) = version_of(w.linkage.rig(), &w.linkage.result).await;
    let user = user.expect("a version user");
    assert_eq!(user.name, "FileSquid installation");
    assert_eq!(user.address, Some(format!("urn:uuid:{INSTALLATION}")));
    assert!(message.starts_with("filesquid.operation.v1 "), "{message}");
    let id = &w.linkage.operation;
    assert!(
        message.contains(&format!("\"operation_id\":\"{id}\"")),
        "{message}"
    );
    assert!(
        message.contains(&format!("\"kind\":\"{kind}\"")),
        "{message}"
    );
}

#[then(expr = "the new version should hold its PROV-O record")]
async fn holds_record(w: &mut MonitorWorld) {
    let (_, _, paths) = version_of(w.linkage.rig(), &w.linkage.result).await;
    let version = w.linkage.result["version"].as_str().expect("version");
    assert!(
        paths.contains(&format!("provenance/{version}.jsonld")),
        "{paths:?}"
    );
    assert!(
        paths.contains(&format!("provenance/{version}.journal.jsonl")),
        "{paths:?}"
    );
}

#[then(expr = "no journal entry should contain the statement's value")]
fn no_value(w: &mut MonitorWorld) {
    let text = serde_json::to_string(&w.linkage.rig().journal()).expect("journal json");
    assert!(!text.contains(PRIVATE_VALUE));
    assert!(!text.contains("dcterms:title"));
}

#[then(expr = "the new version's message should carry the label {string}")]
async fn carries_label(w: &mut MonitorWorld, label: String) {
    let (_, message, _) = version_of(w.linkage.rig(), &w.linkage.result).await;
    assert!(
        message.contains(&format!("\"label\":\"{label}\"")),
        "{message}"
    );
}

#[then(expr = "the move should be journaled once as refused with its operation ID")]
fn refused_once(w: &mut MonitorWorld) {
    let refusals = journaled_for(w.linkage.rig(), "file.move", &w.linkage.operation);
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert_eq!(refusals[0].outcome, Outcome::Failed);
}

#[then(expr = "no {string} entry and no new version should exist for that operation")]
async fn nothing_committed(w: &mut MonitorWorld, operation: String) {
    let entries = journaled_for(w.linkage.rig(), &operation, &w.linkage.operation);
    assert!(entries.is_empty(), "{entries:?}");
    assert_eq!(w.linkage.rig().head().await, w.linkage.head_before);
}
