//! `c2patool` interoperability oracle harness (C2PA-S-007).
//!
//! The official `c2patool` binary runs out of process on each C2PA conformance
//! file and its JSON report is reduced to an [`OracleReport`]. [`compare`]
//! checks an engine report against it. The official tool is never linked.
//!
//! Configuration:
//! - `FILESQUID_C2PATOOL`: path to `c2patool` (default: `c2patool` on `PATH`).
//! - `FILESQUID_C2PA_CONFORMANCE_DIR`: a checkout of the C2PA public test
//!   files (`https://github.com/c2pa-org/public-testfiles`).
//!
//! When either is missing the harness reports [`OracleSetup::Skipped`] with the
//! reason, so the gate passes on machines without the oracle.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Environment variable naming the `c2patool` binary.
pub const TOOL_ENV: &str = "FILESQUID_C2PATOOL";

/// Environment variable naming the conformance corpus directory.
pub const CORPUS_ENV: &str = "FILESQUID_C2PA_CONFORMANCE_DIR";

/// Asset extensions the harness feeds to the oracle (lowercase).
pub const ASSET_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "c2pa"];

/// Cap on corpus files per run.
pub const MAX_CORPUS_FILES: usize = 1000;

/// Cap on corpus directory depth.
pub const MAX_DEPTH: usize = 12;

/// A ready-to-run oracle.
#[derive(Debug, Clone)]
pub struct OracleConfig {
    /// The `c2patool` binary.
    pub tool: PathBuf,
    /// Root of the conformance corpus.
    pub corpus: PathBuf,
}

/// Whether the oracle can run here.
#[derive(Debug)]
pub enum OracleSetup {
    /// Tool and corpus were found.
    Ready(OracleConfig),
    /// The oracle cannot run; the string says why and how to fix it.
    Skipped(String),
}

/// Overall verdict of a validator on one asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The asset carries no manifest store.
    NoManifest,
    /// Well-formed and cryptographically valid, signer not on a trust list.
    Valid,
    /// Valid and the signer chains to a supplied trust anchor.
    Trusted,
    /// At least one failure status code.
    Invalid,
}

/// A validator's result for one asset: verdict plus sorted failure codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleReport {
    /// Overall verdict.
    pub verdict: Verdict,
    /// C2PA failure status codes, sorted and unique.
    pub failure_codes: Vec<String>,
}

/// Result of comparing the engine with the oracle on one asset.
#[derive(Debug, PartialEq, Eq)]
pub enum Agreement {
    /// Same verdict and same failure codes.
    Agree,
    /// The engine and the oracle differ.
    Disagree {
        /// What `c2patool` reported.
        oracle: OracleReport,
        /// What the engine reported.
        engine: OracleReport,
    },
    /// The engine has no reader for this asset yet (manifest reading lands in
    /// CP-0.2 to CP-0.5); the oracle verdict is still recorded.
    EnginePending,
}

/// Find the tool and corpus from the environment.
pub fn oracle_setup_from_env() -> OracleSetup {
    let Some(tool) = find_tool() else {
        return OracleSetup::Skipped(format!(
            "c2patool not found: set {TOOL_ENV} or put c2patool on PATH (cargo install c2patool --locked)"
        ));
    };
    let Some(corpus) = std::env::var_os(CORPUS_ENV).map(PathBuf::from) else {
        return OracleSetup::Skipped(format!(
            "{CORPUS_ENV} is not set: clone https://github.com/c2pa-org/public-testfiles and point it there"
        ));
    };
    if !corpus.is_dir() {
        return OracleSetup::Skipped(format!(
            "{CORPUS_ENV}={} is not a directory",
            corpus.display()
        ));
    }
    OracleSetup::Ready(OracleConfig { tool, corpus })
}

/// Resolve `c2patool` from [`TOOL_ENV`] or by searching `PATH`.
fn find_tool() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(TOOL_ENV).map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|dir| [dir.join("c2patool"), dir.join("c2patool.exe")])
        .find(|candidate| candidate.is_file())
}

/// Return the sorted asset files under `dir`. More than `max_files` assets or
/// nesting deeper than [`MAX_DEPTH`] is an error. `.git` directories are skipped.
pub fn corpus_files(dir: &Path, max_files: usize) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk(dir, 0, max_files, &mut files)?;
    files.sort();
    Ok(files)
}

/// Collect assets under `dir` in sorted entry order. Exceeding `max` files
/// or [`MAX_DEPTH`] is an error, so a run never checks an arbitrary subset.
fn walk(dir: &Path, depth: usize, max: usize, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "{} is deeper than {MAX_DEPTH} levels below the corpus root",
            dir.display()
        ));
    }
    for path in sorted_entries(dir)? {
        if path.is_dir() && path.file_name().is_some_and(|name| name != ".git") {
            walk(&path, depth + 1, max, out)?;
        } else if is_asset(&path) {
            if out.len() >= max {
                return Err(format!("the corpus has more than {max} assets"));
            }
            out.push(path);
        }
    }
    Ok(())
}

/// The entries of `dir`, sorted by path.
fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut paths = entries
        .map(|entry| entry.map(|e| e.path()).map_err(|err| err.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}

/// Return whether `path` has one of the [`ASSET_EXTENSIONS`].
pub fn is_asset(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|ext| ASSET_EXTENSIONS.contains(&ext.as_str()))
}

/// Top-level fields that identify a `c2patool` manifest-store report.
pub const REPORT_FIELDS: &[&str] = &[
    "validation_state",
    "validation_status",
    "validation_results",
    "active_manifest",
];

/// Reduce a `c2patool` JSON report to an [`OracleReport`].
///
/// The JSON MUST carry at least one of [`REPORT_FIELDS`]; anything else (for
/// example `{}`) is an error, never a verdict.
pub fn parse_c2patool_report(json: &str) -> Result<OracleReport, String> {
    let report: Value =
        serde_json::from_str(json).map_err(|err| format!("c2patool JSON: {err}"))?;
    if !REPORT_FIELDS
        .iter()
        .any(|field| report.get(field).is_some())
    {
        return Err(format!(
            "not a c2patool report: none of {REPORT_FIELDS:?} is present"
        ));
    }
    let failure_codes = failure_codes(&report);
    let verdict = match report.get("validation_state").and_then(Value::as_str) {
        Some("Trusted") => Verdict::Trusted,
        Some("Valid") => Verdict::Valid,
        Some("Invalid") => Verdict::Invalid,
        Some(other) => return Err(format!("unknown validation_state {other}")),
        None if failure_codes.is_empty() => Verdict::Valid,
        None => Verdict::Invalid,
    };
    Ok(OracleReport {
        verdict,
        failure_codes,
    })
}

/// Collect failure codes from `validation_status` (legacy) and from
/// `validation_results.activeManifest.failure`.
fn failure_codes(report: &Value) -> Vec<String> {
    let legacy = report.get("validation_status").and_then(Value::as_array);
    let active = report
        .pointer("/validation_results/activeManifest/failure")
        .and_then(Value::as_array);
    let mut codes: Vec<String> = legacy
        .into_iter()
        .chain(active)
        .flatten()
        .filter_map(|entry| entry.get("code").and_then(Value::as_str))
        .filter(|code| is_failure_code(code))
        .map(str::to_string)
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

/// C2PA success and informational codes that legacy `validation_status`
/// lists may carry; everything else there is a failure.
fn is_failure_code(code: &str) -> bool {
    !(code.ends_with(".validated")
        || code.ends_with(".match")
        || code.ends_with(".trusted")
        || code.ends_with(".insideValidity")
        || code.ends_with(".notRevoked"))
}

/// Text `c2patool` prints when an asset has no manifest store.
const NO_MANIFEST_MARKERS: &[&str] = &["No claim found", "JumbfNotFound", "no JUMBF data found"];

/// Run `c2patool <file>` and reduce its report.
pub fn run_c2patool(tool: &Path, file: &Path) -> Result<OracleReport, String> {
    let output = Command::new(tool)
        .arg(file)
        .output()
        .map_err(|err| format!("run {}: {err}", tool.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        return parse_c2patool_report(&stdout);
    }
    if NO_MANIFEST_MARKERS
        .iter()
        .any(|marker| stderr.contains(marker) || stdout.contains(marker))
    {
        return Ok(OracleReport {
            verdict: Verdict::NoManifest,
            failure_codes: Vec::new(),
        });
    }
    Err(format!(
        "{} exited with {} on {} without a report: {}",
        tool.display(),
        output.status,
        file.display(),
        stderr.lines().next().unwrap_or("").trim()
    ))
}

/// The engine's report for an asset. Returns `None` until the engine can
/// read manifest stores (CP-0.2 to CP-0.5 wire `filesquid_c2pa` in here).
pub fn engine_report(_asset: &[u8]) -> Option<OracleReport> {
    None
}

/// Compare an engine report with the oracle report for one asset.
pub fn compare(oracle: &OracleReport, engine: Option<&OracleReport>) -> Agreement {
    match engine {
        None => Agreement::EnginePending,
        Some(engine) if engine == oracle => Agreement::Agree,
        Some(engine) => Agreement::Disagree {
            oracle: oracle.clone(),
            engine: engine.clone(),
        },
    }
}
