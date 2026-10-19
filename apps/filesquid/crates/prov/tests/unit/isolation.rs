//! PROV-S-001: `filesquid-prov` stands alone. Its default dependencies name no
//! other FileSquid crate, no feature adds `filesquid-native` or
//! `filesquid-c2pa`, no host, SQLite or media crate is linked, and the
//! library does no filesystem, network or process I/O. Each check is also
//! run on a synthetic manifest, so a violation is proven to fail.

use std::fs;
use std::path::{Path, PathBuf};

/// Packages no feature of this crate may ever add.
const NEVER: &[&str] = &["filesquid-native", "filesquid-c2pa"];

/// Hosts, SQLite and media crates the library MUST NOT depend on.
const FORBIDDEN: &[&str] = &[
    "axum",
    "tauri",
    "rusqlite",
    "sqlx",
    "libsqlite3-sys",
    "libvips",
    "ffmpeg",
    "ffmpeg-sys",
    "ffmpeg-next",
    "tokio",
    "reqwest",
    "hyper",
];

/// Source fragments that would mean filesystem, network or process I/O.
const IO_MARKERS: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "File::open",
    "File::create",
];

/// Most source files the I/O scan reads (AGENTS.md section 22).
const MAX_SOURCE_FILES: usize = 512;

/// The crate's own manifest text.
fn cargo_toml() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(path).expect("Cargo.toml")
}

/// Non-comment lines of the manifest table named `table` (for example `[features]`).
fn table_lines(manifest: &str, table: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == table;
            continue;
        }
        if inside && !trimmed.is_empty() && !trimmed.starts_with('#') {
            lines.push(trimmed.to_string());
        }
    }
    lines
}

/// Package key of a dependency line (`name = ...`).
fn dependency_name(line: &str) -> String {
    line.split([' ', '=']).next().unwrap_or("").to_string()
}

/// Every rule the manifest breaks, as readable messages.
fn manifest_violations(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let deps = table_lines(manifest, "[dependencies]");
    let features = table_lines(manifest, "[features]");
    let default = features
        .iter()
        .find(|line| line.starts_with("default"))
        .cloned()
        .unwrap_or_default();
    for line in &deps {
        let name = dependency_name(line);
        let optional = line.contains("optional = true");
        if name.starts_with("filesquid") && !optional {
            found.push(format!("{name} is a default dependency"));
        }
        if name.starts_with("filesquid") && default.contains(&name) {
            found.push(format!("{name} is enabled by the default feature"));
        }
        if FORBIDDEN.contains(&name.as_str()) || NEVER.contains(&name.as_str()) {
            found.push(format!("{name} is forbidden"));
        }
    }
    for line in &features {
        for never in NEVER {
            if line.contains(never) || line.contains(&never.replace('-', "_")) {
                found.push(format!("a feature names {never}"));
            }
        }
    }
    found
}

#[test]
fn the_manifest_keeps_every_isolation_rule() {
    let found = manifest_violations(&cargo_toml());
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn library_dependencies_are_serde_serde_json_and_thiserror() {
    let lines = table_lines(&cargo_toml(), "[dependencies]");
    let names = |optional: bool| {
        let mut names: Vec<String> = lines
            .iter()
            .filter(|line| line.contains("optional = true") == optional)
            .map(|line| dependency_name(line))
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(false), ["serde", "serde_json", "thiserror"]);
    assert_eq!(
        names(true),
        ["filesquid-ocfl"],
        "the ocfl feature's engine is the only optional dependency (PV-0.2)"
    );
}

#[test]
fn a_default_filesquid_dependency_would_fail() {
    let manifest = "[dependencies]\nfilesquid-core = { path = \"../core\" }\n";
    assert_eq!(
        manifest_violations(manifest),
        ["filesquid-core is a default dependency"]
    );
}

#[test]
fn an_optional_filesquid_dependency_in_default_would_fail() {
    let manifest = "[features]\ndefault = [\"filesquid-ocfl\"]\n\n[dependencies]\nfilesquid-ocfl = { path = \"../ocfl\", optional = true }\n";
    assert_eq!(
        manifest_violations(manifest),
        ["filesquid-ocfl is enabled by the default feature"]
    );
}

#[test]
fn a_feature_naming_native_or_c2pa_would_fail() {
    let manifest = "[features]\nextra = [\"dep:filesquid-native\"]\nsign = [\"filesquid_c2pa\"]\n";
    assert_eq!(
        manifest_violations(manifest),
        [
            "a feature names filesquid-native",
            "a feature names filesquid-c2pa"
        ]
    );
}

#[test]
fn hosts_sqlite_and_media_crates_would_fail() {
    for name in FORBIDDEN {
        let manifest = format!("[dependencies]\n{name} = \"1\"\n");
        assert_eq!(
            manifest_violations(&manifest),
            [format!("{name} is forbidden")]
        );
    }
}

/// Every `.rs` file under `dir`, depth-first, at most [`MAX_SOURCE_FILES`].
fn source_files(dir: &Path) -> Vec<PathBuf> {
    let mut pending = vec![dir.to_path_buf()];
    let mut files = Vec::new();
    while let Some(next) = pending.pop() {
        assert!(
            files.len() + pending.len() < MAX_SOURCE_FILES,
            "too many files"
        );
        for entry in fs::read_dir(&next).expect("read src") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn the_library_does_no_filesystem_network_or_process_io() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = source_files(&src);
    assert!(!files.is_empty());
    for file in files {
        let text = fs::read_to_string(&file).expect("source");
        for marker in IO_MARKERS {
            assert!(!text.contains(marker), "{marker} in {}", file.display());
        }
    }
}

#[test]
fn the_crate_root_denies_missing_docs() {
    let lib = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let text = fs::read_to_string(lib).expect("lib.rs");
    assert!(text.contains("#![deny(missing_docs)]"));
}

#[test]
fn the_crate_is_mit_licensed_and_unpublished_for_now() {
    let manifest = cargo_toml();
    assert!(manifest.contains("name = \"filesquid-prov\""));
    assert!(manifest.contains("license = \"MIT\""));
    assert!(manifest.contains("publish = false"));
    assert!(
        !manifest.contains("path = \"../../"),
        "no path outside apps/filesquid"
    );
}
