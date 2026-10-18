//! Step definitions for the content-fixture write scenario of
//! `OCFL_1.1_fixture_conformance.feature` (OCFL-S-010 (b)) and for
//! `read_only_copy_OCFL-S-019.feature` (IO-0.7).
//!
//! Content fixtures are written through the engine writer into a real
//! directory, so the community validators (`crate::community`) can read
//! them. Copy scenarios use the write rig of `write_order_OCFL-S-018`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use cucumber::{given, then, when};
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, ObjectMeta, StorageBackend, StorageKey};
use filesquid_ocfl::validate::validate_object;

use crate::community::validators_or_skip;
use crate::content::{assert_state_matches, fixture_names, load, write_fixture};
use crate::rig::{key, ready, v, ROOT};
use crate::write_steps::rig;
use crate::{OcflWorld, Scratch};

/// What the payload scenarios carry between steps.
#[derive(Debug, Default)]
pub struct PayloadState {
    storage: Option<FsBackend>,
    scratch: Option<Scratch>,
    written: Vec<String>,
    other: Option<MemoryBackend>,
    copy: Option<Result<ObjectMeta, ReadError>>,
    before: Option<Option<Vec<u8>>>,
}

/// The directory storage the content fixtures were written into.
fn storage(world: &OcflWorld) -> &FsBackend {
    world
        .payload
        .storage
        .as_ref()
        .expect("When I write every content fixture")
}

#[when("I write one object version per version directory of every content fixture")]
fn write_content_fixtures(world: &mut OcflWorld) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-payload-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    world.payload.written = fixture_names();
    for name in &world.payload.written {
        write_fixture(&storage, name, &load(name));
    }
    world.payload.storage = Some(storage);
    world.payload.scratch = Some(Scratch(dir));
}

#[then("every written object should validate with zero errors in filesquid-ocfl")]
fn engine_accepts_written(world: &mut OcflWorld) {
    let storage = storage(world);
    assert_eq!(world.payload.written.len(), 7, "content fixtures written");
    for name in &world.payload.written {
        let report = ready(validate_object(storage, name)).expect("validation ran");
        assert!(report.errors().is_empty(), "{name}: {:?}", report.errors());
    }
}

#[then(
    "every written object should validate with zero errors in an independent community validator"
)]
fn community_accepts_written(world: &mut OcflWorld) {
    let storage = storage(world);
    for validator in validators_or_skip() {
        for name in &world.payload.written {
            let verdict = validator.run_validation(&storage.root().join(name));
            assert!(
                verdict.valid,
                "{} rejected {name}:\n{}",
                validator.name(),
                verdict.output
            );
        }
    }
}

#[then("the logical state of every written version should equal its source content")]
fn written_state_matches(world: &mut OcflWorld) {
    let storage = storage(world);
    for name in &world.payload.written {
        assert_state_matches(storage, name, &load(name));
    }
}

#[given("the storage holds an OCFL storage root declaration")]
fn storage_root_declaration(world: &mut OcflWorld) {
    let declaration = key("0=ocfl_1.1");
    ready(rig(world).storage.put(&declaration, b"ocfl_1.1\n")).expect("declare");
}

/// The bytes at `raw` in the rig's storage, or `None` when absent.
fn stored_bytes(world: &OcflWorld, raw: &str) -> Option<Vec<u8>> {
    let rig = rig(world);
    rig.stored(raw).then(|| rig.bytes(raw))
}

#[when(expr = "I copy {string} of {string} to {string} in the same storage")]
fn copy_in_same_storage(world: &mut OcflWorld, path: String, version: String, dest: String) {
    world.payload.before = Some(stored_bytes(world, &dest));
    let rig = rig(world);
    let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
    let dest = key(&dest);
    let result = ready(object.copy_file(&v(&version), &path, &rig.storage, &dest, None));
    world.payload.copy = Some(result);
}

#[when(expr = "I copy {string} of {string} to {string} on another backend")]
fn copy_to_other_backend(world: &mut OcflWorld, path: String, version: String, dest: String) {
    let other = MemoryBackend::new();
    let rig = rig(world);
    let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
    let result = ready(object.copy_file(&v(&version), &path, &other, &key(&dest), None));
    world.payload.copy = Some(result);
    world.payload.other = Some(other);
}

#[then(expr = "the copy should hold the bytes of {string}")]
fn copy_holds_bytes(world: &mut OcflWorld, path: String) {
    let meta = world
        .payload
        .copy
        .take()
        .expect("a copy ran")
        .expect("copied");
    let other = world.payload.other.as_ref().expect("another backend");
    let page = ready(other.list("", None, 10)).expect("list");
    let copied: &StorageKey = page.keys.first().expect("one copied file");
    let bytes = ready(filesquid_ocfl::storage::read_verified(
        other,
        copied,
        1 << 20,
    ))
    .expect("read");
    let expected = rig(world).bytes(&format!("{ROOT}/v1/content/{path}"));
    assert_eq!(bytes, expected);
    assert_eq!(meta.size, expected.len() as u64);
}

#[then("the copy should be refused as a write into an OCFL root")]
fn copy_refused(world: &mut OcflWorld) {
    let result = world.payload.copy.take().expect("a copy ran");
    assert!(
        matches!(result, Err(ReadError::CopyIntoOcflRoot { .. })),
        "{result:?}"
    );
}

#[then(expr = "{string} should be unchanged")]
fn destination_unchanged(world: &mut OcflWorld, dest: String) {
    let before = world
        .payload
        .before
        .take()
        .expect("the copy recorded the destination");
    assert_eq!(stored_bytes(world, &dest), before, "{dest} changed");
}
