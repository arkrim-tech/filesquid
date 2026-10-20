//! Integration tests of Task API-F1.4 (API-S-026, API-S-027, API-S-018, MD-S-013) through the
//! router, against a real OCFL storage root on the local filesystem: an upload creates the
//! file's own object with its original and metadata record in one version, a later upload
//! appends an import entry, and the content, download and listing answers take the display
//! name and media type from the record.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use filesquid_metadata::file_record::FileRecord;
use filesquid_ocfl::read::Object;
use serde_json::Value;

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

/// Upload `bytes` as a new file named `name` with `rest`, and answer the upload view.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8], rest: &str) -> Value {
    let uri = upload_uri(name, bytes.len(), rest);
    let answer = rig.call(Method::PUT, &uri, &[], bytes.to_vec()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()
}

/// The query naming the file object of `upload` and `rest`.
fn object_query(upload: &Value, rest: &str) -> String {
    let object = upload["object_id"].as_str().expect("object");
    format!("library_id={LIBRARY}&object_id={}&{rest}", encode(object))
}

/// The record at the head of the file object of `upload`, read from the storage root.
async fn head_record(rig: &FilesRig, upload: &Value) -> (Vec<String>, FileRecord) {
    let id = upload["file_id"].as_str().expect("file id");
    let object = Object::open(rig.root.as_ref(), &format!("files/{id}"))
        .await
        .expect("file object");
    let state = object.state(object.head()).expect("state");
    let paths = state.paths().map(str::to_string).collect();
    let record_path = format!("original/{id}.metadata.md");
    let size = filesquid_ocfl::view::file_details(&object, object.head(), &record_path)
        .await
        .expect("record")
        .size;
    let bytes = filesquid_ocfl::view::read_range(&object, object.head(), &record_path, 0, size)
        .await
        .expect("record bytes");
    let record = FileRecord::from_markdown(&String::from_utf8(bytes).expect("utf-8"));
    (paths, record.expect("a file record"))
}

#[tokio::test]
async fn an_upload_creates_the_file_object_with_its_original_and_record_in_one_version() {
    let rig = FilesRig::new().await;
    let rest = "media_type=image%2Fjpeg&last_modified=2026-10-01T08%3A00%3A00Z";
    let upload = upload(&rig, "Holiday 2023.JPG", b"jpeg bytes", rest).await;
    let id = upload["file_id"].as_str().expect("file id").to_string();
    assert_eq!(upload["object_id"], format!("urn:uuid:{id}"));
    assert_eq!(upload["version"], "v1");
    let (paths, record) = head_record(&rig, &upload).await;
    assert_eq!(
        paths,
        [
            format!("original/{id}.JPG"),
            format!("original/{id}.metadata.md")
        ]
    );
    assert_eq!(record.display_name(), "Holiday 2023.JPG");
    assert_eq!(record.extension(), Some("JPG"));
    assert_eq!(record.media_type(), Some("image/jpeg"));
    let entry = &record.imports()[0];
    assert_eq!(entry.acquisition.size, 10);
    assert_eq!(entry.source.verified_size, 10);
    assert_eq!(entry.acquisition.object_version, "v1");
    let text = serde_json::to_string(entry).expect("json");
    assert!(
        text.contains("not_provided_by_upload"),
        "missing facts are listed"
    );
    assert!(text.contains("2026-10-01T08:00:00Z"));
}

#[tokio::test]
async fn an_upload_whose_declared_size_differs_fails_verification_and_commits_nothing() {
    let rig = FilesRig::new().await;
    let uri = upload_uri("a.txt", 4, "");
    let answer = rig.call(Method::PUT, &uri, &[], b"three".to_vec()).await;
    assert_error(&answer, 400, "invalid_request");
    assert!(!rig.dir.join("root").join("files").exists());
}

#[tokio::test]
async fn a_name_that_is_not_ascii_needs_its_normalisation_form() {
    let rig = FilesRig::new().await;
    let uri = upload_uri("Été.txt", 2, "");
    let answer = rig.call(Method::PUT, &uri, &[], b"ab".to_vec()).await;
    assert_error(&answer, 400, "invalid_request");
    let upload = upload(&rig, "Été.txt", b"ab", "name_normalization=nfc").await;
    let (_, record) = head_record(&rig, &upload).await;
    assert_eq!(record.display_name(), "Été.txt");
}

#[tokio::test]
async fn a_later_upload_of_the_same_file_is_a_new_version_with_one_more_import_entry() {
    let rig = FilesRig::new().await;
    let first = upload(&rig, "report.pdf", b"first", "").await;
    let id = first["file_id"].as_str().expect("file id");
    let again = upload(
        &rig,
        "report (final).pdf",
        b"second!",
        &format!("file_id={id}"),
    )
    .await;
    assert_eq!(again["file_id"], id);
    assert_eq!(again["version"], "v2");
    let (paths, record) = head_record(&rig, &again).await;
    assert_eq!(paths.len(), 2, "the same stored path: {paths:?}");
    assert_eq!(record.imports().len(), 2);
    assert_eq!(
        record.display_name(),
        "report.pdf",
        "the display name stays"
    );
    assert_eq!(record.imports()[1].acquisition.object_version, "v2");
    let unknown = upload_uri("x.pdf", 1, "file_id=0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19");
    let answer = rig.call(Method::PUT, &unknown, &[], b"x".to_vec()).await;
    assert_error(&answer, 404, "not_found");
}

#[tokio::test]
async fn content_download_and_listing_take_the_name_and_media_type_from_the_record() {
    let rig = FilesRig::new().await;
    let upload = upload(
        &rig,
        "Café notes.TXT",
        b"hello",
        "media_type=text%2Fmarkdown&name_normalization=nfc",
    )
    .await;
    let id = upload["file_id"].as_str().expect("file id");
    let original = encode(&format!("original/{id}.TXT"));
    let content = rig
        .get(&format!(
            "/api/v1/files/content?{}",
            object_query(&upload, &format!("path={original}"))
        ))
        .await;
    assert_eq!(content.body, b"hello");
    assert_eq!(content.header("content-type"), "text/markdown");
    assert!(content
        .header("content-disposition")
        .contains("filename*=UTF-8''Caf%C3%A9%20notes.TXT"));
    let download = rig
        .get(&format!(
            "/api/v1/files/download?{}",
            object_query(&upload, &format!("path={original}"))
        ))
        .await;
    assert_eq!(download.header("content-type"), "text/markdown");
    assert!(download
        .header("content-disposition")
        .starts_with("attachment"));
    let listing = rig
        .get(&format!(
            "/api/v1/files/listing?{}",
            object_query(&upload, "prefix=original")
        ))
        .await
        .json();
    let entries = listing["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1, "the record is not listed: {entries:?}");
    assert_eq!(entries[0]["display_name"], "Café notes.TXT");
}

// ---- Statements, namespaces and facts ----

/// `PUT /api/v1/files/metadata/statements` of `curie` = `value` on the file of `upload`.
async fn set(rig: &FilesRig, upload: &Value, curie: &str, value: &str) -> Answer {
    let body = serde_json::json!({
        "library_id": LIBRARY,
        "file_id": upload["file_id"],
        "curie": curie,
        "value": value,
    });
    let headers = [("content-type", "application/json")];
    let uri = "/api/v1/files/metadata/statements";
    rig.call(Method::PUT, uri, &headers, body.to_string().into_bytes())
        .await
}

/// `GET /api/v1/files/metadata` of the file of `upload`.
async fn statements(rig: &FilesRig, upload: &Value, rest: &str) -> Value {
    let id = upload["file_id"].as_str().expect("file id");
    let uri = format!("/api/v1/files/metadata?library_id={LIBRARY}&file_id={id}{rest}");
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()
}

/// The `curie=value` pairs of a statements answer.
fn pairs(view: &Value) -> Vec<(String, String)> {
    view["statements"]
        .as_array()
        .expect("statements")
        .iter()
        .map(|s| {
            let curie = s["curie"].as_str().expect("curie").to_string();
            (curie, s["value"].as_str().expect("value").to_string())
        })
        .collect()
}

/// Register `prefix` at `iri` as `kind` with `terms` in the rig's library.
async fn register(rig: &FilesRig, prefix: &str, kind: &str, terms: Value) -> Answer {
    let body = serde_json::json!({
        "library_id": LIBRARY,
        "prefix": prefix,
        "iri": format!("https://example.org/{prefix}#"),
        "kind": kind,
        "terms": terms,
    });
    rig.post("/api/v1/files/metadata/registrations", body).await
}

#[tokio::test]
async fn statements_are_set_read_and_removed_one_version_each() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "harbour.jpg", b"jpeg", "").await;
    let first = set(&rig, &upload, "dcterms:title", "Harbour").await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json());
    assert_eq!(first.json()["version"], "v2");
    assert_eq!(
        set(&rig, &upload, "dc:creator", "Ana").await.json()["version"],
        "v3"
    );
    assert_eq!(
        set(&rig, &upload, "schema:keywords", "sea").await.json()["version"],
        "v4"
    );
    let view = statements(&rig, &upload, "").await;
    assert_eq!(view["version"], "v4");
    assert_eq!(
        pairs(&view),
        [
            ("dcterms:title".to_string(), "Harbour".to_string()),
            ("dcterms:creator".to_string(), "Ana".to_string()),
            ("schema:keywords".to_string(), "sea".to_string()),
        ]
    );
    assert_eq!(view["statements"][0]["source"], "asserted");
    assert!(pairs(&statements(&rig, &upload, "&version=v1").await).is_empty());
    let removal = serde_json::json!({
        "library_id": LIBRARY, "file_id": upload["file_id"], "curie": "dc:creator"
    });
    let removed = rig.post("/api/v1/files/metadata/removals", removal).await;
    assert_eq!(removed.json()["version"], "v5", "{:?}", removed.json());
    assert_eq!(pairs(&statements(&rig, &upload, "").await).len(), 2);
    let (_, record) = head_record(&rig, &upload).await;
    assert_eq!(record.imports().len(), 1, "the import entry is untouched");
    let sets = rig.journaled("files.metadata.set");
    assert_eq!(sets.len(), 3);
    assert!(sets.iter().all(|e| e
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id))));
}

#[tokio::test]
async fn unknown_dcterms_names_and_unregistered_prefixes_are_refused_without_a_version() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "a.txt", b"a", "").await;
    for curie in [
        "dcterms:colour",
        "dcterms:accrualPeriodicity",
        "rico:title",
        "title",
    ] {
        let answer = set(&rig, &upload, curie, "x").await;
        assert_error(&answer, 400, "invalid_request");
    }
    let unregistered = set(&rig, &upload, "rico:title", "x").await;
    assert!(unregistered.json()["message"]
        .as_str()
        .expect("message")
        .contains("rico"));
    assert_eq!(
        set(&rig, &upload, "dcterms:title", "A").await.status,
        StatusCode::OK
    );
    let same = set(&rig, &upload, "dcterms:title", "A").await;
    assert_error(&same, 409, "conflict");
    let (_, record) = head_record(&rig, &upload).await;
    assert_eq!(record.statements().statements.len(), 1);
}

#[tokio::test]
async fn a_registered_namespace_accepts_its_terms_and_is_kept_in_the_snapshot_object() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "a.txt", b"a", "").await;
    let answer = register(&rig, "rico", "extension", serde_json::json!(["title"])).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.json()["object_id"], format!("urn:uuid:{LIBRARY}"));
    assert_eq!(
        set(&rig, &upload, "rico:title", "Fonds").await.status,
        StatusCode::OK
    );
    assert_error(
        &set(&rig, &upload, "rico:other", "x").await,
        400,
        "invalid_request",
    );
    let again = register(&rig, "rico", "extension", serde_json::json!(["title"])).await;
    assert_error(&again, 409, "conflict");
    let reserved = register(&rig, "dcterms", "custom", Value::Null).await;
    assert_error(&reserved, 400, "invalid_request");
    let listed = rig
        .get(&format!(
            "/api/v1/files/metadata/namespaces?library_id={LIBRARY}"
        ))
        .await
        .json();
    assert_eq!(listed["namespaces"][0]["prefix"], "rico");
    let snapshot = Object::open(rig.root.as_ref(), "snapshot")
        .await
        .expect("snapshot");
    let state = snapshot.state(snapshot.head()).expect("state");
    assert!(state.get("namespaces.md").is_some());
}

#[tokio::test]
async fn a_term_name_is_offered_dcterms_first_then_schema_then_registered_namespaces() {
    let rig = FilesRig::new().await;
    register(&rig, "rico", "extension", serde_json::json!(["title"])).await;
    register(&rig, "mine", "custom", Value::Null).await;
    let lookup =
        |name: &str| format!("/api/v1/files/metadata/terms?library_id={LIBRARY}&name={name}");
    let title = rig.get(&lookup("title")).await.json();
    let curies: Vec<&str> = title["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .map(|c| c["curie"].as_str().expect("curie"))
        .collect();
    assert_eq!(
        curies,
        ["dcterms:title", "schema:title", "rico:title", "mine:title"]
    );
    let colour = rig.get(&lookup("colour")).await.json();
    assert_eq!(colour["candidates"][0]["curie"], "schema:colour");
    assert_eq!(colour["candidates"][0]["kind"], "schema");
}

#[tokio::test]
async fn the_facts_projection_has_no_absolute_path_host_or_account() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "notes.txt", b"hello", "").await;
    let id = upload["file_id"].as_str().expect("file id");
    let answer = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={id}"
        ))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let facts = &answer.json()["facts"];
    assert_eq!(facts["display_name"], "notes.txt");
    assert_eq!(facts["size"], 5);
    assert_eq!(facts["imports"][0]["method"], "api_upload");
    assert_eq!(facts["imports"][0]["object_version"], "v1");
    let text = answer.json().to_string();
    for private in ["absolute_path", "import_root", "host_name", "account"] {
        assert!(!text.contains(private), "{private} in {text}");
    }
}

#[tokio::test]
async fn a_file_the_desktop_committed_is_edited_through_the_same_record() {
    use filesquid_api::canonical::{
        commit_file_original, file_object_id, file_object_root, new_upload_record, OwnerReadable,
        UploadInput,
    };
    let rig = FilesRig::new().await;
    let file = filesquid_core::FileId::new("7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80").expect("id");
    let input = UploadInput {
        name: "desk.jpg",
        name_normalization: filesquid_metadata::import::NameForm::Nfc,
        declared_size: 4,
        media_type: "image/jpeg",
        last_modified: None,
        operation_id: Some("op-desktop"),
        started: "2026-10-19T00:00:00Z",
        finished: "2026-10-19T00:00:01Z",
        object_version: "v1",
    };
    let record = new_upload_record(&file, &input, b"desk").expect("record");
    let root = file_object_root(&file).expect("root");
    let staging =
        filesquid_ocfl::change::StagingArea::new(rig.staging.as_ref(), "desktop").expect("staging");
    let model = filesquid_ocfl::change::ChangeModel::new(
        rig.root.as_ref(),
        rig.leases.as_ref(),
        "desktop",
        &root,
        file_object_id(&file),
        staging,
    );
    let meta = filesquid_ocfl::change::VersionMeta::new("2026-10-19T00:00:02.000Z".to_string());
    commit_file_original(&model, &OwnerReadable, &file, &record, None, b"desk", meta)
        .await
        .expect("desktop commit");
    let upload = serde_json::json!({ "file_id": file.as_str() });
    let answer = set(&rig, &upload, "dcterms:title", "From the desktop").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let view = statements(&rig, &upload, "").await;
    assert_eq!(
        pairs(&view),
        [("dcterms:title".to_string(), "From the desktop".to_string())]
    );
}
