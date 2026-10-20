//! Step definitions for `metadata_property_routes_API-S-034.feature` (Task API-F1.4): uploads
//! that create a file's own object with its record, statement edits, namespace registration,
//! term placement and the facts projection, through the router over a real OCFL storage root
//! on the local filesystem.

use crate::files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use crate::MonitorWorld;
use axum::http::Method;
use cucumber::{given, then, when};
use filesquid_ocfl::read::Object;
use serde_json::{json, Value};

/// Per-scenario state of the metadata steps.
#[derive(Default)]
pub struct MetadataWorld {
    rig: Option<FilesRig>,
    upload: Option<Value>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for MetadataWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataWorld").finish_non_exhaustive()
    }
}

impl MetadataWorld {
    /// The rig, created on first use.
    async fn rig(&mut self) -> &FilesRig {
        if self.rig.is_none() {
            self.rig = Some(FilesRig::new().await);
        }
        self.rig.as_ref().expect("rig")
    }

    fn file_id(&self) -> String {
        let upload = self.upload.as_ref().expect("an uploaded file");
        upload["file_id"].as_str().expect("file id").to_string()
    }

    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("answer")
    }
}

/// Upload `bytes` as a new file named `name` with `rest`.
async fn upload(w: &mut MetadataWorld, name: &str, bytes: &[u8], rest: &str) {
    let uri = upload_uri(name, bytes.len(), rest);
    let answer = w
        .rig()
        .await
        .call(Method::PUT, &uri, &[], bytes.to_vec())
        .await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    w.upload = Some(answer.json());
    w.answer = Some(answer);
}

/// The head version of the uploaded file's object and its logical paths.
async fn head(w: &mut MetadataWorld) -> (String, Vec<String>) {
    let id = w.file_id();
    let rig = w.rig().await;
    let object = Object::open(rig.root.as_ref(), &format!("files/{id}"))
        .await
        .expect("file object");
    let state = object.state(object.head()).expect("state");
    let paths = state.paths().map(str::to_string).collect();
    (object.head().to_string(), paths)
}

#[when(expr = "the owner uploads {string} as a new file")]
async fn upload_new(w: &mut MonitorWorld, name: String) {
    let rest = "name_normalization=nfc";
    upload(&mut w.metadata, &name, b"jpeg bytes", rest).await;
}

#[given(expr = "the owner uploaded {string}")]
async fn uploaded(w: &mut MonitorWorld, name: String) {
    upload(&mut w.metadata, &name, b"hello", "").await;
}

#[given(expr = "the owner uploaded {string} as {string}")]
async fn uploaded_as(w: &mut MonitorWorld, name: String, media_type: String) {
    let rest = format!("media_type={}&name_normalization=nfc", encode(&media_type));
    upload(&mut w.metadata, &name, b"hello", &rest).await;
}

#[when(expr = "the owner uploads {string} declaring {int} bytes but sending {int}")]
async fn upload_mismatch(w: &mut MonitorWorld, name: String, declared: usize, sent: usize) {
    let uri = upload_uri(&name, declared, "");
    let rig = w.metadata.rig().await;
    let answer = rig.call(Method::PUT, &uri, &[], vec![b'x'; sent]).await;
    w.metadata.answer = Some(answer);
}

#[then(
    expr = "the file's object should hold the original and its metadata record in version {string}"
)]
async fn object_holds(w: &mut MonitorWorld, version: String) {
    let id = w.metadata.file_id();
    let (head, paths) = head(&mut w.metadata).await;
    assert_eq!(head, version);
    assert_eq!(
        paths,
        [
            format!("original/{id}.JPG"),
            format!("original/{id}.metadata.md")
        ]
    );
}

#[then(
    expr = "the record should name {string} with its upload facts and every other fact unavailable"
)]
async fn record_names(w: &mut MonitorWorld, name: String) {
    let id = w.metadata.file_id();
    let rig = w.metadata.rig().await;
    let facts = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={id}"
        ))
        .await
        .json();
    assert_eq!(facts["facts"]["original_name"], name);
    assert_eq!(facts["facts"]["imports"][0]["method"], "api_upload");
    let object = Object::open(rig.root.as_ref(), &format!("files/{id}"))
        .await
        .expect("object");
    let path = format!("original/{id}.metadata.md");
    let size = filesquid_ocfl::view::file_details(&object, object.head(), &path)
        .await
        .expect("record")
        .size;
    let text = filesquid_ocfl::view::read_range(&object, object.head(), &path, 0, size)
        .await
        .expect("bytes");
    assert!(String::from_utf8(text)
        .expect("utf-8")
        .contains("not_provided_by_upload"));
}

#[when(expr = "the owner sets {string} to {string}")]
async fn set(w: &mut MonitorWorld, curie: String, value: String) {
    let body = json!({
        "library_id": LIBRARY, "file_id": w.metadata.file_id(), "curie": curie, "value": value
    });
    let headers = [("content-type", "application/json")];
    let rig = w.metadata.rig().await;
    let uri = "/api/v1/files/metadata/statements";
    let answer = rig
        .call(Method::PUT, uri, &headers, body.to_string().into_bytes())
        .await;
    w.metadata.answer = Some(answer);
}

#[when(expr = "the owner removes {string}")]
async fn remove(w: &mut MonitorWorld, curie: String) {
    let body = json!({ "library_id": LIBRARY, "file_id": w.metadata.file_id(), "curie": curie });
    let rig = w.metadata.rig().await;
    let answer = rig.post("/api/v1/files/metadata/removals", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    w.metadata.answer = Some(answer);
}

#[then(expr = "the file should show only {string} as {string}")]
async fn shows_only(w: &mut MonitorWorld, curie: String, value: String) {
    let id = w.metadata.file_id();
    let rig = w.metadata.rig().await;
    let view = rig
        .get(&format!(
            "/api/v1/files/metadata?library_id={LIBRARY}&file_id={id}"
        ))
        .await
        .json();
    let statements = view["statements"].as_array().expect("statements");
    assert_eq!(statements.len(), 1, "{view}");
    assert_eq!(statements[0]["curie"], curie);
    assert_eq!(statements[0]["value"], value);
}

#[then(expr = "the file's object should be at version {string}")]
async fn at_version(w: &mut MonitorWorld, version: String) {
    assert_eq!(head(&mut w.metadata).await.0, version);
}

#[then(expr = "the metadata answer should be the error {string}")]
fn metadata_error(w: &mut MonitorWorld, code: String) {
    let answer = w.metadata.answer();
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// Register `prefix` with `term` in the rig's library.
async fn register(w: &mut MetadataWorld, prefix: &str, term: &str) {
    let body = json!({
        "library_id": LIBRARY,
        "prefix": prefix,
        "iri": format!("https://example.org/{prefix}#"),
        "kind": "extension",
        "terms": [term],
    });
    let rig = w.rig().await;
    let answer = rig.post("/api/v1/files/metadata/registrations", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
}

#[when(expr = "the owner registers the namespace {string} with the term {string}")]
async fn registers(w: &mut MonitorWorld, prefix: String, term: String) {
    register(&mut w.metadata, &prefix, &term).await;
}

#[given(expr = "the owner registered the namespace {string} with the term {string}")]
async fn registered(w: &mut MonitorWorld, prefix: String, term: String) {
    register(&mut w.metadata, &prefix, &term).await;
}

#[when(expr = "the owner asks where {string} can go")]
async fn asks(w: &mut MonitorWorld, name: String) {
    let rig = w.metadata.rig().await;
    let uri = format!("/api/v1/files/metadata/terms?library_id={LIBRARY}&name={name}");
    let answer = rig.get(&uri).await;
    w.metadata.answer = Some(answer);
}

#[then(expr = "the answer should offer {string}, {string} and {string} in that order")]
fn offers(w: &mut MonitorWorld, a: String, b: String, c: String) {
    let view = w.metadata.answer().json();
    let curies: Vec<&str> = view["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .map(|c| c["curie"].as_str().expect("curie"))
        .collect();
    assert_eq!(curies, [a, b, c]);
}

#[when(expr = "the owner reads the file's facts")]
async fn reads_facts(w: &mut MonitorWorld) {
    let id = w.metadata.file_id();
    let rig = w.metadata.rig().await;
    let answer = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={id}"
        ))
        .await;
    w.metadata.answer = Some(answer);
}

#[then(expr = "the facts should name {string} with its size and method {string}")]
fn facts_name(w: &mut MonitorWorld, name: String, method: String) {
    let facts = &w.metadata.answer().json()["facts"];
    assert_eq!(facts["display_name"], name);
    assert_eq!(facts["size"], 5);
    assert_eq!(facts["imports"][0]["method"], method);
}

#[then(expr = "the facts should hold no absolute path, import root, host or account name")]
fn facts_private(w: &mut MonitorWorld) {
    let text = w.metadata.answer().json().to_string();
    for private in ["absolute_path", "import_root", "host_name", "account"] {
        assert!(!text.contains(private), "{private}");
    }
}

#[when(expr = "the owner reads the file's original")]
async fn reads_original(w: &mut MonitorWorld) {
    let id = w.metadata.file_id();
    let rig = w.metadata.rig().await;
    let object = encode(&format!("urn:uuid:{id}"));
    let path = encode(&format!("original/{id}.md"));
    let uri = format!("/api/v1/files/content?library_id={LIBRARY}&object_id={object}&path={path}");
    let answer = rig.get(&uri).await;
    w.metadata.answer = Some(answer);
}

#[then(expr = "the answer should carry the media type {string} and the name {string}")]
fn carries(w: &mut MonitorWorld, media_type: String, name: String) {
    let answer = w.metadata.answer();
    assert_eq!(answer.header("content-type"), media_type);
    let encoded = encode(&name).replace("%2E", ".");
    assert!(
        answer.header("content-disposition").contains(&encoded),
        "{}",
        answer.header("content-disposition")
    );
}
