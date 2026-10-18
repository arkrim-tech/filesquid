//! Write conformance against the pinned OCFL 1.1 `content` fixtures
//! (IO-0.7; OCFL-S-010 (b), OCFL-S-002, OCFL-S-003, OCFL-S-011,
//! OCFL-S-018; PRD F-051).
//!
//! Every content fixture is written through the engine writer, one OCFL
//! version per version directory, from caller-supplied logical files and
//! nothing else (no FileSquid application-layer type). Each written object
//! must validate with zero errors in the engine validator and in every
//! independent community validator available (`ocfl-py`, `rocfl`; see
//! `support/community_validator.rs`), and its logical state at every
//! version must equal the source directory.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend, StorageKey};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::VersionNum;

#[path = "../support/community_validator.rs"]
mod community;
#[path = "../support/content_fixtures.rs"]
mod content;
#[path = "../support/fixtures.rs"]
mod fixtures;

use content::{
    assert_state_matches, commit_version, fixture_names, load, ready, write_fixture, PINNED_CONTENT,
};

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
        .join(format!("ocfl-content-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
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

/// Every key under `prefix` with its entity tag.
fn snapshot<B: StorageBackend>(storage: &B, prefix: &str) -> BTreeMap<String, String> {
    let page = ready(storage.list(prefix, None, 1_000)).expect("list");
    assert!(page.next.is_none(), "snapshot fits one page");
    page.keys
        .iter()
        .map(|k| {
            let meta = ready(storage.head(k)).expect("head");
            (k.as_str().to_string(), meta.etag.as_str().to_string())
        })
        .collect()
}

#[test]
fn the_pinned_content_fixtures_are_all_written() {
    let pinned: Vec<&str> = PINNED_CONTENT.iter().map(|(name, _)| *name).collect();
    assert_eq!(fixture_names(), pinned, "content fixtures at the pin");
    for (name, versions) in PINNED_CONTENT {
        let fixture = load(name);
        assert_eq!(
            fixture.versions.len(),
            versions,
            "{name}: version directories"
        );
        for (index, source) in fixture.versions.iter().enumerate() {
            assert_eq!(
                source.name,
                format!("v{}", index + 1),
                "{name}: version order"
            );
            assert!(!source.files.is_empty(), "{name} {}: no files", source.name);
        }
    }
}

#[test]
fn every_content_fixture_is_written_one_version_per_directory_in_memory() {
    let storage = MemoryBackend::new();
    for name in fixture_names() {
        let fixture = load(&name);
        let receipts = write_fixture(&storage, &name, &fixture);
        assert_eq!(receipts.len(), fixture.versions.len(), "{name}: commits");
        assert_engine_valid(&storage, &name);
        assert_state_matches(&storage, &name, &fixture);
    }
}

#[test]
fn every_content_fixture_written_to_a_directory_passes_the_engine_and_community_validators() {
    let (storage, _scratch) = fs_backend();
    let names = fixture_names();
    for name in &names {
        let fixture = load(name);
        write_fixture(&storage, name, &fixture);
        assert_engine_valid(&storage, name);
        assert_state_matches(&storage, name, &fixture);
        println!(
            "filesquid-ocfl {name}: VALID ({} versions)",
            fixture.versions.len()
        );
    }
    for validator in community::validators_or_skip() {
        for name in &names {
            let verdict = validator.run_validation(&storage.root().join(name));
            assert!(
                verdict.valid,
                "{} rejected {name}:\n{}",
                validator.name(),
                verdict.output
            );
            println!(
                "{} {name}: VALID\n{}",
                validator.name(),
                verdict.output.trim_end()
            );
        }
    }
}

#[test]
fn written_objects_use_sha256_and_its_sidecar() {
    let storage = MemoryBackend::new();
    let fixture = load("spec-ex-full");
    write_fixture(&storage, "obj", &fixture);
    let object = ready(Object::open(&storage, "obj")).expect("open");
    assert_eq!(object.inventory().digest_algorithm, DigestAlgorithm::Sha256);
    for key in ["obj/inventory.json.sha256", "obj/v3/inventory.json.sha256"] {
        let key = StorageKey::new(key).expect("key");
        assert!(ready(storage.exists(&key)).expect("exists"), "{key}");
    }
}

#[test]
fn unchanged_and_reinstated_bytes_are_not_written_again() {
    let storage = MemoryBackend::new();
    let cf3 = write_fixture(&storage, "cf3", &load("cf3"));
    assert_eq!(cf3[1].written, ["v2/content/a_file.txt"]);
    assert!(cf3[2].written.is_empty(), "cf3 v3 equals v1: {:?}", cf3[2]);
    let full = write_fixture(&storage, "full", &load("spec-ex-full"));
    assert_eq!(
        full[0].written,
        [
            "v1/content/empty.txt",
            "v1/content/foo/bar.xml",
            "v1/content/image.tiff"
        ]
    );
    assert_eq!(full[1].written, ["v2/content/foo/bar.xml"]);
    assert!(full[2].written.is_empty(), "v3 reinstates: {:?}", full[2]);
}

#[test]
fn earlier_versions_are_never_changed_by_later_commits() {
    let (fs, _scratch) = fs_backend();
    for name in ["cf2", "cf3", "spec-ex-full"] {
        earlier_versions_stay_unchanged(&MemoryBackend::new(), name);
        earlier_versions_stay_unchanged(&fs, name);
    }
}

/// Commit each version of `name` in turn; the keys and entity tags of every
/// earlier version directory must not change.
fn earlier_versions_stay_unchanged<B: StorageBackend>(storage: &B, name: &str) {
    let fixture = load(name);
    let mut head: Option<VersionNum> = None;
    let mut before: Vec<BTreeMap<String, String>> = Vec::new();
    for index in 0..fixture.versions.len() {
        let receipt = commit_version(storage, name, &fixture, index, head.as_ref());
        for (earlier, snap) in before.iter().enumerate() {
            let prefix = format!("{name}/v{}/", earlier + 1);
            assert_eq!(&snapshot(storage, &prefix), snap, "{prefix} changed");
        }
        before.push(snapshot(storage, &format!("{name}/{}/", receipt.version)));
        head = Some(receipt.version);
    }
}

#[test]
fn spec_ex_full_keeps_the_callers_version_metadata() {
    let storage = MemoryBackend::new();
    write_fixture(&storage, "obj", &load("spec-ex-full"));
    let object = ready(Object::open(&storage, "obj")).expect("open");
    assert_eq!(object.id(), "ark:/12345/bcd987");
    let history = object.history();
    let v1 = &history[0];
    assert_eq!(v1.version, v("v1"));
    assert_eq!(v1.created, "2018-01-01T01:01:01Z");
    assert_eq!(v1.message.as_deref(), Some("Initial import"));
    assert_eq!(
        v1.user,
        Some(User {
            name: "Alice".to_string(),
            address: Some("alice@example.com".to_string()),
        })
    );
    let messages: Vec<_> = history.iter().map(|h| h.message.clone()).collect();
    assert_eq!(
        messages,
        [
            Some("Initial import".to_string()),
            Some("Fix bar.xml, remove image.tiff, add empty2.txt".to_string()),
            Some("Reinstate image.tiff, delete empty.txt".to_string()),
        ]
    );
}
