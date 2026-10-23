//! Step definitions for `file_command_envelopes_API-S-002.feature` (Task API-F2.13; API-S-002,
//! API-S-019, UI-S-012): the file write routes as commands, through the router, against a real
//! OCFL root on the local filesystem.

use crate::files_fixture::{command_body, upload_input, Answer, FilesRig, LIBRARY, OBJECT};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::journal::Outcome;
use serde_json::{json, Value};

/// What the command scenarios keep between steps.
#[derive(Default)]
pub struct CommandWorld {
    rig: Option<FilesRig>,
    answers: Vec<Answer>,
}

impl CommandWorld {
    /// The scenario's rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    /// The last answer.
    fn last(&self) -> &Answer {
        self.answers.last().expect("an answer")
    }
}

/// The rig's object input plus `fields`.
fn object_input(fields: Value) -> Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        all[key] = value.clone();
    }
    all
}

/// Send the command `input` to `route` as `operation` expecting `revision`.
async fn send(
    w: &mut MonitorWorld,
    route: &str,
    input: Value,
    (operation, revision): (&str, &str),
) {
    let body = command_body(&input, operation, revision);
    let uri = format!("/api/v1/files/{route}");
    let answer = w
        .commands
        .rig()
        .send_command(Method::POST, &uri, &body)
        .await;
    w.commands.answers.push(answer);
}

#[given("the owner's library holds an object at its first version")]
async fn library(w: &mut MonitorWorld) {
    w.commands.rig = Some(FilesRig::with_files(&[("a.txt", b"a"), ("b.txt", b"b")]).await);
}

#[given(expr = "the owner moved {string} to {string} as operation {string} expecting {string}")]
#[when(expr = "the owner moves {string} to {string} as operation {string} expecting {string}")]
async fn move_path(w: &mut MonitorWorld, from: String, to: String, op: String, rev: String) {
    let input = object_input(json!({ "from": from, "to": to }));
    send(w, "moves", input, (&op, &rev)).await;
}

#[when(expr = "the owner removes {string} as operation {string} expecting {string}")]
async fn remove(w: &mut MonitorWorld, path: String, op: String, rev: String) {
    let input = object_input(json!({ "path": path }));
    send(w, "removals", input, (&op, &rev)).await;
}

#[given(expr = "the owner created the folder {string} as operation {string} expecting {string}")]
#[when(expr = "the owner creates the folder {string} as operation {string} expecting {string}")]
async fn folder(w: &mut MonitorWorld, path: String, op: String, rev: String) {
    let input = object_input(json!({ "path": path }));
    send(w, "folders", input, (&op, &rev)).await;
}

#[given(expr = "a file {string} is then committed beneath that folder")]
async fn beneath(w: &mut MonitorWorld, path: String) {
    w.commands.rig().commit(&[(path.as_str(), b"x")]).await;
}

#[when(expr = "the owner uploads {string} as operation {string} expecting {string}")]
async fn upload(w: &mut MonitorWorld, name: String, op: String, rev: String) {
    let input = upload_input(&name, 5, json!({}));
    let envelope = command_body(&input, &op, &rev)["envelope"].clone();
    let answer = w
        .commands
        .rig()
        .send_upload(&envelope, &input, b"notes")
        .await;
    w.commands.answers.push(answer);
}

#[when(expr = "the owner sends a removal of {string} without an envelope")]
async fn without_envelope(w: &mut MonitorWorld, path: String) {
    // The input as a command carries it, with the envelope left out and nothing else wrong.
    let body = json!({ "input": object_input(json!({ "path": path })) });
    let uri = "/api/v1/files/removals";
    let answer = w
        .commands
        .rig()
        .send_command(Method::POST, uri, &body)
        .await;
    w.commands.answers.push(answer);
}

#[then(expr = "the answer should be a committed outcome of operation {string}")]
fn committed(w: &mut MonitorWorld, op: String) {
    let answer = w.commands.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let json = answer.json();
    assert_eq!(json["operation_id"], op.as_str());
    assert_eq!(json["kind"], "committed");
    assert_eq!(json["error"], Value::Null);
}

#[then(expr = "its result should name version {string}")]
fn result_version(w: &mut MonitorWorld, version: String) {
    assert_eq!(w.commands.last().result()["version"], version.as_str());
}

#[then(expr = "its result should name a new file at version {string}")]
fn new_file(w: &mut MonitorWorld, version: String) {
    let result = w.commands.last().result();
    let file_id = result["file_id"].as_str().expect("file id");
    assert_eq!(result["object_id"], format!("urn:uuid:{file_id}").as_str());
    assert_eq!(result["version"], version.as_str());
}

#[then("its result should say the folder is no longer pending")]
fn not_pending(w: &mut MonitorWorld) {
    assert_eq!(w.commands.last().result()["pending"], false);
}

#[then(expr = "the answer should be the first answer of operation {string}")]
fn first_answer(w: &mut MonitorWorld, op: String) {
    let answers = &w.commands.answers;
    let [first, again] = &answers[answers.len() - 2..] else {
        panic!("two answers");
    };
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json(), first.json());
    assert_eq!(again.json()["operation_id"], op.as_str());
}

#[then(expr = "the answer should be HTTP 409 with a conflict outcome of operation {string}")]
fn conflict(w: &mut MonitorWorld, op: String) {
    let answer = w.commands.last();
    assert_eq!(answer.status, StatusCode::CONFLICT, "{:?}", answer.json());
    let json = answer.json();
    assert_eq!(json["operation_id"], op.as_str());
    assert_eq!(json["kind"], "conflict");
    assert_eq!(json["code"], "conflict");
}

#[then(expr = "the command should be refused with {string}")]
fn error(w: &mut MonitorWorld, code: String) {
    assert_eq!(w.commands.last().json()["code"], code.as_str());
}

#[then(expr = "the object's head should be {string}")]
async fn head(w: &mut MonitorWorld, version: String) {
    assert_eq!(w.commands.rig().head().await, version);
}

#[then(expr = "the journal should record one {string} success under operation {string}")]
fn journaled(w: &mut MonitorWorld, operation: String, op: String) {
    let entries: Vec<_> = w
        .commands
        .rig()
        .journaled(&operation)
        .into_iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].operation_id.as_deref(), Some(op.as_str()));
}

#[then(expr = "the journal should record one {string} refusal")]
fn refusal(w: &mut MonitorWorld, operation: String) {
    let entries = w.commands.rig().journaled(&operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].outcome, Outcome::Failed);
    assert_eq!(entries[0].error_code.as_deref(), Some("invalid_request"));
}
