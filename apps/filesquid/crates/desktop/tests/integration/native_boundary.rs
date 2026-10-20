//! Task DE-0.8: the native-processing library boundary (DE-S-015; PRD TC-005, ADR 001,
//! ADR 033 amendment 2026-10-18). SQLite storage, the OCFL coordinator and the job and media
//! workers live in `filesquid-native` behind its `desktop` feature; the Tauri package is a
//! thin host. The rules read the workspace's Cargo metadata and the desktop crate's sources,
//! and each is also run on a synthetic case, so a violation is proven to fail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Crates that would give the host a SQLite driver of its own.
const SQLITE_CRATES: [&str; 3] = ["rusqlite", "libsqlite3-sys", "sqlx"];

/// Packages allowed to link `filesquid-native`, with the features each may enable.
const NATIVE_LINKERS: [(&str, &[&str]); 2] = [
    ("filesquid-desktop", &["desktop"]),
    ("filesquid-server", &[]),
];

/// Calls that open storage directly or drive a worker loop: native processing that belongs
/// in `filesquid-native`, never in the host.
const NATIVE_ONLY_PATTERNS: [&str; 10] = [
    "rusqlite",
    "Connection::open",
    "FsBackend::new",
    "LeaseStore::new",
    "ChangeModel::new",
    "StagingArea::new",
    "std::fs::",
    "thread::scope",
    ".backfill_library(",
    ".renew()",
];

/// Host sources allowed to use `std::fs`: the build-time capability check and the
/// test-instrumented range fixture (never shipped).
const FS_ALLOWED: [&str; 2] = ["capability_inputs.rs", "test_fixture.rs"];

/// The direct dependencies of each workspace package with their enabled features (I/O:
/// spawns the `cargo` running the tests).
fn workspace_dependencies() -> BTreeMap<String, Vec<(String, Vec<String>)>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    dependencies_of(&metadata)
}

/// The direct dependencies of each package in `metadata`, with their features. Pure.
fn dependencies_of(metadata: &serde_json::Value) -> BTreeMap<String, Vec<(String, Vec<String>)>> {
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    packages
        .iter()
        .map(|package| {
            let deps = package["dependencies"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let deps = deps
                .iter()
                .map(|dep| {
                    let features = dep["features"].as_array().cloned().unwrap_or_default();
                    let features = features
                        .iter()
                        .filter_map(|f| f.as_str().map(str::to_string))
                        .collect();
                    (
                        dep["name"].as_str().unwrap_or_default().to_string(),
                        features,
                    )
                })
                .collect();
            (
                package["name"].as_str().unwrap_or_default().to_string(),
                deps,
            )
        })
        .collect()
}

/// Every breach of the linking rules in `deps`: a host with a SQLite driver of its own, a
/// package that links `filesquid-native` without being a host, or a host that enables
/// features it may not. Pure.
fn link_violations(deps: &BTreeMap<String, Vec<(String, Vec<String>)>>) -> Vec<String> {
    let mut found = Vec::new();
    for (package, list) in deps {
        for (dep, features) in list {
            let host = NATIVE_LINKERS.iter().find(|(name, _)| name == package);
            if SQLITE_CRATES.contains(&dep.as_str()) && host.is_some() {
                found.push(format!("{package} links {dep} directly"));
            }
            if dep != "filesquid-native" {
                continue;
            }
            match host {
                None => found.push(format!("{package} links filesquid-native")),
                Some((_, allowed)) if features.iter().any(|f| !allowed.contains(&f.as_str())) => {
                    found.push(format!(
                        "{package} enables {features:?} on filesquid-native"
                    ))
                }
                Some(_) => {}
            }
        }
    }
    found
}

/// The native-only patterns in the source `text` of `file`. Pure.
fn source_violations(file: &str, text: &str) -> Vec<String> {
    NATIVE_ONLY_PATTERNS
        .iter()
        .filter(|pattern| text.contains(*pattern))
        .filter(|pattern| !(**pattern == "std::fs::" && FS_ALLOWED.contains(&file)))
        .map(|pattern| format!("{file}: {pattern}"))
        .collect()
}

/// Every native-only pattern in the desktop crate's `src/` (filesystem I/O, one level: the
/// host keeps its modules flat).
fn desktop_source_violations(src: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(src).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&path).expect("source");
        found.extend(source_violations(name, &text));
    }
    found
}

#[test]
fn only_the_hosts_link_filesquid_native_and_the_desktop_has_no_sqlite_driver() {
    let deps = workspace_dependencies();
    let desktop = deps.get("filesquid-desktop").expect("desktop package");
    assert!(
        desktop
            .iter()
            .any(|(name, features)| name == "filesquid-native" && features == &["desktop"]),
        "the desktop links filesquid-native with its desktop feature"
    );
    let found = link_violations(&deps);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_linking_rules_reject_a_sqlite_driver_in_a_host_and_native_in_the_api() {
    let deps = BTreeMap::from([
        (
            "filesquid-desktop".to_string(),
            vec![("rusqlite".to_string(), vec![])],
        ),
        (
            "filesquid-api".to_string(),
            vec![("filesquid-native".to_string(), vec![])],
        ),
        (
            "filesquid-server".to_string(),
            vec![("filesquid-native".to_string(), vec!["desktop".to_string()])],
        ),
    ]);
    assert_eq!(
        link_violations(&deps).len(),
        3,
        "{:?}",
        link_violations(&deps)
    );
}

#[test]
fn the_desktop_host_holds_no_native_storage_or_worker_code() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = desktop_source_violations(&src);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_source_rule_rejects_storage_opens_and_worker_loops() {
    let text = "let c = rusqlite::Connection::open(p);\nstd::thread::scope(|s| {});\n";
    assert_eq!(source_violations("lib.rs", text).len(), 3);
    assert!(source_violations("test_fixture.rs", "std::fs::write(p, b)").is_empty());
    assert_eq!(source_violations("lib.rs", "std::fs::write(p, b)").len(), 1);
}
