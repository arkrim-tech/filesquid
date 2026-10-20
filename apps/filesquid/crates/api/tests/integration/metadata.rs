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
