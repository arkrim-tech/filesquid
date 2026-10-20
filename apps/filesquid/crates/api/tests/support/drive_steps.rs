//! Step definitions for `preferences_downloads_listings_API-S-011.feature` (Task API-F2.2):
//! per-user preferences, the streamed whole-file download, the reserved folders left out of
//! listings and the listing by navigation node, through the router over a real OCFL storage
//! root on the local filesystem.

use crate::drive_fixture::{
    get_preferences, grid_preferences, keep_preferences, put_preferences, OneScope,
};
use crate::files_fixture::{encode, pattern, query, Answer, FilesRig};
use crate::navigation_fixture::NavRig;
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::files::MAX_ANSWER_BYTES;
use filesquid_api::journal::Outcome;
use filesquid_api::navigation::{FolderScope, NodeRef};
use filesquid_api::preferences::{MemoryPreferenceStore, PreferenceStore, UPDATE_OPERATION};
use filesquid_core::CollectionId;
use serde_json::{json, Value};
use std::sync::Arc;

/// Per-scenario state of the drive steps.
#[derive(Default)]
pub struct DriveWorld {
    rig: Option<FilesRig>,
    nav: Option<NavRig>,
    store: Option<Arc<MemoryPreferenceStore>>,
    expected: Vec<u8>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for DriveWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriveWorld").finish_non_exhaustive()
    }
}

impl DriveWorld {
    /// The rig of the scenario: its own, or the navigation fixture's Photos rig.
    fn rig(&self) -> &FilesRig {
        match &self.nav {
            Some(nav) => nav.photos_rig.as_ref(),
            None => self.rig.as_ref().expect("rig"),
        }
    }

    fn nav(&self) -> &NavRig {
        self.nav.as_ref().expect("navigation fixture")
    }

    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("answer")
    }
}

/// A collection node handle the navigation service could write.
fn collection_handle(id: &str) -> String {
    let id = CollectionId::new(id).expect("collection id");
    NodeRef::Collection(id).to_handle().expect("handle")
}

/// The paths of a listing page's entries.
fn paths(page: &Value) -> Vec<String> {
    page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["path"].as_str().expect("path").to_string())
        .collect()
}

#[given(expr = "a drive host that keeps preferences")]
async fn keeps_preferences(w: &mut MonitorWorld) {
    let rig = FilesRig::new().await;
    w.drive.store = Some(keep_preferences(&rig));
    w.drive.rig = Some(rig);
}

#[when(expr = "the user reads their preferences")]
async fn read_preferences(w: &mut MonitorWorld) {
    w.drive.answer = Some(get_preferences(w.drive.rig()).await);
}

#[when(expr = "the user saves the {string} layout with one pinned collection")]
async fn save_layout(w: &mut MonitorWorld, layout: String) {
    let mut body = grid_preferences(&collection_handle("c-1"));
    body["layout"] = json!(layout);
    let saved = put_preferences(w.drive.rig(), &body).await;
    assert_eq!(saved.status.as_u16(), 200, "{:?}", saved.json());
    w.drive.answer = Some(get_preferences(w.drive.rig()).await);
}

#[when(expr = "the user saves {int} pinned destinations")]
async fn save_pins(w: &mut MonitorWorld, count: usize) {
    let pins: Vec<Value> = (0..count)
        .map(|i| json!({ "node": collection_handle(&format!("c-{i}")), "kind": "collection" }))
        .collect();
    let mut body = grid_preferences(&collection_handle("c-1"));
    body["navigation"]["pinned"] = json!(pins);
    w.drive.answer = Some(put_preferences(w.drive.rig(), &body).await);
}

#[then(expr = "the preferences should show the {string} layout and {int} pinned destinations")]
fn shows_layout(w: &mut MonitorWorld, layout: String, pins: usize) {
    let answer = w.drive.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    assert_eq!(answer.json()["layout"], layout);
    let pinned = answer.json()["navigation"]["pinned"].clone();
    assert_eq!(pinned.as_array().expect("pinned").len(), pins);
}

#[then(expr = "the save should be journaled once without its values")]
fn save_journaled(w: &mut MonitorWorld) {
    let entries = w.drive.rig().journaled(UPDATE_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert!(entries[0].target_ids.is_empty());
    assert!(entries[0].object_ref.is_none());
}

#[then(expr = "the drive answer should be the error {string}")]
fn drive_error(w: &mut MonitorWorld, code: String) {
    let answer = w.drive.answer();
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

#[then(expr = "no preferences should be kept for the user")]
fn nothing_kept(w: &mut MonitorWorld) {
    let store = w.drive.store.as_ref().expect("store");
    assert_eq!(store.load("owner").expect("load"), None);
}

#[given(expr = "a drive file {string} larger than one content answer")]
async fn large_file(w: &mut MonitorWorld, path: String) {
    let bytes = pattern(MAX_ANSWER_BYTES + 3);
    w.drive.rig = Some(FilesRig::with_files(&[(path.as_str(), &bytes)]).await);
    w.drive.expected = bytes;
}

#[when(expr = "the user streams {string} whole")]
async fn stream_whole(w: &mut MonitorWorld, path: String) {
    let uri = format!(
        "/api/v1/files/download?{}",
        query(&format!("path={}", encode(&path)))
    );
    w.drive.answer = Some(w.drive.rig().get(&uri).await);
}

#[then(expr = "the stream should carry every byte as an attachment named {string}")]
fn whole_stream(w: &mut MonitorWorld, name: String) {
    let answer = w.drive.answer();
    assert_eq!(answer.status.as_u16(), 200);
    assert!(answer.body == w.drive.expected, "every byte, in order");
    let disposition = answer.header("content-disposition");
    assert!(disposition.starts_with(&format!("attachment; filename=\"{name}\"")));
}

#[given(expr = "a drive object holding {string}, {string} and {string}")]
async fn object_holding(w: &mut MonitorWorld, a: String, b: String, c: String) {
    let files = [
        (a.as_str(), b"a".as_slice()),
        (b.as_str(), b"{}"),
        (c.as_str(), b"{}"),
    ];
    w.drive.rig = Some(FilesRig::with_files(&files).await);
}

#[when(expr = "the user lists the drive object's root folder")]
async fn list_root(w: &mut MonitorWorld) {
    let uri = format!("/api/v1/files/listing?{}", query(""));
    w.drive.answer = Some(w.drive.rig().get(&uri).await);
}

#[then(expr = "the drive listing should show only {string}")]
fn shows_only(w: &mut MonitorWorld, path: String) {
    assert_eq!(paths(&w.drive.answer().json()), [path]);
}

#[then(expr = "listing inside {string} should not be found")]
async fn inside_not_found(w: &mut MonitorWorld, prefix: String) {
    let uri = format!(
        "/api/v1/files/listing?{}",
        query(&format!("prefix={}", encode(&prefix)))
    );
    let answer = w.drive.rig().get(&uri).await;
    assert_eq!(answer.status.as_u16(), 404);
    assert_eq!(answer.json()["code"], "not_found");
}

#[given(expr = "the {string} collection is served as one OCFL object")]
async fn collection_object(w: &mut MonitorWorld, _name: String) {
    let nav = NavRig::new().await;
    let scope = FolderScope::Collection(nav.photos.clone());
    assert!(nav
        .photos_rig
        .service
        .install_scope_objects(Arc::new(OneScope(scope))));
    w.drive.nav = Some(nav);
}

#[when(expr = "the user lists the folder node {string}")]
async fn list_folder_node(w: &mut MonitorWorld, path: String) {
    let node = encode(&w.drive.nav().photos_folder(&path));
    let uri = format!("/api/v1/files/listing?node={node}");
    w.drive.answer = Some(w.drive.rig().get(&uri).await);
}

#[when(expr = "the user lists the library node")]
async fn list_library_node(w: &mut MonitorWorld) {
    let nav = w.drive.nav();
    let node = encode(&nav.handle(&NodeRef::Library(nav.library.clone())));
    let uri = format!("/api/v1/files/listing?node={node}");
    w.drive.answer = Some(w.drive.rig().get(&uri).await);
}

#[then(expr = "the drive listing should show {string} and {string}")]
fn shows_two(w: &mut MonitorWorld, first: String, second: String) {
    let answer = w.drive.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    assert_eq!(paths(&answer.json()), [first, second]);
}
