//! Dependency isolation rules for `filesquid-c2pa` (C2PA-S-007, C2PA-S-008).
//!
//! The resolved graph comes from `cargo tree` with normal and build edges
//! only, so dev-dependencies (where the official `c2pa` crate MAY appear as an
//! oracle) are never reported. Every name is checked against [`RULES`].

use std::path::PathBuf;
use std::process::Command;

/// How a rule matches a crate name.
#[derive(Debug, Clone, Copy)]
pub enum Match {
    /// The crate name equals the pattern.
    Exact,
    /// The crate name starts with the pattern.
    Prefix,
}

/// One forbidden crate (or crate family) and why it is forbidden.
pub struct Rule {
    /// Name or name prefix.
    pub pattern: &'static str,
    /// Exact or prefix match.
    pub kind: Match,
    /// Requirement the rule enforces, shown on failure.
    pub reason: &'static str,
}

const FILESQUID: &str = "C2PA-S-008: no other FileSquid crate";
const DISTRIBUTION: &str = "C2PA-S-008: no distribution code";
const HOST: &str = "C2PA-S-008: no Axum or Tauri";
const STORAGE: &str = "C2PA-S-008: no SQLite";
const MEDIA: &str = "C2PA-S-008: no libvips or FFmpeg";
const ORACLE: &str = "C2PA-S-007: the official c2pa crate is an oracle, never a runtime dependency";
const NETWORK: &str = "C2PA-S-008: no network client or async runtime in the core";

/// Every forbidden crate. Names use the Cargo package spelling.
pub const RULES: &[Rule] = &[
    rule("filesquid-", Match::Prefix, FILESQUID),
    rule("imagesquid", Match::Prefix, DISTRIBUTION),
    rule("arkrim", Match::Prefix, DISTRIBUTION),
    rule("axum", Match::Prefix, HOST),
    rule("tauri", Match::Prefix, HOST),
    rule("rusqlite", Match::Exact, STORAGE),
    rule("sqlite", Match::Exact, STORAGE),
    rule("libsqlite3-sys", Match::Exact, STORAGE),
    rule("sqlx", Match::Prefix, STORAGE),
    rule("libvips", Match::Prefix, MEDIA),
    rule("ffmpeg", Match::Prefix, MEDIA),
    rule("c2pa", Match::Exact, ORACLE),
    rule("c2pa-", Match::Prefix, ORACLE),
    rule("c2patool", Match::Exact, ORACLE),
    rule("reqwest", Match::Exact, NETWORK),
    rule("hyper", Match::Prefix, NETWORK),
    rule("ureq", Match::Exact, NETWORK),
    rule("isahc", Match::Exact, NETWORK),
    rule("surf", Match::Exact, NETWORK),
    rule("curl", Match::Prefix, NETWORK),
    rule("attohttpc", Match::Exact, NETWORK),
    rule("tokio", Match::Prefix, NETWORK),
    rule("async-std", Match::Exact, NETWORK),
    rule("mio", Match::Exact, NETWORK),
];

const fn rule(pattern: &'static str, kind: Match, reason: &'static str) -> Rule {
    Rule {
        pattern,
        kind,
        reason,
    }
}

/// This crate's own package name, which the `filesquid-` prefix rule allows.
pub const SELF_NAME: &str = "filesquid-c2pa";

/// Return why `name` is forbidden, or `None` when it is allowed.
pub fn forbidden_reason(name: &str) -> Option<&'static str> {
    if name == SELF_NAME {
        return None;
    }
    RULES
        .iter()
        .find(|rule| match rule.kind {
            Match::Exact => name == rule.pattern,
            Match::Prefix => name.starts_with(rule.pattern),
        })
        .map(|rule| rule.reason)
}

/// Return `"name: reason"` for every forbidden crate in `names`.
pub fn violations<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    names
        .into_iter()
        .filter_map(|name| forbidden_reason(name).map(|reason| format!("{name}: {reason}")))
        .collect()
}

/// The FileSquid Cargo workspace manifest (`apps/filesquid/Cargo.toml`).
pub fn workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")
}

/// The `cargo` binary that is running the tests, or `cargo` from `PATH`.
pub fn cargo_binary() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// Run `cargo tree` (normal and build edges, every target) and return the
/// package names in the resolved graph of `filesquid-c2pa`.
///
/// `feature_args` selects the feature set, for example `&[]` for the
/// defaults or `&["--all-features"]`.
pub fn resolved_runtime_packages(feature_args: &[&str]) -> Result<Vec<String>, String> {
    let output = Command::new(cargo_binary())
        .arg("tree")
        .arg("--manifest-path")
        .arg(workspace_manifest())
        .args(["-p", SELF_NAME, "-e", "normal,build", "--target", "all"])
        .args(["--prefix", "none", "--format", "{p}", "--locked"])
        .args(feature_args)
        .output()
        .map_err(|err| format!("run cargo tree: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(package_names(&String::from_utf8_lossy(&output.stdout)))
}

/// Extract sorted, unique package names from `cargo tree --format {p}` lines
/// such as `sha2 v0.11.1` or `filesquid-c2pa v0.1.0 (/path)`.
pub fn package_names(tree: &str) -> Vec<String> {
    let mut names: Vec<String> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}
