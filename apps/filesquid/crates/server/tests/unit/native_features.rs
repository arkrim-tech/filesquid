//! `filesquid-server` links `filesquid-native` with its default features only: the SQLite
//! journal store, the data-directory checks and `HostMonitor`, never the desktop-only native
//! processing behind the `desktop` feature ([ADR 033] amendment 2026-10-18, SRS-F-061).
//!
//! The check resolves the server's own feature graph with `cargo tree -p filesquid-server -e
//! features`, which is what `cargo build -p filesquid-server` compiles. (A whole-workspace
//! build unifies features with `filesquid-desktop`; server release builds use `-p`.)
//!
//! [ADR 033]: ../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md

use std::path::PathBuf;
use std::process::Command;

/// The `cargo tree -e features` node a desktop-feature dependency would add.
const DESKTOP_FEATURE_NODE: &str = "filesquid-native feature \"desktop\"";

/// The node that proves the server links `filesquid-native` at all.
const NATIVE_FEATURE_NODE: &str = "filesquid-native feature \"default\"";

/// Run `cargo tree` for the server's feature graph (I/O: spawns the `cargo` running the tests)
/// and return its lines.
fn run_server_feature_tree() -> Vec<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(cargo)
        .arg("tree")
        .arg("--manifest-path")
        .arg(manifest)
        .args([
            "-p",
            "filesquid-server",
            "-e",
            "features",
            "--target",
            "all",
        ])
        .args(["--prefix", "none", "--locked"])
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The lines of a feature tree that enable `filesquid-native`'s `desktop` feature.
fn desktop_feature_lines(lines: &[String]) -> Vec<&String> {
    lines
        .iter()
        .filter(|line| line.contains(DESKTOP_FEATURE_NODE))
        .collect()
}

#[test]
fn the_rule_rejects_a_tree_with_the_desktop_feature() {
    let tree = [
        "filesquid-native feature \"default\"".to_string(),
        "filesquid-native feature \"desktop\"".to_string(),
    ];
    assert_eq!(desktop_feature_lines(&tree).len(), 1);
}

#[test]
fn the_server_links_filesquid_native_with_default_features_only() {
    let lines = run_server_feature_tree();
    assert!(
        lines.iter().any(|line| line.contains(NATIVE_FEATURE_NODE)),
        "filesquid-server must link filesquid-native"
    );
    let found = desktop_feature_lines(&lines);
    assert!(
        found.is_empty(),
        "filesquid-server enables filesquid-native's desktop feature: {found:?}"
    );
}
