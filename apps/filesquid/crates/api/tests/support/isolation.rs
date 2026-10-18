//! Application-layer dependency rules for `filesquid-api` ([API-S-006], Task CL-0.1).
//!
//! The **graph guard** checks every package in the resolved runtime graph (`cargo tree`,
//! normal and build edges, every target) against [`RULES`], for the default, the no-default
//! and the all-features sets, so Cargo features are never the only boundary. The **source
//! guard** is `source_scan.rs`. Both must find nothing: the SQLite journal store and the
//! data-directory checks live in `filesquid-native`, which the hosts inject.
//!
//! [API-S-006]: ../../resources/srs.md

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

/// One forbidden crate (or crate family) and the requirement it enforces.
pub struct Rule {
    /// Name or name prefix, in Cargo package spelling.
    pub pattern: &'static str,
    /// Exact or prefix match.
    pub kind: Match,
    /// Requirement the rule enforces, shown on failure.
    pub reason: &'static str,
}

const HOST: &str = "API-S-006: no Tauri host APIs";
const MEDIA: &str = "API-S-006: no libvips or FFmpeg";
const SQLITE: &str = "API-S-006: no SQLite driver";
const STORAGE: &str = "API-S-006: no storage I/O crate; hosts inject storage backends";
const NETWORK: &str = "API-S-006: no network client; hosts inject storage backends";
const RUNTIME: &str = "API-S-006: no alternative async runtime crate (tokio itself is allowed; its features are not checked)";
const NATIVE: &str = "API-S-006: the API never links native processing";

/// Every forbidden crate. Names use the Cargo package spelling.
pub const RULES: &[Rule] = &[
    rule("tauri", Match::Prefix, HOST),
    rule("libvips", Match::Prefix, MEDIA),
    rule("ffmpeg", Match::Prefix, MEDIA),
    rule("rusqlite", Match::Exact, SQLITE),
    rule("libsqlite3-sys", Match::Exact, SQLITE),
    rule("sqlite", Match::Exact, SQLITE),
    rule("sqlx", Match::Prefix, SQLITE),
    rule("fs4", Match::Exact, STORAGE),
    rule("filesquid-native", Match::Exact, NATIVE),
    rule("reqwest", Match::Exact, NETWORK),
    rule("ureq", Match::Exact, NETWORK),
    rule("isahc", Match::Exact, NETWORK),
    rule("surf", Match::Exact, NETWORK),
    rule("curl", Match::Prefix, NETWORK),
    rule("attohttpc", Match::Exact, NETWORK),
    rule("async-std", Match::Exact, RUNTIME),
    rule("smol", Match::Exact, RUNTIME),
    rule("async-executor", Match::Exact, RUNTIME),
    rule("async-global-executor", Match::Exact, RUNTIME),
    rule("async-io", Match::Exact, RUNTIME),
    rule("async-net", Match::Exact, RUNTIME),
    rule("actix-rt", Match::Exact, RUNTIME),
    rule("glommio", Match::Exact, RUNTIME),
    rule("monoio", Match::Exact, RUNTIME),
    rule("compio", Match::Prefix, RUNTIME),
    rule("tokio-uring", Match::Exact, RUNTIME),
];

const fn rule(pattern: &'static str, kind: Match, reason: &'static str) -> Rule {
    Rule {
        pattern,
        kind,
        reason,
    }
}

/// This crate's package name.
pub const SELF_NAME: &str = "filesquid-api";

/// FileSquid module crates the application layer MAY link ([ADR 025], PRD TC-005).
///
/// [ADR 025]: ../../../../../../resources/architecture/025-client-side-filesquid-engines.md
pub const ALLOWED_MODULES: &[&str] = &[
    "filesquid-core",
    "filesquid-ocfl",
    "filesquid-metadata",
    "filesquid-c2pa",
];

/// The feature sets the graph guard checks, as a label and the `cargo tree` arguments.
pub const FEATURE_SETS: &[(&str, &[&str])] = &[
    ("default", &[]),
    ("no-default-features", &["--no-default-features"]),
    ("all-features", &["--all-features"]),
];

/// The `cargo tree` arguments for a feature-set label from [`FEATURE_SETS`].
pub fn feature_args(label: &str) -> Option<&'static [&'static str]> {
    FEATURE_SETS
        .iter()
        .find(|(name, _)| *name == label)
        .map(|(_, args)| *args)
}

/// Return why `name` is forbidden, or `None` when it is allowed.
pub fn forbidden_reason(name: &str) -> Option<&'static str> {
    RULES
        .iter()
        .find(|rule| match rule.kind {
            Match::Exact => name == rule.pattern,
            Match::Prefix => name.starts_with(rule.pattern),
        })
        .map(|rule| rule.reason)
}

/// Return the sorted, unique forbidden package names in `names`.
pub fn violations<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut found: Vec<String> = names
        .into_iter()
        .filter(|name| forbidden_reason(name).is_some())
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();
    found
}

/// A crate name that `rule` matches: the pattern itself for an exact rule, the pattern plus a
/// suffix for a prefix rule.
pub fn sample_name(rule: &Rule) -> String {
    match rule.kind {
        Match::Exact => rule.pattern.to_string(),
        Match::Prefix => format!("{}-sample", rule.pattern),
    }
}

/// The `filesquid-api` crate directory.
pub fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The FileSquid Cargo workspace manifest (`apps/filesquid/Cargo.toml`).
pub fn workspace_manifest() -> PathBuf {
    crate_dir().join("../../Cargo.toml")
}

/// Run `cargo tree` (I/O: spawns the `cargo` that runs the tests) for `filesquid-api` with
/// normal and build edges on every target, and return the package names of the graph.
pub fn run_cargo_tree(feature_args: &[&str]) -> Result<Vec<String>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
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

/// Extract sorted, unique package names from `cargo tree --format {p}` lines such as
/// `sha2 v0.11.1` or `filesquid-api v0.1.0 (/path)`.
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
