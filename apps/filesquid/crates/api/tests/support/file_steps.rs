//! Step definitions for `file_operation_routes_API-S-008.feature` (Task API-F1.3; API-S-004,
//! API-S-005, API-S-008, API-S-017, OCFL-S-012): the file routes through the router, against
//! a real OCFL root on the local filesystem.

use crate::files_fixture::{encode, pattern, query, send, Answer, FilesRig, LIBRARY, OBJECT};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::files::{DisplayNames, NameFuture, StoredFile, MAX_ANSWER_BYTES};
use filesquid_api::journal::Outcome;
use serde_json::json;

/// What the file-route scenarios keep between steps.
#[derive(Default)]
pub struct FileWorld {
    rig: Option<FilesRig>,
    head_before: String,
    answers: Vec<Answer>,
}

impl FileWorld {
    /// The scenario's rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a rig")
    }

    /// The last answer.
    fn last(&self) -> &Answer {
        self.answers.last().expect("an answer")
    }
}

/// The version number of `head` (`v3` is 3).
fn number(head: &str) -> u32 {
    head.trim_start_matches('v').parse().expect("version")
}

/// A JSON body naming the rig's library and object plus `fields`.
fn body(fields: serde_json::Value) -> serde_json::Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        all[key] = value.clone();
    }
    all
}

#[given(expr = "the owner's library holds an object with {string}, {string} and {string}")]
async fn library_object(w: &mut MonitorWorld, a: String, b: String, c: String) {
    let files: [(&str, &[u8]); 3] = [(&a, b"a"), (&b, b"b"), (&c, b"notes")];
    let rig = FilesRig::with_files(&files).await;
    w.files.head_before = rig.head().await;
    w.files.rig = Some(rig);
}

#[given(expr = "the object holds a 20 MiB file {string}")]
async fn big_file(w: &mut MonitorWorld, path: String) {
    w.files
        .rig()
        .commit(&[(&path, &pattern(20 * 1024 * 1024))])
        .await;
}

#[when(expr = "the owner lists the folder {string}")]
async fn list_folder(w: &mut MonitorWorld, prefix: String) {
    let uri = format!(
        "/api/v1/files/listing?{}",
        query(&format!("prefix={}", encode(&prefix)))
    );
    let answer = w.files.rig().get(&uri).await;
    w.files.answers.push(answer);
}

#[then(expr = "the listing should name {string} and {string}")]
fn listing_names(w: &mut MonitorWorld, a: String, b: String) {
    let json = w.files.last().json();
    let paths: Vec<&str> = json["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["path"].as_str().expect("path"))
        .collect();
    assert_eq!(paths, [a.as_str(), b.as_str()]);
}

#[then(expr = "no answer should carry a filesystem path or a storage key")]
fn no_storage_paths(w: &mut MonitorWorld) {
    let dir = w.files.rig().dir.to_str().expect("dir").to_string();
    for answer in &w.files.answers {
        let text = String::from_utf8_lossy(&answer.body);
        assert!(!text.contains(&dir), "{text}");
        assert!(!text.contains("objects/"), "{text}");
        assert!(!text.contains("/content/"), "{text}");
    }
}

#[when(expr = "the owner uploads {string}")]
async fn upload(w: &mut MonitorWorld, path: String) {
    upload_bytes(w, &path, b"new bytes").await;
}

#[when(expr = "the owner uploads {string} with its current content")]
async fn upload_same(w: &mut MonitorWorld, path: String) {
    upload_bytes(w, &path, b"notes").await;
}

/// `PUT /api/v1/files/upload` of `bytes` as `path`.
async fn upload_bytes(w: &mut MonitorWorld, path: &str, bytes: &[u8]) {
    let uri = format!(
        "/api/v1/files/upload?{}",
        query(&format!("path={}", encode(path)))
    );
    let answer = w
        .files
        .rig()
        .call(Method::PUT, &uri, &[], bytes.to_vec())
        .await;
    w.files.answers.push(answer);
}

#[then(expr = "the object should have exactly one new version")]
async fn one_new_version(w: &mut MonitorWorld) {
    assert_new_versions(w, 1).await;
}

#[then(expr = "the object should have exactly two new versions")]
async fn two_new_versions(w: &mut MonitorWorld) {
    assert_new_versions(w, 2).await;
}

#[then(expr = "no version should be published")]
async fn no_new_version(w: &mut MonitorWorld) {
    assert_new_versions(w, 0).await;
}

/// Assert the head moved by exactly `count` versions.
async fn assert_new_versions(w: &MonitorWorld, count: u32) {
    let head = w.files.rig().head().await;
    assert_eq!(
        number(&head),
        number(&w.files.head_before) + count,
        "{head}"
    );
}

#[then(expr = "the activity journal should record one {string} entry with opaque identifiers only")]
async fn journaled_opaque(w: &mut MonitorWorld, operation: String) {
    let entries = w.files.rig().journaled(&operation);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.outcome, Outcome::Succeeded);
    let head = w.files.rig().head().await;
    let expected = [LIBRARY.to_string(), OBJECT.to_string(), head];
    assert_eq!(
        entry.target_ids, expected,
        "library, object and new version"
    );
    assert!(entry
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id)));
    let text = serde_json::to_string(entry).expect("json");
    assert!(!text.contains("photos/"), "{text}");
}

#[when(expr = "the owner moves {string} to {string}")]
async fn move_path(w: &mut MonitorWorld, from: String, to: String) {
    let request = body(json!({ "from": from, "to": to }));
    let answer = w.files.rig().post("/api/v1/files/moves", request).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    w.files.answers.push(answer);
}

#[when(expr = "the owner restores {string} from version {word}")]
async fn restore(w: &mut MonitorWorld, path: String, version: String) {
    let request = body(json!({ "version": version, "paths": [path] }));
    let answer = w
        .files
        .rig()
        .post("/api/v1/files/restorations", request)
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    w.files.answers.push(answer);
}

#[then(expr = "{string} and {string} should both be readable")]
async fn both_readable(w: &mut MonitorWorld, a: String, b: String) {
    for path in [a, b] {
        let uri = format!(
            "/api/v1/files/content?{}",
            query(&format!("path={}", encode(&path)))
        );
        let answer = w.files.rig().get(&uri).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}");
        assert_eq!(answer.body, b"a".to_vec(), "{path}");
    }
}

#[when(expr = "the owner creates the folder {string}")]
async fn create_folder(w: &mut MonitorWorld, path: String) {
    let answer = w
        .files
        .rig()
        .post("/api/v1/files/folders", body(json!({ "path": path })))
        .await;
    w.files.answers.push(answer);
}

#[then(expr = "the folder should be pending and no version should be published")]
async fn folder_pending(w: &mut MonitorWorld) {
    assert_eq!(w.files.last().json()["pending"], true);
    assert_new_versions(w, 0).await;
}

#[then(expr = "the folder {string} should be listed")]
async fn folder_listed(w: &mut MonitorWorld, path: String) {
    let listing = w
        .files
        .rig()
        .get(&format!("/api/v1/files/listing?{}", query("")))
        .await
        .json();
    let listed = listing["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .any(|e| e["kind"] == "folder" && e["path"] == path.as_str());
    assert!(listed, "{listing}");
}

#[when(expr = "the owner lists the root one entry at a time")]
async fn list_one_at_a_time(w: &mut MonitorWorld) {
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let rest = match &cursor {
            Some(c) => format!("limit=1&cursor={}", encode(c)),
            None => "limit=1".to_string(),
        };
        let answer = w
            .files
            .rig()
            .get(&format!("/api/v1/files/listing?{}", query(&rest)))
            .await;
        cursor = answer.json()["next_cursor"].as_str().map(str::to_string);
        w.files.answers.push(answer);
        if cursor.is_none() {
            return;
        }
    }
    panic!("the listing never ended");
}

#[then(expr = "every entry should be listed once, folders first")]
fn listed_once(w: &mut MonitorWorld) {
    let entries: Vec<(String, String)> = w
        .files
        .answers
        .iter()
        .flat_map(|a| a.json()["entries"].as_array().cloned().unwrap_or_default())
        .map(|e| (e["kind"].to_string(), e["path"].to_string()))
        .collect();
    let expected = [("\"folder\"", "\"photos\""), ("\"file\"", "\"notes.txt\"")];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(k, p)| (k.to_string(), p.to_string()))
        .collect();
    assert_eq!(entries, expected);
}

#[when(expr = "the owner reads {string} without a range")]
async fn read_whole(w: &mut MonitorWorld, path: String) {
    let uri = format!(
        "/api/v1/files/content?{}",
        query(&format!("path={}", encode(&path)))
    );
    let answer = w.files.rig().get(&uri).await;
    w.files.answers.push(answer);
}

#[then(expr = "the answer should be 206 with the first 16 MiB")]
fn first_16_mib(w: &mut MonitorWorld) {
    let answer = w.files.last();
    assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(answer.body.len(), MAX_ANSWER_BYTES);
    let first = &pattern(20 * 1024 * 1024)[..MAX_ANSWER_BYTES];
    assert!(answer.body == first, "the body is the file's first 16 MiB");
    let total = 20 * 1024 * 1024;
    let range = format!("bytes 0-{}/{total}", MAX_ANSWER_BYTES - 1);
    assert_eq!(answer.header("content-range"), range);
}

#[then(expr = "the write should be refused as {string}")]
fn write_refused(w: &mut MonitorWorld, code: String) {
    let answer = w.files.last();
    assert_eq!(answer.status, status_of(&code));
    assert_eq!(answer.json()["code"], code.as_str());
}

#[then(expr = "the activity journal should record one refused {string} entry")]
fn one_refused(w: &mut MonitorWorld, operation: String) {
    let entries = w.files.rig().journaled(&operation);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Failed);
    assert_eq!(entries[0].error_code.as_deref(), Some("conflict"));
}

#[when(expr = "a client without a token reads {string}")]
async fn anonymous_read(w: &mut MonitorWorld, path: String) {
    let uri = format!(
        "/api/v1/files/content?{}",
        query(&format!("path={}", encode(&path)))
    );
    let answer = send(&w.files.rig().state, Method::GET, &uri, &[], Vec::new()).await;
    w.files.answers.push(answer);
}

#[when(expr = "a client without a token removes {string}")]
async fn anonymous_remove(w: &mut MonitorWorld, path: String) {
    let request = body(json!({ "path": path })).to_string().into_bytes();
    let headers = [("content-type", "application/json")];
    let state = &w.files.rig().state;
    let answer = send(
        state,
        Method::POST,
        "/api/v1/files/removals",
        &headers,
        request,
    )
    .await;
    w.files.answers.push(answer);
}

#[then(expr = "both calls should be refused as {string}")]
fn both_refused(w: &mut MonitorWorld, code: String) {
    let answers = &w.files.answers;
    assert_eq!(answers.len(), 2);
    for answer in answers {
        assert_eq!(answer.status, status_of(&code));
        assert_eq!(answer.json()["code"], code.as_str());
        assert_eq!(answer.json()["reason"], "missing_credentials");
    }
}

/// The HTTP status the API answers with error `code`.
fn status_of(code: &str) -> StatusCode {
    match code {
        "conflict" => StatusCode::CONFLICT,
        "unauthorized" => StatusCode::UNAUTHORIZED,
        other => panic!("no status listed for {other}"),
    }
}

/// One file record standing in for the host's: a stored path and its original name.
struct OneRecord(String, String);

impl DisplayNames for OneRecord {
    fn original_name<'a>(&'a self, file: StoredFile<'a>) -> NameFuture<'a> {
        let found = (file.path == self.0).then(|| self.1.clone());
        Box::pin(async move { found })
    }
}

#[given(expr = "the owner's original name of {string} is {string}")]
fn original_name(w: &mut MonitorWorld, path: String, name: String) {
    let record = std::sync::Arc::new(OneRecord(path, name));
    assert!(w.files.rig().service.install_display_names(record));
}

#[when(expr = "the owner downloads {string}")]
async fn download(w: &mut MonitorWorld, path: String) {
    let rest = format!("path={}&download=true", encode(&path));
    let uri = format!("/api/v1/files/content?{}", query(&rest));
    let answer = w.files.rig().get(&uri).await;
    w.files.answers.push(answer);
}

#[then(expr = "the listing should show {string} as {string}")]
fn listing_shows(w: &mut MonitorWorld, path: String, name: String) {
    let listing = w.files.answers[0].json();
    let entry = listing["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["path"] == path.as_str())
        .cloned()
        .expect("listed");
    assert_eq!(entry["display_name"], name.as_str());
}

#[then(expr = "the download should be named {string} with an ASCII fallback")]
fn download_named(w: &mut MonitorWorld, name: String) {
    let header = w.files.last().header("content-disposition").to_string();
    assert!(header.starts_with("attachment; filename=\""), "{header}");
    let fallback = header.split('"').nth(1).expect("fallback");
    assert!(fallback.is_ascii(), "{fallback}");
    let encoded = header.split("filename*=UTF-8''").nth(1).expect("filename*");
    let decoded = percent_decode(encoded);
    assert_eq!(decoded, name);
}

/// Decode `%XX` escapes of an RFC 5987 value.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("hex");
            out.push(u8::from_str_radix(hex, 16).expect("hex digit"));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("UTF-8")
}
