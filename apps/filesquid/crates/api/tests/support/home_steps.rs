//! Step definitions for `home_collections_API-S-011.feature` (Task API-F2.14; API-S-011
//! **Destination** and **Same logical path**, API-S-019, API-S-021, API-S-026): a files rig
//! whose memory home collections hold the collection "Holidays" of its library, every
//! upload and copy sent through the router.

use crate::files_fixture::{
    command_body, library, operation_id, upload_input, Answer, FilesRig, HOME_COLLECTION, LIBRARY,
};
use crate::MonitorWorld;
use axum::http::StatusCode;
use cucumber::{given, then, when};
use filesquid_api::canonical::collection_path::MAX_SUFFIX;
use filesquid_api::events::{topic, ApiEvent};
use filesquid_api::files::NEW_FILE_REVISION;
use filesquid_api::home::HomeCollections;
use filesquid_api::journal::Outcome;
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
    pending: Option<(String, Value, FileId)>,
    placed: u32,
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
    let minted = FileId::new("0b6f6a58-5c1d-4b5e-9f3e-0000000c4a5e").expect("id");
    let reserved = rig
        .home
        .reserve(&library(), &operation, &minted)
        .expect("reserve");
    let input = facts(&name, None);
    let mut query: filesquid_api::files::UploadQuery =
        serde_json::from_value(input.clone()).expect("facts");
    query.reserved_file_id = Some(reserved.as_str().to_string());
    let bytes = name.as_bytes();
    let committed = rig
        .service
        .upload(query, bytes, "owner", NEW_FILE_REVISION)
        .await;
    assert!(committed.is_ok(), "the first run committed its object");
    w.home.pending = Some((operation, input, reserved));
}

#[when("the owner retries the upload with the same operation ID")]
async fn retry(w: &mut MonitorWorld) {
    let (operation, input, _) = w.home.pending.clone().expect("a crashed upload");
    let envelope = command_body(&input, &operation, NEW_FILE_REVISION)["envelope"].clone();
    let bytes = input["name"].as_str().expect("name").as_bytes().to_vec();
    let answer = w.home.rig().send_upload(&envelope, &input, &bytes).await;
    w.home.answers.push(answer);
}

#[then("the retry should answer the same file at its first version")]
fn same_file(w: &mut MonitorWorld) {
    let (_, _, reserved) = w.home.pending.clone().expect("a crashed upload");
    let answer = w.home.last();
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.result()["file_id"], reserved.as_str());
    assert_eq!(answer.result()["version"], "v1");
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
    assert_eq!(w.home.rig().home.placement(&file), Some((home(), path)));
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

#[then(expr = "the upload should be refused as {string} and journaled once")]
fn refused(w: &mut MonitorWorld, code: String) {
    let answer = w.home.last();
    assert!(answer.status.is_client_error(), "{:?}", answer.json());
    assert!(answer.json()["message"]
        .as_str()
        .is_some_and(|m| m.contains(&code)));
    let failed: Vec<_> = w
        .home
        .rig()
        .journaled("file.upload")
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
