//! Validator conformance against every object of the pinned OCFL 1.1
//! community fixtures (OCFL-S-009, OCFL-S-011; `OCFL/fixtures` commit
//! `d323fc207126becec066998b43408d998bf0fb1b`, committed under
//! `tests/fixtures/ocfl/1.1`).
//!
//! - (a) every `good-objects` object validates with zero errors;
//! - (b) every `warn-objects` object validates with zero errors and reports
//!   each warning code its name starts with;
//! - (c) every `bad-objects` object is rejected. A code its name starts with
//!   that the validator does not report is allowed (the fixtures README lets
//!   validators stop early) but must be one of [`EXPECTED_GAPS`], which the
//!   task Walkthrough lists per fixture.
//!
//! No fixture is skipped: the group counts are asserted against the pinned
//! commit. Each test prints one line per fixture; run with `--nocapture` to
//! see them.

use std::collections::BTreeMap;
use std::path::Path;

use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use filesquid_ocfl::validate::{validate_object, ValidationReport};

#[path = "../support/fixtures.rs"]
mod fixtures;
use fixtures::{group_backend, objects, prefix_codes, tree, PINNED_GROUPS};

/// Bad fixtures whose name lists a code the validator does not report, with
/// those codes. Each is rejected with other codes; the reason is in the
/// task Walkthrough. Any change to this list is a change in conformance and
/// must be explained there.
const EXPECTED_GAPS: &[(&str, &[&str])] = &[
    ("E008_E036_no_versions_no_head", &["E008"]),
    ("E011_E013_invalid_padded_head_version", &["E013"]),
    ("E049_E050_E054_bad_version_block_values", &["E050", "E054"]),
    ("E053_E052_invalid_logical_paths", &["E052"]),
    ("E100_E099_fixity_invalid_content_paths", &["E100"]),
    ("E100_E099_manifest_invalid_content_paths", &["E100"]),
];

/// The pinned object count of `group`.
fn pinned_count(group: &str) -> usize {
    PINNED_GROUPS
        .iter()
        .find(|(name, _)| *name == group)
        .map(|(_, count)| *count)
        .expect("a pinned group")
}

/// Validate one fixture object through the filesystem backend.
fn validate(backend: &FsBackend, name: &str) -> ValidationReport {
    complete_immediately(validate_object(backend, name))
        .expect("fs futures complete at once")
        .unwrap_or_else(|err| panic!("{name}: the validator could not run: {err}"))
}

/// Every object of `group` with its report, after checking the pinned count.
fn validate_group(group: &str) -> Vec<(String, ValidationReport)> {
    let dir = tree().join(group);
    let backend = group_backend(&dir, "fixture-conformance");
    let names = objects(&dir);
    assert_eq!(names.len(), pinned_count(group), "{group}: pinned count");
    names
        .into_iter()
        .map(|name| {
            let report = validate(&backend, &name);
            (name, report)
        })
        .collect()
}

/// The codes of a set, comma separated, or `none`.
fn codes<'a>(set: impl IntoIterator<Item = &'a str>) -> String {
    let list: Vec<&str> = set.into_iter().collect();
    if list.is_empty() {
        return "none".to_string();
    }
    list.join(", ")
}

/// One result line for the Walkthrough.
fn describe(group: &str, name: &str, report: &ValidationReport) -> String {
    let verdict = if report.is_valid() {
        "valid"
    } else {
        "invalid"
    };
    format!(
        "{group}/{name}: {verdict}; errors: {}; warnings: {}",
        codes(report.error_codes()),
        codes(report.warning_codes())
    )
}

#[test]
fn every_good_fixture_validates_with_zero_errors() {
    for (name, report) in validate_group("good-objects") {
        println!("{}", describe("good-objects", &name, &report));
        assert!(report.errors().is_empty(), "{name}: {:#?}", report.errors());
    }
}

#[test]
fn every_warning_fixture_has_zero_errors_and_its_prefix_warnings() {
    for (name, report) in validate_group("warn-objects") {
        println!("{}", describe("warn-objects", &name, &report));
        assert!(report.errors().is_empty(), "{name}: {:#?}", report.errors());
        let warnings = report.warning_codes();
        for code in prefix_codes(&name) {
            assert!(warnings.contains(code), "{name}: {code} not reported");
        }
    }
}

/// The prefix codes of `name` that `report` does not include.
fn missing_codes<'a>(name: &'a str, report: &ValidationReport) -> Vec<&'a str> {
    let errors = report.error_codes();
    prefix_codes(name)
        .into_iter()
        .filter(|code| !errors.contains(code))
        .collect()
}

#[test]
fn every_bad_fixture_is_rejected() {
    let mut gaps: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let results = validate_group("bad-objects");
    for (name, report) in &results {
        println!("{}", describe("bad-objects", name, report));
        assert!(!report.is_valid(), "{name} must be rejected");
        let missing = missing_codes(name, report);
        if !missing.is_empty() {
            gaps.insert(name.clone(), missing);
        }
    }
    for (name, missing) in &gaps {
        println!(
            "gap: bad-objects/{name}: {} not reported",
            codes(missing.clone())
        );
    }
    let expected: BTreeMap<String, Vec<&str>> = EXPECTED_GAPS
        .iter()
        .map(|(name, codes)| ((*name).to_string(), codes.to_vec()))
        .collect();
    assert_eq!(gaps, expected, "prefix codes not reported");
}

#[test]
fn the_tree_is_the_pinned_one_with_no_object_left_out() {
    let dir = tree();
    let groups: Vec<String> = objects(&dir);
    assert_eq!(
        groups,
        ["bad-objects", "content", "good-objects", "warn-objects"],
        "the 1.1 tree holds the four upstream groups"
    );
    for (group, count) in PINNED_GROUPS {
        assert_eq!(objects(&dir.join(group)).len(), count, "{group}");
    }
    assert!(Path::new(&dir).join("content").is_dir());
}
