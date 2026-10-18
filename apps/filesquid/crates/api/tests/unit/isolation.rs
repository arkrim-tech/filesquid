//! API-S-006 / Task CL-0.1: `filesquid-api` is the application layer and links no Tauri,
//! libvips, FFmpeg, SQLite driver or network client, and its `src/` performs no direct
//! storage I/O. The rule table is also tested on synthetic names and lines, so a forbidden
//! dependency or call is proven to fail the guard.

#[path = "../support/isolation.rs"]
#[allow(dead_code)]
mod isolation;

use isolation::{
    forbidden_reason, line_uses_storage_io, package_names, pinned_graph_deviations,
    read_sources_with_storage_io, run_cargo_tree, sample_name, violations, ALLOWED_MODULES,
    FEATURE_SETS, PINNED_SOURCE_DEVIATIONS, RULES, SELF_NAME,
};

#[test]
fn crate_is_named_filesquid_api_in_the_workspace() {
    assert_eq!(env!("CARGO_PKG_NAME"), SELF_NAME);
    let dir = isolation::crate_dir();
    assert!(dir.ends_with("crates/api"), "{}", dir.display());
    let workspace = std::fs::read_to_string(isolation::workspace_manifest()).expect("workspace");
    assert!(workspace.contains("\"crates/api\""), "workspace member");
}

#[test]
fn module_srs_states_the_application_layer_boundary() {
    let srs = std::fs::read_to_string(isolation::crate_dir().join("resources/srs.md"))
        .expect("module srs");
    let row = srs
        .lines()
        .find(|line| line.contains("API-S-006"))
        .expect("API-S-006 row");
    for phrase in [
        "MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver",
        "MUST NOT perform storage I/O itself",
        "Cargo features MUST NOT be the only boundary",
    ] {
        assert!(row.contains(phrase), "API-S-006 must say: {phrase}");
    }
}

#[test]
fn every_rule_rejects_a_matching_name() {
    for rule in RULES {
        let name = sample_name(rule);
        assert_eq!(forbidden_reason(&name), Some(rule.reason), "{name}");
    }
}

#[test]
fn rules_reject_every_named_crate() {
    for name in [
        "tauri",
        "tauri-runtime-wry",
        "libvips",
        "ffmpeg-sys",
        "ffmpeg-sys-next",
        "ffmpeg-next",
        "rusqlite",
        "libsqlite3-sys",
        "sqlx",
        "sqlx-sqlite",
        "fs4",
        "reqwest",
        "ureq",
        "curl-sys",
    ] {
        assert!(forbidden_reason(name).is_some(), "{name} must be forbidden");
    }
}

#[test]
fn rules_allow_the_modules_and_the_router() {
    let others = ["filesquid-api", "axum", "tokio", "serde", "sha2", "surface"];
    for name in ALLOWED_MODULES.iter().copied().chain(others) {
        assert_eq!(forbidden_reason(name), None, "{name} must be allowed");
    }
}

#[test]
fn a_forbidden_dependency_would_fail_the_graph_guard() {
    let found = violations(["axum", "tauri", "rusqlite", "filesquid-ocfl", "rusqlite"]);
    assert_eq!(found, ["rusqlite", "tauri"]);
}

#[test]
fn package_names_parse_cargo_tree_lines() {
    let tree = "filesquid-api v0.1.0 (/w/crates/api)\naxum v0.8.4\naxum v0.8.4\n\n";
    assert_eq!(package_names(tree), ["axum", "filesquid-api"]);
}

#[test]
fn storage_io_lines_are_detected() {
    for line in [
        "    std::fs::create_dir_all(dir)?;",
        "use std::fs;",
        "use std::{fs, io};",
        "use std::{io, fs::File};",
        "let f = tokio::fs::read(p).await;",
        "fs4::available_space(dir)",
        "use std::os::unix::fs::PermissionsExt;",
        "use std::os::windows::fs::OpenOptionsExt;",
        "::std :: fs::write(p, b)",
    ] {
        assert!(line_uses_storage_io(line), "{line}");
    }
}

#[test]
fn comments_and_lookalikes_are_not_storage_io() {
    for line in [
        "//! The crate MUST NOT call std::fs directly.",
        "let x = 1; // std::fs is forbidden here",
        "use std::{io, sync::Arc};",
        "let offsets = fs_like::read();",
        "use std::io::Write;",
    ] {
        assert!(!line_uses_storage_io(line), "{line}");
    }
}

/// Fail when `cargo tree` produced an empty or unrelated graph, so the guard cannot pass
/// vacuously.
fn assert_graph_holds_this_crate(label: &str, names: &[String]) {
    assert!(
        names.iter().any(|n| n == SELF_NAME),
        "{label}: cargo tree did not list {SELF_NAME}: {names:?}"
    );
}

#[test]
fn every_feature_set_links_only_the_pinned_deviations() {
    for (label, args) in FEATURE_SETS {
        let names = run_cargo_tree(args).expect("cargo tree");
        assert_graph_holds_this_crate(label, &names);
        let found = violations(names.iter().map(String::as_str));
        assert_eq!(
            found,
            pinned_graph_deviations(label),
            "{label}: forbidden runtime dependencies differ from the pinned deviations \
             (see resources/tasks/Task-CL-0.1-Conflict.md)"
        );
    }
}

#[test]
fn only_the_pinned_files_perform_storage_io() {
    let found = read_sources_with_storage_io().expect("scan src");
    assert_eq!(
        found, PINNED_SOURCE_DEVIATIONS,
        "files under src/ with direct storage I/O differ from the pinned deviations \
         (see resources/tasks/Task-CL-0.1-Conflict.md)"
    );
}
