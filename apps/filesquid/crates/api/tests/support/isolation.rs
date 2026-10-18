//! Application-layer isolation rules for `filesquid-api` ([API-S-006], Task CL-0.1).
//!
//! Two guards share this module:
//!
//! - the **graph guard** checks every package in the resolved runtime graph (`cargo tree`,
//!   normal and build edges, every target) against [`RULES`], for the default, the
//!   no-default and the all-features sets, so Cargo features are never the only boundary;
//! - the **source guard** scans every file under `src/` for direct storage I/O
//!   ([`STORAGE_IO_PATTERNS`]).
//!
//! `main` already deviates from API-S-006 in a few places that the documents do not yet say
//! how to move. Those deviations are pinned exactly in [`pinned_graph_deviations`] and
//! [`PINNED_SOURCE_DEVIATIONS`] and described in the Conflict Artifact
//! `resources/tasks/Task-CL-0.1-Conflict.md`. A new violation fails the guard, and so does a
//! pinned one that disappears, so the pin list can only shrink deliberately.
//!
//! [API-S-006]: ../../resources/srs.md

use std::path::{Path, PathBuf};
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
    rule("reqwest", Match::Exact, NETWORK),
    rule("ureq", Match::Exact, NETWORK),
    rule("isahc", Match::Exact, NETWORK),
    rule("surf", Match::Exact, NETWORK),
    rule("curl", Match::Prefix, NETWORK),
    rule("attohttpc", Match::Exact, NETWORK),
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

/// Forbidden packages that `main` already links, per feature set, sorted. Pinned by the
/// Conflict Artifact `Task-CL-0.1-Conflict.md`: the `sqlite` feature's journal store
/// (`rusqlite`, `libsqlite3-sys`) and the free-space probe (`fs4`).
pub fn pinned_graph_deviations(feature_set: &str) -> &'static [&'static str] {
    match feature_set {
        "no-default-features" => &["fs4"],
        _ => &["fs4", "libsqlite3-sys", "rusqlite"],
    }
}

/// Source files under `src/` that already perform storage I/O on `main`, sorted. Pinned by
/// the Conflict Artifact `Task-CL-0.1-Conflict.md`: the SQLite journal store and the host
/// monitor's data-directory checks.
pub const PINNED_SOURCE_DEVIATIONS: &[&str] = &["src/journal/sqlite.rs", "src/monitor.rs"];

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

/// Substrings that mark direct storage I/O in Rust source.
pub const STORAGE_IO_PATTERNS: &[&str] = &[
    "std::fs",
    "tokio::fs",
    "fs4::",
    "os::unix::fs",
    "os::windows::fs",
];

/// Whether one source line performs or imports direct storage I/O. Text after `//` is
/// ignored, and a grouped `use std::{..., fs, ...}` counts as `std::fs`.
pub fn line_uses_storage_io(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or("");
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if STORAGE_IO_PATTERNS.iter().any(|p| compact.contains(p)) {
        return true;
    }
    compact.contains("std::{")
        && compact
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|t| t == "fs")
}

/// Whether any line of `source` performs or imports direct storage I/O.
pub fn source_uses_storage_io(source: &str) -> bool {
    source.lines().any(line_uses_storage_io)
}

/// Maximum directory depth and file count the source walk visits (AGENTS.md §22).
const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 2_000;

/// List every `.rs` file under `dir` (I/O: reads directories), at most [`MAX_DEPTH`] deep and
/// [`MAX_FILES`] files; reaching a cap is an error, never a silent truncation.
pub fn list_rust_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{} is deeper than {MAX_DEPTH}", dir.display()));
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            list_rust_files(&path, depth + 1, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
        if out.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} source files"));
        }
    }
    Ok(())
}

/// Read every file under `src/` (I/O) and return the sorted crate-relative paths (with `/`
/// separators) of those that perform direct storage I/O.
pub fn read_sources_with_storage_io() -> Result<Vec<String>, String> {
    let root = crate_dir();
    let mut files = Vec::new();
    list_rust_files(&root.join("src"), 0, &mut files)?;
    let mut found = Vec::new();
    for file in files {
        let source =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if source_uses_storage_io(&source) {
            found.push(relative_name(&root, &file));
        }
    }
    found.sort();
    Ok(found)
}

/// `file` relative to `root`, with `/` separators on every platform.
fn relative_name(root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file);
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
