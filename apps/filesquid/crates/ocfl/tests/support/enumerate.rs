//! Shared helpers for the committed-logical-state enumerator tests (IO-0.9):
//! run the enumerator to its end, describe its events in words, load
//! fixture objects into a storage root, and an independent oracle that reads
//! a fixture inventory with `serde_json` instead of the engine's parser.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;

use filesquid_ocfl::read::{CommittedEvent, EnumerateError, StorageRoot, Versions};
use filesquid_ocfl::storage::{
    complete_immediately, read_verified, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use serde_json::Value;

/// The most events one test reads before it calls the enumerator stuck.
pub const MAX_EVENTS: usize = 100_000;

/// The deepest directory a fixture loader descends into.
const MAX_TREE_DEPTH: usize = 16;

/// One enumerator item as the tests keep it.
pub type Item = Result<CommittedEvent, EnumerateError>;

/// Drive a backend future that never waits (memory and filesystem).
pub fn ready<F: Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// Open the storage root at the root of `backend` and enumerate it to its
/// end with `versions`.
pub fn enumerate_all<B: StorageBackend>(backend: &B, versions: Versions) -> Vec<Item> {
    let root = ready(StorageRoot::open(backend)).expect("open the storage root");
    let mut cursor = root.enumerate(versions);
    let mut items = Vec::new();
    for _ in 0..MAX_EVENTS {
        match ready(cursor.next_event()) {
            Some(item) => items.push(item),
            None => return items,
        }
    }
    panic!("the enumerator did not end within {MAX_EVENTS} events");
}

/// One item in words, as the tests and features compare them.
pub fn describe(item: &Item) -> String {
    match item {
        Ok(CommittedEvent::Object(object)) => {
            format!("object {} {} head {}", object.root, object.id, object.head)
        }
        Ok(CommittedEvent::Version(version)) => format!(
            "version {} {} files {}",
            version.object_id, version.version, version.files
        ),
        Ok(CommittedEvent::File(file)) => format!(
            "file {} {} {} at {}",
            file.object_id, file.version, file.path, file.content_key
        ),
        Err(err) => format!("error {} {}", err.root, err.code().unwrap_or("none")),
    }
}

/// Every item in words.
pub fn describe_all(items: &[Item]) -> Vec<String> {
    items.iter().map(describe).collect()
}

/// The file events as full rows: object id, version, logical path, digest
/// and content key.
pub fn file_rows(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Ok(CommittedEvent::File(file)) => Some(format!(
                "{} {} {} {} {}",
                file.object_id, file.version, file.path, file.digest, file.content_key
            )),
            _ => None,
        })
        .collect()
}

/// Read every file event's content key and check its bytes against the
/// event's digest. Returns how many were checked.
pub fn check_content<B: StorageBackend>(backend: &B, items: &[Item]) -> usize {
    let mut checked = 0;
    for item in items {
        let Ok(CommittedEvent::File(file)) = item else {
            continue;
        };
        let bytes = ready(read_verified(backend, &file.content_key, MAX_CHUNK_BYTES))
            .unwrap_or_else(|err| panic!("{}: {err}", file.content_key));
        let actual = file.digest_algorithm.digest_hex(&bytes);
        assert!(
            file.digest_algorithm.matches(&actual, &file.digest),
            "{} does not match {}",
            file.content_key,
            file.digest
        );
        checked += 1;
    }
    checked
}

/// Store a file at `raw` through `backend`.
pub fn put_key<B: StorageBackend>(backend: &B, raw: &str, bytes: &[u8]) {
    let key = StorageKey::new(raw).expect("valid key");
    ready(backend.put(&key, bytes)).expect("put");
}

/// Declare the backend root an OCFL 1.1 storage root.
pub fn declare_storage_root<B: StorageBackend>(backend: &B) {
    put_key(backend, "0=ocfl_1.1", b"ocfl_1.1\n");
}

/// Copy every file under `dir` to keys under `prefix`, descending at most
/// [`MAX_TREE_DEPTH`] levels.
pub fn load_tree<B: StorageBackend>(backend: &B, dir: &Path, prefix: &str) {
    load_level(backend, dir, prefix, 0);
}

/// One directory level of [`load_tree`].
fn load_level<B: StorageBackend>(backend: &B, dir: &Path, prefix: &str, depth: usize) {
    assert!(depth < MAX_TREE_DEPTH, "{} is too deep", dir.display());
    for entry in std::fs::read_dir(dir).expect("list fixture directory") {
        let entry = entry.expect("fixture entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let key = format!("{prefix}/{name}");
        if entry.path().is_dir() {
            load_level(backend, &entry.path(), &key, depth + 1);
        } else {
            let bytes = std::fs::read(entry.path()).expect("read fixture file");
            put_key(backend, &key, &bytes);
        }
    }
}

/// The file rows an enumerator must give for the object stored at `root`
/// from the fixture directory `object_dir`, computed from its root
/// `inventory.json` with `serde_json`: every version when `all`, otherwise
/// only `head`; versions oldest first, logical paths in byte order, and the
/// first content path the manifest lists for each digest.
pub fn oracle_rows(object_dir: &Path, root: &str, all: bool) -> Vec<String> {
    let text = std::fs::read(object_dir.join("inventory.json")).expect("read inventory");
    let inventory: Value = serde_json::from_slice(&text).expect("inventory JSON");
    let id = inventory["id"].as_str().expect("id");
    let head = inventory["head"].as_str().expect("head");
    let mut versions: Vec<(&String, &Value)> = inventory["versions"]
        .as_object()
        .expect("versions")
        .iter()
        .filter(|(name, _)| all || name.as_str() == head)
        .collect();
    versions.sort_by_key(|(name, _)| version_order(name));
    let mut rows = Vec::new();
    for (name, block) in versions {
        rows.extend(version_rows(&inventory, id, name, block, root));
    }
    rows
}

/// The file rows of one version block, in logical-path order.
fn version_rows(inventory: &Value, id: &str, name: &str, block: &Value, root: &str) -> Vec<String> {
    let mut by_path = BTreeMap::new();
    for (digest, paths) in block["state"].as_object().expect("state") {
        let content = inventory["manifest"][digest][0].as_str().expect("manifest");
        for path in paths.as_array().expect("state paths") {
            let path = path.as_str().expect("logical path");
            by_path.insert(path.to_string(), (digest.clone(), content.to_string()));
        }
    }
    by_path
        .into_iter()
        .map(|(path, (digest, content))| format!("{id} {name} {path} {digest} {root}/{content}"))
        .collect()
}

/// A sort key for a version name such as `v3` or `v0003` that orders by
/// its decimal number without converting it, so any number of digits works:
/// the digits without leading zeros, shorter first, then by text.
fn version_order(name: &str) -> (usize, String) {
    let digits = name[1..].trim_start_matches('0');
    (digits.len(), digits.to_string())
}

/// The object events as rows: root, id, head and digest algorithm.
pub fn object_rows(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Ok(CommittedEvent::Object(o)) => Some(format!(
                "{} {} {} {}",
                o.root,
                o.id,
                o.head,
                o.digest_algorithm.name()
            )),
            _ => None,
        })
        .collect()
}

/// The object row an enumerator must give for the fixture at `object_dir`
/// stored at `root`, from its root `inventory.json` read with `serde_json`.
pub fn oracle_object_row(object_dir: &Path, root: &str) -> String {
    let text = std::fs::read(object_dir.join("inventory.json")).expect("read inventory");
    let inventory: Value = serde_json::from_slice(&text).expect("inventory JSON");
    let field = |name: &str| inventory[name].as_str().expect(name).to_string();
    format!(
        "{root} {} {} {}",
        field("id"),
        field("head"),
        field("digestAlgorithm")
    )
}
