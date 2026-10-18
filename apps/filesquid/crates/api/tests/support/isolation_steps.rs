//! Steps for `application_layer_isolation_API-S-006.feature` (API-S-006, Task CL-0.1): the
//! application-layer crate's name and SRS, its resolved runtime graph per feature set, and
//! the source scan for direct storage I/O, with the rules proven on synthetic inputs.

use crate::isolation::{
    crate_dir, feature_args, forbidden_reason, run_cargo_tree, violations, workspace_manifest,
    SELF_NAME,
};
use crate::source_scan::{io_findings_outside_allowed, scan_text, IO_PATTERNS};
use crate::MonitorWorld;
use cucumber::{given, then, when};

/// Per-scenario state of the isolation steps, kept in [`MonitorWorld::isolation`].
#[derive(Debug, Default)]
pub struct IsolationWorld {
    /// Text under test: the workspace manifest or one source line.
    pub input: String,
    /// The crate name, feature-set label or scan subject of the scenario.
    pub subject: String,
    /// Package names of the resolved (or synthetic) graph.
    pub packages: Vec<String>,
    /// Forbidden packages, or `file:line pattern` storage-I/O findings, from the last check.
    pub found: Vec<String>,
}

#[given(expr = "the FileSquid Cargo workspace")]
fn workspace(w: &mut MonitorWorld) {
    let manifest = std::fs::read_to_string(workspace_manifest()).expect("workspace manifest");
    w.isolation.input = manifest;
}

#[when(expr = "I look up the application-layer crate")]
fn look_up(w: &mut MonitorWorld) {
    w.isolation.subject = env!("CARGO_PKG_NAME").to_string();
}

#[then(regex = r"^it should be the package filesquid-api in crates/api$")]
fn is_filesquid_api(w: &mut MonitorWorld) {
    assert_eq!(w.isolation.subject, SELF_NAME);
    assert!(crate_dir().ends_with("crates/api"));
    assert!(
        w.isolation.input.contains("\"crates/api\""),
        "workspace member"
    );
}

#[then(expr = "its module SRS should state the API-S-006 boundary")]
fn srs_states_boundary(_w: &mut MonitorWorld) {
    let srs = std::fs::read_to_string(crate_dir().join("resources/srs.md")).expect("module srs");
    let row = srs.lines().find(|l| l.contains("API-S-006")).expect("row");
    assert!(row.contains("MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver"));
    assert!(row.contains("MUST NOT perform storage I/O itself"));
    assert!(row.contains("Cargo features MUST NOT be the only boundary"));
}

#[given(expr = "the resolved runtime graph of filesquid-api with the {word} feature set")]
fn resolved_graph(w: &mut MonitorWorld, features: String) {
    let args = feature_args(&features).expect("known feature set");
    w.isolation.packages = run_cargo_tree(args).expect("cargo tree");
    w.isolation.subject = features;
}

#[when(expr = "I check it against the application-layer rules")]
fn check_rules(w: &mut MonitorWorld) {
    w.isolation.found = violations(w.isolation.packages.iter().map(String::as_str));
}

#[then(
    expr = "it should hold no Tauri, libvips, FFmpeg, SQLite driver, storage, native-processing, network client or alternative async runtime crate"
)]
fn graph_is_isolated(w: &mut MonitorWorld) {
    assert!(w.isolation.packages.iter().any(|p| p == SELF_NAME), "graph");
    assert!(
        w.isolation.found.is_empty(),
        "{}: {:?}",
        w.isolation.subject,
        w.isolation.found
    );
}

#[given(expr = "a dependency on {word}")]
fn dependency_on(w: &mut MonitorWorld, name: String) {
    w.isolation.packages = vec![SELF_NAME.to_string(), name];
}

#[then(expr = "the dependency should be allowed")]
fn allowed(w: &mut MonitorWorld) {
    assert!(w.isolation.found.is_empty(), "{:?}", w.isolation.found);
}

#[then(expr = "the dependency should be rejected citing API-S-006")]
fn rejected(w: &mut MonitorWorld) {
    assert_eq!(w.isolation.found.len(), 1, "{:?}", w.isolation.found);
    let reason = forbidden_reason(&w.isolation.found[0]).expect("reason");
    assert!(reason.starts_with("API-S-006"), "{reason}");
}

#[given(expr = "the source files of filesquid-api")]
fn source_files(w: &mut MonitorWorld) {
    w.isolation.subject = "src".to_string();
}

/// The feature writes a double quote inside a source line as `QUOTE`.
#[given(regex = r#"^a source line "(.*)"$"#)]
fn source_line(w: &mut MonitorWorld, line: String) {
    w.isolation.input = line.replace("QUOTE", "\"");
    w.isolation.subject = "line".to_string();
}

#[when(regex = r"^I scan them for direct storage I/O$")]
fn scan_sources(w: &mut MonitorWorld) {
    let (findings, scanned) = io_findings_outside_allowed().expect("scan src");
    assert!(scanned > 10, "only {scanned} source files scanned");
    w.isolation.found = findings
        .iter()
        .map(|f| format!("{}:{} {}", f.file, f.line, f.pattern))
        .collect();
}

#[when(regex = r"^I scan it for direct storage I/O$")]
fn scan_line(w: &mut MonitorWorld) {
    w.isolation.found = scan_text("line.rs", &w.isolation.input, IO_PATTERNS)
        .iter()
        .map(|f| f.pattern.to_string())
        .collect();
}

#[then(expr = "no source file should perform it")]
fn no_source_io(w: &mut MonitorWorld) {
    assert!(w.isolation.found.is_empty(), "{:?}", w.isolation.found);
}

#[then(regex = r"^the line should be reported as storage I/O$")]
fn line_reported(w: &mut MonitorWorld) {
    assert!(!w.isolation.found.is_empty(), "{}", w.isolation.input);
}

#[then(regex = r"^the line should not be reported as storage I/O$")]
fn line_not_reported(w: &mut MonitorWorld) {
    assert!(
        w.isolation.found.is_empty(),
        "{}: {:?}",
        w.isolation.input,
        w.isolation.found
    );
}
