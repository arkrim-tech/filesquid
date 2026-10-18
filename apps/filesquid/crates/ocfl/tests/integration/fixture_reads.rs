//! Read conformance against the pinned OCFL 1.1 community fixtures
//! (OCFL-S-010 (a), OCFL-S-011; `OCFL/fixtures` commit
//! `d323fc207126becec066998b43408d998bf0fb1b`).
//!
//! The fixtures are committed under `tests/fixtures/ocfl/1.1` (IO-0.5), so
//! this test always runs; `FILESQUID_OCFL_FIXTURES` may name another copy.
//!
//! Checked here, through `FsBackend` on the fixture directories:
//! - every version, `v1` to `head`, of every `good-objects` fixture resolves
//!   through inventory `state` and `manifest`, and every logical file streams
//!   with bytes that match its inventory digest (checked by the reader and
//!   again here);
//! - every `warn-objects` fixture reads the same way (warnings are not
//!   errors);
//! - the bad fixtures whose fault a reader meets are refused with the code
//!   their name lists ([`READ_REFUSALS`]).

use std::path::{Path, PathBuf};

use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{complete_immediately, FsBackend, MAX_CHUNK_BYTES};
use filesquid_ocfl::VersionNum;

/// Bad fixtures a reader must refuse, with the code it must give: faults in
/// the declaration, the root inventory's presence and sidecar, the head
/// version inventory, and content bytes. Rules about one inventory's JSON are
/// IO-0.3's (`integration_fixture_inventories`); the rest is structural
/// validation (IO-0.5).
///
/// `E092_E093_content_path_does_not_exist` is not here: its manifest lists
/// two content paths for one digest and only the second is missing, so the
/// logical file reads correctly from the first. The validator reports the
/// missing path (`integration_fixture_conformance`).
const READ_REFUSALS: &[(&str, &str)] = &[
    ("E003_E063_empty", "E003"),
    ("E003_no_decl", "E003"),
    ("E007_bad_declaration_contents", "E007"),
    ("E058_no_sidecar", "E058"),
    ("E060_E064_root_inventory_digest_mismatch", "E060"),
    ("E063_no_inv", "E063"),
    ("E064_different_root_and_latest_inventories", "E064"),
    ("E092_content_file_digest_mismatch", "E092"),
];

/// Objects in `1.1/good-objects` at the pinned commit.
const PINNED_GOOD_OBJECTS: usize = 12;

/// Objects in `1.1/warn-objects` at the pinned commit.
const PINNED_WARN_OBJECTS: usize = 13;

#[path = "../support/fixtures.rs"]
mod fixtures;

/// A read-only backend over one fixture group (such as `good-objects`),
/// with its staging directory in the test target directory.
fn backend(group: &Path) -> FsBackend {
    let name = group.file_name().expect("group name").to_string_lossy();
    let staging = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("fixture-reads-{name}"));
    FsBackend::new(group, staging).expect("fs backend")
}

/// The fixture object names in `group`, sorted.
fn objects(group: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(group)
        .expect("list fixtures")
        .map(|entry| entry.expect("entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Drive a filesystem future, which never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("fs futures complete at once")
}

/// Stream one logical file to its end and return its bytes.
fn stream(
    object: &Object<'_, FsBackend>,
    version: &VersionNum,
    path: &str,
) -> Result<Vec<u8>, ReadError> {
    let mut reader = object.stream_file(version, path, MAX_CHUNK_BYTES)?;
    let mut out = Vec::new();
    for _ in 0..1_024 {
        match ready(reader.next_chunk())? {
            Some(bytes) => out.extend_from_slice(&bytes),
            None => return Ok(out),
        }
    }
    panic!("{path} did not end within 1024 chunks");
}

/// Resolve every version of one object and stream every logical file.
/// Returns the number of versions and files read.
fn read_whole(object: &Object<'_, FsBackend>) -> Result<(usize, usize), ReadError> {
    let algorithm = object.inventory().digest_algorithm;
    let mut files = 0;
    for info in object.history() {
        let state = object.state(&info.version)?;
        for file in state.files() {
            let bytes = stream(object, &info.version, &file.path)?;
            assert!(
                algorithm.matches(&algorithm.digest_hex(&bytes), &file.digest),
                "{} {} does not match {}",
                info.version,
                file.path,
                file.digest
            );
            files += 1;
        }
    }
    Ok((object.history().len(), files))
}

/// Read every object in `group`, which must hold exactly `expected`
/// objects at the pinned commit; every one must read without error.
fn read_group(dir: &Path, group: &str, expected: usize) {
    let root = dir.join(group);
    let backend = backend(&root);
    let names = objects(&root);
    assert_eq!(
        names.len(),
        expected,
        "{group} must hold the {expected} objects of the pinned commit"
    );
    for name in &names {
        let opened = ready(Object::open(&backend, name));
        let object = opened.unwrap_or_else(|err| panic!("{group}/{name}: open: {err}"));
        assert_eq!(object.history().len().to_string(), object.head().number());
        let (versions, files) =
            read_whole(&object).unwrap_or_else(|err| panic!("{group}/{name}: {err}"));
        println!("{group}/{name}: {versions} versions, {files} logical files read");
    }
    println!("{group}: {} objects read", names.len());
}

#[test]
fn every_version_of_every_good_fixture_reads_with_matching_digests() {
    let dir = fixtures::tree();
    read_group(&dir, "good-objects", PINNED_GOOD_OBJECTS);
}

#[test]
fn every_warning_fixture_reads_with_matching_digests() {
    let dir = fixtures::tree();
    read_group(&dir, "warn-objects", PINNED_WARN_OBJECTS);
}

/// Open and read one bad fixture and return the first refusal.
fn first_refusal(backend: &FsBackend, name: &str) -> ReadError {
    let object = match ready(Object::open(backend, name)) {
        Ok(object) => object,
        Err(err) => return err,
    };
    read_whole(&object).expect_err("the fixture must be refused")
}

#[test]
fn bad_fixtures_a_reader_meets_are_refused_with_their_codes() {
    let dir = fixtures::tree();
    let backend = backend(&dir.join("bad-objects"));
    for (name, expected) in READ_REFUSALS {
        let err = first_refusal(&backend, name);
        println!("bad-objects/{name}: refused with {:?}: {err}", err.code());
        assert_eq!(err.code(), Some(*expected), "{name}: {err}");
    }
}
