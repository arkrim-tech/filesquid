//! C2PA-S-001: every manifest store in the C2PA public test files parses.
//!
//! The JPEG files' APP11 segments are reassembled by test-only support code
//! (the library's JPEG handler is CP-0.3) and each store is parsed. Every
//! store and every manifest in it must parse completely, including those in
//! files whose signatures or hashes are deliberately broken (those faults
//! belong to validation, not to parsing). Skipped, with the reason printed,
//! when `FILESQUID_C2PA_CONFORMANCE_DIR` is not set (see the crate README).

#[path = "../support/jpeg_app11.rs"]
mod jpeg_app11;
#[path = "../support/oracle.rs"]
#[allow(dead_code)]
mod oracle;

use std::fs;
use std::path::{Path, PathBuf};

use filesquid_c2pa::manifest::{ManifestStore, ParseLimits};

/// Outcome counts of one corpus run.
#[derive(Debug, Default)]
struct Tally {
    /// Files with an embedded store.
    stores: usize,
    /// Files without one.
    no_store: usize,
    /// Manifests parsed completely.
    manifests_ok: usize,
    /// Manifests with a typed manifest error, as `file: error`.
    manifest_errors: Vec<String>,
    /// Stores whose framing failed, as `file: error`.
    store_errors: Vec<String>,
}

/// The corpus directory, or why the test is skipped.
fn corpus_dir() -> Result<PathBuf, String> {
    let dir = std::env::var_os(oracle::CORPUS_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{} is not set", oracle::CORPUS_ENV))?;
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    Ok(dir)
}

/// Parse every store in the JPEG at `path` into `tally`.
fn parse_file(path: &Path, tally: &mut Tally) {
    let bytes = fs::read(path).expect("asset bytes");
    let stores = jpeg_app11::jpeg_jumbf(&bytes).unwrap_or_default();
    if stores.is_empty() {
        tally.no_store += 1;
        return;
    }
    for jumbf in stores {
        tally.stores += 1;
        match ManifestStore::from_bytes(jumbf, &ParseLimits::default()) {
            Err(err) => tally
                .store_errors
                .push(format!("{}: {err}", path.display())),
            Ok(store) => record_manifests(path, &store, tally),
        }
    }
}

/// Count each manifest's outcome.
fn record_manifests(path: &Path, store: &ManifestStore, tally: &mut Tally) {
    for entry in store.manifests() {
        match &entry.content {
            Ok(_) => tally.manifests_ok += 1,
            Err(err) => tally
                .manifest_errors
                .push(format!("{}: {err}", path.display())),
        }
    }
}

#[test]
fn every_conformance_manifest_store_parses() {
    let dir = match corpus_dir() {
        Ok(dir) => dir,
        Err(reason) => {
            eprintln!("SKIPPED conformance parse: {reason}");
            return;
        }
    };
    let files = oracle::corpus_files(&dir, oracle::MAX_CORPUS_FILES).expect("corpus walk");
    let mut tally = Tally::default();
    for file in files.iter().filter(|f| has_extension(f, &["jpg", "jpeg"])) {
        parse_file(file, &mut tally);
    }
    eprintln!(
        "conformance parse: {} stores, {} files without a store, {} manifests parsed, {} manifest errors, {} store errors",
        tally.stores,
        tally.no_store,
        tally.manifests_ok,
        tally.manifest_errors.len(),
        tally.store_errors.len()
    );
    assert!(tally.stores > 0, "no manifest stores found");
    assert!(tally.store_errors.is_empty(), "{:#?}", tally.store_errors);
    assert!(
        tally.manifest_errors.is_empty(),
        "{:#?}",
        tally.manifest_errors
    );
}

/// Whether `path` has one of `extensions` (case-insensitive).
fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}
