//! Enumerating committed logical state (IO-0.9; OCFL-S-004, T-039,
//! ADR 023 engine core item 6) on the in-memory and local filesystem
//! backends, without SQLite, PostgreSQL or FileSquid catalog types.
//!
//! Checked here:
//! - each file event names the object id, version, logical path, digest and
//!   content locator, and the locator holds bytes matching the digest;
//! - state resolves through the inventory: unchanged bytes in an earlier
//!   version directory resolve, unlisted files are absent, and a version
//!   above the committed head is not enumerated;
//! - the caller chooses each object's head or every committed version;
//! - a broken object yields one error naming its root and the enumeration
//!   goes on with the next object;
//! - every pinned `good-objects` and `warn-objects` fixture, stored in one
//!   storage root, enumerates exactly what an independent reading of its
//!   inventory JSON says (OCFL-S-010 (a), OCFL-S-011).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::read::{CommittedEvent, ReadError, StorageRoot, Versions};
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::{DigestAlgorithm, VersionNum};

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/enumerate.rs"]
mod enumerate;
#[path = "../support/fixtures.rs"]
mod fixtures;

use builder::{build, delete, put, write_inventory, ObjectSpec};
use enumerate::{
    check_content, declare_storage_root, describe_all, enumerate_all, file_rows, load_tree,
    object_rows, oracle_object_row, oracle_rows, put_key, ready, Item,
};

/// v2 renames b.txt to c.txt and adds d.txt; v3 changes a.txt and removes
/// d.txt.
fn three_versions(id: &str) -> ObjectSpec<'_> {
    ObjectSpec::new(
        id,
        vec![
            vec![("a.txt", b"alpha" as &[u8]), ("b.txt", b"bravo")],
            vec![
                ("a.txt", b"alpha"),
                ("c.txt", b"bravo"),
                ("d.txt", b"delta"),
            ],
            vec![("a.txt", b"alpha 2"), ("c.txt", b"bravo")],
        ],
    )
}

/// One version with one file.
fn one_file(id: &str) -> ObjectSpec<'_> {
    ObjectSpec::new(id, vec![vec![("only.txt", b"only" as &[u8])]])
}

/// A storage root holding `ab/obj1` (three versions) and `cd/obj2` (one).
fn two_objects<B: StorageBackend>(backend: &B) {
    declare_storage_root(backend);
    build(backend, "ab/obj1", &three_versions("urn:example:one"));
    build(backend, "cd/obj2", &one_file("urn:example:two"));
}

/// A scratch filesystem backend in the test target directory.
fn fs_backend(tag: &str) -> FsBackend {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("io-0.9-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend")
}

/// Removes a scratch backend's directory when the test ends.
struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The scratch directory that holds `backend`'s root and staging.
fn cleanup(backend: &FsBackend) -> Cleanup {
    Cleanup(backend.root().parent().expect("scratch dir").to_path_buf())
}

/// What a head enumeration of [`two_objects`] must give.
const HEAD_EVENTS: [&str; 8] = [
    "object ab/obj1 urn:example:one head v3",
    "version urn:example:one v3 files 2",
    "file urn:example:one v3 a.txt at ab/obj1/v3/content/a.txt",
    "file urn:example:one v3 c.txt at ab/obj1/v1/content/b.txt",
    "object cd/obj2 urn:example:two head v1",
    "version urn:example:two v1 files 1",
    "file urn:example:two v1 only.txt at cd/obj2/v1/content/only.txt",
    "done",
];

/// The described items followed by `done`.
fn described(items: &[Item]) -> Vec<String> {
    let mut words = describe_all(items);
    words.push("done".to_string());
    words
}

/// Check a head enumeration of [`two_objects`] on `backend`.
fn check_head_enumeration<B: StorageBackend>(backend: &B) {
    two_objects(backend);
    let items = enumerate_all(backend, Versions::Head);
    assert_eq!(described(&items), HEAD_EVENTS);
    assert_eq!(check_content(backend, &items), 3);
    let Ok(CommittedEvent::File(file)) = &items[2] else {
        panic!("the third event is a file: {:?}", items[2]);
    };
    assert_eq!(file.object_id, "urn:example:one");
    assert_eq!(file.version, VersionNum::parse("v3").expect("version"));
    assert_eq!(file.path, "a.txt");
    assert_eq!(file.digest_algorithm, DigestAlgorithm::Sha256);
    assert_eq!(file.digest, DigestAlgorithm::Sha256.digest_hex(b"alpha 2"));
    assert_eq!(file.content_key.as_str(), "ab/obj1/v3/content/a.txt");
}

#[test]
fn head_events_name_every_field_of_each_committed_file_in_memory() {
    check_head_enumeration(&MemoryBackend::new());
}

#[test]
fn head_events_name_every_field_of_each_committed_file_on_a_filesystem() {
    let backend = fs_backend("head");
    let _cleanup = cleanup(&backend);
    check_head_enumeration(&backend);
}

#[test]
fn every_committed_version_is_enumerated_oldest_first() {
    let backend = MemoryBackend::new();
    declare_storage_root(&backend);
    build(&backend, "obj", &three_versions("urn:example:one"));
    let items = enumerate_all(&backend, Versions::All);
    let expected = [
        "object obj urn:example:one head v3",
        "version urn:example:one v1 files 2",
        "file urn:example:one v1 a.txt at obj/v1/content/a.txt",
        "file urn:example:one v1 b.txt at obj/v1/content/b.txt",
        "version urn:example:one v2 files 3",
        "file urn:example:one v2 a.txt at obj/v1/content/a.txt",
        "file urn:example:one v2 c.txt at obj/v1/content/b.txt",
        "file urn:example:one v2 d.txt at obj/v2/content/d.txt",
        "version urn:example:one v3 files 2",
        "file urn:example:one v3 a.txt at obj/v3/content/a.txt",
        "file urn:example:one v3 c.txt at obj/v1/content/b.txt",
    ];
    assert_eq!(describe_all(&items), expected);
    assert_eq!(check_content(&backend, &items), 7);
}

#[test]
fn files_the_inventory_does_not_list_are_not_enumerated() {
    let backend = MemoryBackend::new();
    two_objects(&backend);
    put(&backend, "ab/obj1", "v3/content/stray.txt", b"not listed");
    put(&backend, "ab/obj1", "v1/content/c.txt", b"not listed");
    let items = enumerate_all(&backend, Versions::All);
    let words = describe_all(&items).join("\n");
    assert!(!words.contains("stray.txt"), "{words}");
    assert!(!words.contains("v1/content/c.txt"), "{words}");
}

#[test]
fn a_version_above_the_committed_head_is_not_enumerated() {
    let backend = MemoryBackend::new();
    two_objects(&backend);
    let mut spec = three_versions("urn:example:one");
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let next = build(&MemoryBackend::new(), "x", &spec);
    put(&backend, "ab/obj1", "v4/content/e.txt", b"echo");
    write_inventory(&backend, "ab/obj1", "v4", &next);
    let items = enumerate_all(&backend, Versions::All);
    let words = describe_all(&items).join("\n");
    assert!(words.contains("object ab/obj1 urn:example:one head v3"));
    assert!(!words.contains("v4"), "{words}");
}

#[test]
fn a_root_inventory_its_sidecar_does_not_commit_reports_an_unpublished_head() {
    let backend = MemoryBackend::new();
    two_objects(&backend);
    let mut spec = three_versions("urn:example:one");
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let next = build(&MemoryBackend::new(), "x", &spec);
    put(&backend, "ab/obj1", "v4/content/e.txt", b"echo");
    write_inventory(&backend, "ab/obj1", "v4", &next);
    put(&backend, "ab/obj1", "inventory.json", &next.to_json());
    let items = enumerate_all(&backend, Versions::Head);
    let Ok(CommittedEvent::Object(object)) = &items[0] else {
        panic!("the first event is an object: {:?}", items[0]);
    };
    assert_eq!(object.head.to_string(), "v3");
    let unpublished = object.unpublished_head.as_ref().map(ToString::to_string);
    assert_eq!(unpublished.as_deref(), Some("v4"));
    assert_eq!(described(&items), HEAD_EVENTS);
}

#[test]
fn a_broken_object_yields_one_error_and_the_next_object_is_still_enumerated() {
    let backend = MemoryBackend::new();
    two_objects(&backend);
    build(&backend, "b/broken", &one_file("urn:example:broken"));
    delete(&backend, "b/broken", "inventory.json");
    let items = enumerate_all(&backend, Versions::Head);
    let words = describe_all(&items);
    let error_at = words
        .iter()
        .position(|w| w == "error b/broken E063")
        .unwrap_or_else(|| panic!("no E063 error for b/broken in {words:?}"));
    assert_eq!(
        words[error_at + 1],
        "object cd/obj2 urn:example:two head v1"
    );
    assert_eq!(words.iter().filter(|w| w.starts_with("error")).count(), 1);
    assert_eq!(file_rows(&items).len(), 3);
}

#[test]
fn an_object_whose_sidecar_or_head_inventory_is_wrong_is_refused_with_its_code() {
    let backend = MemoryBackend::new();
    declare_storage_root(&backend);
    for (root, file, code) in [
        ("e058", "inventory.json.sha256", "E058"),
        ("e060", "inventory.json.sha256", "E060"),
        ("e064", "v1/inventory.json", "E064"),
    ] {
        build(&backend, root, &one_file("urn:example:wrong"));
        match code {
            "E058" => delete(&backend, root, file),
            _ => put(&backend, root, file, b"0000 inventory.json\n"),
        }
    }
    let items = enumerate_all(&backend, Versions::All);
    assert_eq!(
        describe_all(&items),
        ["error e058 E058", "error e060 E060", "error e064 E064"]
    );
}

#[test]
fn an_empty_storage_root_enumerates_nothing() {
    let backend = MemoryBackend::new();
    declare_storage_root(&backend);
    assert!(enumerate_all(&backend, Versions::All).is_empty());
}

#[test]
fn a_git_directory_in_an_object_root_stops_the_storage_root_from_opening() {
    let backend = MemoryBackend::new();
    two_objects(&backend);
    put_key(&backend, "cd/obj2/.git/HEAD", b"ref: refs/heads/main\n");
    let err = ready(StorageRoot::open(&backend)).expect_err("a .git in an object root");
    assert!(matches!(err, ReadError::Guard(_)), "{err:?}");
}

/// Store every object of the fixture groups in one storage root, under
/// `<group>/<name>`, and return `(root, fixture directory)` for each.
fn load_fixture_groups<B: StorageBackend>(backend: &B, groups: &[&str]) -> Vec<(String, PathBuf)> {
    let tree = fixtures::tree();
    declare_storage_root(backend);
    let mut objects = Vec::new();
    for group in groups {
        let dir = tree.join(group);
        for name in fixtures::objects(&dir) {
            let root = format!("{group}/{name}");
            load_tree(backend, &dir.join(&name), &root);
            objects.push((root, dir.join(&name)));
        }
    }
    objects
}

/// The oracle's rows for every loaded fixture, in root order.
fn oracle_for(objects: &[(String, PathBuf)], all: bool) -> Vec<String> {
    let mut sorted: Vec<&(String, PathBuf)> = objects.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted
        .iter()
        .flat_map(|(root, dir)| oracle_rows(dir, root, all))
        .collect()
}

/// Enumerate every good and warning fixture with `versions` and compare
/// with the oracle; return the number of file events.
fn check_fixtures<B: StorageBackend>(backend: &B, versions: Versions, all: bool) -> usize {
    let objects = load_fixture_groups(backend, &["good-objects", "warn-objects"]);
    let expected_objects: usize = fixtures::PINNED_GROUPS[..2].iter().map(|g| g.1).sum();
    assert_eq!(objects.len(), expected_objects);
    let items = enumerate_all(backend, versions);
    let errors: Vec<String> = describe_all(&items)
        .into_iter()
        .filter(|w| w.starts_with("error"))
        .collect();
    assert!(errors.is_empty(), "fixtures refused: {errors:?}");
    let mut sorted: Vec<&(String, PathBuf)> = objects.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let expected_rows: Vec<String> = sorted
        .iter()
        .map(|(root, dir)| oracle_object_row(dir, root))
        .collect();
    assert_eq!(object_rows(&items), expected_rows);
    assert!(
        expected_rows.iter().any(|row| row.ends_with(" sha512"))
            && expected_rows.iter().any(|row| row.ends_with(" sha256")),
        "the fixtures cover both content-addressing algorithms"
    );
    assert_eq!(file_rows(&items), oracle_for(&objects, all));
    check_content(backend, &items)
}

#[test]
fn every_good_and_warning_fixture_enumerates_as_its_inventory_says_in_memory() {
    let all = check_fixtures(&MemoryBackend::new(), Versions::All, true);
    let head = check_fixtures(&MemoryBackend::new(), Versions::Head, false);
    println!("in memory: {all} file events over every version, {head} at head");
    assert!(all > head && head > 0);
}

#[test]
fn every_good_and_warning_fixture_enumerates_as_its_inventory_says_on_a_filesystem() {
    let backend = fs_backend("fixtures");
    let _cleanup = cleanup(&backend);
    let files = check_fixtures(&backend, Versions::All, true);
    println!("on a filesystem: {files} file events over every version");
}

/// Bad fixtures that are discovered as objects but cannot be opened, with
/// the code their refusal carries.
const BAD_OPEN: [(&str, &str); 5] = [
    ("E007_bad_declaration_contents", "E007"),
    ("E058_no_sidecar", "E058"),
    ("E060_E064_root_inventory_digest_mismatch", "E060"),
    ("E063_no_inv", "E063"),
    ("E064_different_root_and_latest_inventories", "E064"),
];

#[test]
fn bad_fixtures_among_good_ones_are_reported_and_the_good_ones_still_enumerate() {
    let backend = MemoryBackend::new();
    let tree = fixtures::tree();
    declare_storage_root(&backend);
    for (name, _) in BAD_OPEN {
        load_tree(&backend, &tree.join("bad-objects").join(name), name);
    }
    let good = tree.join("good-objects").join("spec-ex-full");
    load_tree(&backend, &good, "spec-ex-full");
    let items = enumerate_all(&backend, Versions::All);
    let errors: Vec<String> = describe_all(&items)
        .into_iter()
        .filter(|w| w.starts_with("error"))
        .collect();
    let expected: Vec<String> = BAD_OPEN
        .iter()
        .map(|(name, code)| format!("error {name} {code}"))
        .collect();
    assert_eq!(errors, expected);
    assert_eq!(file_rows(&items), oracle_rows(&good, "spec-ex-full", true));
    assert_eq!(check_content(&backend, &items), file_rows(&items).len());
}

/// The fixture tree must exist so these tests never skip (OCFL-S-011).
#[test]
fn the_pinned_fixture_tree_is_present() {
    assert!(Path::new(&fixtures::tree()).join("warn-objects").is_dir());
}
