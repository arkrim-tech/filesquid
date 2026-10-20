//! Step definitions for `display_and_download_names_API-S-027.feature` (Task API-F1.13):
//! display names on history and versions, a rename that changes only the display name, the
//! refused move of a stored original, download names and the sharing default, through the
//! router over a real OCFL storage root on the local filesystem.

use crate::files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use crate::MonitorWorld;
use axum::http::Method;
use cucumber::{given, then, when};
use filesquid_api::canonical::{outgoing_name, OutgoingNames};
use filesquid_core::FileId;
use filesquid_metadata::file_record::FileRecord;
use filesquid_ocfl::read::Object;
use serde_json::{json, Value};

/// Per-scenario state of the name steps.
#[derive(Default)]
pub struct NamesWorld {
    rig: Option<FilesRig>,
    upload: Option<Value>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for NamesWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamesWorld").finish_non_exhaustive()
    }
}

impl NamesWorld {
    /// The rig, created on first use.
    async fn rig(&mut self) -> &FilesRig {
        if self.rig.is_none() {
            self.rig = Some(FilesRig::new().await);
        }
        self.rig.as_ref().expect("rig")
    }

    /// The uploaded file's identifier.
    fn file_id(&self) -> String {
        let upload = self.upload.as_ref().expect("an uploaded file");
        upload["file_id"].as_str().expect("file id").to_string()
    }

    /// The query naming the uploaded file's object, then `rest`.
    fn object_query(&self, rest: &str) -> String {
        let upload = self.upload.as_ref().expect("an uploaded file");
        let object = upload["object_id"].as_str().expect("object");
        format!("library_id={LIBRARY}&object_id={}&{rest}", encode(object))
    }

    /// The stored path of the uploaded original (every scenario's name ends in `.jpg`,
    /// `.JPG` or `.txt`).
    fn original(&self, ext: &str) -> String {
        format!("original/{}.{ext}", self.file_id())
    }

    /// The last answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("answer")
    }
}

/// GET `uri` and answer its JSON body.
async fn get_json(w: &mut NamesWorld, uri: &str) -> Value {
    let answer = w.rig().await.get(uri).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    answer.json()
}

/// The `Content-Disposition` of the download of the uploaded `.jpg` original.
async fn download_header(w: &mut NamesWorld) -> String {
    let rest = format!("path={}", encode(&w.original("jpg")));
    let uri = format!("/api/v1/files/download?{}", w.object_query(&rest));
    let answer = w.rig().await.get(&uri).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    answer.header("content-disposition").to_string()
}

/// The metadata record at the head of the uploaded file's object.
async fn head_record(w: &mut NamesWorld) -> FileRecord {
    let id = w.file_id();
    let rig = w.rig().await;
    let object = Object::open(rig.root.as_ref(), &format!("files/{id}"))
        .await
        .expect("object");
    let path = format!("original/{id}.metadata.md");
    let size = filesquid_ocfl::view::file_details(&object, object.head(), &path)
        .await
        .expect("record")
        .size;
    let bytes = filesquid_ocfl::view::read_range(&object, object.head(), &path, 0, size)
        .await
        .expect("bytes");
    FileRecord::from_markdown(&String::from_utf8(bytes).expect("utf-8")).expect("record")
}

#[given(expr = "the owner has uploaded {string}")]
async fn uploaded(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let uri = upload_uri(&name, 4, "name_normalization=nfc");
    let answer = w
        .rig()
        .await
        .call(Method::PUT, &uri, &[], b"data".to_vec())
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    w.upload = Some(answer.json());
}

#[when(expr = "the owner renames the file to {string}")]
async fn renames(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let name = name.replace("\\\"", "\"").replace("\\\\", "\\");
    let body = json!({ "library_id": LIBRARY, "file_id": w.file_id(), "display_name": name });
    let answer = w.rig().await.post("/api/v1/files/renames", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    w.answer = Some(answer);
}

#[when(expr = "the owner moves the file's original to {string}")]
async fn moves_original(w: &mut MonitorWorld, to: String) {
    let w = &mut w.names;
    let upload = w.upload.clone().expect("an uploaded file");
    let body = json!({
        "library_id": LIBRARY,
        "object_id": upload["object_id"],
        "from": w.original("txt"),
        "to": to,
    });
    w.answer = Some(w.rig().await.post("/api/v1/files/moves", body).await);
}

#[then(expr = "the file's history should name it {string}")]
async fn history_names(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let rest = format!("path={}", encode(&w.original("jpg")));
    let uri = format!("/api/v1/files/history?{}", w.object_query(&rest));
    assert_eq!(get_json(w, &uri).await["display_name"], name);
}

#[then(expr = "the file's versions should name it {string}")]
async fn versions_name(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let uri = format!("/api/v1/files/versions?{}", w.object_query(""));
    assert_eq!(get_json(w, &uri).await["display_name"], name);
}

#[then(expr = "the rename should publish version {string}")]
async fn rename_version(w: &mut MonitorWorld, version: String) {
    assert_eq!(w.names.answer().json()["version"], version);
}

#[then(expr = "the listing should show {string} at the same stored path")]
async fn listing_shows(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let uri = format!(
        "/api/v1/files/listing?{}",
        w.object_query("prefix=original")
    );
    let listing = get_json(w, &uri).await;
    assert_eq!(listing["entries"][0]["path"], w.original("JPG"));
    assert_eq!(listing["entries"][0]["display_name"], name);
}

#[then(expr = "the facts should still hold the original name {string}")]
async fn facts_keep(w: &mut MonitorWorld, name: String) {
    let w = &mut w.names;
    let uri = format!(
        "/api/v1/files/facts?library_id={LIBRARY}&file_id={}",
        w.file_id()
    );
    let facts = get_json(w, &uri).await;
    assert!(facts.to_string().contains(&name), "{facts}");
}

#[then(expr = "the rename should be journaled as {string} with opaque IDs only")]
async fn rename_journaled(w: &mut MonitorWorld, operation: String) {
    let entries = w.names.rig().await.journaled(&operation);
    assert_eq!(entries.len(), 1);
    assert!(entries[0]
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id)));
}

#[then("the move should be refused with a pointer to the rename route")]
async fn move_refused(w: &mut MonitorWorld) {
    let answer = w.names.answer();
    assert_eq!(answer.status.as_u16(), 400, "{:?}", answer.json());
    let message = answer.json()["message"].as_str().unwrap_or("").to_string();
    assert!(message.contains("/api/v1/files/renames"), "{message}");
}

#[then(expr = "the download should be named {string}")]
async fn download_named(w: &mut MonitorWorld, expected: String) {
    let expected = expected.replace("\\\"", "\"");
    assert_eq!(download_header(&mut w.names).await, expected);
}

#[then(expr = "the download fallback should be the file identifier plus {string}")]
async fn download_fallback(w: &mut MonitorWorld, ext: String) {
    let header = download_header(&mut w.names).await;
    let fallback = format!("filename=\"{}{ext}\";", w.names.file_id());
    assert!(header.contains(&fallback), "{header}");
}

#[then(expr = "the default outgoing name should be the file identifier plus {string}")]
async fn default_outgoing(w: &mut MonitorWorld, ext: String) {
    let record = head_record(&mut w.names).await;
    let id = FileId::new(w.names.file_id()).expect("file id");
    let name = outgoing_name(&id, &record, OutgoingNames::default()).expect("name");
    assert_eq!(name, format!("{}{ext}", id.as_str()));
}

#[then(expr = "the outgoing name under the original-name policy should be {string}")]
async fn original_outgoing(w: &mut MonitorWorld, expected: String) {
    let record = head_record(&mut w.names).await;
    let id = FileId::new(w.names.file_id()).expect("file id");
    let name = outgoing_name(&id, &record, OutgoingNames::OriginalName).expect("name");
    assert_eq!(name, expected);
}
