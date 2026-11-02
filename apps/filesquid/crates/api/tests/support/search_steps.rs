//! Step definitions for `search_operation_API-S-012.feature` (Task API-F2.3): free text,
//! operators, unknown and unavailable operators, permission filtering before ranking and
//! case folding, through the router over a real OCFL storage root on the local filesystem and
//! a seeded library catalog.

use crate::drive_catalog::DriveCatalog;
use crate::files_fixture::{encode, upload_facts, Answer, FilesRig, LIBRARY};
use crate::MonitorWorld;
use axum::http::Method;
use cucumber::{given, then, when};
use filesquid_api::auth::Principal;
use filesquid_api::navigation::{NavigationAccess, NodeRef};
use filesquid_core::CollectionId;
use serde_json::json;
use std::sync::Arc;

/// The collection the owner may not read.
const SECRET: &str = "4e2f3a4b-5c6d-4e7f-8a9b-0c1d2e3f4a5b";

/// Per-scenario state of the search steps.
#[derive(Default)]
pub struct SearchWorld {
    rig: Option<FilesRig>,
    drive: Option<DriveCatalog>,
    answer: Option<Answer>,
}

impl std::fmt::Debug for SearchWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchWorld").finish_non_exhaustive()
    }
}

impl SearchWorld {
    /// The rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a library")
    }

    /// Upload `bytes` as `name` of `media` and answer its file ID.
    async fn upload(&self, name: &str, media: &str, bytes: &[u8]) -> String {
        let rest = format!("name_normalization=nfc&media_type={}", encode(media));
        let uri = upload_facts(name, bytes.len(), &rest);
        let answer = self.rig().upload(uri, bytes).await;
        assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
        answer.result()["file_id"].as_str().expect("id").to_string()
    }

    /// Set `curie` to `value` on `file`.
    async fn describe(&self, file: &str, curie: &str, value: &str) {
        let body =
            json!({ "library_id": LIBRARY, "file_id": file, "curie": curie, "value": value });
        let headers = [("content-type", "application/json")];
        let uri = "/api/v1/files/metadata/statements";
        let answer = self
            .rig()
            .call(Method::PUT, uri, &headers, body.to_string().into_bytes())
            .await;
        assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    }
}

/// Reads everything but the secret collection.
struct NoSecret;

impl NavigationAccess for NoSecret {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        let secret = NodeRef::Collection(CollectionId::new(SECRET).expect("id"));
        *node != secret
    }
}

#[given(
    expr = "a library whose files are {string} titled {string}, {string} described as {string} and {string}"
)]
async fn library(
    w: &mut MonitorWorld,
    budget: String,
    title: String,
    photo: String,
    description: String,
    deck: String,
) {
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::with_access(LIBRARY, Arc::new(NoSecret));
    assert!(rig.state.install_navigation(drive.navigation.clone()));
    w.search.rig = Some(rig);
    let files = [
        (budget, "application/pdf", 300),
        (photo, "image/jpeg", 50),
        (deck, "application/vnd.ms-powerpoint", 9),
    ];
    let mut ids = Vec::new();
    for (name, media, size) in files {
        let id = w.search.upload(&name, media, &vec![1u8; size]).await;
        drive.file_in_collection(&id);
        ids.push(id);
    }
    w.search.describe(&ids[0], "dcterms:title", &title).await;
    w.search
        .describe(&ids[1], "dcterms:description", &description)
        .await;
    w.search.drive = Some(drive);
}

#[given(expr = "the further files {string} and {string}")]
async fn further_files(w: &mut MonitorWorld, a: String, b: String) {
    for name in [a, b] {
        let id = w.search.upload(&name, "text/plain", b"z").await;
        w.search
            .drive
            .as_ref()
            .expect("a library")
            .file_in_collection(&id);
    }
}

#[given(expr = "a file {string} in a collection the owner may not read")]
async fn secret_file(w: &mut MonitorWorld, name: String) {
    let id = w.search.upload(&name, "application/pdf", b"abc").await;
    let drive = w.search.drive.as_ref().expect("a library");
    drive.file_in_new_collection(&id, SECRET, "Secret plans");
}

#[when(expr = "the owner searches for {string}")]
async fn searches(w: &mut MonitorWorld, query: String) {
    let query = query.replace("\\\"", "\"");
    let uri = format!("/api/v1/files/search?q={}&sort=name", encode(&query));
    w.search.answer = Some(w.search.rig().get(&uri).await);
}

#[then(expr = "the results should be {string}")]
async fn results(w: &mut MonitorWorld, expected: String) {
    let answer = w.search.answer.as_ref().expect("an answer");
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    let names: Vec<String> = answer.json()["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            let name = row.get("display_name").unwrap_or(&row["name"]);
            name.as_str().expect("name").to_string()
        })
        .collect();
    let expected: Vec<String> = expected.split(", ").map(str::to_string).collect();
    assert_eq!(names, expected);
}

#[then(expr = "the refusal message should name {string}")]
async fn refusal_names(w: &mut MonitorWorld, operator: String) {
    let answer = w.search.answer.as_ref().expect("an answer");
    let message = answer.json()["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(message.contains(&operator), "{message}");
}

#[then(expr = "the search should be refused as {string}")]
async fn refused(w: &mut MonitorWorld, code: String) {
    let answer = w.search.answer.as_ref().expect("an answer");
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
    assert!(answer.json().get("rows").is_none(), "no results");
}
