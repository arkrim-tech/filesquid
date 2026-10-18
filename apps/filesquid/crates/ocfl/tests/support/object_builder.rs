//! Builds small OCFL 1.1 objects in any storage backend for the read tests
//! (IO-0.4). Each version is a list of logical files; bytes already in the
//! manifest are not stored again, so unchanged files stay in the version
//! directory where they first appeared, as OCFL forward-delta objects do.
//!
//! This is test support, not the engine's writer (IO-0.6 onward): it puts
//! files in publication order (content, version inventory and sidecar, then
//! the root inventory, its sidecar and the declaration) without reservation
//! or compare-and-swap.

#![allow(dead_code)]

use std::collections::BTreeMap;

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{sidecar_contents, sidecar_name, Inventory, User, Version};
use filesquid_ocfl::namaste::{Declaration, SpecVersion};
use filesquid_ocfl::storage::{complete_immediately, StorageBackend, StorageKey};
use filesquid_ocfl::VersionNum;

/// One logical file of a version: its logical path and bytes.
pub type FileSpec<'a> = (&'a str, &'a [u8]);

/// How to build an object.
#[derive(Debug, Clone)]
pub struct ObjectSpec<'a> {
    /// The object identifier.
    pub id: &'a str,
    /// The content-addressing algorithm.
    pub algorithm: DigestAlgorithm,
    /// Write manifest and state digests in uppercase.
    pub uppercase: bool,
    /// Each version's logical files, `v1` first.
    pub versions: Vec<Vec<FileSpec<'a>>>,
}

impl<'a> ObjectSpec<'a> {
    /// A sha256 object with lowercase digests.
    pub fn new(id: &'a str, versions: Vec<Vec<FileSpec<'a>>>) -> Self {
        Self {
            id,
            algorithm: DigestAlgorithm::Sha256,
            uppercase: false,
            versions,
        }
    }
}

/// The key of `rel` inside the object root `root` (empty for the backend root).
pub fn key_in(root: &str, rel: &str) -> StorageKey {
    let raw = if root.is_empty() {
        rel.to_string()
    } else {
        format!("{root}/{rel}")
    };
    StorageKey::new(&raw).expect("valid key")
}

/// Store `bytes` at `rel` inside `root`.
pub fn put<B: StorageBackend>(backend: &B, root: &str, rel: &str, bytes: &[u8]) {
    let key = key_in(root, rel);
    complete_immediately(backend.put(&key, bytes))
        .expect("ready")
        .expect("put");
}

/// Remove `rel` inside `root`.
pub fn delete<B: StorageBackend>(backend: &B, root: &str, rel: &str) {
    let key = key_in(root, rel);
    complete_immediately(backend.delete(&key))
        .expect("ready")
        .expect("delete");
}

/// Build the object described by `spec` at `root` and return its inventory.
pub fn build<B: StorageBackend>(backend: &B, root: &str, spec: &ObjectSpec<'_>) -> Inventory {
    let mut inventory = empty_inventory(spec);
    for (index, files) in spec.versions.iter().enumerate() {
        let version = VersionNum::parse(&format!("v{}", index + 1)).expect("version");
        add_version(backend, root, spec, &mut inventory, &version, files);
        write_inventory(backend, root, &version.to_string(), &inventory);
    }
    write_inventory(backend, root, "", &inventory);
    let declaration = Declaration::object(SpecVersion::V1_1);
    put(
        backend,
        root,
        &declaration.file_name(),
        declaration.contents().as_bytes(),
    );
    inventory
}

/// An inventory with no versions yet.
fn empty_inventory(spec: &ObjectSpec<'_>) -> Inventory {
    Inventory {
        id: spec.id.to_string(),
        spec: SpecVersion::V1_1,
        digest_algorithm: spec.algorithm,
        head: VersionNum::first(),
        content_directory: None,
        manifest: BTreeMap::new(),
        versions: BTreeMap::new(),
        fixity: None,
    }
}

/// Store the new bytes of one version and record its state.
fn add_version<B: StorageBackend>(
    backend: &B,
    root: &str,
    spec: &ObjectSpec<'_>,
    inventory: &mut Inventory,
    version: &VersionNum,
    files: &[FileSpec<'_>],
) {
    let mut state: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, bytes) in files {
        let digest = digest_of(spec, bytes);
        if !inventory.manifest.contains_key(&digest) {
            let content_path = format!("{version}/content/{path}");
            put(backend, root, &content_path, bytes);
            inventory
                .manifest
                .insert(digest.clone(), vec![content_path]);
        }
        state.entry(digest).or_default().push((*path).to_string());
    }
    let block = Version {
        created: created_at(version),
        state,
        message: Some(format!("Version {version}")),
        user: Some(User {
            name: "A Person".to_string(),
            address: Some("mailto:a@example.org".to_string()),
        }),
    };
    inventory.versions.insert(version.clone(), block);
    inventory.head = version.clone();
}

/// A valid RFC 3339 UTC time for `version`: `2026-10-11T00:00:00Z` for v1
/// and one second later per version, for up to 86 400 versions.
pub fn created_at(version: &VersionNum) -> String {
    let n = version.as_u64().expect("a test version number") - 1;
    assert!(n < 86_400, "the builder dates at most 86 400 versions");
    let (h, m, s) = (n / 3_600, n / 60 % 60, n % 60);
    format!("2026-10-11T{h:02}:{m:02}:{s:02}Z")
}

/// The digest of `bytes` as the spec writes it.
fn digest_of(spec: &ObjectSpec<'_>, bytes: &[u8]) -> String {
    let digest = spec.algorithm.digest_hex(bytes);
    if spec.uppercase {
        return digest.to_uppercase();
    }
    digest
}

/// Write `inventory` and its sidecar into the directory `dir` of the object
/// (empty for the object root).
pub fn write_inventory<B: StorageBackend>(
    backend: &B,
    root: &str,
    dir: &str,
    inventory: &Inventory,
) {
    let bytes = inventory.to_json();
    let at = |name: &str| {
        if dir.is_empty() {
            name.to_string()
        } else {
            format!("{dir}/{name}")
        }
    };
    put(backend, root, &at("inventory.json"), &bytes);
    let sidecar = sidecar_contents(inventory.digest_algorithm, &bytes);
    let name = sidecar_name(inventory.digest_algorithm);
    put(backend, root, &at(&name), sidecar.as_bytes());
}
