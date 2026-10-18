//! Isolation: filesquid-ocfl is an optional dependency behind the `ocfl` store feature (MD-S-003).

use std::fs;
use std::path::PathBuf;

fn cargo_toml() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(path).expect("Cargo.toml")
}

fn library_dependency_lines(manifest: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
            lines.push(trimmed.to_string());
        }
    }
    lines
}

/// Return the `[features]` table body of the manifest.
fn feature_lines(manifest: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_features = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_features = trimmed == "[features]";
            continue;
        }
        if in_features && !trimmed.is_empty() && !trimmed.starts_with('#') {
            lines.push(trimmed.to_string());
        }
    }
    lines
}

fn library_dependency_package_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = trimmed.split([' ', '=']).next().unwrap_or("");
        if !key.is_empty() {
            names.push(key.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

const FORBIDDEN: &[&str] = &[
    "axum",
    "tauri",
    "libvips",
    "ffmpeg-sys",
    "ffmpeg",
    "filesquid-core",
    "filesquid_core",
    "imagesquid_core",
    "rusqlite",
    "sqlx",
];

#[test]
fn filesquid_ocfl_is_an_optional_dependency() {
    let lines = library_dependency_lines(&cargo_toml());
    let ocfl = lines
        .iter()
        .find(|line| line.starts_with("filesquid-ocfl"))
        .expect("filesquid-ocfl stays available behind the `ocfl` feature");
    assert!(ocfl.contains("optional = true"), "{ocfl}");
    assert!(ocfl.contains("default-features = false"), "{ocfl}");
}

#[test]
fn default_features_exclude_filesquid_ocfl() {
    let features = feature_lines(&cargo_toml());
    let default = features
        .iter()
        .find(|line| line.starts_with("default"))
        .expect("default features");
    assert!(default.contains("\"sidecar\""), "{default}");
    for excluded in ["ocfl", "ocfl-fs", "cli"] {
        assert!(
            !default.contains(&format!("\"{excluded}\"")),
            "{excluded} must not be a default feature: {default}"
        );
    }
}

#[test]
fn the_ocfl_feature_enables_the_optional_dependency() {
    let features = feature_lines(&cargo_toml());
    assert!(features
        .iter()
        .any(|line| line.starts_with("ocfl =") && line.contains("dep:filesquid-ocfl")));
}

#[test]
fn excludes_hosts_codecs_and_filesquid_core() {
    let names = library_dependency_package_names(&cargo_toml());
    for forbidden in FORBIDDEN {
        assert!(
            !names.iter().any(|name| name == *forbidden),
            "{forbidden} must be absent from [dependencies]"
        );
    }
}
