//! Helpers for the independent-recovery tests (IO-1.1; OCFL-S-022): an
//! ordinary-tool copy of a storage root, a byte snapshot of a directory
//! tree, a tamper toolkit, and an independent reader that rebuilds every
//! version's logical files, folders and sidecar pairs from a copy with only
//! a JSON parser and SHA-2, never the engine.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::storage::FsBackend;
use filesquid_ocfl::DigestAlgorithm;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};

use crate::builder::{build, ObjectSpec};
use crate::enumerate::declare_storage_root;
use crate::rig::{sha256, Rig};

/// The deepest directory the helpers descend into.
const MAX_DEPTH: usize = 16;

/// The most entries one directory walk visits.
const MAX_ENTRIES: usize = 10_000;

/// Every file under a root as relative `/`-separated name and bytes.
pub type Tree = BTreeMap<String, Vec<u8>>;

/// One logical file an independent reader recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered {
    /// The object root, relative to the storage root.
    pub object_root: String,
    /// The version name, such as `v2`.
    pub version: String,
    /// The logical path.
    pub path: String,
    /// The recovered bytes.
    pub bytes: Vec<u8>,
}

/// Copy `from` to `to` the way a file manager or `cp -R` would: create each
/// directory, then copy each file's bytes. Nothing but `std::fs`: no
/// FileSquid, no Git, no database.
pub fn ordinary_copy(from: &Path, to: &Path) {
    let mut budget = MAX_ENTRIES;
    copy_dir(from, to, 0, &mut budget);
}

/// One directory level of [`ordinary_copy`].
fn copy_dir(from: &Path, to: &Path, depth: usize, budget: &mut usize) {
    assert!(depth <= MAX_DEPTH, "tree deeper than {MAX_DEPTH}");
    fs::create_dir_all(to).expect("create destination directory");
    for entry in fs::read_dir(from).expect("read directory") {
        assert!(*budget > 0, "more than {MAX_ENTRIES} entries");
        *budget -= 1;
        let entry = entry.expect("directory entry");
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            copy_dir(&entry.path(), &target, depth + 1, budget);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// Every file under `root` with its bytes, keyed by relative name.
pub fn snapshot(root: &Path) -> Tree {
    let mut tree = Tree::new();
    let mut budget = MAX_ENTRIES;
    walk(root, "", 0, &mut budget, &mut tree);
    tree
}

/// One directory level of [`snapshot`].
fn walk(dir: &Path, prefix: &str, depth: usize, budget: &mut usize, tree: &mut Tree) {
    assert!(depth <= MAX_DEPTH, "tree deeper than {MAX_DEPTH}");
    for entry in fs::read_dir(dir).expect("read directory") {
        assert!(*budget > 0, "more than {MAX_ENTRIES} entries");
        *budget -= 1;
        let entry = entry.expect("directory entry");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().expect("file type").is_dir() {
            walk(&entry.path(), &format!("{name}/"), depth + 1, budget, tree);
        } else {
            tree.insert(name, fs::read(entry.path()).expect("read file"));
        }
    }
}

/// Replace the first byte of `file` with a different one, as bit rot or an
/// in-place edit would.
pub fn flip_first_byte(file: &Path) {
    let mut bytes = fs::read(file).expect("read file");
    bytes[0] ^= 0xFF;
    fs::write(file, bytes).expect("write file");
}

/// Rewrite `file` as text with every `from` replaced by `to`; `from` must
/// occur.
pub fn replace_text(file: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(file).expect("read file");
    assert!(text.contains(from), "{from:?} not in {}", file.display());
    fs::write(file, text.replace(from, to)).expect("write file");
}

/// The lowercase hex digest of `bytes` under `algorithm` (`sha256` or
/// `sha512`).
pub fn hex_digest(algorithm: &str, bytes: &[u8]) -> String {
    let raw: Vec<u8> = match algorithm {
        "sha256" => Sha256::digest(bytes).to_vec(),
        "sha512" => Sha512::digest(bytes).to_vec(),
        other => panic!("the independent reader knows sha256 and sha512, not {other}"),
    };
    raw.iter().map(|b| format!("{b:02x}")).collect()
}

/// Rewrite the digest sidecar next to `inventory` so it matches the
/// inventory's current bytes, as an attacker who edits both would.
pub fn rewrite_sidecar(inventory: &Path, algorithm: &str) {
    let bytes = fs::read(inventory).expect("read inventory");
    let name = inventory.file_name().expect("file name").to_string_lossy();
    let sidecar = inventory.with_file_name(format!("{name}.{algorithm}"));
    let line = format!("{}  {name}\n", hex_digest(algorithm, &bytes));
    fs::write(sidecar, line).expect("write sidecar");
}

/// The object roots under `storage`: every directory holding an
/// `0=ocfl_object_1.1` declaration, as `/`-separated relative names.
pub fn object_roots(storage: &Path) -> Vec<String> {
    snapshot(storage)
        .keys()
        .filter_map(|name| name.strip_suffix("/0=ocfl_object_1.1"))
        .map(str::to_string)
        .collect()
}

/// Read one inventory with `serde_json` only.
fn read_inventory(storage: &Path, object_root: &str) -> Value {
    let path = storage.join(object_root).join("inventory.json");
    let text = fs::read_to_string(path).expect("read root inventory");
    serde_json::from_str(&text).expect("inventory is JSON")
}

/// The manifest as content path by digest (first listed path wins).
fn manifest_paths(inventory: &Value) -> BTreeMap<String, String> {
    let manifest = inventory["manifest"].as_object().expect("manifest");
    let mut paths = BTreeMap::new();
    for (digest, listed) in manifest {
        let first = listed[0].as_str().expect("content path");
        paths.insert(digest.clone(), first.to_string());
    }
    paths
}

/// Recover every logical file of every version of every object under
/// `storage` with a JSON parser and SHA-2 only. Each content file is read
/// from the copy and checked against its manifest digest; a mismatch is an
/// `Err` naming the file, which is how a person with `sha256sum` would find
/// it.
pub fn recover_all(storage: &Path) -> Result<Vec<Recovered>, String> {
    let mut recovered = Vec::new();
    for root in object_roots(storage) {
        recover_object(storage, &root, &mut recovered)?;
    }
    Ok(recovered)
}

/// Recover one object into `out`.
fn recover_object(storage: &Path, root: &str, out: &mut Vec<Recovered>) -> Result<(), String> {
    let inventory = read_inventory(storage, root);
    let algorithm = inventory["digestAlgorithm"].as_str().expect("algorithm");
    let content = manifest_paths(&inventory);
    let versions = inventory["versions"].as_object().expect("versions");
    for (version, block) in versions {
        let state = block["state"].as_object().expect("state");
        for (digest, logical_paths) in state {
            let content_path = content
                .get(digest)
                .ok_or(format!("{root}: {digest} unlisted"))?;
            let file = storage.join(root).join(content_path);
            let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            if hex_digest(algorithm, &bytes) != *digest {
                return Err(format!("{root}/{content_path}: digest mismatch"));
            }
            for path in logical_paths.as_array().expect("logical paths") {
                out.push(Recovered {
                    object_root: root.to_string(),
                    version: version.clone(),
                    path: path.as_str().expect("path").to_string(),
                    bytes: bytes.clone(),
                });
            }
        }
    }
    Ok(())
}

/// The folder tree of one recovered version: each directory (`""` is the
/// top) with the names directly inside it, folders marked with a trailing
/// `/`.
pub fn folders(
    files: &[Recovered],
    root: &str,
    version: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut tree: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files
        .iter()
        .filter(|f| f.object_root == root && f.version == version)
    {
        let parts: Vec<&str> = file.path.split('/').collect();
        for depth in 0..parts.len() {
            let dir = parts[..depth].join("/");
            let leaf = if depth + 1 == parts.len() {
                parts[depth].to_string()
            } else {
                format!("{}/", parts[depth])
            };
            tree.entry(dir).or_default().insert(leaf);
        }
    }
    tree
}

/// The sidecar pairs of one recovered version: each `<file>.xmp` with the
/// file it describes, when both are present.
pub fn sidecar_pairs(files: &[Recovered], root: &str, version: &str) -> Vec<(String, String)> {
    let paths: BTreeSet<&str> = files
        .iter()
        .filter(|f| f.object_root == root && f.version == version)
        .map(|f| f.path.as_str())
        .collect();
    paths
        .iter()
        .filter_map(|p| p.strip_suffix(".xmp").map(|media| (media, *p)))
        .filter(|(media, _)| paths.contains(media))
        .map(|(media, sidecar)| (media.to_string(), sidecar.to_string()))
        .collect()
}

/// A path inside `dir`, joined from `/`-separated parts.
pub fn at(dir: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(dir.to_path_buf(), |p, part| p.join(part))
}

/// A JPEG-like original: not UTF-8, starts with bytes that are not text.
pub const JPEG_1: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
/// A second original.
pub const JPEG_2: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x20, b'E', b'x', b'i', b'f'];

/// Removes the scratch directory when the test ends.
pub struct Scratch(pub PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh scratch directory in the test target directory.
pub fn scratch(tag: &str) -> Scratch {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("io-1.1-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    Scratch(dir)
}

/// A completed snapshot: a storage root with `obj` (sha256, written by the
/// engine: v1 holds one photo, its sidecar and a note; v2 adds a photo and
/// sidecar and changes the note) and `zz/obj2` (sha512, one version).
pub fn completed_snapshot(dir: &Path) -> Rig<FsBackend> {
    let backend = FsBackend::new(dir.join("source"), dir.join("staging")).expect("fs backend");
    let rig = Rig::new(backend);
    declare_storage_root(&rig.storage);
    let v1 = rig.draft(
        "v1",
        &[
            ("albums/holiday/IMG_0001.jpg", JPEG_1),
            ("albums/holiday/IMG_0001.jpg.xmp", b"<xmp one/>"),
            ("docs/notes.md", b"first note"),
        ],
    );
    rig.commit("writer-a", None, &v1).expect("commit v1");
    let mut v2 = rig.draft(
        "v2",
        &[
            ("albums/holiday/IMG_0002.jpg", JPEG_2),
            ("albums/holiday/IMG_0002.jpg.xmp", b"<xmp two/>"),
            ("docs/notes.md", b"second note"),
        ],
    );
    v2.keep("albums/holiday/IMG_0001.jpg", sha256(JPEG_1));
    v2.keep("albums/holiday/IMG_0001.jpg.xmp", sha256(b"<xmp one/>"));
    rig.commit("writer-a", Some("v1"), &v2).expect("commit v2");
    let mut spec = ObjectSpec::new(
        "urn:example:two",
        vec![vec![("only.txt", b"only" as &[u8])]],
    );
    spec.algorithm = DigestAlgorithm::Sha512;
    build(&rig.storage, "zz/obj2", &spec);
    rig
}

/// Copy the snapshot with ordinary tools; return the copy's directory.
pub fn copy_of(dir: &Path) -> PathBuf {
    let copy = dir.join("copy");
    ordinary_copy(&dir.join("source"), &copy);
    copy
}

/// A backend over a copy, with staging outside it.
pub fn backend_over(copy: &Path, dir: &Path) -> FsBackend {
    FsBackend::new(copy, dir.join("copy-staging")).expect("fs backend over the copy")
}
