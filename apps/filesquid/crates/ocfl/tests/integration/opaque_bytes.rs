//! Opaque bytes; the dual profile is a caller (IO-0.8; OCFL-S-021,
//! OCFL-S-002, OCFL-S-003, OCFL-S-007; ADR 010, ADR 023; PRD F-042,
//! TC-002).
//!
//! The engine stores whatever bytes and logical paths its caller supplies
//! and never reads them as anything but bytes. The owner-readable local
//! profile hands it plaintext; the hosted profile hands it what the caller
//! sealed (here the test-only `ToyCipher` of `support/opaque.rs`). Both go
//! through the same engine calls, and over sealed bytes the inventory stays
//! structurally readable: it parses, the engine validator and community
//! validators accept the object, and only the caller recovers plaintext.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::Inventory;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{
    read_verified, FsBackend, MemoryBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::VersionNum;
use sha2::{Digest, Sha256};

#[path = "../support/community_validator.rs"]
mod community;
#[path = "../support/content_fixtures.rs"]
mod content;
#[path = "../support/fixtures.rs"]
mod fixtures;
#[path = "../support/opaque.rs"]
mod opaque;
#[path = "../support/source_scan.rs"]
mod source_scan;

use content::{
    assert_state_matches, fixture_names, load, ready, write_fixture, written_state, ContentFixture,
    SourceVersion,
};
use opaque::{hex, inventory_bytes, plaintext_leaks, ToyCipher};

/// Scratch directory for one filesystem backend, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh `FsBackend` on its own scratch directory.
fn fs_backend() -> (FsBackend, Scratch) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-opaque-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// The test cipher every test uses.
fn cipher() -> ToyCipher {
    ToyCipher::new("io08-test-key")
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// A one-version fixture holding `files`.
fn one_version(name: &str, files: &[(&str, Vec<u8>)]) -> ContentFixture {
    let files = files
        .iter()
        .map(|(path, bytes)| (path.to_string(), bytes.clone()))
        .collect();
    ContentFixture {
        name: name.to_string(),
        versions: vec![SourceVersion {
            name: "v1".to_string(),
            files,
            meta: None,
        }],
    }
}

/// Bytes a reader might mistake for something other than content: every
/// byte value, nothing at all, bytes that are not UTF-8, and copies of an
/// OCFL inventory, sidecar and declaration.
fn awkward_files() -> Vec<(&'static str, Vec<u8>)> {
    let inventory =
        std::fs::read(fixtures::tree().join("good-objects/spec-ex-full/inventory.json"))
            .expect("a pinned inventory");
    vec![
        (
            "every-byte.bin",
            (0..=255u8).cycle().take(4 * 256).collect(),
        ),
        ("empty", Vec::new()),
        ("not-utf8.bin", vec![0xff, 0xfe, 0x80, 0x00, 0xc3]),
        ("inventory.json", inventory),
        ("inventory.json.sha256", b"0123 inventory.json\n".to_vec()),
        ("0=ocfl_object_1.1", b"ocfl_object_1.1\n".to_vec()),
    ]
}

/// Panic unless the engine validator reports zero errors for `root`.
fn assert_engine_valid<B: StorageBackend>(storage: &B, root: &str) {
    let report = ready(validate_object(storage, root)).expect("validation ran");
    assert!(
        report.errors().is_empty(),
        "{root}: engine validator errors {:?}",
        report.errors()
    );
}

/// Panic unless every manifest digest of the object at `root` is the
/// SHA-256 of the bytes stored at its content paths.
fn assert_digests_are_of_stored_bytes<B: StorageBackend>(storage: &B, root: &str) {
    let inventory = Inventory::parse(&inventory_bytes(storage, root)).expect("inventory parses");
    assert_eq!(inventory.digest_algorithm, DigestAlgorithm::Sha256);
    for (digest, paths) in &inventory.manifest {
        for path in paths {
            let key = StorageKey::new(&format!("{root}/{path}")).expect("key");
            let stored = ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("content");
            assert_eq!(&hex(&Sha256::digest(&stored)), digest, "{root}/{path}");
        }
    }
}

/// Store the awkward files as one version at `awkward` and check that the
/// engine validates them, streams them back as given and digests them.
fn awkward_bytes_round_trip<B: StorageBackend>(storage: &B) {
    let fixture = one_version("awkward", &awkward_files());
    write_fixture(storage, "awkward", &fixture);
    assert_engine_valid(storage, "awkward");
    assert_state_matches(storage, "awkward", &fixture);
    assert_digests_are_of_stored_bytes(storage, "awkward");
}

#[test]
fn every_byte_value_and_ocfl_lookalike_content_round_trips() {
    awkward_bytes_round_trip(&MemoryBackend::new());
    let (fs, _scratch) = fs_backend();
    awkward_bytes_round_trip(&fs);
}

#[test]
fn a_file_above_one_chunk_streams_back_in_bounded_chunks() {
    let storage = MemoryBackend::new();
    let big = cipher().apply(b"big", &vec![0u8; MAX_CHUNK_BYTES as usize + 17]);
    let fixture = one_version("big", &[("big.bin", big.clone())]);
    write_fixture(&storage, "big", &fixture);
    let object = ready(Object::open(&storage, "big")).expect("open");
    let mut reader = object
        .stream_file(&v("v1"), "big.bin", MAX_CHUNK_BYTES)
        .expect("stream");
    let mut chunks = Vec::new();
    for _ in 0..4 {
        match ready(reader.next_chunk()).expect("chunk matches its digest") {
            Some(chunk) => chunks.push(chunk),
            None => break,
        }
    }
    assert_eq!(chunks.len(), 2, "8 MiB and 17 bytes stream in two chunks");
    assert!(chunks.iter().all(|c| c.len() as u64 <= MAX_CHUNK_BYTES));
    assert_eq!(chunks.concat(), big);
    assert_digests_are_of_stored_bytes(&storage, "big");
}

/// Write `fixture` sealed (hosted profile) at its opaque name and check
/// the engine's side: zero validator errors, sealed state as given, and
/// digests of the sealed bytes. Returns the root it was written at.
fn store_sealed<B: StorageBackend>(storage: &B, fixture: &ContentFixture) -> String {
    let sealed = cipher().seal_fixture(fixture);
    write_fixture(storage, &sealed.name, &sealed);
    assert_engine_valid(storage, &sealed.name);
    assert_state_matches(storage, &sealed.name, &sealed);
    assert_digests_are_of_stored_bytes(storage, &sealed.name);
    sealed.name
}

/// Panic unless the caller opens every version of the sealed object at
/// `root` to the plaintext of `fixture`.
fn assert_caller_recovers<B: StorageBackend>(storage: &B, root: &str, fixture: &ContentFixture) {
    for source in &fixture.versions {
        let sealed = written_state(storage, root, &v(&source.name));
        assert_eq!(
            cipher().open_files(&sealed),
            source.files,
            "{} {}: the caller recovers the plaintext",
            fixture.name,
            source.name
        );
    }
}

/// Panic if the sealed object at `root` shows plaintext of `fixture`.
fn assert_no_plaintext<B: StorageBackend>(storage: &B, root: &str, fixture: &ContentFixture) {
    let leaks = plaintext_leaks(storage, root, fixture);
    assert!(leaks.is_empty(), "{root}: {leaks:?}");
}

/// The leak check is a real oracle: on the owner-readable object of the
/// same fixture it finds the plaintext names, object id, metadata (user
/// address included) and content.
#[test]
fn the_plaintext_check_finds_an_owner_readable_object() {
    let storage = MemoryBackend::new();
    let fixture = load("spec-ex-full");
    write_fixture(&storage, "plain", &fixture);
    let leaks = plaintext_leaks(&storage, "plain", &fixture);
    for expected in [
        "\"foo/bar.xml\"",
        "\"Initial import\"",
        "\"Alice\"",
        "\"alice@example.com\"",
        "\"ark:/12345/bcd987\"",
        "holds plaintext",
    ] {
        assert!(
            leaks.iter().any(|l| l.contains(expected)),
            "{expected} not found in {leaks:?}"
        );
    }
}

#[test]
fn sealed_content_fixtures_validate_and_only_the_caller_recovers_them() {
    let (storage, _scratch) = fs_backend();
    let mut roots = Vec::new();
    for name in fixture_names() {
        let fixture = load(&name);
        let root = store_sealed(&storage, &fixture);
        assert_caller_recovers(&storage, &root, &fixture);
        assert_no_plaintext(&storage, &root, &fixture);
        println!("filesquid-ocfl sealed {name} at {root}: VALID");
        roots.push((name, root));
    }
    for validator in community::validators_or_skip() {
        for (name, root) in &roots {
            let verdict = validator.run_validation(&storage.root().join(root));
            assert!(
                verdict.valid,
                "{} rejected sealed {name}:\n{}",
                validator.name(),
                verdict.output
            );
            println!("{} sealed {name}: VALID", validator.name());
        }
    }
}

#[test]
fn both_profiles_use_the_same_engine_calls() {
    let storage = MemoryBackend::new();
    let fixture = load("spec-ex-full");
    write_fixture(&storage, "plain", &fixture);
    assert_engine_valid(&storage, "plain");
    assert_state_matches(&storage, "plain", &fixture);
    let sealed = store_sealed(&storage, &fixture);
    assert_caller_recovers(&storage, &sealed, &fixture);
    let plain = ready(Object::open(&storage, "plain")).expect("open plain");
    let opaque = ready(Object::open(&storage, &sealed)).expect("open sealed");
    assert_eq!(
        plain.head(),
        opaque.head(),
        "one version per source version"
    );
    assert_ne!(plain.id(), opaque.id(), "the caller chose an opaque id");
}

#[test]
fn unchanged_sealed_bytes_are_not_written_again() {
    let storage = MemoryBackend::new();
    let cf3 = cipher().seal_fixture(&load("cf3"));
    let receipts = write_fixture(&storage, &cf3.name, &cf3);
    let a_file = cipher().seal_name("a_file.txt");
    assert_eq!(receipts[1].written, [format!("v2/content/{a_file}")]);
    assert!(
        receipts[2].written.is_empty(),
        "v3 equals v1: {:?}",
        receipts[2]
    );
    let full = cipher().seal_fixture(&load("spec-ex-full"));
    let receipts = write_fixture(&storage, &full.name, &full);
    let counts: Vec<usize> = receipts.iter().map(|r| r.written.len()).collect();
    assert_eq!(counts, [3, 1, 0], "new sealed bytes per version");
}

#[test]
fn the_engine_has_no_profile_or_encryption_types() {
    let findings = opaque::engine_profile_findings();
    assert!(findings.is_empty(), "{findings:?}");
}

/// The scan is a real oracle: it finds a profile type and an encryption
/// crate when they are there.
#[test]
fn the_profile_scan_finds_profile_code_and_encryption_crates() {
    let code = "pub struct EncryptedObject; fn seal(p: Plaintext) {}";
    let words = opaque::profile_words_in("x.rs", code);
    assert_eq!(words.len(), 2, "{words:?}");
    let manifest = "[dependencies]\nsha2 = \"0.11\"\naes-gcm = \"0.10\"\n";
    assert_eq!(opaque::crypto_dependencies(manifest), ["aes-gcm"]);
    let renamed = "[dependencies]\nsafe = { package = \"ring\", version = \"0.17\" }\n\
                   [dev-dependencies.sealer]\npackage = \"chacha20poly1305\"\n\
                   [target.'cfg(unix)'.dependencies.aes]\nversion = \"0.8\"\n";
    assert_eq!(
        opaque::crypto_dependencies(renamed),
        ["ring", "chacha20poly1305", "aes"]
    );
    let commented = "pub fn read() {} // encrypt nothing\nconst A: &str = \"cipher\";";
    assert!(opaque::profile_words_in("y.rs", commented).is_empty());
}

/// Manifest digests cover the stored bytes, so a changed sealed file is refused with
/// E092 without the engine knowing any key: the caller never receives
/// damaged ciphertext to open.
#[test]
fn a_changed_sealed_file_is_refused_with_e092() {
    let storage = MemoryBackend::new();
    let root = store_sealed(&storage, &load("spec-ex-minimal"));
    let name = cipher().seal_name("file.txt");
    let key = StorageKey::new(&format!("{root}/v1/content/{name}")).expect("key");
    let mut stored = ready(read_verified(&storage, &key, MAX_CHUNK_BYTES)).expect("content");
    stored[0] ^= 0x01;
    ready(storage.put(&key, &stored)).expect("change one byte");
    let object = ready(Object::open(&storage, &root)).expect("open");
    let mut reader = object
        .stream_file(&v("v1"), &name, MAX_CHUNK_BYTES)
        .expect("stream");
    let mut outcome = Ok(None);
    for _ in 0..4 {
        outcome = ready(reader.next_chunk());
        if !matches!(outcome, Ok(Some(_))) {
            break;
        }
    }
    let err = outcome.expect_err("the changed file is refused");
    assert_eq!(err.code(), Some("E092"), "{err:?}");
}
