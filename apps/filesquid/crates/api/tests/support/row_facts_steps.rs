//! Step definitions for `row_facts_recent_and_destinations_API-S-011.feature` (Task
//! API-F2.21): the row facts of the view and the search, the folder counts, the Recent view,
//! the collection ID on navigation nodes, the destination route, the history bytes and the
//! refusal sub-codes, through the router over a real OCFL storage root on the local
//! filesystem, a seeded library catalog and a folder index.

use crate::drive_catalog::{DriveCatalog, COLLECTION, COLLECTION_TITLE, REPOSITORY};
use crate::files_fixture::{encode, operation_id, upload_input, Answer, FilesRig, LIBRARY};
use crate::MonitorWorld;
use axum::http::Method;
use cucumber::{given, then, when};
use filesquid_api::auth::Principal;
use filesquid_api::journal::{format_rfc3339_ms, JournalEntry, JournalStore, Outcome};
use filesquid_api::library::{
    LibraryCatalog, LibraryService, MemorySettings, MAX_UNTITLED_TITLES, UNTITLED_COLLECTION_TITLE,
};
use filesquid_api::navigation::{
    FolderScope, HostAccess, MemoryFolderIndex, NavigationAccess, NavigationService, NodeRef,
    RepositoryInfo,
};
use filesquid_core::{Collection, CollectionId, FileId, LibraryId, RepositoryId};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// A collection ID no library holds.
const STRANGER: &str = "9e9e9e9e-1234-4abc-8def-0123456789ab";

/// When the second caller acted: later than anything the owner does.
const OTHER_TIME_MS: i64 = i64::MAX / 2;

/// Per-scenario state of the row-facts steps.
#[derive(Default)]
pub struct RowFactsWorld {
    rig: Option<FilesRig>,
    drive: Option<DriveCatalog>,
    folders: Option<Arc<MemoryFolderIndex>>,
    files: HashMap<String, String>,
    answer: Option<Answer>,
    /// The answers of a destination lookup that did not go through the router.
    direct: Option<Value>,
}

impl std::fmt::Debug for RowFactsWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowFactsWorld").finish_non_exhaustive()
    }
}

/// Every node readable but the seeded collection.
struct NoCollection;

impl NavigationAccess for NoCollection {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        *node != NodeRef::Collection(collection())
    }
}

/// The seeded collection's identifier.
fn collection() -> CollectionId {
    CollectionId::new(COLLECTION).expect("collection")
}

/// The seeded library's identifier.
fn library() -> LibraryId {
    LibraryId::new(LIBRARY).expect("library")
}

/// The handle of `node`.
fn handle(node: &NodeRef) -> String {
    node.to_handle().expect("handle")
}

/// The handle of the folder `path` of the seeded collection.
fn folder_node(path: &str) -> String {
    handle(&NodeRef::folder(
        FolderScope::Collection(collection()),
        path,
    ))
}

impl RowFactsWorld {
    /// The rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a library")
    }

    /// The seeded catalog.
    fn drive(&self) -> &DriveCatalog {
        self.drive.as_ref().expect("a library")
    }

    /// The last answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("an answer")
    }

    /// The rows of the last answer.
    fn rows(&self) -> Vec<Value> {
        let answer = self.answer();
        assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
        answer.json()["rows"].as_array().expect("rows").clone()
    }

    /// The row shown as `name` in the last answer.
    fn row(&self, name: &str) -> Value {
        self.rows()
            .into_iter()
            .find(|row| row["display_name"] == name || row["name"] == name)
            .unwrap_or_else(|| panic!("no row {name}"))
    }

    /// A navigation service over the seeded catalog and the folder index, read as `access`
    /// decides.
    fn navigation(&self, access: Arc<dyn NavigationAccess>) -> Arc<NavigationService> {
        Arc::new(NavigationService::new(
            self.drive().catalog.clone() as Arc<dyn LibraryCatalog>,
            self.folders.clone().expect("a folder index"),
            access,
            RepositoryInfo::new(RepositoryId::new(REPOSITORY).expect("repository"), None),
        ))
    }

    /// Upload `bytes` as `name` into `folder` of the collection, a minute after the previous
    /// change, and place it in the catalog; answer its file ID.
    async fn upload(&mut self, name: &str, bytes: &[u8], folder: Option<&str>) -> String {
        let rest = match folder {
            Some(folder) => json!({ "folder": folder, "name_normalization": "nfc" }),
            None => json!({ "name_normalization": "nfc" }),
        };
        let rig = self.rig();
        rig.clock.advance(60_000);
        let answer = rig
            .upload(upload_input(name, bytes.len(), rest), bytes)
            .await;
        assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
        let id = answer.result()["file_id"].as_str().expect("id").to_string();
        self.drive().file_in_collection(&id);
        self.files.insert(name.to_string(), id.clone());
        id
    }

    /// `GET uri` through the router; keep the answer.
    async fn get(&mut self, uri: &str) {
        self.rig().monitor.journal().flush();
        self.answer = Some(self.rig().get(uri).await);
    }

    /// Ask the destination of `node` through the router.
    async fn destination(&mut self, node: &str) {
        let uri = format!("/api/v1/navigation/destination?node={}", encode(node));
        self.get(&uri).await;
    }
}

// ---- Fixtures ----

#[given(
    expr = "row-facts fixture: a library with collection {string} holding {string} at version v2 and {int} files of {int} bytes each in its folder {string}"
)]
async fn fixture(
    w: &mut MonitorWorld,
    title: String,
    name: String,
    count: usize,
    size: usize,
    folder: String,
) {
    assert_eq!(title, COLLECTION_TITLE, "the seeded catalog names it so");
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::new(LIBRARY);
    let folders = Arc::new(MemoryFolderIndex::new());
    w.row_facts.rig = Some(rig);
    w.row_facts.drive = Some(drive);
    w.row_facts.folders = Some(folders.clone());
    let navigation = w.row_facts.navigation(Arc::new(HostAccess));
    assert!(w.row_facts.rig().state.install_navigation(navigation));
    // Uploaded under another spelling and renamed once, so its head is v2.
    let id = w
        .row_facts
        .upload(&name.to_lowercase(), b"1234", None)
        .await;
    w.row_facts.files.insert(name.clone(), id.clone());
    let rig = w.row_facts.rig();
    rig.clock.advance(60_000);
    let renamed = json!({ "library_id": LIBRARY, "file_id": id, "display_name": name });
    assert_eq!(
        rig.command("/api/v1/files/renames", renamed)
            .await
            .status
            .as_u16(),
        200
    );
    for n in 1..=count {
        let file = format!("t{n}.jpg");
        w.row_facts
            .upload(&file, &vec![n as u8; size], Some(&folder))
            .await;
        folders.add_file(
            &FolderScope::Collection(collection()),
            &format!("{folder}/{file}"),
        );
    }
}

#[given(expr = "row-facts fixture: a file {string} registered in no collection")]
async fn loose_file(w: &mut MonitorWorld, name: String) {
    let rig = w.row_facts.rig();
    rig.clock.advance(60_000);
    let input = upload_input(&name, 3, json!({ "name_normalization": "nfc" }));
    let answer = rig.upload(input, b"123").await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    let id = answer.result()["file_id"].as_str().expect("id").to_string();
    w.row_facts.drive().outside_file(&id);
}

#[given(expr = "row-facts fixture: the collection lists more files than one view may count")]
fn too_many(w: &mut MonitorWorld) {
    for n in 0..10_001 {
        let id = format!("0c0c0c0c-1111-4222-8333-{n:012}");
        w.row_facts.drive().file_in_collection(&id);
    }
}

#[given(
    expr = "row-facts fixture: {string} and {string} through {string} are taken in the collection"
)]
fn numbers_taken(w: &mut MonitorWorld, name: String, first: String, last: String) {
    assert_eq!(
        (first.as_str(), last.as_str()),
        ("full-1.txt", "full-9999.txt")
    );
    let home = &w.row_facts.rig().home;
    let placed = |n: u32| FileId::new(format!("0d0d0d0d-1111-4222-8333-{n:012}")).expect("id");
    home.place(&placed(0), &collection(), &name);
    for n in 1..=9_999 {
        home.place(&placed(n), &collection(), &format!("full-{n}.txt"));
    }
}

#[given(
    expr = "row-facts fixture: a deleted file whose collection is gone and every {string} title up to the cap is taken"
)]
async fn untitled_taken(w: &mut MonitorWorld, title: String) {
    assert_eq!(title, UNTITLED_COLLECTION_TITLE);
    let rig = w.row_facts.rig();
    let drive = w.row_facts.drive();
    let service = LibraryService::new(drive.catalog.clone(), rig.monitor.journal())
        .with_settings(Arc::new(MemorySettings::new()));
    assert!(rig.state.install_library_service(Arc::new(service)));
    let deleted = json!({
        "command": "delete_collection", "library_id": LIBRARY, "collection_id": COLLECTION
    });
    let answer = trash_command(w, deleted).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    for n in 1..=MAX_UNTITLED_TITLES {
        let title = match n {
            1 => UNTITLED_COLLECTION_TITLE.to_string(),
            n => format!("{UNTITLED_COLLECTION_TITLE} {n}"),
        };
        let id = CollectionId::new(format!("taken-{n}")).expect("id");
        let taken = Collection::new(id, library(), title).expect("collection");
        let catalog = &w.row_facts.drive().catalog;
        catalog.insert_collection(taken).expect("insert");
    }
}

/// Send the Recently deleted command `input` at the library's current revision.
async fn trash_command(w: &MonitorWorld, input: Value) -> Answer {
    let catalog = &w.row_facts.drive().catalog;
    let revision = catalog
        .library(&library())
        .expect("read")
        .expect("library")
        .revision;
    let body = json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": operation_id(),
            "expected_revision": revision,
            "scope": format!("library:{LIBRARY}"),
        },
        "input": input,
    });
    let uri = "/api/v1/recently-deleted/commands";
    w.row_facts
        .rig()
        .send_command(Method::POST, uri, &body)
        .await
}

#[given(
    expr = "row-facts history fixture: {string} whose v1 content of {int} bytes differs from its head and {string} whose v1 and head share one content"
)]
async fn history_one(w: &mut MonitorWorld, changed: String, size: usize, shared: String) {
    let first = vec![1u8; size];
    let rig = FilesRig::with_files(&[(&changed, &first), (&shared, b"shared")]).await;
    rig.commit(&[(&changed, b"the head content")]).await;
    w.row_facts.rig = Some(rig);
}

#[given(
    expr = "row-facts history fixture: {string} whose v1 and v2 hold one content of {int} bytes and whose head v3 holds another"
)]
async fn history_shared(w: &mut MonitorWorld, name: String, size: usize) {
    let first = vec![2u8; size];
    let rig = FilesRig::with_files(&[(&name, &first)]).await;
    rig.commit(&[("other.txt", b"beside it")]).await;
    rig.commit(&[(&name, b"the head content")]).await;
    w.row_facts.rig = Some(rig);
}

#[given(
    expr = "row-facts caller uploaded {string}, then opened {string}, then renamed {string} to {string}"
)]
async fn three_actions(
    w: &mut MonitorWorld,
    uploaded: String,
    opened: String,
    renamed: String,
    to: String,
) {
    w.row_facts.upload(&uploaded, b"123", None).await;
    let rig = w.row_facts.rig();
    let file = w.row_facts.files.get(&opened).cloned().expect("a file");
    rig.clock.advance(60_000);
    let path = encode(&format!("original/{file}.jpg"));
    let object = encode(&format!("urn:uuid:{file}"));
    let uri = format!("/api/v1/files/download?library_id={LIBRARY}&object_id={object}&path={path}");
    assert_eq!(rig.get(&uri).await.status.as_u16(), 200);
    let file = w.row_facts.files.get(&renamed).cloned().expect("a file");
    rig.clock.advance(60_000);
    let input = json!({ "library_id": LIBRARY, "file_id": file, "display_name": to });
    let answer = rig.command("/api/v1/files/renames", input).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
}

#[given(expr = "row-facts second caller opened {string}")]
fn second_caller(w: &mut MonitorWorld, name: String) {
    let file = w.row_facts.files.get(&name).cloned().expect("a file");
    let mut entry = JournalEntry::new("file.download", Outcome::Succeeded)
        .actor("user:ben")
        .target(LIBRARY)
        .target(format!("urn:uuid:{file}").as_str())
        .target("v9");
    entry.time_ms = OTHER_TIME_MS;
    w.row_facts.rig().journal.append(&[entry]).expect("append");
}

// ---- Reads ----

#[when(expr = "row-facts caller reads the view of the collection")]
async fn view_collection(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Collection(collection()));
    w.row_facts
        .get(&format!("/api/v1/files/view?node={node}"))
        .await;
}

#[when(expr = "row-facts caller reads the view of the library")]
async fn view_library(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Library(library()));
    w.row_facts
        .get(&format!("/api/v1/files/view?node={node}"))
        .await;
}

#[when(expr = "row-facts caller reads the Recent view")]
async fn view_recent(w: &mut MonitorWorld) {
    let repository = RepositoryId::new(REPOSITORY).expect("repository");
    let node = handle(&NodeRef::Repository(repository));
    w.row_facts
        .get(&format!("/api/v1/files/view?node={node}&view=recent"))
        .await;
}

#[when(expr = "row-facts caller searches the repository for {string}")]
async fn search(w: &mut MonitorWorld, q: String) {
    w.row_facts
        .get(&format!("/api/v1/files/search?q={}", encode(&q)))
        .await;
}

#[when(expr = "row-facts caller lists the navigation children of the library")]
async fn children_of_library(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Library(library()));
    w.row_facts
        .get(&format!("/api/v1/navigation?node={node}"))
        .await;
}

#[when(expr = "row-facts caller lists the navigation children of the collection")]
async fn children_of_collection(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Collection(collection()));
    w.row_facts
        .get(&format!("/api/v1/navigation?node={node}"))
        .await;
}

#[when(expr = "row-facts caller reads the library history size")]
async fn history_size(w: &mut MonitorWorld) {
    let uri = format!("/api/v1/files/library-history-size?library_id={LIBRARY}");
    w.row_facts.answer = Some(w.row_facts.rig().get(&uri).await);
}

#[when(expr = "row-facts caller asks the destination of the collection node")]
async fn destination_of_collection(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Collection(collection()));
    w.row_facts.destination(&node).await;
}

#[when(expr = "row-facts caller asks the destination of the folder node {string}")]
async fn destination_of_folder(w: &mut MonitorWorld, path: String) {
    w.row_facts.destination(&folder_node(&path)).await;
}

#[when(expr = "row-facts caller asks the destination of the library node")]
async fn destination_of_library(w: &mut MonitorWorld) {
    let node = handle(&NodeRef::Library(library()));
    w.row_facts.destination(&node).await;
}

#[when(expr = "row-facts caller asks the destination of an unknown node")]
async fn destination_unknown(w: &mut MonitorWorld) {
    let unknown = CollectionId::new(STRANGER).expect("id");
    let node = handle(&NodeRef::Collection(unknown));
    w.row_facts.destination(&node).await;
}

#[when(expr = "row-facts caller asks the destination of a malformed handle")]
async fn destination_malformed(w: &mut MonitorWorld) {
    w.row_facts.destination("n1.not-a-handle").await;
}

#[when(expr = "row-facts caller asks the destination of a folder that no longer exists")]
async fn destination_removed(w: &mut MonitorWorld) {
    let scope = FolderScope::Collection(collection());
    let folders = w.row_facts.folders.clone().expect("a folder index");
    for n in 1..=3 {
        folders.set_ordinary(&scope, &format!("Trips/t{n}.jpg"), false);
    }
    w.row_facts.destination(&folder_node("Trips")).await;
}

#[when(expr = "row-facts caller asks the destination of a node the caller may not read")]
async fn destination_unreadable(w: &mut MonitorWorld) {
    let service = w.row_facts.navigation(Arc::new(NoCollection));
    let node = folder_node("Trips");
    let read = move || service.destination(&Principal::Owner, &node);
    let refused = tokio::task::spawn_blocking(read)
        .await
        .expect("task")
        .expect_err("not readable");
    let body = serde_json::to_value(filesquid_api::ApiError::from(refused)).expect("body");
    w.row_facts.direct = Some(body);
}

// ---- Writes ----

#[when(expr = "row-facts caller uploads with no collection")]
async fn upload_without_collection(w: &mut MonitorWorld) {
    let mut input = upload_input("a.txt", 3, json!({}));
    input
        .as_object_mut()
        .expect("object")
        .remove("collection_id");
    w.row_facts.answer = Some(w.row_facts.rig().upload(input, b"abc").await);
}

#[when(expr = "row-facts caller uploads with an unknown collection")]
async fn upload_to_stranger(w: &mut MonitorWorld) {
    let input = upload_input("a.txt", 3, json!({ "collection_id": STRANGER }));
    w.row_facts.answer = Some(w.row_facts.rig().upload(input, b"abc").await);
}

#[when(expr = "row-facts caller uploads {string} into the collection")]
async fn upload_into_collection(w: &mut MonitorWorld, name: String) {
    let input = upload_input(&name, 3, json!({ "name_normalization": "nfc" }));
    w.row_facts.answer = Some(w.row_facts.rig().upload(input, b"abc").await);
}

#[when(expr = "row-facts caller restores it")]
async fn restore(w: &mut MonitorWorld) {
    let file = w.row_facts.files.get("Beach.jpg").cloned().expect("a file");
    let input = json!({ "command": "restore_files", "library_id": LIBRARY, "file_ids": [file] });
    w.row_facts.answer = Some(trash_command(w, input).await);
}

// ---- Assertions ----

#[then(
    expr = "row-facts row {string} should carry the library ID, head {string}, the collection ID and the title {string}"
)]
fn row_facts(w: &mut MonitorWorld, name: String, head: String, title: String) {
    let row = w.row_facts.row(&name);
    assert_eq!(row["library_id"], LIBRARY, "{row}");
    assert_eq!(row["version"], head.as_str(), "{row}");
    assert_eq!(row["collection_id"], COLLECTION, "{row}");
    assert_eq!(row["collection_title"], title.as_str(), "{row}");
}

#[then(
    expr = "row-facts row {string} should carry a null collection ID and a null collection title"
)]
fn row_without_collection(w: &mut MonitorWorld, name: String) {
    let row = w.row_facts.row(&name);
    assert_eq!(row["collection_id"], Value::Null, "{row}");
    assert_eq!(row["collection_title"], Value::Null, "{row}");
    assert_eq!(row["library_id"], LIBRARY, "{row}");
}

#[then(expr = "row-facts folder row {string} should report {int} files and {int} bytes, complete")]
fn folder_count(w: &mut MonitorWorld, name: String, files: u64, bytes: u64) {
    let row = w.row_facts.row(&name);
    assert_eq!(row["kind"], "folder", "{row}");
    assert_eq!(row["files"], files, "{row}");
    assert_eq!(row["bytes"], bytes, "{row}");
    assert_eq!(row["count_complete"], true, "{row}");
}

#[then(expr = "row-facts folder row {string} should carry the collection ID")]
fn folder_collection(w: &mut MonitorWorld, name: String) {
    assert_eq!(w.row_facts.row(&name)["collection_id"], COLLECTION);
}

#[then(expr = "row-facts folder row {string} should be marked incomplete")]
fn folder_incomplete(w: &mut MonitorWorld, name: String) {
    let row = w.row_facts.row(&name);
    assert_eq!(row["count_complete"], false, "{row}");
}

#[then(expr = "row-facts node {string} should carry the collection ID")]
fn node_collection(w: &mut MonitorWorld, name: String) {
    let page = w.row_facts.answer().json();
    let node = page["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["name"] == name.as_str())
        .unwrap_or_else(|| panic!("no node {name}: {page}"));
    assert_eq!(node["collection_id"], COLLECTION, "{node}");
}

#[then(
    expr = "row-facts destination should be the library ID and the collection ID, with no folder"
)]
fn destination_collection(w: &mut MonitorWorld) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    let expected = json!({ "library_id": LIBRARY, "collection_id": COLLECTION, "folder": null });
    assert_eq!(answer.json(), expected);
}

#[then(
    expr = "row-facts destination should be the library ID, the collection ID and the folder {string}"
)]
fn destination_folder(w: &mut MonitorWorld, folder: String) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    let expected = json!({ "library_id": LIBRARY, "collection_id": COLLECTION, "folder": folder });
    assert_eq!(answer.json(), expected);
}

#[then(
    expr = "row-facts answer should be the same not_found answer as a navigation listing of an unknown node"
)]
async fn same_not_found(w: &mut MonitorWorld) {
    let listing = w
        .row_facts
        .rig()
        .get("/api/v1/navigation?node=n1.not-a-handle")
        .await;
    assert_eq!(listing.status.as_u16(), 404);
    let body = match w.row_facts.direct.take() {
        Some(body) => body,
        None => {
            let answer = w.row_facts.answer();
            assert_eq!(answer.status.as_u16(), 404, "{:?}", answer.json());
            answer.json()
        }
    };
    assert_eq!(body, listing.json(), "one identical answer");
}

#[then(expr = "row-facts answer should be refused as invalid_request")]
fn refused_invalid(w: &mut MonitorWorld) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), 400, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], "invalid_request");
}

#[then(expr = "row-facts answer should be HTTP {int} with sub-code {string}")]
fn sub_code(w: &mut MonitorWorld, status: u16, code: String) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(
        answer.json()["sub_code"],
        code.as_str(),
        "{:?}",
        answer.json()
    );
}

#[then(
    expr = "row-facts first rows should be {string} changed, {string} opened and {string} uploaded, in that order"
)]
fn recent_rows(w: &mut MonitorWorld, changed: String, opened: String, uploaded: String) {
    let rows = w.row_facts.rows();
    let shown: Vec<(String, String)> = rows
        .iter()
        .take(3)
        .map(|row| {
            (
                row["display_name"].as_str().expect("name").to_string(),
                row["recent"]["action"]
                    .as_str()
                    .expect("action")
                    .to_string(),
            )
        })
        .collect();
    let expected = [
        (changed, "changed".to_string()),
        (opened, "opened".to_string()),
        (uploaded, "uploaded".to_string()),
    ];
    assert_eq!(shown, expected);
}

#[then(expr = "row-facts rows should not include the second caller's action")]
fn no_second_caller(w: &mut MonitorWorld) {
    let other = format_rfc3339_ms(OTHER_TIME_MS);
    let rows = w.row_facts.rows();
    assert!(!rows.is_empty(), "my own actions are listed");
    for row in &rows {
        assert_ne!(row["recent"]["at"], other.as_str(), "{row}");
        assert_ne!(row["recent"]["action"], "opened", "{row}");
        assert_eq!(row["opened_by_me"], Value::Null, "{row}");
    }
}

#[then(expr = "row-facts history bytes should be {int}")]
fn history_bytes(w: &mut MonitorWorld, bytes: u64) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    assert_eq!(answer.json()["history_bytes"], bytes, "{:?}", answer.json());
    assert_eq!(answer.json()["truncated"], false);
}

#[then(expr = "row-facts answer should name the path {string}")]
fn answered_path(w: &mut MonitorWorld, path: String) {
    let answer = w.row_facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    assert_eq!(answer.result()["path"], path.as_str());
    assert_eq!(answer.result()["collection_id"], COLLECTION);
}

#[then(expr = "row-facts snapshot registration of the upload should record the path {string}")]
fn registered_path(w: &mut MonitorWorld, path: String) {
    let id = w.row_facts.answer().result()["file_id"]
        .as_str()
        .expect("id")
        .to_string();
    let placed = w
        .row_facts
        .rig()
        .home
        .placed(&FileId::new(id).expect("file id"));
    assert_eq!(placed, Some((collection(), path)));
}

#[then(expr = "row-facts journal should hold opaque IDs only")]
fn journal_is_opaque(w: &mut MonitorWorld) {
    let rig = w.row_facts.rig();
    rig.monitor.journal().flush();
    let journal = format!("{:?}{:?}", rig.journal(), rig.home.journal());
    for word in [
        "beach",
        "Beach",
        "Trips",
        COLLECTION_TITLE,
        "t1.jpg",
        "original/",
    ] {
        assert!(!journal.contains(word), "{word} was journaled");
    }
    let uploads = rig.journaled("file.upload");
    assert!(!uploads.is_empty(), "the upload is journaled");
}
