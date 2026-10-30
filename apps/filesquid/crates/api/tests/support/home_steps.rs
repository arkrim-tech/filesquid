//! Step definitions for `home_collections_API-S-011.feature` (Task API-F2.14; API-S-011
//! **Destination** and **Same logical path**, API-S-019, API-S-021, API-S-026): a files rig
//! whose memory home collections hold the collection "Holidays" of its library, every
//! upload and copy sent through the router; and for
//! `moves_renames_and_folders_API-S-011.feature` (Task API-F2.14 part 2): moves, renames and
//! collection folders through the same rig.

use crate::files_fixture::{
    command_body, encode, operation_id, upload_input, Answer, FilesRig, HOME_COLLECTION, LIBRARY,
};
use crate::MonitorWorld;
use axum::http::StatusCode;
use cucumber::{given, then, when};
use filesquid_api::canonical::collection_path::MAX_SUFFIX;
use filesquid_api::events::{topic, ApiEvent};
use filesquid_api::files::NEW_FILE_REVISION;
use filesquid_api::journal::Outcome;
use filesquid_api::navigation::{FolderScope, NavigationChange, NodeRef};
use filesquid_core::{CollectionId, FileId};
use serde_json::{json, Value};
use tokio::sync::broadcast::Receiver;

/// The title the feature gives the rig's home collection.
const HOLIDAYS: &str = "Holidays";

/// A collection of no library the rig holds.
const STRANGER: &str = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";

/// Per-scenario state of the home-collection steps.
#[derive(Default)]
pub struct HomeWorld {
    rig: Option<FilesRig>,
    events: Option<Receiver<ApiEvent>>,
    answers: Vec<Answer>,
    pending: Option<(String, Value)>,
    placed: u32,
    before: Option<(Vec<u8>, String)>,
}

impl std::fmt::Debug for HomeWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HomeWorld")
            .field("answers", &self.answers.len())
            .finish_non_exhaustive()
    }
}

impl HomeWorld {
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    fn last(&self) -> &Answer {
        self.answers.last().expect("an answer")
    }

    /// Place a file of its own at `path`, as an earlier file holding it.
    fn place(&mut self, path: &str) {
        self.placed += 1;
        let id = FileId::new(format!("0b6f6a58-5c1d-4b5e-9f3e-{:012}", self.placed)).expect("id");
        self.rig().home.place(&id, &home(), path);
    }
}

/// The rig's home collection.
fn home() -> CollectionId {
    CollectionId::new(HOME_COLLECTION).expect("collection")
}

/// The upload facts of `name` (its bytes are the name) in `folder`.
fn facts(name: &str, folder: Option<&str>) -> Value {
    let fields = match folder {
        Some(folder) => json!({ "folder": folder }),
        None => json!({}),
    };
    upload_input(name, name.len(), fields)
}

#[given(expr = "a library whose collection {string} receives uploads")]
async fn rig(w: &mut MonitorWorld, title: String) {
    assert_eq!(title, HOLIDAYS);
    let rig = FilesRig::new().await;
    w.home.events = Some(rig.state.events.subscribe());
    w.home.rig = Some(rig);
}

#[when(expr = "the owner uploads {string} into the folder {string}")]
#[given(expr = "the owner has uploaded {string} into the folder {string}")]
async fn upload_into(w: &mut MonitorWorld, name: String, folder: String) {
    let answer = w
        .home
        .rig()
        .upload(facts(&name, Some(&folder)), name.as_bytes())
        .await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner uploads {string} into the collection's root")]
async fn upload_root(w: &mut MonitorWorld, name: String) {
    let answer = w
        .home
        .rig()
        .upload(facts(&name, None), name.as_bytes())
        .await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner uploads {string} without naming a collection")]
async fn upload_nowhere(w: &mut MonitorWorld, name: String) {
    let mut input = facts(&name, None);
    input
        .as_object_mut()
        .expect("object")
        .remove("collection_id");
    let answer = w.home.rig().upload(input, name.as_bytes()).await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner uploads {string} into a collection this library does not hold")]
async fn upload_stranger(w: &mut MonitorWorld, name: String) {
    let input = upload_input(&name, name.len(), json!({ "collection_id": STRANGER }));
    let answer = w.home.rig().upload(input, name.as_bytes()).await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner copies it into the folder {string}")]
async fn copy_into(w: &mut MonitorWorld, folder: String) {
    let source = w.home.last().result()["file_id"].clone();
    let body = json!({
        "library_id": LIBRARY, "file_id": source,
        "collection_id": HOME_COLLECTION, "folder": folder
    });
    let answer = w.home.rig().command("/api/v1/files/copies", body).await;
    w.home.answers.push(answer);
}

#[given(expr = "the path {string} is already taken in {string}")]
#[given(expr = "a recoverably deleted file holds the path {string} in {string}")]
fn taken(w: &mut MonitorWorld, path: String, title: String) {
    assert_eq!(title, HOLIDAYS);
    w.home.place(&path);
}

#[given(expr = "the path {string} and every number up to 9999 beside it are taken in {string}")]
fn all_taken(w: &mut MonitorWorld, path: String, title: String) {
    assert_eq!(title, HOLIDAYS);
    let (stem, ext) = path.split_once('.').expect("a name with an extension");
    w.home.place(&path);
    for n in 1..=MAX_SUFFIX {
        w.home.place(&format!("{stem}-{n}.{ext}"));
    }
}

#[given(expr = "an upload of {string} committed its file but crashed before its registration")]
async fn crashed(w: &mut MonitorWorld, name: String) {
    let rig = w.home.rig();
    let operation = operation_id();
    let input = facts(&name, None);
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    rig.home.fail_next_registration();
    let answer = rig.send_upload(&envelope, &input, name.as_bytes()).await;
    assert_eq!(
        answer.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{:?}",
        answer.json()
    );
    assert_eq!(rig.home.reserved(), 1, "the crash left its reservation");
    w.home.pending = Some((operation, input));
}

#[when("the owner retries the upload with the same operation ID")]
async fn retry(w: &mut MonitorWorld) {
    let (operation, input) = w.home.pending.clone().expect("a crashed upload");
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    let bytes = input["name"].as_str().expect("name").as_bytes().to_vec();
    let answer = w.home.rig().send_upload(&envelope, &input, &bytes).await;
    w.home.answers.push(answer);
}

#[then("the retry should answer the file at its first version")]
fn first_version(w: &mut MonitorWorld) {
    let answer = w.home.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["version"], "v1", "no second version");
}

#[then("the file should be registered once and the upload journaled once as a success")]
fn exactly_once(w: &mut MonitorWorld) {
    let rig = w.home.rig();
    assert_eq!(rig.home.journal().len(), 1, "one registration");
    assert_eq!(rig.home.reserved(), 0, "the reservation is released");
    let succeeded = rig
        .journaled("file.upload")
        .into_iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .count();
    assert_eq!(succeeded, 1);
}

#[then(expr = "the file should be registered at {string} in {string}")]
#[then(expr = "the copy should be registered at {string} in {string}")]
fn registered(w: &mut MonitorWorld, path: String, title: String) {
    assert_eq!(title, HOLIDAYS);
    let answer = w.home.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let file = answer.result()["file_id"]
        .as_str()
        .expect("file")
        .to_string();
    let file = FileId::new(file).expect("file id");
    assert_eq!(w.home.rig().home.placed(&file), Some((home(), path)));
}

#[then(expr = "the answer should name the collection and the path {string}")]
fn answered(w: &mut MonitorWorld, path: String) {
    let result = w.home.last().result();
    assert_eq!(result["collection_id"], HOME_COLLECTION);
    assert_eq!(result["path"], path);
}

#[then("the upload should be journaled once without its name or folder")]
fn journaled(w: &mut MonitorWorld) {
    let rig = w.home.rig();
    let entries = rig.journaled("file.upload");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    let journal = format!("{:?}", rig.journal());
    assert!(!journal.contains("beach"), "a file name was journaled");
    assert!(!journal.contains("Summer"), "a folder was journaled");
}

#[then("the folder should be announced as changed")]
fn announced(w: &mut MonitorWorld) {
    let events = w.home.events.as_mut().expect("subscribed");
    let topics: Vec<String> = std::iter::from_fn(|| events.try_recv().ok())
        .take(1_000)
        .map(|event| event.topic)
        .collect();
    assert!(
        topics.iter().any(|t| t == topic::NAVIGATION_CHANGE),
        "{topics:?}"
    );
}

/// The journal operation of the command a scenario names. Pure.
fn operation_of(command: &str) -> &'static str {
    match command {
        "upload" => "file.upload",
        "move" => "file.move",
        "rename" => "files.rename",
        _ => "files.create_folder",
    }
}

#[then(
    regex = r#"^the (upload|move|rename|new folder) should be refused as "([^"]+)" and journaled once$"#
)]
fn refused(w: &mut MonitorWorld, command: String, code: String) {
    let answer = w.home.last();
    assert!(answer.status.is_client_error(), "{:?}", answer.json());
    assert!(answer.json()["message"]
        .as_str()
        .is_some_and(|m| m.contains(&code)));
    let failed: Vec<_> = w
        .home
        .rig()
        .journaled(operation_of(&command))
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].error_code.as_deref(), Some(code.as_str()));
}

#[then("nothing should be registered")]
fn nothing_registered(w: &mut MonitorWorld) {
    let rig = w.home.rig();
    assert!(rig.home.journal().is_empty());
    assert_eq!(rig.home.reserved(), 0);
}

/// The file the scenario uploaded first.
fn subject(w: &MonitorWorld) -> Value {
    let first = w.home.answers.first().expect("an upload");
    first.result()["file_id"].clone()
}

/// Drop the events received so far, so the next assertions see only the next command's.
fn drain_events(w: &mut MonitorWorld) {
    let events = w.home.events.as_mut().expect("subscribed");
    let drained = std::iter::from_fn(|| events.try_recv().ok()).take(1_000);
    drained.for_each(drop);
}

/// Send a collection move of the scenario's file with `fields` and keep its answer, with the
/// events of the steps before it drained.
async fn send_move(w: &mut MonitorWorld, fields: Value) {
    w.home.before = Some(original_of(w).await);
    drain_events(w);
    let mut body = json!({ "library_id": LIBRARY, "file_id": subject(w) });
    let body_fields = body.as_object_mut().expect("object");
    body_fields.extend(fields.as_object().expect("fields").clone());
    let answer = w.home.rig().command("/api/v1/files/moves", body).await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner moves it into the folder {string}")]
async fn move_into(w: &mut MonitorWorld, folder: String) {
    let fields = json!({ "collection_id": HOME_COLLECTION, "folder": folder });
    send_move(w, fields).await;
}

#[when("the owner moves it without naming a collection")]
async fn move_nowhere(w: &mut MonitorWorld) {
    send_move(w, json!({ "folder": "Archive" })).await;
}

#[when(expr = "the owner renames it to {string}")]
async fn rename_to(w: &mut MonitorWorld, name: String) {
    drain_events(w);
    let body = json!({ "library_id": LIBRARY, "file_id": subject(w), "display_name": name });
    let answer = w.home.rig().command("/api/v1/files/renames", body).await;
    w.home.answers.push(answer);
}

/// Ask for the collection folder `folder` with `body` and keep the answer.
async fn send_folder(w: &mut MonitorWorld, body: Value) {
    let answer = w.home.rig().command("/api/v1/files/folders", body).await;
    w.home.answers.push(answer);
}

#[when(expr = "the owner creates the folder {string} in {string}")]
async fn create_folder(w: &mut MonitorWorld, folder: String, title: String) {
    assert_eq!(title, HOLIDAYS);
    let body = json!({
        "library_id": LIBRARY, "collection_id": HOME_COLLECTION, "folder": folder
    });
    send_folder(w, body).await;
}

#[when(expr = "the owner creates the folder {string} without naming a collection")]
async fn create_folder_nowhere(w: &mut MonitorWorld, folder: String) {
    send_folder(w, json!({ "library_id": LIBRARY, "folder": folder })).await;
}

#[then(expr = "the file should now be registered at {string} in {string}")]
fn now_registered(w: &mut MonitorWorld, path: String, title: String) {
    assert_eq!(title, HOLIDAYS);
    let file = subject(w);
    let file = FileId::new(file.as_str().expect("file")).expect("file id");
    assert_eq!(w.home.rig().home.placed(&file), Some((home(), path)));
}

#[then(expr = "the answer should carry the path {string}")]
fn carries_path(w: &mut MonitorWorld, path: String) {
    let answer = w.home.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["path"], path);
}

#[then("the move should be journaled once without its name or folder")]
fn move_journaled(w: &mut MonitorWorld) {
    let rig = w.home.rig();
    let entries = rig.journaled("file.move");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    let journal = format!("{:?}", rig.journal());
    for word in ["beach", "Summer", "Archive"] {
        assert!(!journal.contains(word), "{word} was journaled");
    }
}

#[then(regex = r#"^the folders "([^"]+)"(?: and "([^"]+)")? should be announced as changed$"#)]
fn folders_announced(w: &mut MonitorWorld, first: String, second: String) {
    let collection = FolderScope::Collection(home());
    let expected: Vec<String> = [first, second]
        .into_iter()
        .filter(|folder| !folder.is_empty())
        .map(|folder| {
            NodeRef::folder(collection.clone(), &folder)
                .to_handle()
                .expect("handle")
        })
        .collect();
    let events = w.home.events.as_mut().expect("subscribed");
    let announced: Vec<String> = std::iter::from_fn(|| events.try_recv().ok())
        .take(1_000)
        .filter(|event| event.topic == topic::NAVIGATION_CHANGE)
        .flat_map(|event| match serde_json::from_value(event.data) {
            Ok(NavigationChange::Parents { parents }) => parents,
            other => panic!("not a parents change: {other:?}"),
        })
        .collect();
    assert_eq!(
        announced, expected,
        "the folder left, then the folder entered"
    );
}

/// The stored original bytes and the file object's head of the scenario's file.
async fn original_of(w: &MonitorWorld) -> (Vec<u8>, String) {
    let rig = w.home.rig();
    let file_id = subject(w);
    let object = format!("urn:uuid:{}", file_id.as_str().expect("file"));
    let object = encode(&object);
    let uri =
        format!("/api/v1/files/listing?library_id={LIBRARY}&object_id={object}&prefix=original");
    let listing = rig.get(&uri).await;
    let path = encode(
        listing.json()["entries"][0]["path"]
            .as_str()
            .expect("the original"),
    );
    let uri = format!("/api/v1/files/content?library_id={LIBRARY}&object_id={object}&path={path}");
    let content = rig.get(&uri).await;
    assert_eq!(content.status, StatusCode::OK);
    let head = rig.revision_of(&json!({ "file_id": file_id })).await;
    (content.body, head)
}

#[then("the file's bytes and its file object's versions should be unchanged")]
async fn bytes_unchanged(w: &mut MonitorWorld) {
    let before = w.home.before.clone().expect("read before the move");
    let after = original_of(w).await;
    assert_eq!(after, before, "no byte and no file version changed");
}

#[then(expr = "the folder should be answered at {string} and pending")]
fn folder_answered(w: &mut MonitorWorld, path: String) {
    let answer = w.home.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let expected = json!({
        "library_id": LIBRARY, "collection_id": HOME_COLLECTION, "path": path, "pending": true
    });
    assert_eq!(answer.result(), expected);
}
