//! Source scan for `filesquid-c2pa` (C2PA-S-006, C2PA-S-008).
//!
//! The core MUST NOT touch the filesystem, the network or processes directly,
//! and only the crypto backend module may name a cryptography crate. Comment
//! lines are skipped so documentation can describe the rules.

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns that mean direct I/O, a network client or an async runtime.
pub const IO_PATTERNS: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "tokio",
    "async_std",
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "File::open",
    "File::create",
];

/// Patterns that name a cryptography crate.
pub const CRYPTO_PATTERNS: &[&str] = &[
    "sha2::",
    "p256::",
    "p384::",
    "p521::",
    "ecdsa::",
    "ed25519_dalek",
    "rsa::",
    "x509_cert",
    "cms::",
    "ring::",
    "openssl",
    "aws_lc",
];

/// The one module allowed to name cryptography crates, relative to `src/`.
pub const CRYPTO_BACKEND_MODULE: &str = "crypto/rust_crypto.rs";

/// Directory depth cap for the `src/` walk.
const MAX_DEPTH: usize = 8;

/// File-count cap for the `src/` walk.
const MAX_FILES: usize = 512;

/// One forbidden pattern found in a source file.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to `src/`, with `/` separators.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// The pattern that matched.
    pub pattern: &'static str,
}

/// Return the patterns from `patterns` found in non-comment lines of `text`.
pub fn scan_text(file: &str, text: &str, patterns: &[&'static str]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        for pattern in patterns.iter().filter(|p| code.contains(**p)) {
            findings.push(Finding {
                file: file.to_string(),
                line: index + 1,
                pattern,
            });
        }
    }
    findings
}

/// The crate's `src/` directory.
pub fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Read every `.rs` file under `src/` as (relative path, text), bounded by
/// [`MAX_DEPTH`] and [`MAX_FILES`].
pub fn read_sources() -> Result<Vec<(String, String)>, String> {
    let root = src_dir();
    let mut files = Vec::new();
    collect(&root, &root, 0, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{} is deeper than {MAX_DEPTH}", dir.display()));
    }
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|err| err.to_string())?.path();
        if path.is_dir() {
            collect(root, &path, depth + 1, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            push_source(root, &path, out)?;
        }
    }
    Ok(())
}

fn push_source(root: &Path, path: &Path, out: &mut Vec<(String, String)>) -> Result<(), String> {
    if out.len() >= MAX_FILES {
        return Err(format!("more than {MAX_FILES} source files"));
    }
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let relative = path
        .strip_prefix(root)
        .map_err(|err| err.to_string())?
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    out.push((relative, text));
    Ok(())
}

/// Every direct I/O or network finding in `src/`.
pub fn io_findings() -> Result<Vec<Finding>, String> {
    Ok(read_sources()?
        .iter()
        .flat_map(|(file, text)| scan_text(file, text, IO_PATTERNS))
        .collect())
}

/// Every cryptography-crate finding in `src/` outside the backend module.
pub fn crypto_findings_outside_backend() -> Result<Vec<Finding>, String> {
    Ok(read_sources()?
        .iter()
        .filter(|(file, _)| file != CRYPTO_BACKEND_MODULE)
        .flat_map(|(file, text)| scan_text(file, text, CRYPTO_PATTERNS))
        .collect())
}
