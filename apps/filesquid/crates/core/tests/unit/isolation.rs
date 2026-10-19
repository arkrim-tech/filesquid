//! Unit tests that the portable-contracts crate stays free of host and codec I/O.

use std::fs;
use std::path::PathBuf;

fn cargo_toml() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(path).expect("Cargo.toml")
}

fn quoted_package_rename(line: &str) -> Option<String> {
    let idx = line.find("package")?;
    let after = line[idx + "package".len()..].trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let inner = after.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn library_dependency_package_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if let Some(rest) = trimmed.strip_prefix("[dependencies.") {
                if let Some(name) = rest.strip_suffix(']') {
                    names.push(name.to_string());
                }
                in_deps = false;
                continue;
            }
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = trimmed
            .split([' ', '='])
            .next()
            .filter(|name| !name.is_empty());
        let Some(key) = key else {
            continue;
        };
        names.push(quoted_package_rename(trimmed).unwrap_or_else(|| key.to_string()));
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
    "filesquid-ocfl",
    "filesquid_ocfl",
    "imagesquid-ocfl",
    "imagesquid_ocfl",
    "filesquid-metadata",
    "filesquid_metadata",
    "filesquid-prov",
    "filesquid_prov",
];

#[test]
fn library_dependencies_exclude_hosts_codecs_and_ocfl_io() {
    let names = library_dependency_package_names(&cargo_toml());
    assert_eq!(
        names,
        vec!["serde".to_string()],
        "portable contracts [dependencies] must be serde only"
    );
    for forbidden in FORBIDDEN {
        assert!(
            !names.iter().any(|name| name == forbidden),
            "{forbidden} must be absent from [dependencies]"
        );
    }
}

#[test]
fn crate_root_denies_missing_docs() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lib = fs::read_to_string(path).expect("lib.rs");
    assert!(
        lib.contains("#![deny(missing_docs)]"),
        "portable contracts must deny missing rustdoc"
    );
}
