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

/// `std` modules whose import in a group (`use std::{fs, ...}`) means I/O.
const IO_MODULES: &[&str] = &["fs", "net", "process", "env"];

/// Most `std::{...}` groups the I/O scan reads in one file (AGENTS.md section 22).
const MAX_GROUPS: usize = 256;

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

/// A package name in one spelling: lower case, `_` read as `-` (Cargo
/// treats both alike).
fn normalize(name: &str) -> String {
    name.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_ascii_lowercase()
        .replace('_', "-")
}

/// Package key of a dependency line: the first dotted segment of the key,
/// unquoted and normalized (`tokio.workspace = true` and `"tokio" = "1"`
/// both name `tokio`).
fn dependency_name(line: &str) -> String {
    let key = line.split('=').next().unwrap_or("");
    normalize(key.split('.').next().unwrap_or(""))
}

/// The package a table header names when the table is one dependency's
/// own table (`[dependencies.tokio]`, `[target.'cfg(unix)'.dependencies.tokio]`),
/// `Some("")` for a dependency list (`[dependencies]`,
/// `[target.'cfg(unix)'.dependencies]`), and `None` for any other table
/// (`[dev-dependencies]` included: tests may use what the library may not).
fn dependency_table(header: &str) -> Option<String> {
    let header = header.split('#').next().unwrap_or("").trim();
    let inner = header.strip_prefix('[')?.strip_suffix(']')?.trim();
    let tail = strip_target(inner)?.strip_prefix("dependencies")?;
    match tail.strip_prefix('.') {
        Some(name) => Some(normalize(name)),
        None => tail.is_empty().then(String::new),
    }
}

/// A table name without its `target.<spec>.` prefix (the spec quoted, as in
/// `target.'cfg(unix)'`, or bare, as in `target.wasm32-unknown-unknown`);
/// other names are returned as they are.
fn strip_target(inner: &str) -> Option<&str> {
    let Some(rest) = inner.strip_prefix("target.") else {
        return Some(inner);
    };
    let end = match rest.chars().next()? {
        quote @ ('\'' | '"') => rest[1..].find(quote)? + 2,
        _ => rest.find('.')?,
    };
    rest[end..].strip_prefix('.')
}

/// Every library dependency of `manifest`, in every dependency table, as
/// its normalized package name and whether it is optional.
fn dependencies(manifest: &str) -> Vec<(String, bool)> {
    let mut found: Vec<(String, bool)> = Vec::new();
    let mut table: Option<String> = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            table = dependency_table(line);
            if let Some(name) = table.as_ref().filter(|name| !name.is_empty()) {
                found.push((name.clone(), false));
            }
            continue;
        }
        let Some(own) = table.as_ref() else { continue };
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let optional = line.replace(' ', "").contains("optional=true");
        let name = match own.is_empty() {
            true => dependency_name(line),
            false => own.clone(),
        };
        match found.iter_mut().find(|(known, _)| *known == name) {
            Some(entry) => entry.1 |= optional,
            None => found.push((name, optional)),
        }
    }
    found
}

/// Whether `list` names the normalized package `name`.
fn names(list: &[&str], name: &str) -> bool {
    list.iter().any(|entry| normalize(entry) == name)
}

/// Every rule the manifest breaks, as readable messages.
fn manifest_violations(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let features = table_lines(manifest, "[features]");
    let default = features
        .iter()
        .find(|line| line.starts_with("default"))
        .map(|line| normalize(line))
        .unwrap_or_default();
    for (name, optional) in dependencies(manifest) {
        if name.starts_with("filesquid") && !optional {
            found.push(format!("{name} is a default dependency"));
        }
        if name.starts_with("filesquid") && default.contains(&name) {
            found.push(format!("{name} is enabled by the default feature"));
        }
        if names(FORBIDDEN, &name) || names(NEVER, &name) {
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
    let deps = dependencies(&cargo_toml());
    let names = |optional: bool| {
        let mut names: Vec<String> = deps
            .iter()
            .filter(|(_, is_optional)| *is_optional == optional)
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        names
    };
    // Names are normalized: `serde_json` reads as `serde-json`.
    assert_eq!(names(false), ["serde", "serde-json", "thiserror"]);
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

#[test]
fn dotted_and_quoted_dependency_keys_would_fail() {
    let manifest = "[dependencies]\ntokio.workspace = true\n\"rusqlite\" = \"0.32\"\naxum . version = \"0.7\"\n";
    assert_eq!(
        manifest_violations(manifest),
        [
            "tokio is forbidden",
            "rusqlite is forbidden",
            "axum is forbidden"
        ]
    );
}

#[test]
fn dependency_tables_of_every_form_are_checked() {
    let manifest = "[dependencies.tokio]\nversion = \"1\"\n\n\
        [target.'cfg(unix)'.dependencies]\nhyper = \"1\"\n\n\
        [target.'cfg(target_os = \"linux\")'.dependencies.reqwest]\nversion = \"0.12\"\n\n\
        [target.wasm32-unknown-unknown.dependencies]\nfilesquid-core = { path = \"../core\" }\n\n\
        [dev-dependencies]\naxum = \"0.7\"\n";
    assert_eq!(
        manifest_violations(manifest),
        [
            "tokio is forbidden",
            "hyper is forbidden",
            "reqwest is forbidden",
            "filesquid-core is a default dependency"
        ]
    );
}

#[test]
fn an_optional_dependency_table_is_read_as_optional() {
    let manifest = "[dependencies.filesquid-ocfl]\npath = \"../ocfl\"\noptional = true\n";
    assert_eq!(
        dependencies(manifest),
        [("filesquid-ocfl".to_string(), true)]
    );
    assert!(manifest_violations(manifest).is_empty());
}

#[test]
fn underscore_spellings_of_never_and_forbidden_packages_would_fail() {
    let manifest = "[dependencies]\nfilesquid_native = { path = \"../native\", optional = true }\nffmpeg_next = \"7\"\n";
    assert_eq!(
        manifest_violations(manifest),
        ["filesquid-native is forbidden", "ffmpeg-next is forbidden"]
    );
}

/// Every I/O marker that `text` names, including the members of grouped
/// `std::{...}` imports (`use std::{fs, net};`).
fn io_violations(text: &str) -> Vec<String> {
    let mut found: Vec<String> = IO_MARKERS
        .iter()
        .filter(|marker| text.contains(**marker))
        .map(|marker| marker.to_string())
        .collect();
    for group in std_groups(text) {
        for member in top_level_members(group) {
            let head = member.split("::").next().unwrap_or("").trim();
            if IO_MODULES.contains(&head) {
                found.push(format!("std::{{{head}}}"));
            }
        }
    }
    found
}

/// The text inside each `std::{ ... }` group of `text`, up to its matching
/// brace (at most [`MAX_GROUPS`] groups).
fn std_groups(text: &str) -> Vec<&str> {
    let mut groups = Vec::new();
    for (start, _) in text.match_indices("std::{").take(MAX_GROUPS) {
        let body = &text[start + "std::{".len()..];
        let mut depth = 1usize;
        let end = body.char_indices().find_map(|(i, c)| {
            depth = match c {
                '{' => depth + 1,
                '}' => depth - 1,
                _ => depth,
            };
            (depth == 0).then_some(i)
        });
        groups.push(&body[..end.unwrap_or(body.len())]);
    }
    groups
}

/// The comma-separated members of a group at brace depth zero.
fn top_level_members(group: &str) -> Vec<&str> {
    let mut members = Vec::new();
    let (mut depth, mut from) = (0usize, 0usize);
    for (i, c) in group.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                members.push(&group[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    members.push(&group[from..]);
    members
}

#[test]
fn grouped_std_imports_of_io_modules_would_fail() {
    for text in [
        "use std::{fs, net};",
        "use std::{collections::BTreeMap, process::Command};",
        "use std::{\n    env,\n    fmt,\n};",
        "use std::{io::{Read, Write}, net::TcpStream};",
    ] {
        assert!(!io_violations(text).is_empty(), "{text}");
    }
    for text in [
        "use std::{collections::BTreeMap, fmt};",
        "use std::{cmp::Ordering, ops::Bound};",
    ] {
        assert!(io_violations(text).is_empty(), "{text}");
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
        let found = io_violations(&text);
        assert!(found.is_empty(), "{found:?} in {}", file.display());
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
