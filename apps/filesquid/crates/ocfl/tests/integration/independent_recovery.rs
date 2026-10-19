//! Independent recovery of a completed snapshot (IO-1.1; OCFL-S-022,
//! OCFL-S-019, F-001, F-042, RC-010). The snapshot is written by the
//! engine's writer to a local filesystem root, copied with ordinary
//! filesystem tools, and then read back three ways: by the engine, by an
//! independent reader that uses only a JSON parser and SHA-2, and (when one
//! is installed) by a community validator. Altered bytes and inventories in
//! the copy must be detected.

use std::path::Path;

use filesquid_ocfl::read::{Object, StorageRoot, Versions};
use filesquid_ocfl::storage::{read_verified, FsBackend, MAX_CHUNK_BYTES};
use filesquid_ocfl::validate::{validate_object, ValidationReport};
use filesquid_ocfl::write::CommitError;
use filesquid_ocfl::VersionNum;

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/community_validator.rs"]
mod community;
#[path = "../support/enumerate.rs"]
mod enumerate;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/recovery.rs"]
mod recovery;
#[path = "../support/write_rig.rs"]
mod rig;

use enumerate::{check_content, enumerate_all, file_rows, ready};
use recovery::{
    at, backend_over, completed_snapshot, copy_of, hex_digest, scratch, snapshot, JPEG_1, JPEG_2,
};
use rig::{key, sha256, Rig};

/// Validate the object at `root` in `backend`.
fn validate(backend: &FsBackend, root: &str) -> ValidationReport {
    ready(validate_object(backend, root)).expect("validation ran")
}

/// What the copy's logical files should be at each version of `obj`.
fn expected_obj() -> Vec<(&'static str, &'static str, Vec<u8>)> {
    vec![
        ("v1", "albums/holiday/IMG_0001.jpg", JPEG_1.to_vec()),
        (
            "v1",
            "albums/holiday/IMG_0001.jpg.xmp",
            b"<xmp one/>".to_vec(),
        ),
        ("v1", "docs/notes.md", b"first note".to_vec()),
        ("v2", "albums/holiday/IMG_0001.jpg", JPEG_1.to_vec()),
        (
            "v2",
            "albums/holiday/IMG_0001.jpg.xmp",
            b"<xmp one/>".to_vec(),
        ),
        ("v2", "albums/holiday/IMG_0002.jpg", JPEG_2.to_vec()),
        (
            "v2",
            "albums/holiday/IMG_0002.jpg.xmp",
            b"<xmp two/>".to_vec(),
        ),
        ("v2", "docs/notes.md", b"second note".to_vec()),
    ]
}

#[test]
fn a_copy_made_with_ordinary_tools_opens_validates_and_enumerates_like_the_source() {
    let dir = scratch("copy");
    let rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    let backend = backend_over(&copy, &dir.0);

    for root in ["obj", "zz/obj2"] {
        let report = validate(&backend, root);
        assert!(report.errors().is_empty(), "{root}: {:?}", report.errors());
    }
    let from_copy = enumerate_all(&backend, Versions::All);
    let from_source = enumerate_all(&rig.storage, Versions::All);
    assert_eq!(file_rows(&from_copy), file_rows(&from_source));
    assert_eq!(
        file_rows(&from_copy).len(),
        9,
        "8 files of obj and 1 of obj2"
    );
    assert_eq!(check_content(&backend, &from_copy), 9);
    assert_eq!(
        snapshot(&dir.0.join("source")),
        snapshot(&copy),
        "an exact copy"
    );
}

#[test]
fn the_copy_holds_only_ocfl_files_no_database_staging_or_git() {
    let dir = scratch("only-ocfl");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);

    let names: Vec<String> = snapshot(&copy).into_keys().collect();
    let object_files = |n: &str| {
        n == "0=ocfl_1.1"
            || n.contains("/0=ocfl_object_1.1")
            || n.contains("inventory.json")
            || n.contains("/content/")
    };
    let stray: Vec<&String> = names.iter().filter(|n| !object_files(n)).collect();
    assert!(stray.is_empty(), "files that are not OCFL: {stray:?}");
    assert!(names.iter().all(|n| !n.contains(".git")), "{names:?}");
}

#[test]
fn an_independent_reader_rebuilds_files_folders_and_sidecar_pairs_from_the_copy() {
    let dir = scratch("independent");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);

    let files = recovery::recover_all(&copy).expect("every digest matches");
    for (version, path, bytes) in expected_obj() {
        let found = files
            .iter()
            .find(|f| f.object_root == "obj" && f.version == version && f.path == path)
            .unwrap_or_else(|| panic!("{version} {path} not recovered"));
        assert_eq!(found.bytes, bytes, "{version} {path}");
    }
    let only = files
        .iter()
        .find(|f| f.object_root == "zz/obj2")
        .expect("obj2");
    assert_eq!(
        (only.version.as_str(), only.bytes.as_slice()),
        ("v1", &b"only"[..])
    );

    let v2 = recovery::folders(&files, "obj", "v2");
    assert_eq!(
        v2[""].iter().cloned().collect::<Vec<_>>(),
        ["albums/", "docs/"]
    );
    assert_eq!(
        v2["albums"].iter().cloned().collect::<Vec<_>>(),
        ["holiday/"]
    );
    assert_eq!(v2["albums/holiday"].len(), 4);
    let pairs = recovery::sidecar_pairs(&files, "obj", "v2");
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].0, "albums/holiday/IMG_0001.jpg");
    assert_eq!(pairs[1].1, "albums/holiday/IMG_0002.jpg.xmp");
    assert_eq!(recovery::sidecar_pairs(&files, "obj", "v1").len(), 1);
}

#[test]
fn inspecting_the_copy_changes_neither_the_copy_nor_the_source() {
    let dir = scratch("immutable");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    let before_source = snapshot(&dir.0.join("source"));
    let before_copy = snapshot(&copy);
    let backend = backend_over(&copy, &dir.0);

    let root = ready(StorageRoot::open(&backend)).expect("open the storage root");
    let object = ready(root.open_object("obj")).expect("open obj");
    let state = object
        .state(&VersionNum::parse("v2").expect("v2"))
        .expect("state");
    assert_eq!(state.len(), 5);
    let _ = validate(&backend, "obj");
    let items = enumerate_all(&backend, Versions::All);
    assert_eq!(check_content(&backend, &items), 9);
    let _ = recovery::recover_all(&copy).expect("recover");

    assert_eq!(snapshot(&copy), before_copy, "the copy is unchanged");
    assert_eq!(
        snapshot(&dir.0.join("source")),
        before_source,
        "so is the source"
    );
}

#[test]
fn a_changed_original_is_reported_as_a_digest_error_and_never_served() {
    let dir = scratch("flip");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    recovery::flip_first_byte(&at(&copy, "obj/v1/content/albums/holiday/IMG_0001.jpg"));
    let backend = backend_over(&copy, &dir.0);

    let report = validate(&backend, "obj");
    assert!(
        report.error_codes().contains("E092"),
        "{:?}",
        report.errors()
    );
    let content = key("obj/v1/content/albums/holiday/IMG_0001.jpg");
    let read = ready(read_verified(&backend, &content, MAX_CHUNK_BYTES)).expect("raw read");
    assert_ne!(
        hex_digest("sha256", &read),
        sha256(JPEG_1),
        "the bytes changed"
    );
    assert_eq!(stream_end("obj", &backend), Some("E092"), "never verified");
    let independent = recovery::recover_all(&copy).expect_err("sha256sum finds it too");
    assert!(independent.contains("digest mismatch"), "{independent}");
    let items = enumerate_all(&backend, Versions::Head);
    let other_files = file_rows(&items).len();
    assert!(
        other_files > 0,
        "the engine still enumerates the other files"
    );
}

/// Stream `IMG_0001.jpg` of v1 to its end and return the OCFL code of the
/// error that stopped it, or `None` when the stream ended as verified.
fn stream_end(root: &str, backend: &FsBackend) -> Option<&'static str> {
    let object = ready(Object::open(backend, root)).expect("the inventory is intact");
    let v1 = VersionNum::parse("v1").expect("v1");
    let path = "albums/holiday/IMG_0001.jpg";
    let mut stream = object
        .stream_file(&v1, path, MAX_CHUNK_BYTES)
        .expect("stream");
    for _ in 0..4 {
        match ready(stream.next_chunk()) {
            Ok(Some(_)) => continue,
            Ok(None) => return None,
            Err(err) => return err.code(),
        }
    }
    panic!("the stream neither ended nor failed within 4 chunks");
}

#[test]
fn a_changed_root_inventory_with_its_sidecar_untouched_is_refused_on_open() {
    let dir = scratch("inventory");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    recovery::replace_text(
        &at(&copy, "obj/inventory.json"),
        "docs/notes.md",
        "docs/notez.md",
    );
    let backend = backend_over(&copy, &dir.0);

    let report = validate(&backend, "obj");
    assert!(
        report.error_codes().contains("E060"),
        "{:?}",
        report.errors()
    );
    let Err(err) = ready(Object::open(&backend, "obj")) else {
        panic!("open must be refused");
    };
    assert_eq!(err.code(), Some("E060"), "{err}");
}

/// Change the digest of `IMG_0002.jpg` to the digest of other bytes in the
/// manifest and every state of the root inventory, then recompute its sidecar
/// as an attacker covering their tracks would.
fn rewrite_inventory_and_sidecar(copy: &Path) {
    let inventory = at(copy, "obj/inventory.json");
    recovery::replace_text(&inventory, &sha256(JPEG_2), &sha256(b"forged"));
    recovery::rewrite_sidecar(&inventory, "sha256");
}

#[test]
fn a_changed_inventory_with_a_recomputed_sidecar_is_still_detected() {
    let dir = scratch("forged");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    rewrite_inventory_and_sidecar(&copy);
    let backend = backend_over(&copy, &dir.0);

    let report = validate(&backend, "obj");
    assert!(
        report.error_codes().contains("E092"),
        "{:?}",
        report.errors()
    );
    let independent = recovery::recover_all(&copy).expect_err("the content no longer matches");
    assert!(independent.contains("digest mismatch"), "{independent}");
}

#[test]
fn a_changed_earlier_version_inventory_is_detected() {
    let dir = scratch("earlier");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    recovery::replace_text(
        &at(&copy, "obj/v1/inventory.json"),
        "docs/notes.md",
        "docs/notez.md",
    );
    let backend = backend_over(&copy, &dir.0);

    let report = validate(&backend, "obj");
    assert!(
        !report.is_valid(),
        "an edited v1 inventory must not validate"
    );
    assert!(
        report.error_codes().contains("E060"),
        "{:?}",
        report.errors()
    );
}

#[test]
fn a_copy_without_a_head_version_inventory_stays_refused_and_is_not_repaired() {
    let dir = scratch("w010");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    std::fs::remove_file(at(&copy, "obj/v2/inventory.json")).expect("remove");
    std::fs::remove_file(at(&copy, "obj/v2/inventory.json.sha256")).expect("remove");
    let before = snapshot(&copy);
    let backend = backend_over(&copy, &dir.0);

    let report = validate(&backend, "obj");
    assert!(
        report.warning_codes().contains("W010"),
        "{:?}",
        report.warnings()
    );
    assert!(report.errors().is_empty(), "{:?}", report.errors());
    let outcome = commit_on_copy(&copy, &dir.0);
    assert!(
        matches!(outcome, Err(CommitError::MissingVersionInventory { .. })),
        "{outcome:?}"
    );
    let _ = enumerate_all(&backend, Versions::All);
    assert_eq!(snapshot(&copy), before, "nothing repaired it");
}

/// Try to commit a v3 onto the copy as a writer would.
fn commit_on_copy(copy: &Path, dir: &Path) -> Result<(), CommitError> {
    let rig = Rig::new(backend_over(copy, dir));
    let mut draft = rig.draft("v3", &[("docs/notes.md", b"third note")]);
    draft.keep("albums/holiday/IMG_0001.jpg", sha256(JPEG_1));
    rig.commit("writer-b", Some("v2"), &draft).map(|_| ())
}

/// Run every community validator found on both the copy and a tampered copy.
#[test]
fn a_community_validator_accepts_the_copy_and_rejects_a_tampered_one() {
    let validators = community::validators_or_skip();
    let dir = scratch("community");
    let _rig = completed_snapshot(&dir.0);
    let copy = copy_of(&dir.0);
    for validator in &validators {
        let verdict = validator.run_validation(&copy.join("obj"));
        assert!(
            verdict.valid,
            "{} rejected the copy: {}",
            validator.name(),
            verdict.output
        );
    }
    recovery::flip_first_byte(&at(&copy, "obj/v1/content/albums/holiday/IMG_0001.jpg"));
    for validator in &validators {
        let verdict = validator.run_validation(&copy.join("obj"));
        assert!(
            !verdict.valid,
            "{} accepted a changed original",
            validator.name()
        );
    }
}
