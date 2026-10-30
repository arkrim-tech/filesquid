//! Step definitions for `prov_o_history_F-067.feature` (Task API-F2.9; API-S-028, API-S-029,
//! SRS-F-067, ADR 040): the PROV-O record each file and metadata route commit writes into its
//! own version, read back through the router and from the storage root with a JSON parser
//! only.
//!
//! The scenarios the API crate cannot drive stay unbound and are reported as skipped:
//! operations only the desktop catalog commits (import, hide, unhide, purge, restore from
//! the recently deleted, catalog organisation) and the restart, run in the desktop harness;
//! the history object of exports, scrubbed exports and publications, which have no route
//! yet; the repair follow-up, the rebase and the backfill, bound in
//! `prov_o_history_records_routes_and_backfill_API-S-029.feature` and covered by unit tests;
//! the crash reconciliation at library open (DE-S-011); and sharing with a preview.

use crate::files_fixture::{
    command_body, operation_id, send, upload_facts, Answer, FilesRig, HOME_COLLECTION, LIBRARY,
    OBJECT,
};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::history::ProvenanceContext;
use filesquid_api::journal::JournalEntry;
use filesquid_core::InstallationId;
use filesquid_ocfl::inventory::VersionNum;
use filesquid_ocfl::read::Object;
use filesquid_prov::HostFacts;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The installation of these scenarios.
const INSTALLATION: &str = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";

/// Where the rig's object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// Most directory levels the storage-root walks descend (AGENTS.md §22).
const MAX_DEPTH: usize = 12;

/// What the PROV-O scenarios keep between steps.
#[derive(Default)]
pub struct ProvWorld {
    rig: Option<FilesRig>,
    uploaded: Value,
    operation: String,
    result: Value,
    versions_before: usize,
    records_before: usize,
    previous_version: String,
    copy: Option<PathBuf>,
}

impl ProvWorld {
    /// The scenario's rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    /// Remember how many versions and records the storage root holds now.
    fn snapshot(&mut self) {
        let root = self.rig().dir.join("root");
        self.versions_before = count_versions(&root);
        self.records_before = count_records(&root);
    }

    /// Keep a command answer under `operation`.
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

/// The directories below `dir`, at most [`MAX_DEPTH`] levels down, `dir` included.
fn directories(dir: &Path) -> Vec<PathBuf> {
    let mut found = vec![dir.to_path_buf()];
    let mut level = vec![dir.to_path_buf()];
    for _ in 0..MAX_DEPTH {
        let next: Vec<PathBuf> = level
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        found.extend(next.iter().cloned());
        level = next;
    }
    found
}

/// The OCFL versions under `root`: `vN` directories of object roots.
fn count_versions(root: &Path) -> usize {
    directories(root)
        .iter()
        .filter(|d| d.join("0=ocfl_object_1.1").exists())
        .flat_map(|d| {
            std::fs::read_dir(d)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
        })
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with('v') && name[1..].chars().all(|c| c.is_ascii_digit())
        })
        .count()
}

/// The PROV-O record files under `root`.
fn record_files(root: &Path) -> Vec<PathBuf> {
    directories(root)
        .iter()
        .filter(|d| d.ends_with("provenance"))
        .flat_map(|d| {
            std::fs::read_dir(d)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
        })
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".jsonld"))
        .collect()
}

/// How many PROV-O record files `root` holds.
fn count_records(root: &Path) -> usize {
    record_files(root).len()
}

/// Copy the directory `from` to `to` with ordinary file operations.
fn copy_tree(from: &Path, to: &Path) {
    for dir in directories(from) {
        let target = to.join(dir.strip_prefix(from).expect("below"));
        std::fs::create_dir_all(&target).expect("create");
        for entry in std::fs::read_dir(&dir)
            .expect("read")
            .filter_map(Result::ok)
        {
            if entry.path().is_file() {
                std::fs::copy(entry.path(), target.join(entry.file_name())).expect("copy");
            }
        }
    }
}

/// The nodes of a JSON-LD record: its `@graph`, or the array itself.
fn nodes(record: &Value) -> Vec<Value> {
    let graph = record.get("@graph").unwrap_or(record);
    graph.as_array().cloned().unwrap_or_default()
}

/// The nodes of `record` whose `@type` is `kind`.
fn typed(record: &Value, kind: &str) -> Vec<Value> {
    nodes(record)
        .into_iter()
        .filter(|n| n["@type"] == kind)
        .collect()
}

/// The `@id` values a relation member names, one reference or a list of them.
fn references(member: &Value) -> Vec<String> {
    let list = match member {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };
    list.iter()
        .filter_map(|r| r["@id"].as_str().map(str::to_string))
        .collect()
}

/// The PROV-O record of the version `result` names, read from the storage root.
async fn record_of(rig: &FilesRig, result: &Value) -> Value {
    let object_id = result["object_id"].as_str().expect("object id");
    let version = result["version"].as_str().expect("version");
    let object = Object::open(rig.root.as_ref(), &object_root(object_id))
        .await
        .expect("object");
    let num = VersionNum::parse(version).expect("version");
    let path = format!("provenance/{version}.jsonld");
    let size = filesquid_ocfl::view::file_details(&object, &num, &path)
        .await
        .expect("the record")
        .size;
    let bytes = filesquid_ocfl::view::read_range(&object, &num, &path, 0, size)
        .await
        .expect("record bytes");
    serde_json::from_slice(&bytes).expect("JSON")
}

/// The journal entry IDs an activity names in `fsq:journalEntry`, each as text.
fn journal_ids(activity: &Value) -> Vec<String> {
    activity["fsq:journalEntry"]
        .as_array()
        .expect("journal entries")
        .iter()
        .map(|id| id.as_str().map_or_else(|| id.to_string(), str::to_string))
        .collect()
}

/// The reference `<object>@<version>` of `result`.
fn reference_of(result: &Value) -> String {
    let object = result["object_id"].as_str().expect("object id");
    format!("{object}@{}", result["version"].as_str().expect("version"))
}

/// The entries naming the object version of `result`.
fn entries_of(rig: &FilesRig, result: &Value) -> Vec<JournalEntry> {
    let reference = reference_of(result);
    rig.journal()
        .into_iter()
        .filter(|e| e.object_ref.as_deref() == Some(reference.as_str()))
        .collect()
}

/// The operation ID of the single `ocfl.commit` entry naming the version of `result`.
fn commit_operation(rig: &FilesRig, result: &Value) -> String {
    let commits: Vec<JournalEntry> = entries_of(rig, result)
        .into_iter()
        .filter(|e| e.operation == "ocfl.commit")
        .collect();
    assert_eq!(commits.len(), 1, "{commits:?}");
    commits[0].operation_id.clone().expect("an operation id")
}

/// Send the command `input` with `method` to `uri` under a new operation ID; keep the answer.
async fn command(w: &mut MonitorWorld, method: Method, uri: &str, input: Value) {
    let id = operation_id();
    let revision = w.prov.rig().revision_of(&input).await;
    let body = command_body(&input, &id, &revision);
    let answer = w.prov.rig().send_command(method, uri, &body).await;
    w.prov.keep(id, &answer);
}

/// `PUT /api/v1/files/metadata/statements` of `curie` = `value` on the uploaded file.
async fn set_statement(w: &MonitorWorld, curie: &str, value: &str) -> Answer {
    let body = json!({
        "library_id": LIBRARY, "file_id": w.prov.uploaded["file_id"],
        "curie": curie, "value": value,
    });
    let headers = [("content-type", "application/json")];
    let uri = "/api/v1/files/metadata/statements";
    let rig = w.prov.rig();
    rig.call(Method::PUT, uri, &headers, body.to_string().into_bytes())
        .await
}

/// Set a statement on the uploaded file and keep the answer with the operation ID its commit
/// entry names (the route takes no envelope).
async fn edit_metadata(w: &mut MonitorWorld, value: &str) {
    let answer = set_statement(w, "dcterms:title", value).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let id = commit_operation(w.prov.rig(), &answer.json());
    w.prov.keep(id, &answer);
}

/// Upload a new file and keep it as the scenario's file.
async fn upload_file(w: &mut MonitorWorld) {
    let rig = w.prov.rig();
    let answer = rig
        .upload(upload_facts("harbour.jpg", 4, ""), b"jpeg")
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    w.prov.uploaded = answer.result();
}

/// Rename the uploaded file through its command route.
async fn rename_file(w: &mut MonitorWorld) {
    let file_id = w.prov.uploaded["file_id"].clone();
    let input = json!({ "library_id": LIBRARY, "file_id": file_id, "display_name": "Quay" });
    command(w, Method::POST, "/api/v1/files/renames", input).await;
}

#[given(expr = "a desktop library with no c2pa module")]
async fn desktop_library(w: &mut MonitorWorld) {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    w.prov.rig = Some(rig);
}

#[given(expr = "the installation UUID of this FileSquid installation is known")]
fn installation_known(w: &mut MonitorWorld) {
    let context = ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    };
    let rig = w.prov.rig();
    assert!(rig.service.install_provenance(context, rig.journal.clone()));
}

#[given(expr = "a file in the library")]
async fn a_file(w: &mut MonitorWorld) {
    upload_file(w).await;
    w.prov.snapshot();
}

#[when(regex = r"^I (upload|copy|reinstate|label|rename|move|delete|edit the metadata|revert) it$")]
async fn operate(w: &mut MonitorWorld, operation: String) {
    let file_id = w.prov.uploaded["file_id"].clone();
    match operation.as_str() {
        "upload" => {
            let id = file_id.as_str().expect("id").to_string();
            let input = upload_facts("harbour.jpg", 5, &format!("file_id={id}"));
            let op = operation_id();
            let revision = w.prov.rig().revision_of(&input).await;
            let envelope = command_body(&input, &op, &revision)["envelope"].clone();
            let answer = w.prov.rig().send_upload(&envelope, &input, b"jpeg2").await;
            w.prov.keep(op, &answer);
        }
        "copy" => {
            let input = json!({
                "library_id": LIBRARY, "file_id": file_id, "collection_id": HOME_COLLECTION
            });
            command(w, Method::POST, "/api/v1/files/copies", input).await;
        }
        "rename" => rename_file(w).await,
        "edit the metadata" => edit_metadata(w, "Harbour").await,
        "move" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
            command(w, Method::POST, "/api/v1/files/moves", input).await;
        }
        "delete" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "path": "a.txt" });
            command(w, Method::POST, "/api/v1/files/removals", input).await;
        }
        "reinstate" | "revert" => {
            let paths = match operation.as_str() {
                "revert" => json!(["a.txt", "b.txt"]),
                _ => json!(["a.txt"]),
            };
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "paths": paths });
            command(w, Method::POST, "/api/v1/files/restorations", input).await;
        }
        "label" => {
            let input = json!({ "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "label": "First" });
            command(w, Method::PUT, "/api/v1/files/labels", input).await;
        }
        other => panic!("no such operation: {other}"),
    }
}

#[then(expr = "exactly one new OCFL version should be committed")]
fn one_version(w: &mut MonitorWorld) {
    let after = count_versions(&w.prov.rig().dir.join("root"));
    assert_eq!(after, w.prov.versions_before + 1);
}

#[then(
    expr = "that version should contain its PROV-O record named after the new version under {string}"
)]
async fn record_in_version(w: &mut MonitorWorld, prefix: String) {
    let result = &w.prov.result;
    let object = Object::open(
        w.prov.rig().root.as_ref(),
        &object_root(result["object_id"].as_str().expect("object")),
    )
    .await
    .expect("object");
    let version = result["version"].as_str().expect("version");
    let num = VersionNum::parse(version).expect("version");
    let state = object.state(&num).expect("state");
    assert!(state.get(&format!("{prefix}{version}.jsonld")).is_some());
}

#[then(
    expr = "the record should hold exactly one prov:Activity of kind {string} with the operation ID and the object@version"
)]
async fn one_activity(w: &mut MonitorWorld, kind: String) {
    let record = record_of(w.prov.rig(), &w.prov.result).await;
    let activities = typed(&record, "prov:Activity");
    assert_eq!(activities.len(), 1, "{record}");
    let activity = &activities[0];
    assert_eq!(activity["fsq:kind"], kind.as_str());
    assert_eq!(activity["fsq:operation"], w.prov.operation.as_str());
    let version = &activity["fsq:objectVersion"];
    assert_eq!(version["fsq:objectId"], w.prov.result["object_id"]);
    assert_eq!(version["fsq:version"], w.prov.result["version"]);
}

#[then(expr = "the record should name the journal entry IDs of that commit")]
async fn names_journal_ids(w: &mut MonitorWorld) {
    let record = record_of(w.prov.rig(), &w.prov.result).await;
    let activity = typed(&record, "prov:Activity").remove(0);
    let named = journal_ids(&activity);
    let entries = entries_of(w.prov.rig(), &w.prov.result);
    assert!(!entries.is_empty());
    for entry in entries {
        let id = entry.id.expect("an id").to_string();
        assert!(named.contains(&id), "{id} not in {named:?}");
    }
}

#[given(expr = "a file whose metadata record has one earlier version")]
async fn metadata_with_history(w: &mut MonitorWorld) {
    upload_file(w).await;
    edit_metadata(w, "Harbour").await;
    w.prov.previous_version = w.prov.result["version"].as_str().expect("v").to_string();
}

#[when(expr = "I edit its metadata")]
async fn edit_again(w: &mut MonitorWorld) {
    edit_metadata(w, "Harbour at dusk").await;
}

#[then(
    expr = "the new metadata state should be a prov:Entity with object ID, version, logical path and SHA-256"
)]
async fn metadata_entity(w: &mut MonitorWorld) {
    let record = record_of(w.prov.rig(), &w.prov.result).await;
    let file_id = w.prov.uploaded["file_id"].as_str().expect("id");
    let path = format!("original/{file_id}.metadata.md");
    let version = w.prov.result["version"].as_str().expect("version");
    let entity = typed(&record, "prov:Entity")
        .into_iter()
        .find(|e| e["fsq:path"] == path.as_str() && e["fsq:version"] == version)
        .expect("the new metadata state");
    assert_eq!(entity["fsq:objectId"], w.prov.result["object_id"]);
    assert!(entity["fsq:sha256"].as_str().is_some_and(|d| d.len() == 64));
}

#[then(
    expr = "it should be prov:wasGeneratedBy the activity and prov:wasDerivedFrom the previous metadata state"
)]
async fn generated_and_derived(w: &mut MonitorWorld) {
    let record = record_of(w.prov.rig(), &w.prov.result).await;
    let activity = typed(&record, "prov:Activity").remove(0);
    let activity_id = activity["@id"].as_str().expect("activity id").to_string();
    let version = w.prov.result["version"].as_str().expect("version");
    let entity = typed(&record, "prov:Entity")
        .into_iter()
        .find(|e| {
            e["fsq:version"] == version
                && e["fsq:path"]
                    .as_str()
                    .is_some_and(|p| p.ends_with(".metadata.md"))
        })
        .expect("the new metadata state");
    assert_eq!(references(&entity["prov:wasGeneratedBy"]), [activity_id]);
    let object_id = w.prov.result["object_id"].as_str().expect("object id");
    let file_id = w.prov.uploaded["file_id"].as_str().expect("file id");
    let path = format!("original/{file_id}.metadata.md");
    let previous = filesquid_prov::entity_id(object_id, &w.prov.previous_version, &path);
    assert_eq!(references(&entity["prov:wasDerivedFrom"]), [previous]);
}

#[when(expr = "I rename a file")]
async fn rename_a_file(w: &mut MonitorWorld) {
    if w.prov.rig.is_none() {
        // The F-061 scenario of the same wording has a background of its own.
        desktop_library(w).await;
        installation_known(w);
    }
    upload_file(w).await;
    rename_file(w).await;
}

#[then(
    expr = "the journal should hold one {string} entry with the operation ID and object reference of the file object and the new version"
)]
fn one_journal_entry(w: &mut MonitorWorld, operation: String) {
    let entries: Vec<JournalEntry> = entries_of(w.prov.rig(), &w.prov.result)
        .into_iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(
        entries[0].operation_id.as_deref(),
        Some(w.prov.operation.as_str())
    );
}

#[then(expr = "the PROV-O activity should list that entry's ID")]
async fn activity_lists_entry(w: &mut MonitorWorld) {
    let record = record_of(w.prov.rig(), &w.prov.result).await;
    let activity = typed(&record, "prov:Activity").remove(0);
    let commit = entries_of(w.prov.rig(), &w.prov.result)
        .into_iter()
        .find(|e| e.operation == "ocfl.commit")
        .expect("the commit entry");
    let id = commit.id.expect("an id").to_string();
    let named = journal_ids(&activity);
    assert!(named.contains(&id), "{id} not in {named:?}");
}

#[given(expr = "a file whose metadata is unchanged")]
async fn unchanged_metadata(w: &mut MonitorWorld) {
    upload_file(w).await;
    edit_metadata(w, "Harbour").await;
    w.prov.snapshot();
}

#[when(expr = "I save its metadata without changes")]
async fn save_unchanged(w: &mut MonitorWorld) {
    let answer = set_statement(w, "dcterms:title", "Harbour").await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{:?}", answer.json());
}

#[then(expr = "no OCFL version should be committed")]
fn no_version(w: &mut MonitorWorld) {
    let after = count_versions(&w.prov.rig().dir.join("root"));
    assert_eq!(after, w.prov.versions_before);
}

#[then(expr = "no PROV-O record should be written")]
fn no_record(w: &mut MonitorWorld) {
    let after = count_records(&w.prov.rig().dir.join("root"));
    assert_eq!(after, w.prov.records_before);
}

#[when(expr = "a caller without the per-launch token tries to rename a file")]
async fn rename_without_token(w: &mut MonitorWorld) {
    upload_file(w).await;
    w.prov.snapshot();
    let input = json!({
        "library_id": LIBRARY, "file_id": w.prov.uploaded["file_id"], "display_name": "Quay"
    });
    let body = command_body(&input, &operation_id(), "v1");
    let headers = [("content-type", "application/json")];
    let state = &w.prov.rig().state;
    let answer = send(
        state,
        Method::POST,
        "/api/v1/files/renames",
        &headers,
        body.to_string().into_bytes(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
}

#[then(expr = "the journal should record the refusal")]
fn refusal_recorded(w: &mut MonitorWorld) {
    assert_eq!(w.prov.rig().journaled("api.auth.denied").len(), 1);
}

#[when(expr = "I download one file")]
async fn download_one(w: &mut MonitorWorld) {
    upload_file(w).await;
    w.prov.snapshot();
    let object = w.prov.uploaded["object_id"].as_str().expect("object");
    let file_id = w.prov.uploaded["file_id"].as_str().expect("id");
    let uri = format!(
        "/api/v1/files/download?library_id={LIBRARY}&object_id={}&path={}",
        crate::files_fixture::encode(object),
        crate::files_fixture::encode(&format!("original/{file_id}.jpg"))
    );
    let answer = w.prov.rig().get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK);
}

#[then(expr = "the journal should record a {string} entry with the file ID and version")]
fn download_journaled(w: &mut MonitorWorld, operation: String) {
    let entries = w.prov.rig().journaled(&operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let file_id = w.prov.uploaded["file_id"].as_str().expect("id");
    let targets = &entries[0].target_ids;
    assert!(targets.iter().any(|t| t.contains(file_id)), "{targets:?}");
    let version = w.prov.uploaded["version"].as_str().expect("version");
    assert!(targets.iter().any(|t| t == version), "{targets:?}");
}

#[given(expr = "a library with recorded history")]
async fn recorded_history(w: &mut MonitorWorld) {
    upload_file(w).await;
    rename_file(w).await;
    let input =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    command(w, Method::POST, "/api/v1/files/moves", input).await;
}

#[when(expr = "I close FileSquid and copy the storage root with ordinary file tools")]
fn copy_root(w: &mut MonitorWorld) {
    let rig = w.prov.rig();
    // Closing the library stops every write to its storage root before the copy.
    assert!(rig.service.close_library(&crate::files_fixture::library()));
    let copy = rig.dir.join("ordinary-copy");
    copy_tree(&rig.dir.join("root"), &copy);
    w.prov.copy = Some(copy);
}

#[then(expr = "a JSON parser alone should read every activity, entity and agent from the copy")]
fn parse_copy(w: &mut MonitorWorld) {
    let copy = w.prov.copy.as_ref().expect("a copy");
    let files = record_files(copy);
    assert!(
        files.len() >= 3,
        "upload, rename and move records: {files:?}"
    );
    let installation = format!("urn:uuid:{INSTALLATION}");
    for file in files {
        let record: Value =
            serde_json::from_slice(&std::fs::read(&file).expect("read")).expect("JSON");
        assert_eq!(
            typed(&record, "prov:Activity").len(),
            1,
            "{}",
            file.display()
        );
        assert!(
            !typed(&record, "prov:Entity").is_empty(),
            "{}",
            file.display()
        );
        let agents = typed(&record, "prov:Agent");
        assert!(agents.iter().any(|a| a["@id"] == installation.as_str()));
        assert!(!typed(&record, "prov:SoftwareAgent").is_empty());
    }
}

/// The journal entries the version of `result` carries in `provenance/<version>.journal.jsonl`.
async fn version_journal(rig: &FilesRig, result: &Value) -> Vec<Value> {
    let object_id = result["object_id"].as_str().expect("object id");
    let version = result["version"].as_str().expect("version");
    let object = Object::open(rig.root.as_ref(), &object_root(object_id))
        .await
        .expect("object");
    let num = VersionNum::parse(version).expect("version");
    let path = format!("provenance/{version}.journal.jsonl");
    let size = filesquid_ocfl::view::file_details(&object, &num, &path)
        .await
        .expect("the journal file")
        .size;
    let bytes = filesquid_ocfl::view::read_range(&object, &num, &path, 0, size)
        .await
        .expect("journal bytes");
    String::from_utf8(bytes)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON entry per line"))
        .collect()
}

#[then(
    expr = "the new OCFL version should contain the operation's journal entries beside its PROV-O record"
)]
async fn journal_beside_record(w: &mut MonitorWorld) {
    record_in_version(w, "provenance/".to_string()).await;
    let lines = version_journal(w.prov.rig(), &w.prov.result).await;
    let operations: Vec<&str> = lines
        .iter()
        .filter_map(|line| line["operation"].as_str())
        .collect();
    assert_eq!(operations, ["ocfl.commit", "files.rename", "metadata.edit"]);
}

#[then(
    expr = "each entry should carry the operation ID and the object reference of the new version"
)]
async fn entries_carry_links(w: &mut MonitorWorld) {
    let lines = version_journal(w.prov.rig(), &w.prov.result).await;
    let reference = reference_of(&w.prov.result);
    for line in lines {
        assert_eq!(line["operation_id"], w.prov.operation.as_str(), "{line}");
        assert_eq!(line["object_ref"], reference.as_str(), "{line}");
    }
}
