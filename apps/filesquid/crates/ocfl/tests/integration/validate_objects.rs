//! Structural validation of OCFL 1.1 objects (IO-0.5; OCFL-S-009 (d),
//! OCFL-S-004, OCFL-S-017). Each test builds a valid object, breaks one
//! rule and checks the OCFL 1.1 code the validator reports. Hierarchy
//! faults (empty directories, links) run on a real directory through
//! `FsBackend`; everything else on the in-memory backend.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::fixity::FixityAlgorithm;
use filesquid_ocfl::inventory::{DigestMap, Inventory};
use filesquid_ocfl::namaste::SpecVersion;
use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::{
    complete_immediately, read_verified, ByteRange, CompletedPart, FsBackend, ListPage,
    MemoryBackend, ObjectMeta, Precondition, StorageBackend, StorageError, StorageKey, UploadId,
    MAX_CHUNK_BYTES, MAX_LIST_PAGE,
};
use filesquid_ocfl::validate::{validate_object, ValidationReport};
use filesquid_ocfl::VersionNum;

#[path = "../support/object_builder.rs"]
mod builder;
use builder::{build, delete, key_in, put, write_inventory, ObjectSpec};

/// The object root every test uses.
const ROOT: &str = "obj";

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// Three versions: v2 renames b.txt to c.txt and adds d.txt; v3 changes
/// a.txt and removes d.txt.
fn three(algorithm: DigestAlgorithm) -> ObjectSpec<'static> {
    let mut spec = ObjectSpec::new(
        "urn:example:three",
        vec![
            vec![("a.txt", b"alpha" as &[u8]), ("b.txt", b"bravo")],
            vec![
                ("a.txt", b"alpha"),
                ("c.txt", b"bravo"),
                ("d.txt", b"delta"),
            ],
            vec![("a.txt", b"alpha 2"), ("c.txt", b"bravo")],
        ],
    );
    spec.algorithm = algorithm;
    spec
}

/// A memory backend holding the three-version sha512 object at [`ROOT`].
fn object() -> MemoryBackend {
    let backend = MemoryBackend::new();
    build(&backend, ROOT, &three(DigestAlgorithm::Sha512));
    backend
}

/// Validate the object at [`ROOT`]; the validator itself must run.
fn validate<B: StorageBackend>(backend: &B) -> ValidationReport {
    ready(validate_object(backend, ROOT)).expect("the validator runs")
}

/// The report's error codes.
fn errors(report: &ValidationReport) -> BTreeSet<&'static str> {
    report.error_codes()
}

/// Assert the object is invalid and `code` is among its errors.
fn assert_error(report: &ValidationReport, code: &str) {
    assert!(!report.is_valid(), "expected {code}; the object validated");
    assert!(
        errors(report).contains(code),
        "expected {code}, got {:#?}",
        report.errors()
    );
}

/// Assert the object is valid and `code` is among its warnings.
fn assert_warning(report: &ValidationReport, code: &str) {
    assert!(
        report.is_valid(),
        "unexpected errors {:#?}",
        report.errors()
    );
    assert!(
        report.warning_codes().contains(code),
        "expected {code}, got {:#?}",
        report.warnings()
    );
}

/// The inventory in `dir` of the object (empty for the root).
fn inventory_in<B: StorageBackend>(backend: &B, dir: &str) -> Inventory {
    let rel = if dir.is_empty() {
        "inventory.json".to_string()
    } else {
        format!("{dir}/inventory.json")
    };
    let bytes = ready(read_verified(backend, &key_in(ROOT, &rel), MAX_CHUNK_BYTES)).expect("read");
    Inventory::parse(&bytes).expect("a valid inventory")
}

/// Change the inventory (and its sidecar) in each of `dirs`.
fn rewrite<B: StorageBackend>(backend: &B, dirs: &[&str], change: impl Fn(&mut Inventory)) {
    for dir in dirs {
        let mut inventory = inventory_in(backend, dir);
        change(&mut inventory);
        write_inventory(backend, ROOT, dir, &inventory);
    }
}

/// Every inventory of the three-version object: the root and v1 to v3.
const ALL: [&str; 4] = ["", "v1", "v2", "v3"];

/// The root and the head version inventory, which must stay equal (E064).
const ROOT_AND_HEAD: [&str; 2] = ["", "v3"];

#[test]
fn a_valid_object_has_no_errors_or_warnings() {
    let report = validate(&object());
    assert!(report.is_valid(), "{:#?}", report.errors());
    assert!(report.errors().is_empty());
    assert!(report.warnings().is_empty(), "{:#?}", report.warnings());
}

#[test]
fn a_sha256_object_is_valid_with_w004() {
    let backend = MemoryBackend::new();
    build(&backend, ROOT, &three(DigestAlgorithm::Sha256));
    let report = validate(&backend);
    assert_warning(&report, "W004");
    assert_eq!(report.warning_codes(), BTreeSet::from(["W004"]));
}

#[test]
fn every_finding_carries_an_ocfl_code_and_its_location() {
    let backend = object();
    put(&backend, ROOT, "extra.txt", b"x");
    let report = validate(&backend);
    let finding = &report.errors()[0];
    assert_eq!(finding.code, "E001");
    assert_eq!(finding.at, "obj/extra.txt");
    assert!(finding.to_string().starts_with("E001"), "{finding}");
}

#[test]
fn declaration_faults_are_e003_and_e007() {
    let backend = object();
    put(&backend, ROOT, "0=ocfl_object_1.1", b"ocfl_object_1.0\n");
    assert_error(&validate(&backend), "E007");
    delete(&backend, ROOT, "0=ocfl_object_1.1");
    assert_error(&validate(&backend), "E003");
}

#[test]
fn an_ocfl_1_0_object_is_outside_the_validators_scope() {
    let backend = object();
    delete(&backend, ROOT, "0=ocfl_object_1.1");
    put(&backend, ROOT, "0=ocfl_object_1.0", b"ocfl_object_1.0\n");
    let result = ready(validate_object(&backend, ROOT));
    assert!(
        matches!(result, Err(ReadError::UnsupportedVersion { .. })),
        "{result:?}"
    );
}

#[test]
fn a_missing_root_inventory_is_e063() {
    let backend = object();
    delete(&backend, ROOT, "inventory.json");
    assert_error(&validate(&backend), "E063");
}

#[test]
fn root_sidecar_faults_are_e058_e060_and_e061() {
    let backend = object();
    put(&backend, ROOT, "inventory.json.sha512", b"nonsense");
    assert_error(&validate(&backend), "E061");
    let wrong = format!("{} inventory.json\n", "0".repeat(128));
    put(&backend, ROOT, "inventory.json.sha512", wrong.as_bytes());
    assert_error(&validate(&backend), "E060");
    delete(&backend, ROOT, "inventory.json.sha512");
    assert_error(&validate(&backend), "E058");
}

#[test]
fn an_unparsable_root_inventory_reports_its_code() {
    let backend = object();
    put(&backend, ROOT, "inventory.json", b"[]");
    assert_error(&validate(&backend), "E033");
}

#[test]
fn unknown_entries_in_the_object_root_are_e001() {
    for rel in ["extra.txt", "extra/file.txt", ".git/HEAD", "v9"] {
        let backend = object();
        put(&backend, ROOT, rel, b"x");
        assert_error(&validate(&backend), "E001");
    }
}

#[test]
fn a_root_inventory_of_another_specification_is_e038() {
    let backend = object();
    rewrite(&backend, &ROOT_AND_HEAD, |inv| inv.spec = SpecVersion::V1_0);
    assert_error(&validate(&backend), "E038");
}

#[test]
fn a_version_directory_inventory_alone_is_not_a_commit() {
    let backend = object();
    let root = inventory_in(&backend, "");
    write_inventory(&backend, ROOT, "v4", &root);
    put(&backend, ROOT, "v4/content/e.txt", b"echo");
    let report = validate(&backend);
    assert_error(&report, "E046");
    assert!(report.errors().iter().any(|f| f.at == "obj/v4"));
}

#[test]
fn a_missing_version_directory_is_e010() {
    let backend = MemoryBackend::new();
    let mut spec = ObjectSpec::new(
        "urn:example:same",
        vec![
            vec![("a.txt", b"alpha" as &[u8])],
            vec![("a.txt", b"alpha")],
        ],
    );
    spec.algorithm = DigestAlgorithm::Sha512;
    build(&backend, ROOT, &spec);
    delete(&backend, ROOT, "v1/inventory.json");
    delete(&backend, ROOT, "v1/inventory.json.sha512");
    delete(&backend, ROOT, "v1/content/a.txt");
    assert_error(&validate(&backend), "E010");
}

#[test]
fn extensions_hold_only_extension_directories() {
    let backend = object();
    put(
        &backend,
        ROOT,
        "extensions/0005-mutable-head/config.json",
        b"{}",
    );
    put(&backend, ROOT, "logs/actions.txt", b"log");
    let report = validate(&backend);
    assert!(report.is_valid(), "{:#?}", report.errors());
    assert!(report.warnings().is_empty(), "{:#?}", report.warnings());
    put(&backend, ROOT, "extensions/unregistered/x", b"x");
    assert_warning(&validate(&backend), "W013");
    put(&backend, ROOT, "extensions/extra_file", b"x");
    assert_error(&validate(&backend), "E067");
}

#[test]
fn version_directory_entries_are_e015_and_w002() {
    let backend = object();
    put(&backend, ROOT, "v1/other/x.txt", b"x");
    assert_warning(&validate(&backend), "W002");
    put(&backend, ROOT, "v1/extra.txt", b"x");
    assert_error(&validate(&backend), "E015");
}

#[test]
fn a_version_directory_holds_only_its_own_inventorys_sidecar() {
    let backend = object();
    let sha256 = DigestAlgorithm::Sha256.digest_hex(b"other");
    let sidecar = format!("{sha256} inventory.json\n");
    put(
        &backend,
        ROOT,
        "v1/inventory.json.sha256",
        sidecar.as_bytes(),
    );
    let report = validate(&backend);
    assert_error(&report, "E015");
    assert!(report
        .errors()
        .iter()
        .any(|f| f.at == "obj/v1/inventory.json.sha256"));
}

#[test]
fn a_sidecar_without_its_version_inventory_is_e015() {
    let backend = object();
    delete(&backend, ROOT, "v1/inventory.json");
    let report = validate(&backend);
    assert_error(&report, "E015");
    assert!(report.warning_codes().contains("W010"));
}

#[test]
fn a_version_inventory_manifest_path_outside_a_content_directory_is_e042() {
    let backend = object();
    rewrite(&backend, &["v1"], |inv| {
        for paths in inv.manifest.values_mut() {
            for path in paths.iter_mut().filter(|p| *p == "v1/content/a.txt") {
                *path = "v1/a.txt".into();
            }
        }
    });
    let report = validate(&backend);
    assert_error(&report, "E042");
    assert!(report
        .errors()
        .iter()
        .any(|f| f.code == "E042" && f.at == "obj/v1/a.txt"));
}

#[test]
fn a_missing_version_inventory_is_w010() {
    let backend = object();
    delete(&backend, ROOT, "v1/inventory.json");
    delete(&backend, ROOT, "v1/inventory.json.sha512");
    assert_warning(&validate(&backend), "W010");
}

#[test]
fn version_inventory_sidecars_are_checked() {
    let backend = object();
    let wrong = format!("{} inventory.json\n", "0".repeat(128));
    put(&backend, ROOT, "v1/inventory.json.sha512", wrong.as_bytes());
    assert_error(&validate(&backend), "E060");
    delete(&backend, ROOT, "v1/inventory.json.sha512");
    assert_error(&validate(&backend), "E058");
}

#[test]
fn an_unparsable_version_inventory_reports_its_code() {
    let backend = object();
    put(&backend, ROOT, "v1/inventory.json", b"{}");
    let sidecar = DigestAlgorithm::Sha512.digest_hex(b"{}");
    put(
        &backend,
        ROOT,
        "v1/inventory.json.sha512",
        format!("{sidecar} inventory.json\n").as_bytes(),
    );
    assert_error(&validate(&backend), "E036");
}

#[test]
fn a_version_inventory_must_name_its_own_version_as_head() {
    let backend = object();
    let root = inventory_in(&backend, "");
    write_inventory(&backend, ROOT, "v2", &root);
    assert_error(&validate(&backend), "E040");
}

#[test]
fn a_version_inventory_with_another_id_is_e037() {
    let backend = object();
    rewrite(&backend, &["v1"], |inv| inv.id = "urn:example:other".into());
    assert_error(&validate(&backend), "E037");
}

#[test]
fn a_changed_content_directory_is_e019() {
    let backend = object();
    rewrite(&backend, &["v1"], |inv| {
        inv.content_directory = Some("stuff".into());
    });
    assert_error(&validate(&backend), "E019");
}

#[test]
fn a_version_inventory_of_an_older_specification_is_e103() {
    let backend = object();
    rewrite(&backend, &["v2"], |inv| inv.spec = SpecVersion::V1_0);
    assert_error(&validate(&backend), "E103");
}

#[test]
fn a_head_version_inventory_that_differs_from_the_root_is_e064() {
    let backend = object();
    rewrite(&backend, &["v3"], |inv| {
        let head = inv.versions.get_mut(&v("v3")).expect("v3");
        head.message = Some("another message".into());
    });
    assert_error(&validate(&backend), "E064");
}

#[test]
fn a_prior_version_state_that_differs_is_e066() {
    let backend = object();
    rewrite(&backend, &["v2"], |inv| {
        let block = inv.versions.get_mut(&v("v1")).expect("v1");
        for paths in block.state.values_mut() {
            paths.iter_mut().for_each(|p| p.insert_str(0, "renamed-"));
        }
    });
    assert_error(&validate(&backend), "E066");
}

#[test]
fn prior_version_metadata_that_differs_is_w011() {
    let backend = object();
    rewrite(&backend, &["v1"], |inv| {
        let block = inv.versions.get_mut(&v("v1")).expect("v1");
        block.message = Some("another message".into());
    });
    assert_warning(&validate(&backend), "W011");
}

#[test]
fn a_content_file_missing_from_the_manifest_is_e023() {
    let backend = object();
    put(&backend, ROOT, "v1/content/extra.txt", b"x");
    assert_error(&validate(&backend), "E023");
}

#[test]
fn a_manifest_path_outside_a_content_directory_is_e042() {
    let backend = object();
    rewrite(&backend, &ROOT_AND_HEAD, |inv| {
        for paths in inv.manifest.values_mut() {
            for path in paths.iter_mut() {
                if path == "v1/content/b.txt" {
                    *path = "v1/b.txt".into();
                }
            }
        }
    });
    put(&backend, ROOT, "v1/b.txt", b"bravo");
    let report = validate(&backend);
    assert_error(&report, "E042");
    assert_error(&report, "E015");
}

#[test]
fn content_that_does_not_match_its_digest_is_e092() {
    let backend = object();
    put(&backend, ROOT, "v1/content/a.txt", b"tampered");
    assert_error(&validate(&backend), "E092");
}

#[test]
fn a_missing_content_file_is_e092() {
    let backend = object();
    delete(&backend, ROOT, "v1/content/b.txt");
    assert_error(&validate(&backend), "E092");
}

/// A fixity block of one algorithm with one digest for `path`.
fn fixity(algorithm: &str, digest: String, paths: &[&str]) -> Option<BTreeMapFixity> {
    let mut block = DigestMap::new();
    block.insert(digest, paths.iter().map(|p| (*p).to_string()).collect());
    Some(BTreeMapFixity::from([(algorithm.to_string(), block)]))
}

/// The inventory's fixity map type.
type BTreeMapFixity = std::collections::BTreeMap<String, DigestMap>;

/// The md5 of `bytes`.
fn md5(bytes: &[u8]) -> String {
    FixityAlgorithm::from_name("md5")
        .expect("md5")
        .digest_hex(bytes)
}

#[test]
fn fixity_values_are_verified_and_unknown_algorithms_ignored() {
    let backend = object();
    let good = fixity("md5", md5(b"alpha"), &["v1/content/a.txt"]);
    rewrite(&backend, &ROOT_AND_HEAD, |inv| inv.fixity = good.clone());
    assert!(validate(&backend).is_valid());
    let unknown = fixity("crc32", "0000".into(), &["v1/content/a.txt"]);
    rewrite(&backend, &ROOT_AND_HEAD, |inv| inv.fixity = unknown.clone());
    assert!(validate(&backend).is_valid());
    let wrong = fixity("md5", md5(b"other"), &["v1/content/a.txt"]);
    rewrite(&backend, &ROOT_AND_HEAD, |inv| inv.fixity = wrong.clone());
    assert_error(&validate(&backend), "E093");
}

#[test]
fn a_missing_content_path_that_has_fixity_is_e092_and_e093() {
    let backend = object();
    let paths = ["v1/content/a.txt", "v1/content/bonus.txt"];
    let block = fixity("md5", md5(b"alpha"), &paths);
    rewrite(&backend, &ROOT_AND_HEAD, |inv| {
        let digest = DigestAlgorithm::Sha512.digest_hex(b"alpha");
        inv.manifest
            .insert(digest, paths.iter().map(|p| (*p).to_string()).collect());
        inv.fixity = block.clone();
    });
    let report = validate(&backend);
    assert_error(&report, "E092");
    assert_error(&report, "E093");
}

/// A change to an inventory, for table-driven tests.
type Change = fn(&mut Inventory);

#[test]
fn inventory_warnings_have_their_codes() {
    let cases: [(&str, Change); 4] = [
        ("W005", |inv| inv.id = "not a uri".into()),
        ("W007", |inv| {
            inv.versions.values_mut().for_each(|b| b.message = None)
        }),
        ("W008", |inv| {
            for block in inv.versions.values_mut() {
                block.user.as_mut().expect("user").address = None;
            }
        }),
        ("W009", |inv| {
            for block in inv.versions.values_mut() {
                block.user.as_mut().expect("user").address = Some("1 Wonky Way".into());
            }
        }),
    ];
    for (code, change) in cases {
        let backend = object();
        rewrite(&backend, &ALL, change);
        assert_warning(&validate(&backend), code);
    }
}

/// Scratch directory for one filesystem backend, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A filesystem backend holding the three-version sha512 object.
fn fs_object() -> (FsBackend, Scratch) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-validate-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    build(&backend, ROOT, &three(DigestAlgorithm::Sha512));
    (backend, Scratch(dir))
}

#[test]
fn a_valid_object_on_a_filesystem_has_no_errors() {
    let (backend, _scratch) = fs_object();
    let report = validate(&backend);
    assert!(report.is_valid(), "{:#?}", report.errors());
}

#[test]
fn an_empty_directory_is_e024() {
    let (backend, _scratch) = fs_object();
    std::fs::create_dir_all(backend.root().join("obj/v1/content/empty")).expect("dir");
    assert_error(&validate(&backend), "E024");
}

#[test]
fn validation_goes_on_after_an_empty_directory() {
    let (backend, _scratch) = fs_object();
    std::fs::create_dir_all(backend.root().join("obj/v1/content/empty")).expect("dir");
    put(&backend, ROOT, "extra.txt", b"x");
    let report = validate(&backend);
    assert_error(&report, "E024");
    assert_error(&report, "E001");
}

#[cfg(unix)]
#[test]
fn a_soft_link_is_e090() {
    let (backend, scratch) = fs_object();
    let outside = scratch.0.join("outside.txt");
    std::fs::write(&outside, b"outside").expect("outside");
    let link = backend.root().join("obj/v1/content/link.txt");
    std::os::unix::fs::symlink(outside, link).expect("symlink");
    assert_error(&validate(&backend), "E090");
}

#[test]
fn a_root_that_is_not_a_key_prefix_is_refused() {
    let backend = object();
    let result = ready(validate_object(&backend, "../obj"));
    assert!(matches!(result, Err(ReadError::Storage(_))), "{result:?}");
}

/// A backend whose listing pages hold at most one key, as a backend may
/// return (`list` promises at most `limit` keys).
struct ShortPages(MemoryBackend);

impl StorageBackend for ShortPages {
    async fn put(&self, k: &StorageKey, b: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.0.put(k, b).await
    }
    async fn put_if(
        &self,
        k: &StorageKey,
        b: &[u8],
        p: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.0.put_if(k, b, p).await
    }
    async fn get_range(&self, k: &StorageKey, r: ByteRange) -> Result<Vec<u8>, StorageError> {
        self.0.get_range(k, r).await
    }
    async fn head(&self, k: &StorageKey) -> Result<ObjectMeta, StorageError> {
        self.0.head(k).await
    }
    async fn exists(&self, k: &StorageKey) -> Result<bool, StorageError> {
        self.0.exists(k).await
    }
    async fn list(
        &self,
        p: &str,
        a: Option<&StorageKey>,
        _limit: usize,
    ) -> Result<ListPage, StorageError> {
        self.0.list(p, a, 1).await
    }
    async fn delete(&self, k: &StorageKey) -> Result<(), StorageError> {
        self.0.delete(k).await
    }
    async fn create_multipart(&self, k: &StorageKey) -> Result<UploadId, StorageError> {
        self.0.create_multipart(k).await
    }
    async fn upload_part(
        &self,
        u: &UploadId,
        n: u32,
        b: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.0.upload_part(u, n, b).await
    }
    async fn complete_multipart(
        &self,
        u: &UploadId,
        p: &[CompletedPart],
        c: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.0.complete_multipart(u, p, c).await
    }
    async fn abort_multipart(&self, u: &UploadId) -> Result<(), StorageError> {
        self.0.abort_multipart(u).await
    }
}

#[test]
fn an_object_listed_in_short_pages_validates() {
    let count = MAX_LIST_PAGE + 100;
    let names: Vec<String> = (0..count).map(|n| format!("f{n:05}.txt")).collect();
    let bodies: Vec<Vec<u8>> = (0..count)
        .map(|n| format!("file {n}").into_bytes())
        .collect();
    let files = names
        .iter()
        .zip(&bodies)
        .map(|(name, body)| (name.as_str(), body.as_slice()))
        .collect();
    let mut spec = ObjectSpec::new("urn:example:many", vec![files]);
    spec.algorithm = DigestAlgorithm::Sha512;
    let inner = MemoryBackend::new();
    build(&inner, ROOT, &spec);
    let report = validate(&ShortPages(inner));
    assert!(report.is_valid(), "{:#?}", report.errors());
}
