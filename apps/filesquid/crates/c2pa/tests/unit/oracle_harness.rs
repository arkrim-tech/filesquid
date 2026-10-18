//! C2PA-S-007: unit tests for the `c2patool` oracle harness that need neither
//! the tool nor the corpus: report parsing, comparison and corpus discovery.

#[path = "../support/oracle.rs"]
#[allow(dead_code)]
mod oracle;

use std::fs;
use std::path::{Path, PathBuf};

use oracle::{
    compare, corpus_files, is_asset, parse_c2patool_report, run_c2patool, Agreement, OracleReport,
    Verdict, MAX_DEPTH,
};

/// A trimmed `c2patool` report for a valid, untrusted asset.
const VALID_REPORT: &str = r#"{
  "active_manifest": "urn:c2pa:1",
  "manifests": {},
  "validation_results": {
    "activeManifest": {
      "success": [{"code": "claimSignature.validated"}],
      "informational": [],
      "failure": [{"code": "signingCredential.untrusted"}]
    }
  },
  "validation_state": "Valid"
}"#;

/// A legacy report with only `validation_status` and a data-hash mismatch.
const LEGACY_INVALID_REPORT: &str = r#"{
  "active_manifest": "urn:c2pa:2",
  "validation_status": [
    {"code": "assertion.dataHash.mismatch"},
    {"code": "claimSignature.validated"},
    {"code": "assertion.dataHash.mismatch"}
  ]
}"#;

#[test]
fn parses_validation_state_and_failure_codes() {
    let report = parse_c2patool_report(VALID_REPORT).expect("report");
    assert_eq!(report.verdict, Verdict::Valid);
    assert_eq!(report.failure_codes, ["signingCredential.untrusted"]);
}

#[test]
fn legacy_status_without_state_is_invalid_on_failure() {
    let report = parse_c2patool_report(LEGACY_INVALID_REPORT).expect("report");
    assert_eq!(report.verdict, Verdict::Invalid);
    assert_eq!(report.failure_codes, ["assertion.dataHash.mismatch"]);
}

#[test]
fn trusted_state_is_recognised() {
    let report = parse_c2patool_report(r#"{"validation_state":"Trusted"}"#).expect("report");
    assert_eq!(report.verdict, Verdict::Trusted);
}

#[test]
fn json_without_report_fields_is_an_error() {
    for json in ["{}", r#"{"manifests":{}}"#, "[]", "null"] {
        let err = parse_c2patool_report(json).expect_err(json);
        assert!(err.contains("not a c2patool report"), "{json}: {err}");
    }
}

#[test]
fn unexpected_tool_exit_is_an_error_not_a_verdict() {
    // `cargo` given an unknown subcommand exits non-zero with unrelated
    // stderr, standing in for a broken or incompatible oracle binary.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let result = run_c2patool(Path::new(&cargo), Path::new("no-such-asset.jpg"));
    let err = result.expect_err("unexpected exit must be an error");
    assert!(err.contains("without a report"), "{err}");
}

#[test]
fn malformed_json_and_unknown_state_are_errors() {
    assert!(parse_c2patool_report("not json").is_err());
    assert!(parse_c2patool_report(r#"{"validation_state":"Maybe"}"#).is_err());
}

fn report(verdict: Verdict, codes: &[&str]) -> OracleReport {
    OracleReport {
        verdict,
        failure_codes: codes.iter().map(|c| (*c).to_string()).collect(),
    }
}

#[test]
fn comparison_agrees_on_identical_reports() {
    let oracle = report(Verdict::Invalid, &["assertion.dataHash.mismatch"]);
    assert_eq!(compare(&oracle, Some(&oracle.clone())), Agreement::Agree);
}

#[test]
fn comparison_reports_a_different_verdict() {
    let oracle = report(Verdict::Invalid, &["assertion.dataHash.mismatch"]);
    let engine = report(Verdict::Valid, &[]);
    assert!(matches!(
        compare(&oracle, Some(&engine)),
        Agreement::Disagree { .. }
    ));
}

#[test]
fn comparison_reports_different_failure_codes() {
    let oracle = report(Verdict::Invalid, &["assertion.dataHash.mismatch"]);
    let engine = report(Verdict::Invalid, &["claimSignature.mismatch"]);
    assert!(matches!(
        compare(&oracle, Some(&engine)),
        Agreement::Disagree { .. }
    ));
}

#[test]
fn comparison_without_engine_report_is_pending() {
    let oracle = report(Verdict::NoManifest, &[]);
    assert_eq!(compare(&oracle, None), Agreement::EnginePending);
}

/// Create a scratch corpus tree unique to this process and `tag`.
fn scratch_corpus(tag: &str) -> PathBuf {
    let name = format!("filesquid-c2pa-corpus-{}-{tag}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove stale {}: {err}", dir.display()),
    }
    fs::create_dir_all(dir.join("image/jpeg")).expect("corpus dir");
    fs::create_dir_all(dir.join(".git")).expect(".git dir");
    for name in [
        "image/jpeg/a.jpg",
        "image/jpeg/B.JPEG",
        "image/x.png",
        "README.md",
        ".git/c.jpg",
    ] {
        fs::write(dir.join(name), b"x").expect("corpus file");
    }
    dir
}

#[test]
fn corpus_discovery_finds_assets_and_skips_git() {
    let dir = scratch_corpus("discovery");
    let files = corpus_files(&dir, 100).expect("walk");
    let names: Vec<String> = files
        .iter()
        .map(|p| {
            p.strip_prefix(&dir)
                .expect("prefix")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(
        names,
        ["image/jpeg/B.JPEG", "image/jpeg/a.jpg", "image/x.png"]
    );
    fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn corpus_discovery_is_bounded() {
    let dir = scratch_corpus("bounded").join("bounded");
    fs::create_dir_all(&dir).expect("dir");
    for i in [4, 0, 3, 1, 2] {
        fs::write(dir.join(format!("{i}.jpg")), b"x").expect("file");
    }
    let names: Vec<String> = corpus_files(&dir, 5)
        .expect("walk at the cap")
        .iter()
        .map(|p| p.file_name().expect("name").to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["0.jpg", "1.jpg", "2.jpg", "3.jpg", "4.jpg"]);
    let err = corpus_files(&dir, 3).expect_err("over the cap");
    assert!(err.contains("more than 3 assets"), "{err}");
    fs::remove_dir_all(dir.parent().expect("parent")).expect("cleanup");
}

#[test]
fn corpus_deeper_than_the_cap_is_an_error() {
    let root = scratch_corpus("deep");
    let mut dir = root.clone();
    for level in 0..=MAX_DEPTH {
        dir = dir.join(format!("d{level}"));
    }
    fs::create_dir_all(&dir).expect("deep dirs");
    fs::write(dir.join("a.jpg"), b"x").expect("file");
    let err = corpus_files(&root, 100).expect_err("too deep");
    assert!(err.contains("deeper than"), "{err}");
    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn asset_extensions_are_case_insensitive() {
    assert!(is_asset(Path::new("a.JPG")));
    assert!(is_asset(Path::new("m.c2pa")));
    assert!(is_asset(Path::new("v.mp4")));
    assert!(!is_asset(Path::new("notes.txt")));
}
