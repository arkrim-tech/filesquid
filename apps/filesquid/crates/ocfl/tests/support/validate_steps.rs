//! Step definitions for `structural_validation_OCFL-S-009.feature` and the
//! validation and pin scenarios of `OCFL_1.1_fixture_conformance.feature`
//! (IO-0.5; OCFL-S-009, OCFL-S-011).

use std::collections::BTreeSet;
use std::path::PathBuf;

use cucumber::{given, then, when};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{complete_immediately, MemoryBackend, StorageBackend};
use filesquid_ocfl::validate::{validate_object, ValidationReport};

use crate::builder::{build, delete, put, write_inventory, ObjectSpec};
use crate::fixtures::{committed_dir, group_backend, objects, prefix_codes, tree, PINNED_COMMIT};
use crate::OcflWorld;

/// What the validation scenarios carry between steps.
#[derive(Debug, Default)]
pub struct ValidateState {
    backend: Option<MemoryBackend>,
    root: String,
    report: Option<ValidationReport>,
    group: Vec<(String, ValidationReport)>,
    notice: String,
    cargo: String,
}

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// Validate the object at `root` of `backend`; the validator must run.
fn run<B: StorageBackend>(backend: &B, root: &str) -> ValidationReport {
    ready(validate_object(backend, root)).expect("the validator runs")
}

/// The scenario's backend.
fn backend(world: &OcflWorld) -> &MemoryBackend {
    world
        .validate
        .backend
        .as_ref()
        .expect("Given an OCFL object")
}

#[given(
    expr = "a valid OCFL 1.1 object {string} with three versions in an in-memory storage backend"
)]
fn valid_object(world: &mut OcflWorld, root: String) {
    let backend = MemoryBackend::new();
    let mut spec = ObjectSpec::new(
        "urn:example:validate",
        vec![
            vec![("a.txt", b"alpha" as &[u8]), ("b.txt", b"bravo")],
            vec![("a.txt", b"alpha"), ("c.txt", b"bravo")],
            vec![("a.txt", b"alpha 2"), ("c.txt", b"bravo")],
        ],
    );
    spec.algorithm = DigestAlgorithm::Sha512;
    build(&backend, &root, &spec);
    world.validate.backend = Some(backend);
    world.validate.root = root;
}

#[given(regex = r#"^the file "(.+)" is (added|removed|replaced)$"#)]
fn change_file(world: &mut OcflWorld, rel: String, change: String) {
    let root = world.validate.root.clone();
    let backend = backend(world);
    match change.as_str() {
        "removed" => delete(backend, &root, &rel),
        _ => put(backend, &root, &rel, b"bytes the inventory does not know"),
    }
}

#[given(expr = "a version directory {string} with its own inventory and sidecar")]
fn uncommitted_version(world: &mut OcflWorld, dir: String) {
    let root = world.validate.root.clone();
    let backend = backend(world);
    let key = crate::builder::key_in(&root, "inventory.json");
    let bytes = ready(filesquid_ocfl::storage::read_verified(
        backend,
        &key,
        filesquid_ocfl::storage::MAX_CHUNK_BYTES,
    ))
    .expect("read root inventory");
    let inventory = filesquid_ocfl::Inventory::parse(&bytes).expect("inventory");
    write_inventory(backend, &root, &dir, &inventory);
}

#[when("I validate the object")]
fn validate_the_object(world: &mut OcflWorld) {
    let report = run(backend(world), &world.validate.root);
    world.validate.report = Some(report);
}

/// The report from the When step.
fn report(world: &OcflWorld) -> &ValidationReport {
    world
        .validate
        .report
        .as_ref()
        .expect("the object was validated")
}

#[then("the object should be valid")]
fn object_valid(world: &mut OcflWorld) {
    let report = report(world);
    assert!(report.is_valid(), "{:#?}", report.errors());
}

#[then("the object should be invalid")]
fn object_invalid(world: &mut OcflWorld) {
    assert!(!report(world).is_valid(), "the object validated");
}

#[then("no warning should be reported")]
fn no_warning(world: &mut OcflWorld) {
    let report = report(world);
    assert!(report.warnings().is_empty(), "{:#?}", report.warnings());
}

#[then(expr = "error {string} should be reported")]
fn error_reported(world: &mut OcflWorld, code: String) {
    let report = report(world);
    assert!(
        report.error_codes().contains(code.as_str()),
        "{code} not in {:#?}",
        report.errors()
    );
}

#[then(expr = "warning {string} should be reported")]
fn warning_reported(world: &mut OcflWorld, code: String) {
    let report = report(world);
    assert!(
        report.warning_codes().contains(code.as_str()),
        "{code} not in {:#?}",
        report.warnings()
    );
}

#[when(regex = r"^I validate every object in (good|warn|bad)-objects$")]
fn validate_group(world: &mut OcflWorld, kind: String) {
    let dir = tree().join(format!("{kind}-objects"));
    let backend = group_backend(&dir, "features-fixture-validation");
    world.validate.group = objects(&dir)
        .into_iter()
        .map(|name| {
            let report = run(&backend, &name);
            (name, report)
        })
        .collect();
    assert!(!world.validate.group.is_empty(), "no fixtures in {kind}");
}

#[then("every object should validate with zero errors")]
fn group_has_no_errors(world: &mut OcflWorld) {
    for (name, report) in &world.validate.group {
        assert!(report.errors().is_empty(), "{name}: {:#?}", report.errors());
    }
}

#[then("every object should report each warning code in its directory prefix")]
fn group_reports_prefix_warnings(world: &mut OcflWorld) {
    for (name, report) in &world.validate.group {
        let warnings = report.warning_codes();
        for code in prefix_codes(name) {
            assert!(warnings.contains(code), "{name}: {code} not reported");
        }
    }
}

#[then("every object should be rejected as invalid")]
fn group_is_rejected(world: &mut OcflWorld) {
    for (name, report) in &world.validate.group {
        assert!(!report.is_valid(), "{name} must be rejected");
    }
}

/// The fixtures whose name lists an error code their report lacks.
fn gaps(group: &[(String, ValidationReport)]) -> BTreeSet<&str> {
    group
        .iter()
        .filter(|(name, report)| {
            let errors = report.error_codes();
            prefix_codes(name).iter().any(|code| !errors.contains(code))
        })
        .map(|(name, _)| name.as_str())
        .collect()
}

#[then("each error code in a directory prefix that was not reported should be listed in the Walkthrough")]
fn gaps_are_listed(world: &mut OcflWorld) {
    let walkthrough = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/tasks/Task-IO-0.5-Walkthrough.md");
    let text = std::fs::read_to_string(&walkthrough).expect("read the IO-0.5 Walkthrough");
    for name in gaps(&world.validate.group) {
        assert!(
            text.contains(&format!("`{name}`")),
            "{name} is not listed in the Walkthrough"
        );
    }
}

#[when("I inspect the OCFL fixture pin")]
fn inspect_pin(world: &mut OcflWorld) {
    let notice = committed_dir().join("NOTICE.md");
    world.validate.notice = std::fs::read_to_string(notice).expect("read NOTICE.md");
    let cargo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    world.validate.cargo = std::fs::read_to_string(cargo).expect("read Cargo.toml");
}

#[then("the fixtures should be pinned to one OCFL/fixtures commit")]
fn pinned_to_one_commit(world: &mut OcflWorld) {
    let notice = &world.validate.notice;
    assert!(
        notice.contains(&format!("`{PINNED_COMMIT}`")),
        "pin missing"
    );
    assert!(notice.contains("CC-BY-4.0"), "licence attribution missing");
    let zenodo = std::fs::read_to_string(committed_dir().join(".zenodo.json")).expect("zenodo");
    assert!(
        zenodo.contains("cc-by-4.0"),
        "upstream licence file missing"
    );
    assert!(committed_dir().join("1.1").is_dir());
    assert!(!committed_dir().join("1.0").exists(), "1.0 is out of scope");
}

#[then("the fixtures should not be packaged in or loaded by the filesquid-ocfl library or CLI")]
fn not_packaged_or_loaded(world: &mut OcflWorld) {
    assert!(
        world
            .validate
            .cargo
            .contains(r#"exclude = ["tests/fixtures/"]"#),
        "Cargo.toml must exclude tests/fixtures/ from the package"
    );
    let needles = ["tests/fixtures", "FILESQUID_OCFL_FIXTURES"];
    let sources = crate::source_scan::read_sources().expect("scan src");
    assert!(sources.len() > 5, "engine sources not found");
    let findings: Vec<&str> = sources
        .iter()
        .filter(|(_, text)| needles.iter().any(|needle| text.contains(needle)))
        .map(|(file, _)| file.as_str())
        .collect();
    assert!(
        findings.is_empty(),
        "the library refers to the fixtures: {findings:?}"
    );
}
