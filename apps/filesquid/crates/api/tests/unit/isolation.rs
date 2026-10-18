//! API-S-006 / Task CL-0.1: `filesquid-api` is the application layer and links no Tauri,
//! libvips, FFmpeg, SQLite driver, storage crate, network client or alternative async
//! runtime, and its `src/` performs no direct storage I/O. The rules are also tested on
//! synthetic names and sources, so a forbidden dependency or call is proven to fail.

#[path = "../support/isolation.rs"]
#[allow(dead_code)]
mod isolation;

#[path = "../support/source_scan.rs"]
mod source_scan;

use isolation::{
    forbidden_reason, package_names, run_cargo_tree, sample_name, violations, ALLOWED_MODULES,
    FEATURE_SETS, RULES, SELF_NAME,
};
use source_scan::{io_findings_outside_allowed, scan_text, IO_ALLOWED, IO_PATTERNS};

/// The patterns `scan_text` finds in `text`.
fn patterns_in(text: &str) -> Vec<&'static str> {
    scan_text("sample.rs", text, IO_PATTERNS)
        .into_iter()
        .map(|f| f.pattern)
        .collect()
}

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
        "filesquid-native",
        "reqwest",
        "ureq",
        "curl-sys",
        "async-std",
        "smol",
        "async-executor",
        "async-global-executor",
        "async-io",
        "async-net",
        "actix-rt",
        "glommio",
        "monoio",
        "compio",
        "compio-runtime",
        "tokio-uring",
    ] {
        assert!(forbidden_reason(name).is_some(), "{name} must be forbidden");
    }
}

#[test]
fn rules_allow_the_modules_the_router_and_lookalikes() {
    let others = [
        "filesquid-api",
        "axum",
        "tokio",
        "serde",
        "sha2",
        "surface",
        "smol_str",
    ];
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
fn every_feature_set_graph_is_isolated() {
    for (label, args) in FEATURE_SETS {
        let names = run_cargo_tree(args).expect("cargo tree");
        assert!(
            names.iter().any(|n| n == SELF_NAME),
            "{label}: cargo tree did not list {SELF_NAME}: {names:?}"
        );
        let found = violations(names.iter().map(String::as_str));
        assert!(
            found.is_empty(),
            "{label}: forbidden runtime dependencies: {found:?}"
        );
    }
}

#[test]
fn storage_io_is_detected_with_its_line() {
    let cases: [(&str, &str); 9] = [
        ("    std::fs::create_dir_all(dir)?;", "std::fs"),
        ("use std::fs;", "std::fs"),
        ("use std::{fs, io};", "std::fs"),
        ("use std::{io, fs::File};", "std::fs"),
        ("let f = tokio::fs::read(p).await;", "tokio::fs"),
        ("fs4::available_space(dir)", "fs4"),
        (
            "use std::os::unix::fs::PermissionsExt;",
            "std::os::unix::fs",
        ),
        (
            "use std::os::windows::fs::OpenOptionsExt;",
            "std::os::windows::fs",
        ),
        ("smol::block_on(f)", "smol"),
    ];
    for (line, pattern) in cases {
        assert!(patterns_in(line).contains(&pattern), "{line}");
    }
}

#[test]
fn multi_line_grouped_imports_are_detected() {
    let text = "use std::{\n    collections::HashMap,\n    fs,\n};\n";
    let found = scan_text("sample.rs", text, IO_PATTERNS);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].line, found[0].pattern), (1, "std::fs"));
}

#[test]
fn comments_and_string_literals_are_not_storage_io() {
    for text in [
        "//! The crate MUST NOT call std::fs directly.",
        "let x = 1; // std::fs is forbidden here",
        "/* std::fs\n   tokio::fs */ let y = 2;",
        "let msg = \"do not use std::fs\";",
        "use std::{io, sync::Arc};",
        "let offsets = fs_like::read(); let s = smol_str::SmolStr::new(\"a\");",
    ] {
        assert!(patterns_in(text).is_empty(), "{text}");
    }
}

#[test]
fn a_url_string_does_not_hide_io_after_it() {
    let text = "let url = \"https://example.org\"; std::fs::write(p, b)?;";
    assert_eq!(patterns_in(text), ["std::fs"]);
}

#[test]
fn no_source_file_is_exempt_and_none_performs_storage_io() {
    assert!(IO_ALLOWED.is_empty());
    let (findings, scanned) = io_findings_outside_allowed().expect("scan src");
    assert!(scanned > 10, "only {scanned} files scanned");
    assert!(
        findings.is_empty(),
        "direct storage I/O in src/: {findings:?}"
    );
}
