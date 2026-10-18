//! Write conformance against the pinned OCFL 1.1 `content` fixtures
//! (IO-0.7; OCFL-S-010 (b), OCFL-S-011).
//!
//! A content fixture is a directory of version directories `v1`, `v2`, ...
//! Each version directory holds that version's whole logical state. The
//! `README.md` and `vN_inventory.json` files beside them are not content;
//! a `vN_inventory.json` (only `spec-ex-full` has them) gives the object id
//! and the version's `created`, `message` and `user`, which the writer is
//! given as caller metadata. The fixtures are read only; nothing here
//! writes under `tests/fixtures/`.
//!
//! [`write_fixture`] stages each version's files on a staging backend
//! outside the storage root and commits one version per directory through
//! the engine's only write path, [`ObjectWriter`].

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use filesquid_ocfl::inventory::User;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{
    complete_immediately, MemoryBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::write::{
    rfc3339_utc, Clock, CommitReceipt, LeaseConfig, LeaseStore, ObjectWriter, SystemClock,
    VersionDraft,
};
use filesquid_ocfl::VersionNum;
use serde_json::Value;

use crate::fixtures;

/// The content fixtures at the pinned commit with their version counts.
/// A fixture added or removed by a new pin fails the test until this list
/// is reviewed (no fixture is skipped silently).
pub const PINNED_CONTENT: [(&str, usize); 7] = [
    ("cf1", 1),
    ("cf2", 3),
    ("cf3", 3),
    ("cf4", 1),
    ("spec-ex-diff-paths", 1),
    ("spec-ex-full", 3),
    ("spec-ex-minimal", 1),
];

/// Deepest directory nesting read inside one version directory.
const MAX_DEPTH: usize = 8;

/// Most files read from one version directory.
const MAX_FILES: usize = 1_000;

/// Most chunks streamed from one written logical file.
const MAX_STREAM_CHUNKS: usize = 10_000;

/// One version directory of a content fixture.
#[derive(Debug, Clone)]
pub struct SourceVersion {
    /// The version directory name, such as `v2`.
    pub name: String,
    /// Every logical path (relative to the version directory, `/`-separated)
    /// with its bytes.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The version metadata from `vN_inventory.json`, when the fixture has
    /// one.
    pub meta: Option<VersionMeta>,
}

/// Version metadata a fixture's `vN_inventory.json` gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMeta {
    /// The object id the fragment names.
    pub object_id: String,
    /// The version's `created`.
    pub created: String,
    /// The version's `message`.
    pub message: Option<String>,
    /// The version's `user`.
    pub user: Option<User>,
}

/// One content fixture, read from the pinned tree.
#[derive(Debug, Clone)]
pub struct ContentFixture {
    /// The fixture directory name, such as `cf3`.
    pub name: String,
    /// Its version directories in version order.
    pub versions: Vec<SourceVersion>,
}

impl ContentFixture {
    /// The id the written object gets: the fragment's id when the fixture
    /// has one, otherwise `urn:example:ocfl-content:<name>`.
    pub fn object_id(&self) -> String {
        self.versions
            .iter()
            .find_map(|v| v.meta.as_ref().map(|m| m.object_id.clone()))
            .unwrap_or_else(|| format!("urn:example:ocfl-content:{}", self.name))
    }
}

/// The `content` directory of the fixture tree the tests read.
pub fn content_dir() -> PathBuf {
    fixtures::tree().join("content")
}

/// The fixture directory names under `content`, sorted.
pub fn fixture_names() -> Vec<String> {
    fixtures::objects(&content_dir())
}

/// Read the content fixture `name`.
pub fn load(name: &str) -> ContentFixture {
    let dir = content_dir().join(name);
    let versions = version_dirs(&dir)
        .into_iter()
        .map(|version| read_version(&dir, &version))
        .collect();
    ContentFixture {
        name: name.to_string(),
        versions,
    }
}

/// The `vN` directory names in `dir`, in version order. Every directory in
/// a content fixture is a version directory; any other directory fails the
/// load, so a changed or damaged fixture can never pass with part of its
/// payload left untested.
fn version_dirs(dir: &Path) -> Vec<String> {
    let mut versions: Vec<(VersionNum, String)> = fixtures::objects(dir)
        .into_iter()
        .map(|name| match VersionNum::parse(&name) {
            Ok(version) => (version, name),
            Err(err) => panic!(
                "{}: directory {name:?} is not a version directory: {err}",
                dir.display()
            ),
        })
        .collect();
    versions.sort();
    versions.into_iter().map(|(_, name)| name).collect()
}

/// Read one version directory and its metadata fragment.
fn read_version(dir: &Path, version: &str) -> SourceVersion {
    let mut files = BTreeMap::new();
    collect_files(&dir.join(version), "", 0, &mut files);
    let fragment = dir.join(format!("{version}_inventory.json"));
    let meta = fragment
        .is_file()
        .then(|| parse_fragment(&fragment, version));
    SourceVersion {
        name: version.to_string(),
        files,
        meta,
    }
}

/// Add every file under `dir` to `files` under `prefix`, at most
/// [`MAX_DEPTH`] levels and [`MAX_FILES`] files deep.
fn collect_files(dir: &Path, prefix: &str, depth: usize, files: &mut BTreeMap<String, Vec<u8>>) {
    assert!(depth <= MAX_DEPTH, "{} is nested too deep", dir.display());
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.path().is_dir() {
            collect_files(&entry.path(), &format!("{path}/"), depth + 1, files);
            continue;
        }
        let bytes = std::fs::read(entry.path()).expect("read fixture file");
        files.insert(path, bytes);
        assert!(
            files.len() <= MAX_FILES,
            "{} has too many files",
            dir.display()
        );
    }
}

/// The object id and version metadata in a `vN_inventory.json` fragment.
fn parse_fragment(path: &Path, version: &str) -> VersionMeta {
    let bytes = std::fs::read(path).expect("read fragment");
    let json: Value = serde_json::from_slice(&bytes).expect("fragment JSON");
    let block = &json["versions"][version];
    let text = |value: &Value| value.as_str().map(str::to_string);
    let user = block["user"].as_object().map(|user| User {
        name: text(&user["name"]).expect("user name"),
        address: user.get("address").and_then(text),
    });
    VersionMeta {
        object_id: text(&json["id"]).expect("fragment id"),
        created: text(&block["created"]).expect("fragment created"),
        message: text(&block["message"]),
        user,
    }
}

/// Drive a backend future that never waits.
pub fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// The draft of `version` of `fixture`: every file staged on `staging`
/// under `stage/<fixture>/<version>/`, with the fragment's metadata when
/// there is one.
pub fn draft_for(
    staging: &MemoryBackend,
    fixture: &ContentFixture,
    version: usize,
) -> VersionDraft {
    let source = &fixture.versions[version];
    let created = source.meta.as_ref().map_or_else(now, |m| m.created.clone());
    let mut draft = VersionDraft::new(fixture.object_id(), created);
    for (path, bytes) in &source.files {
        let key = StorageKey::new(&format!("stage/{}/{}/{path}", fixture.name, source.name))
            .expect("staging key");
        ready(staging.put(&key, bytes)).expect("stage");
        draft.stage(path.clone(), key);
    }
    let meta = source.meta.as_ref();
    if let Some(message) = meta.and_then(|m| m.message.clone()) {
        draft.set_message(message);
    }
    if let Some(user) = meta.and_then(|m| m.user.clone()) {
        draft.set_user(user);
    }
    draft
}

/// The current time as RFC 3339 UTC.
fn now() -> String {
    rfc3339_utc(SystemClock.now_millis().expect("system clock"))
}

/// Commit version directory `index` (0-based) of `fixture` as the next
/// version of the object at `root` of `storage`, on top of `expected`
/// (`None` for the first). Staging and leases are fresh backends outside
/// the storage root.
pub fn commit_version<B: StorageBackend>(
    storage: &B,
    root: &str,
    fixture: &ContentFixture,
    index: usize,
    expected: Option<&VersionNum>,
) -> CommitReceipt {
    let staging = MemoryBackend::new();
    let leases = LeaseStore::new(
        MemoryBackend::new(),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("lease store");
    let writer = ObjectWriter::new(storage, &leases, "io07-content-fixtures");
    let draft = draft_for(&staging, fixture, index);
    ready(writer.commit(root, expected, &staging, &draft))
        .unwrap_or_else(|e| panic!("{} {}: {e}", fixture.name, fixture.versions[index].name))
}

/// Write every version of `fixture` as an object at `root` of `storage`,
/// one commit per version directory, and return the receipts. Each
/// commit's expected head is the previous receipt's version.
pub fn write_fixture<B: StorageBackend>(
    storage: &B,
    root: &str,
    fixture: &ContentFixture,
) -> Vec<CommitReceipt> {
    let mut receipts: Vec<CommitReceipt> = Vec::new();
    for index in 0..fixture.versions.len() {
        let expected = receipts.last().map(|r| r.version.clone());
        receipts.push(commit_version(
            storage,
            root,
            fixture,
            index,
            expected.as_ref(),
        ));
    }
    receipts
}

/// Every logical file of `version` of the object at `root`, streamed
/// through the engine (each checked against its inventory digest).
pub fn written_state<B: StorageBackend>(
    storage: &B,
    root: &str,
    version: &VersionNum,
) -> BTreeMap<String, Vec<u8>> {
    let object = ready(Object::open(storage, root)).expect("open the written object");
    let state = object.state(version).expect("committed version");
    state
        .paths()
        .map(|path| (path.to_string(), stream(&object, version, path)))
        .collect()
}

/// Stream one logical file whole.
fn stream<B: StorageBackend>(object: &Object<'_, B>, version: &VersionNum, path: &str) -> Vec<u8> {
    let mut reader = object
        .stream_file(version, path, MAX_CHUNK_BYTES)
        .expect("stream");
    let mut bytes = Vec::new();
    for _ in 0..MAX_STREAM_CHUNKS {
        match ready(reader.next_chunk()).expect("chunk matches its digest") {
            Some(chunk) => bytes.extend_from_slice(&chunk),
            None => return bytes,
        }
    }
    panic!("{path} did not end within {MAX_STREAM_CHUNKS} chunks");
}

/// Panic unless the object at `root` has one version per version directory
/// of `fixture` and each version's logical state equals its source.
pub fn assert_state_matches<B: StorageBackend>(storage: &B, root: &str, fixture: &ContentFixture) {
    let object = ready(Object::open(storage, root)).expect("open the written object");
    let head = VersionNum::parse(&format!("v{}", fixture.versions.len())).expect("head");
    assert_eq!(
        object.head(),
        &head,
        "{}: one version per directory",
        fixture.name
    );
    for source in &fixture.versions {
        let version = VersionNum::parse(&source.name).expect("version");
        let written = written_state(storage, root, &version);
        assert_eq!(
            written, source.files,
            "{} {}: logical state differs from the source directory",
            fixture.name, source.name
        );
    }
}
