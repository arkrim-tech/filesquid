//! Step definitions for `opaque_bytes_OCFL-S-020.feature` (IO-0.8): the
//! engine stores opaque bytes, and the owner-readable and sealed profiles
//! are caller transforms over the same engine calls.
//!
//! Objects are written on an in-memory backend through the engine's only
//! write path (`crate::content::write_fixture`); the sealed profile uses
//! the test-only `crate::opaque::ToyCipher`.

use cucumber::{given, then, when};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::Inventory;
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{
    read_verified, MemoryBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::VersionNum;
use sha2::{Digest, Sha256};

use crate::content::{
    assert_state_matches, load, ready, write_fixture, written_state, ContentFixture, SourceVersion,
};
use crate::opaque::{engine_profile_findings, hex, ToyCipher};
use crate::OcflWorld;

/// Most chunks read from one logical file in a step.
const MAX_CHUNKS: usize = 16;

/// What the opaque-bytes scenarios carry between steps.
#[derive(Debug, Default)]
pub struct OpaqueState {
    storage: MemoryBackend,
    /// The plaintext the caller started from.
    source: Option<ContentFixture>,
    /// What the engine was given: the plaintext or its sealed form.
    stored: Option<ContentFixture>,
    /// Whether `stored` was sealed.
    sealed: bool,
    /// The outcome of the last read of one logical file.
    read: Option<Result<Vec<u8>, ReadError>>,
}

/// The test caller's key.
fn cipher() -> ToyCipher {
    ToyCipher::new("io08-bdd-key")
}

/// The fixture the engine was given.
fn stored(world: &OcflWorld) -> &ContentFixture {
    world
        .opaque
        .stored
        .as_ref()
        .expect("When a caller writes an object")
}

/// Write `fixture` as given (`sealed` false) or sealed, and remember both.
fn write(world: &mut OcflWorld, fixture: ContentFixture, sealed: bool) {
    let given = if sealed {
        cipher().seal_fixture(&fixture)
    } else {
        fixture.clone()
    };
    write_fixture(&world.opaque.storage, &given.name, &given);
    world.opaque.source = Some(fixture);
    world.opaque.stored = Some(given);
    world.opaque.sealed = sealed;
}

#[given(expr = "a caller wrote the content fixture {string} sealed with its own key")]
#[when(expr = "a caller writes the content fixture {string} sealed with its own key")]
fn write_sealed(world: &mut OcflWorld, name: String) {
    write(world, load(&name), true);
}

#[when(expr = "a caller writes the content fixture {string} as plaintext")]
fn write_plain(world: &mut OcflWorld, name: String) {
    write(world, load(&name), false);
}

#[when(
    "a caller writes files holding every byte value, no bytes, bytes that are not UTF-8 and an OCFL inventory"
)]
fn write_awkward(world: &mut OcflWorld) {
    let inventory =
        std::fs::read(crate::fixtures::tree().join("good-objects/spec-ex-full/inventory.json"))
            .expect("a pinned inventory");
    let files = [
        ("every-byte.bin", (0..=255u8).collect::<Vec<u8>>()),
        ("empty", Vec::new()),
        ("not-utf8.bin", vec![0xff, 0xfe, 0x80]),
        ("inventory.json", inventory),
        ("0=ocfl_object_1.1", b"ocfl_object_1.1\n".to_vec()),
    ];
    let fixture = ContentFixture {
        name: "awkward".to_string(),
        versions: vec![SourceVersion {
            name: "v1".to_string(),
            files: files.into_iter().map(|(p, b)| (p.to_string(), b)).collect(),
            meta: None,
        }],
    };
    write(world, fixture, false);
}

#[then("the written object should validate with zero errors in filesquid-ocfl")]
fn engine_valid(world: &mut OcflWorld) {
    let root = &stored(world).name;
    let report = ready(validate_object(&world.opaque.storage, root)).expect("validation ran");
    assert!(report.errors().is_empty(), "{root}: {:?}", report.errors());
}

#[then("its inventory should parse with sha256 digests of the stored bytes")]
fn digests_of_stored_bytes(world: &mut OcflWorld) {
    let storage = &world.opaque.storage;
    let root = &stored(world).name;
    let key = StorageKey::new(&format!("{root}/inventory.json")).expect("key");
    let bytes = ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("inventory");
    let inventory = Inventory::parse(&bytes).expect("the inventory parses");
    assert_eq!(inventory.digest_algorithm, DigestAlgorithm::Sha256);
    for (digest, paths) in &inventory.manifest {
        for path in paths {
            let key = StorageKey::new(&format!("{root}/{path}")).expect("key");
            let content = ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("content");
            assert_eq!(&hex(&Sha256::digest(&content)), digest, "{path}");
        }
    }
}

#[then("the caller should recover the plaintext of every version")]
fn caller_recovers(world: &mut OcflWorld) {
    assert!(world.opaque.sealed, "a sealed object was written");
    let source = world.opaque.source.as_ref().expect("a source");
    let root = &stored(world).name;
    for version in &source.versions {
        let pinned = VersionNum::parse(&version.name).expect("version");
        let sealed = written_state(&world.opaque.storage, root, &pinned);
        assert_eq!(
            cipher().open_files(&sealed),
            version.files,
            "{}",
            version.name
        );
    }
}

#[then("every version should read back as its plaintext source")]
fn reads_back_plain(world: &mut OcflWorld) {
    assert!(!world.opaque.sealed, "a plaintext object was written");
    let fixture = stored(world);
    assert_state_matches(&world.opaque.storage, &fixture.name, fixture);
}

#[then("no stored key, inventory text or content file should show the plaintext")]
fn no_plaintext(world: &mut OcflWorld) {
    let source = world.opaque.source.as_ref().expect("a source");
    let root = &stored(world).name;
    let leaks = crate::opaque::plaintext_leaks(&world.opaque.storage, root, source);
    assert!(leaks.is_empty(), "{leaks:?}");
}

#[when("one byte of its sealed content file changes")]
fn change_one_byte(world: &mut OcflWorld) {
    let source = world.opaque.source.as_ref().expect("a source");
    let path = source.versions[0].files.keys().next().expect("a file");
    let name = cipher().seal_name(path);
    let key = StorageKey::new(&format!("{}/v1/content/{name}", stored(world).name)).expect("key");
    let storage = &world.opaque.storage;
    let mut bytes = ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("content");
    bytes[0] ^= 0x01;
    ready(storage.put(&key, &bytes)).expect("change one byte");
    world.opaque.read = Some(read_whole(world, &name));
}

/// Stream the logical file `path` of `v1` whole, or the first refusal.
fn read_whole(world: &OcflWorld, path: &str) -> Result<Vec<u8>, ReadError> {
    let object = ready(Object::open(&world.opaque.storage, &stored(world).name))?;
    let v1 = VersionNum::parse("v1").expect("v1");
    let mut reader = object.stream_file(&v1, path, MAX_CHUNK_BYTES)?;
    let mut bytes = Vec::new();
    for _ in 0..MAX_CHUNKS {
        match ready(reader.next_chunk())? {
            Some(chunk) => bytes.extend_from_slice(&chunk),
            None => return Ok(bytes),
        }
    }
    panic!("{path} did not end within {MAX_CHUNKS} chunks");
}

#[then(expr = "reading that file should be refused with OCFL code {string}")]
fn refused_with(world: &mut OcflWorld, code: String) {
    let result = world.opaque.read.take().expect("a read ran");
    let err = result.expect_err("the changed file is refused");
    assert_eq!(err.code(), Some(code.as_str()), "{err:?}");
}

#[then("no profile or encryption type or encryption crate should be part of the engine")]
fn no_profile_types(_world: &mut OcflWorld) {
    let findings = engine_profile_findings();
    assert!(findings.is_empty(), "{findings:?}");
}
