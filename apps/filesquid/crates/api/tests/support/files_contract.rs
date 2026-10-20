//! Contract fixtures for the file operation and content routes (Task API-F1.3): a file
//! service over an in-memory OCFL root whose object holds the files the sample requests
//! name, the sample request of every file and content route (and of the routes that need a
//! body, such as `PUT /api/v1/preferences` and `POST /api/v1/client-events`), and
//! serialized Rust samples of every file and content-handle schema.

#![allow(dead_code)]

use axum::http::Method;
use filesquid_api::content::ContentTarget;
use filesquid_api::files::{
    CommitView, EntryView, FileChangeView, FileDetailsView, FileHistorySizeView, FileLibrary,
    FileService, FolderPageView, FolderRequest, FolderView, HistoryEntryView, HistoryView,
    LabelRequest, LibraryHistorySizeView, MintRequest, MintView, MoveRequest, RemoveRequest,
    RestoreRequest, UploadView, UserView, VersionView, VersionsView,
};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::monitor::Monitor;
use filesquid_api::ApiState;
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{rfc3339_utc, LeaseConfig, LeaseStore, SystemClock};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The library the samples name.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The object the samples name.
pub const OBJECT: &str = "urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11";

/// The file object the metadata samples name (`files/<FILE>`).
pub const FILE: &str = "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80";

/// The object's percent-encoded identifier, for query strings.
const OBJECT_QUERY: &str = "urn%3Auuid%3A0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11";

/// The lease store of the sample library.
type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// Lease TTL 60 s with a 10 s margin.
const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// Install a file service on `state` whose library holds the object at v2: v1 adds
/// `a.txt`, `move-me.txt`, `remove-me.txt` and `restore-me.txt`; v2 removes
/// `restore-me.txt`, so each sample write changes something whatever order it runs in.
pub async fn install(state: &ApiState, monitor: &Monitor) {
    let root = Arc::new(MemoryBackend::new());
    let ledger = Arc::new(MemoryRepairLedger::new());
    let library = LibraryId::new(LIBRARY).expect("library id");
    LocalLibraryService::new(monitor.journal(), ledger)
        .create_root("owner", &library, root.as_ref())
        .await
        .expect("create root");
    let leases = Arc::new(
        LeaseStore::new(MemoryBackend::new(), "leases", SystemClock, LEASES).expect("leases"),
    );
    commit_samples(root.as_ref(), leases.as_ref()).await;
    commit_file_object(root.as_ref(), leases.as_ref()).await;
    let service = FileService::new();
    let opened = service.open_library(
        &library,
        FileLibrary {
            backend: root,
            reservations: leases,
            staging: Arc::new(MemoryBackend::new()),
            staging_prefix: "api".to_string(),
        },
    );
    assert!(opened);
    assert!(state.install_files(Arc::new(service)));
}

/// Commit the sample object's two versions through the engine.
async fn commit_samples(root: &MemoryBackend, leases: &Leases) {
    let staging = MemoryBackend::new();
    let area = StagingArea::new(&staging, "fixture").expect("staging");
    let model = ChangeModel::new(root, leases, "fixture", "objects/sample", OBJECT, area);
    for path in ["a.txt", "move-me.txt", "remove-me.txt", "restore-me.txt"] {
        model
            .stage_bytes(path, path.as_bytes())
            .await
            .expect("stage");
    }
    model.commit(meta()).await.expect("v1");
    model.stage_remove("restore-me.txt").await.expect("stage");
    model.commit(meta()).await.expect("v2");
}

/// Commit the sample file object `files/<FILE>`: its original `a.txt` and its record, which
/// holds `dcterms:subject` so the removal sample has a statement to remove.
async fn commit_file_object(root: &MemoryBackend, leases: &Leases) {
    use filesquid_api::canonical::{
        commit_file_original, file_object_id, file_object_root, new_upload_record, OwnerReadable,
        UploadInput,
    };
    let file = filesquid_core::FileId::new(FILE).expect("file id");
    let input = UploadInput {
        name: "a.txt",
        name_normalization: filesquid_metadata::import::NameForm::Nfc,
        declared_size: 5,
        media_type: "text/plain",
        last_modified: None,
        operation_id: None,
        started: "2026-10-18T00:00:00Z",
        finished: "2026-10-18T00:00:01Z",
        object_version: "v1",
    };
    let mut record = new_upload_record(&file, &input, b"alpha").expect("record");
    record
        .statements_mut()
        .set(
            filesquid_metadata::MetadataNamespace::Dcterms,
            "subject",
            "samples",
        )
        .expect("statement");
    let staging = MemoryBackend::new();
    let area = StagingArea::new(&staging, "fixture").expect("staging");
    let root_path = file_object_root(&file).expect("root");
    let object_id = file_object_id(&file);
    let model = ChangeModel::new(root, leases, "fixture", &root_path, &object_id, area);
    commit_file_original(
        &model,
        &OwnerReadable,
        &file,
        &record,
        None,
        b"alpha",
        meta(),
    )
    .await
    .expect("file object");
}

/// A version record dated now.
fn meta() -> VersionMeta {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    VersionMeta::new(rfc3339_utc(now))
}

/// The library and object query of every sample, plus `rest`.
fn query(rest: &str) -> String {
    format!("library_id={LIBRARY}&object_id={OBJECT_QUERY}{rest}")
}

/// A JSON body naming the sample library and object plus `fields`.
fn json_body(fields: Value) -> Option<(&'static str, Vec<u8>)> {
    let mut body = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        body[key] = value.clone();
    }
    Some(("application/json", body.to_string().into_bytes()))
}

/// A JSON body naming the sample library and file plus `fields`.
fn file_body(fields: Value) -> Option<(&'static str, Vec<u8>)> {
    let mut body = json!({ "library_id": LIBRARY, "file_id": FILE });
    for (key, value) in fields.as_object().expect("fields") {
        body[key] = value.clone();
    }
    Some(("application/json", body.to_string().into_bytes()))
}

/// The request URI and body of the sample call of `method path` on `state`. A file route
/// gets the query or body that names the sample object; the content route gets a handle
/// minted for `token` when `state` has a file service, and the documented template
/// otherwise; `PUT /api/v1/preferences` gets the default preferences; every other route is
/// called as it is, with no body.
pub fn sample(
    state: &ApiState,
    method: &Method,
    path: &str,
    token: &str,
) -> (String, Option<(&'static str, Vec<u8>)>) {
    let plain = |rest: &str| (format!("{path}?{}", query(rest)), None);
    match (method.as_str(), path) {
        ("GET", "/api/v1/files/listing") | ("GET", "/api/v1/files/versions") => plain(""),
        ("GET" | "HEAD", "/api/v1/files/content")
        | ("GET", "/api/v1/files/download")
        | ("GET", "/api/v1/files/details")
        | ("GET", "/api/v1/files/history") => plain("&path=a.txt"),
        ("PUT", "/api/v1/files/upload") => (
            format!("{path}?library_id={LIBRARY}&name=new.txt&size=3"),
            Some(("application/octet-stream", b"new".to_vec())),
        ),
        ("POST", "/api/v1/files/folders") => (path.into(), json_body(json!({ "path": "albums" }))),
        ("POST", "/api/v1/files/moves") => (
            path.into(),
            json_body(json!({ "from": "move-me.txt", "to": "moved.txt" })),
        ),
        ("POST", "/api/v1/files/removals") => {
            (path.into(), json_body(json!({ "path": "remove-me.txt" })))
        }
        ("POST", "/api/v1/files/restorations") => (
            path.into(),
            json_body(json!({ "version": "v1", "paths": ["restore-me.txt"] })),
        ),
        ("PUT", "/api/v1/files/labels") => (
            path.into(),
            json_body(json!({ "version": "v1", "label": "First draft" })),
        ),
        ("GET", "/api/v1/files/history-size") => plain("&path=a.txt"),
        ("GET", "/api/v1/files/library-history-size") => {
            (format!("{path}?library_id={LIBRARY}"), None)
        }
        ("POST", "/api/v1/content-handles") => (
            path.into(),
            json_body(json!({ "version": "v1", "path": "a.txt" })),
        ),
        (_, "/api/v1/content/{handle}") => (content_url(state, token, path), None),
        ("GET", "/api/v1/files/metadata") | ("GET", "/api/v1/files/facts") => {
            (format!("{path}?library_id={LIBRARY}&file_id={FILE}"), None)
        }
        ("PUT", "/api/v1/files/metadata/statements") => (
            path.into(),
            file_body(json!({ "curie": "dcterms:title", "value": "Sample" })),
        ),
        ("POST", "/api/v1/files/metadata/removals") => (
            path.into(),
            file_body(json!({ "curie": "dcterms:subject" })),
        ),
        ("POST", "/api/v1/files/renames") => (
            path.into(),
            file_body(json!({ "display_name": "Été à Nice.jpg" })),
        ),
        ("GET", "/api/v1/files/metadata/terms") => {
            (format!("{path}?library_id={LIBRARY}&name=title"), None)
        }
        ("GET", "/api/v1/files/metadata/namespaces") => {
            (format!("{path}?library_id={LIBRARY}"), None)
        }
        ("POST", "/api/v1/files/metadata/registrations") => (
            path.into(),
            Some((
                "application/json",
                json!({
                    "library_id": LIBRARY,
                    "prefix": "rico",
                    "iri": "https://www.ica.org/standards/RiC/ontology#",
                    "kind": "extension",
                    "terms": ["title"],
                })
                .to_string()
                .into_bytes(),
            )),
        ),
        ("PUT", "/api/v1/preferences") => {
            let defaults = filesquid_api::preferences::Preferences::default();
            let body = serde_json::to_vec(&defaults).expect("json");
            (path.into(), Some(("application/json", body)))
        }
        ("POST", "/api/v1/client-events") => (
            path.into(),
            json_body(json!({ "events": [{
                "kind": "console",
                "id": "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab",
                "level": "warn",
                "component": "fs-file-browser",
                "error_code": "fetch_failed"
            }] })),
        ),
        ("GET", "/api/v1/admin/journal/entries/{entry}") => (journal_entry_url(state), None),
        _ => (path.to_string(), None),
    }
}

/// The URL of a journal entry `state`'s monitor holds (one is appended for it), else of
/// entry 1.
fn journal_entry_url(state: &ApiState) -> String {
    use filesquid_api::journal::{JournalEntry, Outcome};
    let id = state
        .monitor()
        .and_then(|m| {
            m.store()
                .append(&[JournalEntry::new("host.started", Outcome::Succeeded)])
                .ok()
        })
        .and_then(|ids| ids.first().copied())
        .unwrap_or(1);
    format!("/api/v1/admin/journal/entries/{id}")
}

/// A content URL minted for `token` when `state` serves files, else `template` with a
/// well-formed handle no store holds, so the request reaches the content denial.
fn content_url(state: &ApiState, token: &str, template: &str) -> String {
    if state.files().is_none() {
        let unknown = "A".repeat(43);
        return template.replace("{handle}", &unknown);
    }
    let target = ContentTarget {
        library_id: LIBRARY.to_string(),
        object_id: OBJECT.to_string(),
        version: "v1".to_string(),
        path: "a.txt".to_string(),
    };
    let credential = {
        use sha2::Digest as _;
        sha2::Sha256::digest(token.as_bytes()).into()
    };
    let minted = state
        .content_handles()
        .mint(credential, target)
        .expect("mint");
    format!("/api/v1/content/{}", minted.handle)
}

/// Serialize `value` for a schema check.
fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("serializable")
}

/// A version record with and without its optional members.
fn versions() -> [VersionView; 2] {
    let full = VersionView {
        version: "v2".into(),
        created: "2026-10-18T00:00:00Z".into(),
        message: Some("moved".into()),
        user: Some(UserView {
            name: "owner".into(),
            address: Some("mailto:owner@example.org".into()),
        }),
        label: Some("Final draft".into()),
    };
    let bare = VersionView {
        version: "v1".into(),
        created: "2026-10-17T00:00:00Z".into(),
        message: None,
        user: None,
        label: None,
    };
    [full, bare]
}

/// Both kinds of listing entry.
fn entries() -> [EntryView; 2] {
    [
        EntryView::Folder {
            name: "photos".into(),
            path: "photos".into(),
        },
        EntryView::File {
            name: "a.jpg".into(),
            display_name: "Été à Nice 2023.jpg".into(),
            path: "a.jpg".into(),
            digest: "ab".repeat(32),
            size: 3,
            modified_version: "v1".into(),
            modified: "2026-10-17T00:00:00Z".into(),
            type_hint: Some("jpg".into()),
        },
    ]
}

/// A details answer.
fn details() -> FileDetailsView {
    let mut fixity = BTreeMap::new();
    fixity.insert("md5".to_string(), vec!["0".repeat(32)]);
    FileDetailsView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        version: "v2".into(),
        path: "a.jpg".into(),
        name: "a.jpg".into(),
        display_name: "Été à Nice 2023.jpg".into(),
        digest: "ab".repeat(32),
        digest_algorithm: "sha256".into(),
        size: 3,
        fixity,
        last_change: versions()[0].clone(),
        type_hint: None,
        media_type: "image/jpeg".into(),
    }
}

/// A history with every change kind.
fn history() -> HistoryView {
    let changes = [
        (FileChangeView::Added, Some("ab".repeat(32))),
        (FileChangeView::Changed, Some("cd".repeat(32))),
        (FileChangeView::Deleted, None),
    ];
    HistoryView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        path: "a.jpg".into(),
        display_name: "Été à Nice.jpg".into(),
        entries: changes
            .into_iter()
            .map(|(change, digest)| HistoryEntryView {
                version: versions()[1].clone(),
                change,
                size: digest.as_ref().map(|_| 5),
                digest,
            })
            .collect(),
    }
}

/// Serialized samples of every file and content-handle schema.
pub fn samples() -> Vec<(&'static str, Vec<Value>)> {
    let page = |next: Option<String>| FolderPageView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        version: "v2".into(),
        prefix: String::new(),
        entries: entries().to_vec(),
        next_cursor: next,
    };
    let history = history();
    let mut all = vec![
        ("VersionUser", vec![to_value(versions()[0].user.clone())]),
        ("VersionRecord", versions().map(to_value).to_vec()),
        ("FolderEntry", entries().map(to_value).to_vec()),
        (
            "FolderPage",
            vec![to_value(page(Some("c".into()))), to_value(page(None))],
        ),
        ("FileDetails", vec![to_value(details())]),
        (
            "FileChange",
            [
                FileChangeView::Added,
                FileChangeView::Changed,
                FileChangeView::Deleted,
            ]
            .map(to_value)
            .to_vec(),
        ),
        (
            "FileHistoryEntry",
            history.entries.iter().map(to_value).collect(),
        ),
        ("FileHistory", vec![to_value(&history)]),
    ];
    all.extend(answer_samples());
    all.extend(request_samples());
    all.extend(metadata_samples());
    all
}

/// Serialized samples of the write and content-handle answers.
fn answer_samples() -> Vec<(&'static str, Vec<Value>)> {
    let versions = VersionsView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        head: "v2".into(),
        display_name: Some("Été à Nice.jpg".into()),
        versions: versions().to_vec(),
    };
    let commit = CommitView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        version: "v3".into(),
        created: "2026-10-18T00:00:00Z".into(),
    };
    let folder = FolderView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        path: "albums".into(),
        pending: true,
    };
    let upload = UploadView {
        library_id: LIBRARY.into(),
        file_id: "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80".into(),
        object_id: "urn:uuid:7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80".into(),
        version: "v1".into(),
        created: "2026-10-18T00:00:00Z".into(),
    };
    let handle = MintView {
        handle: "A".repeat(43),
        url: format!("/api/v1/content/{}", "A".repeat(43)),
        expires_at: "2026-10-18T01:00:00Z".into(),
    };
    let file_size = FileHistorySizeView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        path: "a.txt".into(),
        versions: 3,
        contents: 2,
        bytes: 11,
    };
    let library_size = LibraryHistorySizeView {
        library_id: LIBRARY.into(),
        objects: 1,
        contents: 2,
        bytes: 11,
        truncated: false,
    };
    vec![
        (
            "ObjectVersions",
            vec![
                to_value(&versions),
                to_value(VersionsView {
                    display_name: None,
                    ..versions.clone()
                }),
            ],
        ),
        ("FileCommit", vec![to_value(commit)]),
        ("UploadCommit", vec![to_value(upload)]),
        ("FileHistorySize", vec![to_value(file_size)]),
        ("LibraryHistorySize", vec![to_value(library_size)]),
        ("FolderAnswer", vec![to_value(folder)]),
        ("ContentHandle", vec![to_value(handle)]),
    ]
}

/// Serialized samples of the request bodies.
fn request_samples() -> Vec<(&'static str, Vec<Value>)> {
    let (library_id, object_id) = (LIBRARY.to_string(), OBJECT.to_string());
    let folder = FolderRequest {
        library_id: library_id.clone(),
        object_id: object_id.clone(),
        path: "albums".into(),
    };
    let moved = MoveRequest {
        library_id: library_id.clone(),
        object_id: object_id.clone(),
        from: "a.txt".into(),
        to: "b.txt".into(),
        message: Some("rename".into()),
    };
    let removed = RemoveRequest {
        library_id: library_id.clone(),
        object_id: object_id.clone(),
        path: "a.txt".into(),
        message: Some("tidy".into()),
    };
    let restored = RestoreRequest {
        library_id: library_id.clone(),
        object_id: object_id.clone(),
        version: "v1".into(),
        paths: vec!["a.txt".into()],
        label: Some("Back to the original".into()),
    };
    let labelled = LabelRequest {
        library_id: library_id.clone(),
        object_id: object_id.clone(),
        version: "v1".into(),
        label: Some("First draft".into()),
    };
    let mint = MintRequest {
        library_id,
        object_id,
        version: "v1".into(),
        path: "a.txt".into(),
    };
    vec![
        ("CreateFolderRequest", vec![to_value(folder)]),
        ("MoveRequest", vec![to_value(moved)]),
        ("RemoveRequest", vec![to_value(removed)]),
        ("RestoreRequest", vec![to_value(restored)]),
        ("LabelRequest", vec![to_value(labelled)]),
        ("ContentHandleRequest", vec![to_value(mint)]),
    ]
}

/// Serialized samples of the metadata property schemas (Task API-F1.4).
fn metadata_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::files::statements::{
        CandidateKind, MetadataCommitView, NamespacesView, RegisterNamespaceRequest,
        RemoveStatementRequest, RenameRequest, SetStatementRequest, StatementSource, StatementView,
        StatementsView, TermCandidate, TermsView,
    };
    use filesquid_metadata::namespace::{NamespaceKind, Registrant, Registration};
    let statement = |prefix: &str, source| StatementView {
        curie: format!("{prefix}:title"),
        prefix: prefix.into(),
        term: "title".into(),
        value: "Harbour".into(),
        source,
    };
    let statements = StatementsView {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        version: "v2".into(),
        statements: vec![
            statement("dcterms", StatementSource::Asserted),
            statement("exif", StatementSource::Extracted),
        ],
    };
    let candidate = |prefix: &str, kind| TermCandidate {
        curie: format!("{prefix}:title"),
        kind,
    };
    let candidates = vec![
        candidate("dcterms", CandidateKind::Dcterms),
        candidate("schema", CandidateKind::Schema),
        candidate("rico", CandidateKind::Extension),
        candidate("mine", CandidateKind::Custom),
    ];
    let registrations = vec![
        Registration::extension(
            "rico",
            "https://example.org/rico#",
            Some(vec!["title".into()]),
            Registrant::Owner,
        ),
        Registration::extension(
            "arkrim",
            "https://example.org/arkrim#",
            None,
            Registrant::Distribution("arkrim".into()),
        ),
        Registration::extension(
            "c2pa",
            "https://example.org/c2pa#",
            None,
            Registrant::Module("filesquid-c2pa".into()),
        ),
        Registration::custom("mine", "https://example.org/mine#"),
    ];
    let commit = MetadataCommitView {
        library_id: LIBRARY.into(),
        object_id: format!("urn:uuid:{FILE}"),
        version: "v2".into(),
        created: "2026-10-18T00:00:00Z".into(),
    };
    let set = SetStatementRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        curie: "dcterms:title".into(),
        value: "Harbour".into(),
    };
    let rename = RenameRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        display_name: "Été à Nice.jpg".into(),
    };
    let removal = RemoveStatementRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        curie: "dcterms:title".into(),
    };
    let register = |kind, terms| RegisterNamespaceRequest {
        library_id: LIBRARY.into(),
        prefix: "rico".into(),
        iri: "https://example.org/rico#".into(),
        kind,
        terms,
    };
    let facts = sample_facts();
    let summaries: Vec<Value> = facts["facts"]["imports"]
        .as_array()
        .expect("imports")
        .clone();
    vec![
        (
            "StatementSource",
            vec![
                to_value(StatementSource::Extracted),
                to_value(StatementSource::Asserted),
            ],
        ),
        (
            "MetadataStatement",
            statements.statements.iter().map(to_value).collect(),
        ),
        ("FileStatements", vec![to_value(&statements)]),
        ("SetStatementRequest", vec![to_value(set)]),
        ("RemoveStatementRequest", vec![to_value(removal)]),
        ("RenameRequest", vec![to_value(rename)]),
        ("MetadataCommit", vec![to_value(commit)]),
        (
            "CandidateKind",
            candidates.iter().map(|c| to_value(c.kind)).collect(),
        ),
        ("TermCandidate", candidates.iter().map(to_value).collect()),
        (
            "TermCandidates",
            vec![to_value(TermsView {
                name: "title".into(),
                candidates: candidates.clone(),
            })],
        ),
        (
            "NamespaceKind",
            vec![
                to_value(NamespaceKind::Extension),
                to_value(NamespaceKind::Custom),
            ],
        ),
        (
            "NamespaceRegistrant",
            registrations
                .iter()
                .map(|r| to_value(r.registrant()))
                .collect(),
        ),
        (
            "NamespaceRegistration",
            registrations.iter().map(to_value).collect(),
        ),
        (
            "Namespaces",
            vec![to_value(NamespacesView {
                library_id: LIBRARY.into(),
                namespaces: registrations.clone(),
            })],
        ),
        (
            "RegisterNamespaceRequest",
            vec![
                to_value(register(
                    NamespaceKind::Extension,
                    Some(vec!["title".into()]),
                )),
                to_value(register(NamespaceKind::Custom, None)),
            ],
        ),
        (
            "RawName",
            vec![
                json!("Harbour.jpg"),
                json!({ "encoding": "base64", "value": "/w==" }),
            ],
        ),
        (
            "ImportMethod",
            [
                "file_picker",
                "folder",
                "drag_and_drop",
                "other_application",
                "api_upload",
            ]
            .iter()
            .map(|m| json!(m))
            .collect(),
        ),
        ("ByteStorage", vec![json!("copied"), json!("referenced")]),
        ("ImportSummary", summaries),
        ("FileFacts", vec![facts["facts"].clone()]),
        ("FileFactsAnswer", vec![facts]),
    ]
}

/// The facts answer of the sample file object's record, as the route serializes it.
fn sample_facts() -> Value {
    use filesquid_api::canonical::{new_upload_record, UploadInput};
    use filesquid_api::files::statements::FactsView;
    let file = filesquid_core::FileId::new(FILE).expect("file id");
    let input = UploadInput {
        name: "Harbour.jpg",
        name_normalization: filesquid_metadata::import::NameForm::Nfc,
        declared_size: 5,
        media_type: "image/jpeg",
        last_modified: None,
        operation_id: None,
        started: "2026-10-18T00:00:00Z",
        finished: "2026-10-18T00:00:01Z",
        object_version: "v1",
    };
    let record = new_upload_record(&file, &input, b"alpha").expect("record");
    to_value(FactsView {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        version: "v1".into(),
        facts: record.facts(),
    })
}
