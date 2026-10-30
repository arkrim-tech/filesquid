//! Integration tests of the journal and commit linkage of the file and metadata routes
//! (Task API-F2.8; API-S-028, API-S-017; ADR 040 Decision 6), through the router against a
//! real OCFL storage root on the local filesystem.
//!
//! Every commit that a file route (upload, move, remove, copy, rename, restore, label) or a
//! metadata route (set or remove a statement, register a namespace) publishes writes the
//! `filesquid.operation.v1` message with the operation ID and kind, and is journaled with
//! exactly one `ocfl.commit` entry naming the operation ID and the object version. With
//! provenance installed its version `user` is the installation; without, the caller. Every
//! metadata write is also journaled as `metadata.edit`. A refused attempt is journaled as
//! before and never reaches the version history.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{command_body, operation_id, upload_facts, Answer, FilesRig, LIBRARY, OBJECT};
use filesquid_api::history::ProvenanceContext;
use filesquid_api::journal::{is_opaque, JournalEntry, Outcome};
use filesquid_core::InstallationId;
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::read::Object;
use filesquid_prov::HostFacts;
use serde_json::{json, Value};

/// Where the rig's object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// The installation of these tests.
const INSTALLATION: &str = "0f3c2b1a-9d8e-4f7a-b6c5-d4e3f2a1b0c9";

/// A statement value that must never reach the journal.
const SECRET_VALUE: &str = "Grandmother at the harbour";

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

/// The storage-root path of `object_id`: the rig's object, the library's snapshot object
/// (`snapshot`), or a file object `files/<id>`.
fn object_root(object_id: &str) -> String {
    let id = object_id.trim_start_matches("urn:uuid:");
    match (object_id == OBJECT, id == LIBRARY) {
        (true, _) => OBJECT_ROOT.to_string(),
        (false, true) => filesquid_api::canonical::SNAPSHOT_ROOT.to_string(),
        (false, false) => format!("files/{id}"),
    }
}

/// The `user` and `message` of `version` of `object_id`, and its logical paths.
async fn version_facts(
    rig: &FilesRig,
    object_id: &str,
    version: &str,
) -> (Option<User>, String, Vec<String>) {
    let object = Object::open(rig.root.as_ref(), &object_root(object_id))
        .await
        .expect("object");
    let num = filesquid_ocfl::inventory::VersionNum::parse(version).expect("version");
    let block = &object.inventory().versions[&num];
    let state = object.state(&num).expect("state");
    let paths = state.paths().map(str::to_string).collect();
    (
        block.user.clone(),
        block.message.clone().unwrap_or_default(),
        paths,
    )
}

/// The `ocfl.commit` entries journaled for `operation_id`.
fn commits_of(rig: &FilesRig, operation_id: &str) -> Vec<JournalEntry> {
    rig.journaled("ocfl.commit")
        .into_iter()
        .filter(|entry| entry.operation_id.as_deref() == Some(operation_id))
        .collect()
}

/// The answer's committed result, after asserting a `200`.
fn committed(answer: &Answer) -> Value {
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.result()
}

/// Assert the version `result` names was committed by the installation for the operation
/// `operation_id` of `kind`, carries its PROV-O record and was journaled with exactly one
/// `ocfl.commit` entry naming the operation and the object version.
async fn assert_linked(rig: &FilesRig, result: &Value, operation_id: &str, kind: &str) {
    let object_id = result["object_id"].as_str().expect("object id");
    let version = result["version"].as_str().expect("version");
    let (user, message, paths) = version_facts(rig, object_id, version).await;
    let user = user.expect("a version user");
    assert_eq!(user.name, "FileSquid installation");
    assert_eq!(
        user.address.as_deref(),
        Some(format!("urn:uuid:{INSTALLATION}").as_str())
    );
    assert!(message.starts_with("filesquid.operation.v1 "), "{message}");
    assert!(
        message.contains(&format!("\"operation_id\":\"{operation_id}\"")),
        "{message}"
    );
    assert!(
        message.contains(&format!("\"kind\":\"{kind}\"")),
        "{message}"
    );
    assert!(
        paths.contains(&format!("provenance/{version}.jsonld")),
        "{paths:?}"
    );
    let commits = commits_of(rig, operation_id);
    assert_eq!(commits.len(), 1, "exactly one ocfl.commit: {commits:?}");
    let reference = format!("{object_id}@{version}");
    assert_eq!(commits[0].object_ref.as_deref(), Some(reference.as_str()));
    assert_eq!(commits[0].outcome, Outcome::Succeeded);
}

/// Upload `bytes` as a new file named `name`, and answer the answer.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8]) -> Answer {
    rig.upload(upload_facts(name, bytes.len(), ""), bytes).await
}

/// The operation ID a command answer carries.
fn operation_of(answer: &Answer) -> String {
    answer.json()["operation_id"]
        .as_str()
        .expect("operation id")
        .to_string()
}

/// `PUT /api/v1/files/metadata/statements` of `curie` = `value` on `file_id`.
async fn set(rig: &FilesRig, file_id: &Value, curie: &str, value: &str) -> Answer {
    let body = json!({ "library_id": LIBRARY, "file_id": file_id, "curie": curie, "value": value });
    let headers = [("content-type", "application/json")];
    let uri = "/api/v1/files/metadata/statements";
    rig.call(Method::PUT, uri, &headers, body.to_string().into_bytes())
        .await
}

/// The operation ID of the single `ocfl.commit` entry naming `object_id@version`.
fn operation_of_version(rig: &FilesRig, result: &Value) -> String {
    let reference = format!(
        "{}@{}",
        result["object_id"].as_str().expect("object"),
        result["version"].as_str().expect("version")
    );
    let commits: Vec<JournalEntry> = rig
        .journaled("ocfl.commit")
        .into_iter()
        .filter(|entry| entry.object_ref.as_deref() == Some(reference.as_str()))
        .collect();
    assert_eq!(commits.len(), 1, "exactly one ocfl.commit: {commits:?}");
    commits[0].operation_id.clone().expect("an operation id")
}

// ---- File routes ----

#[tokio::test]
async fn an_upload_is_committed_by_the_installation_and_journaled_once_as_ocfl_commit() {
    let rig = FilesRig::new().await;
    with_provenance(&rig);
    let answer = upload(&rig, "harbour.jpg", b"jpeg").await;
    let result = committed(&answer);
    assert_linked(&rig, &result, &operation_of(&answer), "upload").await;
    let own = rig.journaled("files.upload");
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].operation_id, Some(operation_of(&answer)));
    assert!(own[0].object_ref.is_some());
}

#[tokio::test]
async fn a_move_and_a_remove_are_each_one_linked_commit() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    with_provenance(&rig);
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    assert_linked(&rig, &committed(&moved), &operation_of(&moved), "move").await;
    let removing = json!({ "library_id": LIBRARY, "object_id": OBJECT, "path": "c.txt" });
    let removed = rig.command("/api/v1/files/removals", removing).await;
    assert_linked(
        &rig,
        &committed(&removed),
        &operation_of(&removed),
        "delete",
    )
    .await;
    assert_eq!(rig.journaled("ocfl.commit").len(), 2);
}

#[tokio::test]
async fn a_copy_and_a_rename_are_each_one_linked_commit() {
    let rig = FilesRig::new().await;
    with_provenance(&rig);
    let source = committed(&upload(&rig, "harbour.jpg", b"jpeg").await);
    let copying = json!({ "library_id": LIBRARY, "file_id": source["file_id"] });
    let copied = rig.command("/api/v1/files/copies", copying).await;
    assert_linked(&rig, &committed(&copied), &operation_of(&copied), "copy").await;
    let renaming =
        json!({ "library_id": LIBRARY, "file_id": source["file_id"], "display_name": "Quay" });
    let renamed = rig.command("/api/v1/files/renames", renaming).await;
    assert_linked(
        &rig,
        &committed(&renamed),
        &operation_of(&renamed),
        "rename",
    )
    .await;
    for (answer, operation) in [(&copied, "files.copy"), (&renamed, "files.rename")] {
        let result = committed(answer);
        let own = rig.journaled(operation);
        assert_eq!(own.len(), 1, "{own:?}");
        let reference = format!(
            "{}@{}",
            result["object_id"].as_str().expect("object"),
            result["version"].as_str().expect("version")
        );
        assert_eq!(own[0].object_ref.as_deref(), Some(reference.as_str()));
        assert_eq!(own[0].operation_id, Some(operation_of(answer)));
    }
    let edits = rig.journaled("metadata.edit");
    assert_eq!(edits.len(), 1, "a rename writes the metadata record");
    assert_eq!(edits[0].operation_id, Some(operation_of(&renamed)));
}

#[tokio::test]
async fn a_restore_and_a_label_carry_the_envelope_operation_id() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    rig.commit(&[("a.txt", b"bravo!")]).await;
    with_provenance(&rig);
    let restoring =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "paths": ["a.txt"] });
    let restored = rig.command("/api/v1/files/restorations", restoring).await;
    assert_linked(
        &rig,
        &committed(&restored),
        &operation_of(&restored),
        "revert",
    )
    .await;
    let labelling =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "version": "v1", "label": "Draft" });
    let labelled = rig
        .command_as(Method::PUT, "/api/v1/files/labels", labelling)
        .await;
    assert_linked(
        &rig,
        &committed(&labelled),
        &operation_of(&labelled),
        "label",
    )
    .await;
}

#[tokio::test]
async fn without_provenance_every_commit_still_journals_one_ocfl_commit() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    let result = committed(&moved);
    let commits = commits_of(&rig, &operation_of(&moved));
    assert_eq!(commits.len(), 1, "{commits:?}");
    let reference = format!("{OBJECT}@{}", result["version"].as_str().expect("version"));
    assert_eq!(commits[0].object_ref.as_deref(), Some(reference.as_str()));
    let (user, message, _) = version_facts(&rig, OBJECT, "v2").await;
    let user = user.expect("a version user");
    assert_eq!(user.name, "owner", "without provenance the caller");
    assert_eq!(user.address, None);
    assert!(message.contains("\"kind\":\"move\""), "{message}");
    let id = operation_of(&moved);
    assert!(
        message.contains(&format!("\"operation_id\":\"{id}\"")),
        "{message}"
    );
}

#[tokio::test]
async fn the_callers_message_is_the_version_label() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha"), ("b.txt", b"keep")]).await;
    with_provenance(&rig);
    let moving = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt",
        "message": "Tidy up"
    });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    committed(&moved);
    let (_, message, _) = version_facts(&rig, OBJECT, "v2").await;
    assert!(message.contains("\"label\":\"Tidy up\""), "{message}");
    let long = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "path": "b.txt",
        "message": "x".repeat(filesquid_core::MAX_LABEL_BYTES + 1)
    });
    let refused = rig.command("/api/v1/files/removals", long).await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.json()
    );
    for message in ["   ", "line one\nline two"] {
        let removal = json!({
            "library_id": LIBRARY, "object_id": OBJECT, "path": "b.txt", "message": message
        });
        let refused = rig.command("/api/v1/files/removals", removal).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{message:?}");
    }
    assert_eq!(rig.head().await, "v2", "nothing published");
}

#[tokio::test]
async fn a_refused_commit_is_journaled_as_before_and_never_reaches_the_history() {
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    with_provenance(&rig);
    let input =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let id = operation_id();
    let body = command_body(&input, &id, "v7");
    let answer = rig
        .send_command(Method::POST, "/api/v1/files/moves", &body)
        .await;
    assert_eq!(answer.status, StatusCode::CONFLICT, "{:?}", answer.json());
    let refusals = rig.journaled("files.move");
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].outcome, Outcome::Failed);
    assert_eq!(refusals[0].operation_id.as_deref(), Some(id.as_str()));
    assert!(commits_of(&rig, &id).is_empty(), "no ocfl.commit");
    assert_eq!(rig.head().await, "v1", "no version, so no PROV-O record");
}

// ---- Metadata routes ----

#[tokio::test]
async fn metadata_edits_carry_the_structured_message_the_user_and_a_metadata_edit_entry() {
    let rig = FilesRig::new().await;
    with_provenance(&rig);
    let file = committed(&upload(&rig, "harbour.jpg", b"jpeg").await);
    let set_answer = set(&rig, &file["file_id"], "dcterms:title", SECRET_VALUE).await;
    assert_eq!(set_answer.status, StatusCode::OK, "{:?}", set_answer.json());
    let edit = set_answer.json();
    let set_op = operation_of_version(&rig, &edit);
    assert_linked(&rig, &edit, &set_op, "metadata.edit").await;
    let removal =
        json!({ "library_id": LIBRARY, "file_id": file["file_id"], "curie": "dcterms:title" });
    let removed = rig.post("/api/v1/files/metadata/removals", removal).await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json());
    let remove_op = operation_of_version(&rig, &removed.json());
    assert_linked(&rig, &removed.json(), &remove_op, "metadata.edit").await;
    let registration = json!({
        "library_id": LIBRARY, "prefix": "fam", "iri": "https://example.org/fam#",
        "kind": "custom", "terms": null
    });
    let registered = rig
        .post("/api/v1/files/metadata/registrations", registration)
        .await;
    assert_eq!(registered.status, StatusCode::OK, "{:?}", registered.json());
    let register_op = operation_of_version(&rig, &registered.json());
    assert_linked(&rig, &registered.json(), &register_op, "metadata.edit").await;
    let edits = rig.journaled("metadata.edit");
    let ops: Vec<_> = edits
        .iter()
        .filter_map(|e| e.operation_id.clone())
        .collect();
    assert_eq!(ops, [set_op, remove_op, register_op]);
    assert!(edits.iter().all(|e| e.object_ref.is_some()));
}

#[tokio::test]
async fn a_metadata_edit_entry_is_redacted() {
    let rig = FilesRig::new().await;
    with_provenance(&rig);
    let file = committed(&upload(&rig, "harbour.jpg", b"jpeg").await);
    let answer = set(&rig, &file["file_id"], "dcterms:title", SECRET_VALUE).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let edits = rig.journaled("metadata.edit");
    assert_eq!(edits.len(), 1);
    let edit = &edits[0];
    assert!(is_opaque(&edit.actor), "{}", edit.actor);
    assert!(edit.target_ids.iter().all(|id| is_opaque(id)), "{edit:?}");
    assert!(filesquid_api::journal::is_object_ref(
        edit.object_ref.as_deref().expect("object ref")
    ));
    let text = serde_json::to_string(&rig.journal()).expect("journal json");
    assert!(!text.contains(SECRET_VALUE), "no statement value");
    assert!(!text.contains("dcterms:title"), "no statement name");
    assert!(!text.contains("harbour.jpg"), "no file name");
}

#[tokio::test]
async fn a_refused_metadata_edit_is_journaled_as_before_without_a_commit() {
    let rig = FilesRig::new().await;
    with_provenance(&rig);
    let file = committed(&upload(&rig, "harbour.jpg", b"jpeg").await);
    let answer = set(&rig, &file["file_id"], "nope:title", "x").await;
    assert_eq!(
        answer.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        answer.json()
    );
    let refusals = rig.journaled("files.metadata.set");
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].outcome, Outcome::Failed);
    assert!(rig.journaled("metadata.edit").is_empty());
    assert_eq!(rig.journaled("ocfl.commit").len(), 1, "the upload's only");
}

#[tokio::test]
async fn a_lease_takeover_is_bound_into_the_version_with_its_commit() {
    use filesquid_ocfl::write::VersionReservations;
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    with_provenance(&rig);
    let next = filesquid_ocfl::inventory::VersionNum::parse("v2").expect("v2");
    rig.leases
        .reserve(OBJECT, &next, "stalled")
        .await
        .expect("a stalled writer holds v2");
    rig.clock.advance(5 * 60_000);
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    let result = committed(&moved);
    let id = operation_of(&moved);
    let takeovers: Vec<JournalEntry> = rig
        .journaled("ocfl.lease.takeover")
        .into_iter()
        .filter(|entry| entry.operation_id.as_deref() == Some(id.as_str()))
        .collect();
    assert_eq!(takeovers.len(), 1, "{takeovers:?}");
    let version = result["version"].as_str().expect("version");
    let object = Object::open(rig.root.as_ref(), OBJECT_ROOT)
        .await
        .expect("object");
    let path = format!("provenance/{version}.journal.jsonl");
    let size = filesquid_ocfl::view::file_details(&object, object.head(), &path)
        .await
        .expect("journal file")
        .size;
    let bytes = filesquid_ocfl::view::read_range(&object, object.head(), &path, 0, size)
        .await
        .expect("journal bytes");
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(text.contains("ocfl.lease.takeover"), "{text}");
    let bound_id = takeovers[0].id.expect("an id allocated before the commit");
    assert!(text.contains(&format!("\"id\":{bound_id}")), "{text}");
}

#[tokio::test]
async fn a_stale_empty_staging_index_never_gives_a_move_an_older_base() {
    use filesquid_ocfl::change::{ChangeModel, StagingArea};
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    with_provenance(&rig);
    let backend = filesquid_ocfl::storage::FsBackend::new(
        rig.dir.join("api-staging"),
        rig.dir.join("api-staging-parts"),
    )
    .expect("staging backend");
    let prefix = filesquid_api::files::staging_prefix("api", OBJECT);
    let staging = StagingArea::new(&backend, &prefix).expect("staging area");
    let model = ChangeModel::new(
        rig.root.as_ref(),
        rig.leases.as_ref(),
        "stale",
        OBJECT_ROOT,
        OBJECT,
        staging,
    );
    // A rename and its undo leave an index with no changes on the base v1.
    model.stage_rename("a.txt", "z.txt").await.expect("stage");
    model.stage_rename("z.txt", "a.txt").await.expect("undo");
    assert!(model.status().await.expect("status").changes.is_empty());
    rig.commit(&[("a.txt", b"bravo!")]).await;
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    let result = committed(&moved);
    let object = Object::open(rig.root.as_ref(), OBJECT_ROOT)
        .await
        .expect("object");
    let version =
        filesquid_ocfl::inventory::VersionNum::parse(result["version"].as_str().expect("v"))
            .expect("version");
    let state = object.state(&version).expect("state");
    let moved_file = state.get("c.txt").expect("the moved file");
    assert_eq!(
        moved_file.digest,
        files_fixture::digest(b"bravo!"),
        "the head's content"
    );
}

/// An engine warning raised while a file route commits is journaled with the commit's
/// operation ID (API-S-031, Task API-F2.12): a move that takes over a stalled writer's
/// reservation makes the OCFL engine warn about the takeover inside the commit's activity
/// span.
#[tokio::test]
async fn an_engine_warning_during_a_file_route_commit_carries_its_operation_id() {
    use filesquid_api::trace_layer::JournalLayer;
    use filesquid_ocfl::write::VersionReservations;
    use tracing_subscriber::layer::SubscriberExt;
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let layer = JournalLayer::new();
    layer.attach(rig.monitor.journal());
    let _subscriber = tracing::subscriber::set_default(tracing_subscriber::registry().with(layer));
    let next = filesquid_ocfl::inventory::VersionNum::parse("v2").expect("v2");
    rig.leases
        .reserve(OBJECT, &next, "stalled")
        .await
        .expect("a stalled writer holds v2");
    rig.clock.advance(5 * 60_000);
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    committed(&moved);
    let id = operation_of(&moved);
    let warnings: Vec<JournalEntry> = rig
        .journal()
        .into_iter()
        .filter(|entry| entry.operation.starts_with("ocfl.engine."))
        .collect();
    assert!(!warnings.is_empty(), "the engine warned about the takeover");
    assert!(
        warnings
            .iter()
            .all(|entry| entry.operation_id.as_deref() == Some(id.as_str())),
        "{warnings:?}"
    );
}
