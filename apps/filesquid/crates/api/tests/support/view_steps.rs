//! Step definitions for `view_copy_and_storage_use_API-S-011.feature` (Task API-F2.2, part 2):
//! copy, the view operation by node with stable paging and the caller-only "by me" keys, and
//! storage use per collection, through the router over a real OCFL storage root on the local
//! filesystem and a seeded library catalog.

use crate::drive_catalog::{DriveCatalog, COLLECTION, COLLECTION_TITLE, VIRTUAL_TITLE};
use crate::files_fixture::{encode, upload_facts, Answer, FilesRig, HOME_COLLECTION, LIBRARY};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::journal::{JournalEntry, JournalStore, Outcome};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Per-scenario state of the view steps.
#[derive(Default)]
pub struct ViewWorld {
    rig: Option<FilesRig>,
    drive: Option<DriveCatalog>,
    files: HashMap<String, String>,
    rows: Vec<String>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for ViewWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewWorld").finish_non_exhaustive()
    }
}

impl ViewWorld {
    /// The rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a library")
    }

    /// The seeded catalog.
    fn drive(&self) -> &DriveCatalog {
        self.drive.as_ref().expect("a library")
    }

    /// The file ID of the file uploaded as `name`.
    fn id(&self, name: &str) -> String {
        self.files.get(name).cloned().expect("an uploaded file")
    }

    /// The last answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("an answer")
    }

    /// Upload `bytes` as `name`, a minute after the previous upload.
    async fn upload(&mut self, name: &str, bytes: &[u8]) -> String {
        let rig = self.rig();
        rig.clock.advance(60_000);
        let input = upload_facts(name, bytes.len(), "name_normalization=nfc");
        let answer = rig.upload(input, bytes).await;
        assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
        let id = answer.result()["file_id"].as_str().expect("id").to_string();
        self.files.insert(name.to_string(), id.clone());
        id
    }

    /// Every row name of `node` sorted by `sort` in `direction`, one row per page.
    async fn page_through(&mut self, node: &str, sort: &str, direction: &str) {
        let mut rows = Vec::new();
        let mut cursor = String::new();
        for _ in 0..20 {
            let uri = format!(
                "/api/v1/files/view?node={node}&sort={sort}&direction={direction}&limit=1{cursor}"
            );
            let page = self.rig().get(&uri).await;
            assert_eq!(page.status.as_u16(), 200, "{:?}", page.json());
            let page = page.json();
            rows.extend(row_names(&page));
            match page["next_cursor"].as_str() {
                Some(next) => cursor = format!("&cursor={next}"),
                None => break,
            }
        }
        self.rows = rows;
    }
}

/// The names of a view page's rows.
fn row_names(page: &Value) -> Vec<String> {
    page["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            let name = row.get("display_name").unwrap_or(&row["name"]);
            name.as_str().expect("name").to_string()
        })
        .collect()
}

#[given(expr = "a library with the collection {string} and the virtual collection {string}")]
async fn library(w: &mut MonitorWorld, collection: String, virtual_collection: String) {
    assert_eq!(
        (collection.as_str(), virtual_collection.as_str()),
        (COLLECTION_TITLE, VIRTUAL_TITLE),
        "the seeded catalog names its collections so"
    );
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::new(LIBRARY);
    assert!(rig.state.install_navigation(drive.navigation.clone()));
    w.view.rig = Some(rig);
    w.view.drive = Some(drive);
}

#[given(
    expr = "the files {string} of {int} bytes, {string} of {int} bytes and {string} of {int} bytes in {string}"
)]
#[allow(clippy::too_many_arguments)]
async fn files_in_collection(
    w: &mut MonitorWorld,
    a: String,
    a_len: usize,
    b: String,
    b_len: usize,
    c: String,
    c_len: usize,
    collection: String,
) {
    assert_eq!(collection, COLLECTION_TITLE, "the seeded collection");
    for (name, len) in [(a, a_len), (b, b_len), (c, c_len)] {
        let id = w.view.upload(&name, &vec![7u8; len]).await;
        w.view.drive().file_in_collection(&id);
    }
}

#[given(expr = "the file {string} in another collection {string}")]
async fn in_other_collection(w: &mut MonitorWorld, name: String, title: String) {
    let id = w.view.upload(&name, b"z").await;
    let collection = "4e2f3a4b-5c6d-4e7f-8a9b-0c1d2e3f4a5b";
    w.view
        .drive()
        .file_in_new_collection(&id, collection, &title);
}

#[given(expr = "{string} and {string} are in {string}")]
async fn in_virtual(w: &mut MonitorWorld, a: String, b: String, virtual_collection: String) {
    assert_eq!(
        virtual_collection, VIRTUAL_TITLE,
        "the seeded virtual collection"
    );
    for name in [a, b] {
        let id = w.view.id(&name);
        w.view.drive().add_to_virtual(&id);
    }
}

#[given(expr = "the owner downloaded {string}")]
async fn downloaded(w: &mut MonitorWorld, name: String) {
    let id = w.view.id(&name);
    let ext = name.rsplit('.').next().expect("extension");
    let path = encode(&format!("original/{id}.{ext}"));
    let object = encode(&format!("urn:uuid:{id}"));
    let uri = format!("/api/v1/files/download?library_id={LIBRARY}&object_id={object}&path={path}");
    let answer = w.view.rig().get(&uri).await;
    assert_eq!(answer.status.as_u16(), 200);
}

#[given(expr = "another user opened {string} later")]
async fn another_user_opened(w: &mut MonitorWorld, name: String) {
    let object = format!("urn:uuid:{}", w.view.id(&name));
    let mut entry = JournalEntry::new("file.download", Outcome::Succeeded)
        .actor("user:ben")
        .target(LIBRARY)
        .target(object.as_str())
        .target("v1");
    entry.time_ms = i64::MAX / 2;
    w.view.rig().journal.append(&[entry]).expect("append");
    w.view.rig().monitor.journal().flush();
}

#[given(expr = "{string} was deleted")]
async fn deleted(w: &mut MonitorWorld, name: String) {
    let id = w.view.id(&name);
    w.view.drive().delete(&id);
}

#[when(expr = "the owner copies {string}")]
async fn copies(w: &mut MonitorWorld, name: String) {
    let body = json!({
        "library_id": LIBRARY, "file_id": w.view.id(&name), "collection_id": HOME_COLLECTION
    });
    let answer = w.view.rig().command("/api/v1/files/copies", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    w.view.answer = Some(answer);
}

#[when(expr = "the owner pages through {string} by {string} {string} one row at a time")]
async fn pages_collection(w: &mut MonitorWorld, collection: String, sort: String, dir: String) {
    assert_eq!(collection, COLLECTION_TITLE, "the seeded collection");
    let node = w.view.drive().collection_node();
    w.view.page_through(&node, &sort, &dir).await;
}

#[when(expr = "the owner pages through the library by {string} {string} one row at a time")]
async fn pages_library(w: &mut MonitorWorld, sort: String, direction: String) {
    let node = w.view.drive().library_node();
    w.view.page_through(&node, &sort, &direction).await;
}

#[when(
    expr = "the owner pages through the virtual collection by {string} {string} one row at a time"
)]
async fn pages_virtual(w: &mut MonitorWorld, sort: String, direction: String) {
    let node = w.view.drive().virtual_node();
    w.view.page_through(&node, &sort, &direction).await;
}

#[when(expr = "the owner asks for the storage use of {string}")]
async fn storage_use(w: &mut MonitorWorld, collection: String) {
    assert_eq!(collection, COLLECTION_TITLE, "the seeded collection");
    let uri = format!("/api/v1/files/collection-history-size?collection_id={COLLECTION}");
    w.view.answer = Some(w.view.rig().get(&uri).await);
}

#[when(expr = "the owner opens the {string} view of the library")]
async fn opens_view(w: &mut MonitorWorld, view: String) {
    let node = w.view.drive().library_node();
    let uri = format!("/api/v1/files/view?node={node}&view={view}");
    w.view.answer = Some(w.view.rig().get(&uri).await);
}

#[then(expr = "the copy should have a new file identifier and be shown as {string}")]
async fn copy_shown(w: &mut MonitorWorld, name: String) {
    let copy = w.view.answer().result();
    let id = copy["file_id"].as_str().expect("id");
    assert!(
        !w.view.files.values().any(|known| known == id),
        "a new FileId"
    );
    let object = encode(&format!("urn:uuid:{id}"));
    let uri =
        format!("/api/v1/files/listing?library_id={LIBRARY}&object_id={object}&prefix=original");
    let listing = w.view.rig().get(&uri).await.json();
    assert_eq!(listing["entries"][0]["display_name"], name);
}

#[then("the copy's record should name the source as dcterms:source")]
async fn copy_source(w: &mut MonitorWorld) {
    let copy = w.view.answer().result();
    let id = copy["file_id"].as_str().expect("id");
    let uri = format!("/api/v1/files/metadata?library_id={LIBRARY}&file_id={id}");
    let statements = w.view.rig().get(&uri).await.json();
    let named = statements["statements"]
        .as_array()
        .expect("statements")
        .iter()
        .any(|s| s["curie"] == "dcterms:source" && s["value"] == copy["source"]);
    assert!(named, "{statements}");
}

#[then(expr = "the copy should be journaled as {string} with opaque IDs only")]
async fn copy_journaled(w: &mut MonitorWorld, operation: String) {
    let entries = w.view.rig().journaled(&operation);
    assert_eq!(entries.len(), 1);
    assert!(entries[0]
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id)));
}

#[then(expr = "the rows should be {string}")]
async fn rows_are(w: &mut MonitorWorld, expected: String) {
    let expected: Vec<String> = expected.split(", ").map(str::to_string).collect();
    assert_eq!(w.view.rows, expected);
}

#[then(expr = "it should count {int} files and {int} contents")]
async fn counts(w: &mut MonitorWorld, files: u64, contents: u64) {
    let size = w.view.answer().json();
    assert_eq!(size["files"], files, "{size}");
    assert_eq!(size["contents"], contents, "{size}");
    assert_eq!(size["truncated"], false);
}

#[then(expr = "the view should be refused as {string}")]
async fn refused(w: &mut MonitorWorld, code: String) {
    let answer = w.view.answer();
    assert_eq!(answer.status.as_u16(), 400, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code);
}
