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
    let home = Arc::new(filesquid_api::home::MemoryHomeCollections::new());
    let collection =
        filesquid_core::CollectionId::new(crate::drive_catalog::COLLECTION).expect("collection");
    home.add_collection(&library, &collection);
    assert!(state.install_home_collections(home));
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
        ("GET", "/api/v1/files/view") => {
            let library = filesquid_core::LibraryId::new(LIBRARY).expect("library");
            let node = filesquid_api::navigation::NodeRef::Library(library)
                .to_handle()
                .expect("handle");
            (format!("{path}?node={node}&sort=modified_by_me"), None)
        }
        ("GET", "/api/v1/files/search") => (format!("{path}?q=a&sort=modified_by_me"), None),
        ("GET", "/api/v1/files/collection-history-size") => (
            format!("{path}?collection_id={}", crate::drive_catalog::COLLECTION),
            None,
        ),
        ("POST", "/api/v1/files/copies") => (
            path.into(),
            file_command(
                path,
                json!({ "collection_id": crate::drive_catalog::COLLECTION }),
            ),
        ),
        ("GET" | "HEAD", "/api/v1/files/content")
        | ("GET", "/api/v1/files/download")
        | ("GET", "/api/v1/files/details")
        | ("GET", "/api/v1/files/history") => plain("&path=a.txt"),
        ("PUT", "/api/v1/files/upload") => (path.into(), upload_command(path)),
        ("POST", "/api/v1/files/folders") => (
            path.into(),
            object_command(path, json!({ "path": "albums" })),
        ),
        ("POST", "/api/v1/files/moves") => (
            path.into(),
            object_command(path, json!({ "from": "move-me.txt", "to": "moved.txt" })),
        ),
        ("POST", "/api/v1/files/removals") => (
            path.into(),
            object_command(path, json!({ "path": "remove-me.txt" })),
        ),
        ("POST", "/api/v1/files/restorations") => (
            path.into(),
            object_command(
                path,
                json!({ "version": "v1", "paths": ["restore-me.txt"] }),
            ),
        ),
        ("PUT", "/api/v1/files/labels") => (
            path.into(),
            object_command(path, json!({ "version": "v1", "label": "First draft" })),
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
            file_command(path, json!({ "display_name": "Été à Nice.jpg" })),
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

/// Both kinds of listing entry of one object's listing (no `object_id`).
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
            object_id: None,
        },
    ]
}

/// A file entry of a collection or folder listing, which names its own object.
fn scope_entry() -> EntryView {
    EntryView::File {
        name: "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11.jpg".into(),
        display_name: "Été à Nice 2023.jpg".into(),
        path: "original/0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11.jpg".into(),
        digest: "ab".repeat(32),
        size: 3,
        modified_version: "v1".into(),
        modified: "2026-10-17T00:00:00Z".into(),
        type_hint: Some("jpg".into()),
        object_id: Some("urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11".into()),
    }
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
    let scope_page = FolderPageView {
        library_id: LIBRARY.into(),
        object_id: String::new(),
        version: String::new(),
        prefix: "trips".into(),
        entries: vec![entries()[0].clone(), scope_entry()],
        next_cursor: None,
    };
    let history = history();
    let mut folder_entries = entries().map(to_value).to_vec();
    folder_entries.push(to_value(scope_entry()));
    let mut all = vec![
        ("VersionUser", vec![to_value(versions()[0].user.clone())]),
        ("VersionRecord", versions().map(to_value).to_vec()),
        ("FolderEntry", folder_entries),
        (
            "FolderPage",
            vec![
                to_value(page(Some("c".into()))),
                to_value(page(None)),
                to_value(scope_page),
            ],
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
    all.extend(drive_samples());
    all.extend(command_samples());
    all
}

/// The placeholder of a sample command's expected revision, followed by the object whose
/// head it stands for; the contract test reads that head when it sends the sample.
pub const HEAD_OF: &str = "@head:";

/// The multipart boundary of the sample upload.
const BOUNDARY: &str = "filesquid-contract-boundary";

/// The `content-type` of the sample upload.
const MULTIPART: &str = "multipart/form-data; boundary=filesquid-contract-boundary";

/// The object and the whole placeholder of the first [`HEAD_OF`] in `text`, if any.
pub fn head_marker(text: &str) -> Option<(String, String)> {
    let start = text.find(HEAD_OF)?;
    let rest = &text[start + HEAD_OF.len()..];
    let object: String = rest.chars().take_while(|c| *c != '"').collect();
    let marker = format!("{HEAD_OF}{object}");
    Some((object, marker))
}

/// The envelope of the sample command of `route` whose expected revision is `revision`.
fn sample_envelope(route: &str, revision: &str) -> Value {
    json!({
        "schema_version": 1,
        "operation_id": format!("contract-{}", route.rsplit('/').next().unwrap_or("command")),
        "expected_revision": revision,
        "scope": format!("library:{LIBRARY}"),
    })
}

/// The JSON command body of `route` with `input`, expecting the head of `object`.
fn command(route: &str, input: Value, object: &str) -> Option<(&'static str, Vec<u8>)> {
    let envelope = sample_envelope(route, &format!("{HEAD_OF}{object}"));
    let body = json!({ "envelope": envelope, "input": input });
    Some(("application/json", body.to_string().into_bytes()))
}

/// [`command`] of an input naming the sample library and object plus `fields`.
fn object_command(route: &str, fields: Value) -> Option<(&'static str, Vec<u8>)> {
    let mut input = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields") {
        input[key] = value.clone();
    }
    command(route, input, OBJECT)
}

/// [`command`] of an input naming the sample library and file plus `fields`.
fn file_command(route: &str, fields: Value) -> Option<(&'static str, Vec<u8>)> {
    let mut input = json!({ "library_id": LIBRARY, "file_id": FILE });
    for (key, value) in fields.as_object().expect("fields") {
        input[key] = value.clone();
    }
    command(route, input, &format!("urn:uuid:{FILE}"))
}

/// The multipart body of the sample upload of a new file, `new.txt`.
fn upload_command(route: &str) -> Option<(&'static str, Vec<u8>)> {
    let envelope = sample_envelope(route, "0");
    let input = json!({
        "library_id": LIBRARY, "name": "new.txt", "size": 3,
        "collection_id": crate::drive_catalog::COLLECTION
    });
    let mut body = Vec::new();
    for (name, value) in [("envelope", envelope), ("input", input)] {
        let head = format!(
            "--{BOUNDARY}\r\ncontent-disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        );
        body.extend_from_slice(head.as_bytes());
    }
    let file = format!(
        "--{BOUNDARY}\r\ncontent-disposition: form-data; name=\"file\"; filename=\"new.txt\"\r\n\r\nnew\r\n--{BOUNDARY}--\r\n"
    );
    body.extend_from_slice(file.as_bytes());
    Some((MULTIPART, body))
}

/// Serialized samples of the command schemas of the file write routes (Task API-F2.13).
fn command_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::error::{ApiError, ErrorCode};
    use filesquid_api::files::copy::{CopyRequest, CopyView};
    use filesquid_api::files::statements::{MetadataCommitView, RenameRequest};
    use filesquid_api::files::{CommandAnswer, ConflictAnswer, FileCommand, UploadQuery};
    use filesquid_api::operation::{
        CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationId,
        OperationScope,
    };
    let id = OperationId::new("4b7f2c1e-9a3d-4e5f-8b6a-0c1d2e3f4a5b").expect("id");
    let envelope = CommandEnvelope::new(
        id.clone(),
        ExpectedRevision::new("v2").expect("revision"),
        OperationScope::new(format!("library:{LIBRARY}")).expect("scope"),
    );
    let committed = || CommandOutcome::new(id.clone(), CommandOutcomeKind::Committed);
    let answer = |result: Value| {
        to_value(CommandAnswer {
            outcome: committed(),
            result: Some(result),
        })
    };
    let wrap = |input: Value| {
        to_value(FileCommand {
            envelope: envelope.clone(),
            input,
        })
    };
    let commit = json!({
        "library_id": LIBRARY, "object_id": OBJECT, "version": "v3",
        "created": "2026-10-18T00:00:00Z"
    });
    let upload = UploadView {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        object_id: format!("urn:uuid:{FILE}"),
        version: "v1".into(),
        created: "2026-10-18T00:00:00Z".into(),
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        path: Some("Holidays/Été.txt".into()),
    };
    let copy = CopyView {
        library_id: LIBRARY.into(),
        file_id: "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19".into(),
        object_id: "urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19".into(),
        version: "v1".into(),
        created: "2026-10-20T00:00:00Z".into(),
        source: format!("urn:uuid:{FILE}@v2"),
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        path: Some("Holidays/Été à Nice-1.jpg".into()),
    };
    let metadata = MetadataCommitView {
        library_id: LIBRARY.into(),
        object_id: format!("urn:uuid:{FILE}"),
        version: "v2".into(),
        created: "2026-10-18T00:00:00Z".into(),
    };
    let folder = FolderView {
        library_id: LIBRARY.into(),
        object_id: OBJECT.into(),
        path: "albums".into(),
        pending: true,
    };
    let conflict = ConflictAnswer {
        outcome: CommandOutcome::new(id.clone(), CommandOutcomeKind::Conflict),
        result: None,
        error: ApiError::new(ErrorCode::Conflict, "It changed since it was read."),
    };
    let input = |name: Option<&str>| UploadQuery {
        library_id: LIBRARY.into(),
        file_id: name.map(|_| FILE.to_string()),
        name: "Été.txt".into(),
        size: 3,
        media_type: name.map(|_| "text/plain".to_string()),
        last_modified: name.map(|_| "2026-10-18T00:00:00Z".to_string()),
        name_normalization: name.map(|_| filesquid_metadata::import::NameForm::Nfc),
        message: name.map(str::to_string),
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        folder: name.map(|_| "Holidays".to_string()),
        reserved_file_id: None,
    };
    let requests = request_samples();
    let request = |name: &str| {
        requests
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, values)| values[0].clone())
            .expect("request sample")
    };
    let rename = RenameRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        display_name: "Été à Nice.jpg".into(),
    };
    let copy_request = CopyRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        message: None,
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        folder: None,
        reserved_file_id: None,
    };
    vec![
        ("FileCommitOutcome", vec![answer(commit)]),
        ("UploadCommitOutcome", vec![answer(to_value(upload))]),
        ("FileCopyOutcome", vec![answer(to_value(copy))]),
        ("MetadataCommitOutcome", vec![answer(to_value(metadata))]),
        ("FolderAnswerOutcome", vec![answer(to_value(folder))]),
        ("FileCommandConflict", vec![to_value(conflict)]),
        (
            "UploadInput",
            vec![to_value(input(Some("upload"))), to_value(input(None))],
        ),
        (
            "CreateFolderCommand",
            vec![wrap(request("CreateFolderRequest"))],
        ),
        ("MoveCommand", vec![wrap(request("MoveRequest"))]),
        ("RemoveCommand", vec![wrap(request("RemoveRequest"))]),
        ("RestoreCommand", vec![wrap(request("RestoreRequest"))]),
        ("LabelCommand", vec![wrap(request("LabelRequest"))]),
        ("RenameCommand", vec![wrap(to_value(rename))]),
        ("CopyCommand", vec![wrap(to_value(copy_request))]),
    ]
}

/// Serialized samples of the copy, view and collection storage-use schemas (Task API-F2.2).
fn drive_samples() -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::files::copy::{CopyRequest, CopyView};
    use filesquid_api::files::drive::{
        FileRow, FolderKind, FolderRow, ViewKind, ViewPage, ViewRow,
    };
    use filesquid_api::files::CollectionHistorySizeView;
    use filesquid_api::preferences::{FolderPlacement, SortDirection, SortKey};
    let request = CopyRequest {
        library_id: LIBRARY.into(),
        file_id: FILE.into(),
        message: Some("Copy for the album".into()),
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        folder: Some("Holidays".into()),
        reserved_file_id: None,
    };
    let copy = CopyView {
        library_id: LIBRARY.into(),
        file_id: "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19".into(),
        object_id: "urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f19".into(),
        version: "v1".into(),
        created: "2026-10-20T00:00:00Z".into(),
        source: format!("urn:uuid:{FILE}@v2"),
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        path: Some("Holidays/Été à Nice-1.jpg".into()),
    };
    let folder = FolderRow {
        node: "n1.abc".into(),
        node_kind: FolderKind::Collection,
        name: "Holidays".into(),
    };
    let file = |mine: Option<String>| FileRow {
        file_id: FILE.into(),
        object_id: format!("urn:uuid:{FILE}"),
        display_name: "Été à Nice.jpg".into(),
        media_type: "image/jpeg".into(),
        size: 5,
        modified: "2026-10-20T00:00:00.000Z".into(),
        modified_by_me: mine.clone(),
        opened_by_me: mine,
    };
    let rows = vec![
        ViewRow::Folder(folder.clone()),
        ViewRow::File(file(Some("2026-10-20T01:00:00.000Z".into()))),
        ViewRow::File(file(None)),
    ];
    let page = |next: Option<String>, complete| ViewPage {
        node: "n1.abc".into(),
        view: ViewKind::All,
        sort: SortKey::ModifiedByMe,
        direction: SortDirection::Descending,
        folders: FolderPlacement::Mixed,
        rows: rows.clone(),
        next_cursor: next,
        complete,
    };
    let size = CollectionHistorySizeView {
        library_id: LIBRARY.into(),
        collection_id: "2c0d1e2f-3a4b-4c5d-8e6f-7a8b9c0d1e2f".into(),
        files: 2,
        contents: 3,
        bytes: 42,
        truncated: false,
    };
    let kinds = [
        ViewKind::Home,
        ViewKind::All,
        ViewKind::Personal,
        ViewKind::Recent,
        ViewKind::Favorites,
        ViewKind::Shares,
        ViewKind::Tags,
        ViewKind::Deleted,
    ];
    vec![
        ("CopyRequest", vec![to_value(request)]),
        ("FileCopy", vec![to_value(copy)]),
        ("ViewKind", kinds.map(to_value).to_vec()),
        (
            "ViewFolderKind",
            [FolderKind::Collection, FolderKind::VirtualCollection]
                .map(to_value)
                .to_vec(),
        ),
        ("ViewFolderRow", vec![to_value(ViewRow::Folder(folder))]),
        ("ViewFileRow", rows[1..].iter().map(to_value).collect()),
        ("ViewRow", rows.iter().map(to_value).collect()),
        (
            "ViewPage",
            vec![
                to_value(page(Some("v1.00".into()), true)),
                to_value(page(None, false)),
            ],
        ),
        ("CollectionHistorySize", vec![to_value(size)]),
    ]
    .into_iter()
    .chain(search_samples(rows))
    .collect()
}

/// Serialized samples of the search schemas (Task API-F2.3).
fn search_samples(
    rows: Vec<filesquid_api::files::drive::ViewRow>,
) -> Vec<(&'static str, Vec<Value>)> {
    use filesquid_api::files::search::{SearchPage, SearchSort};
    use filesquid_api::preferences::{FolderPlacement, SortDirection};
    let sorts = [
        SearchSort::Relevance,
        SearchSort::Name,
        SearchSort::Size,
        SearchSort::Modified,
        SearchSort::ModifiedByMe,
        SearchSort::OpenedByMe,
    ];
    let page = |node: Option<String>, next: Option<String>| SearchPage {
        q: "deck type:document -draft".into(),
        node,
        sort: SearchSort::Relevance,
        direction: SortDirection::Descending,
        folders: FolderPlacement::OnTop,
        rows: rows.clone(),
        next_cursor: next,
        complete: true,
    };
    vec![
        ("SearchSort", sorts.map(to_value).to_vec()),
        (
            "SearchPage",
            vec![
                to_value(page(None, Some("v1.00".into()))),
                to_value(page(Some("n1.abc".into()), None)),
            ],
        ),
    ]
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
        collection_id: Some("c0ffee00-1234-4abc-8def-0123456789ab".into()),
        path: Some("Holidays/Été.txt".into()),
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
