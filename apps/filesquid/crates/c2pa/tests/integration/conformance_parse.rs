//! C2PA-S-001, C2PA-S-002 (hard bindings), C2PA-S-005: the C2PA public test
//! files end to end.
//!
//! Every JPEG is read through the library's format handler. The one MP4
//! has its store located by test-only code until the BMFF handler exists
//! (CP-2.2). Every store and every manifest must parse completely. Each
//! active manifest's hard binding is then validated and compared with the
//! hash verdict the corpus records in `manifests/<name>/detailed.json`
//! (`*Hash.match` or `*Hash.mismatch`). Skipped, with the reason printed,
//! when `FILESQUID_C2PA_CONFORMANCE_DIR` is not set (see the crate README).
//!
//! The corpus verdicts come from a validator that does not apply a C2PA 2.4
//! rule (section 15.12.1): the exclusion holding the manifest store may
//! hold only the store. The files in [`STRICTER_THAN_CORPUS`] exclude more
//! (from the start of the file, so SOI and the APP1 Exif segment are
//! unhashed), and this engine must report them as a mismatch for that
//! exact reason.

#[path = "../support/bmff_store.rs"]
mod bmff_store;
#[path = "../support/oracle.rs"]
#[allow(dead_code)]
mod oracle;

use std::fs;
use std::path::{Path, PathBuf};

use filesquid_c2pa::binding::{validate_hard_binding, BindingReport, BindingStatus};
use filesquid_c2pa::crypto::RustCryptoBackend;
use filesquid_c2pa::format::AssetLayout;
use filesquid_c2pa::io::MemorySource;
use filesquid_c2pa::manifest::{ManifestStore, ParseLimits};
use filesquid_c2pa::read::{read_asset, AssetManifest};
use futures::executor::block_on;

/// Files whose recorded `match` C2PA 2.4 turns into a mismatch, with the
/// reason this engine must give.
const STRICTER_THAN_CORPUS: [(&str, &str); 3] = [
    ("truepic-20230212-camera.jpg", STORE_EXCLUSION_TOO_WIDE),
    ("truepic-20230212-landscape.jpg", STORE_EXCLUSION_TOO_WIDE),
    ("truepic-20230212-library.jpg", STORE_EXCLUSION_TOO_WIDE),
];

/// The engine's reason for an over-wide store exclusion.
const STORE_EXCLUSION_TOO_WIDE: &str = "the store's exclusion covers other bytes";

/// Outcome counts of one corpus run.
#[derive(Debug, Default)]
struct Tally {
    /// Files with an embedded store.
    stores: usize,
    /// Files without one.
    no_store: usize,
    /// Manifests parsed completely.
    manifests_ok: usize,
    /// Hard bindings that matched the corpus verdict.
    bindings_agree: usize,
    /// Hard bindings that differ from the corpus as C2PA 2.4 requires.
    bindings_stricter: usize,
    /// Hard bindings with no corpus verdict to compare.
    bindings_unrecorded: usize,
    /// Failures, as `file: reason`.
    failures: Vec<String>,
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

/// The store and layout of `bytes`, through the format handler for JPEG
/// and the test-only locator for MP4.
fn locate(
    path: &Path,
    bytes: &[u8],
) -> Result<Option<(ManifestStore, Option<AssetLayout>)>, String> {
    if has_extension(path, &["mp4"]) {
        let Some(store) = bmff_store::mp4_manifest_store(bytes)? else {
            return Ok(None);
        };
        let store =
            ManifestStore::from_bytes(store, &ParseLimits::default()).map_err(|e| e.to_string())?;
        return Ok(Some((store, None)));
    }
    let read = block_on(read_asset(
        &mut MemorySource::new(bytes.to_vec()),
        &ParseLimits::default(),
    ));
    match read.map_err(|e| e.to_string())? {
        AssetManifest::Embedded { store, layout } => Ok(Some((*store, Some(layout)))),
        AssetManifest::NotEmbedded { .. } => Ok(None),
        other => Err(format!("unexpected read result {other:?}")),
    }
}

/// The corpus's recorded hash verdict for the asset at `path`: `Some(true)`
/// for a match, `Some(false)` for a mismatch, `None` when not recorded.
fn recorded_verdict(path: &Path) -> Option<bool> {
    let stem = path.file_stem()?.to_str()?;
    let report = path
        .parent()?
        .join("manifests")
        .join(stem)
        .join("detailed.json");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(report).ok()?).ok()?;
    let codes = json["validation_status"].as_array()?;
    let hash_codes: Vec<&str> = codes
        .iter()
        .filter_map(|c| c["code"].as_str())
        .filter(|c| c.contains("Hash.match") || c.contains("Hash.mismatch"))
        .collect();
    if hash_codes.is_empty() {
        return None;
    }
    Some(hash_codes.iter().all(|c| c.ends_with(".match")))
}

/// Whether `report` is a match (`Some(true)`), a mismatch (`Some(false)`)
/// or neither (`None`).
fn engine_verdict(report: &BindingReport) -> Option<bool> {
    match &report.status {
        BindingStatus::Valid { .. } => Some(true),
        BindingStatus::Invalid { code, .. } if code.ends_with("Hash.mismatch") => Some(false),
        _ => None,
    }
}

/// Locate, parse and binding-check one asset into `tally`.
fn check_file(path: &Path, tally: &mut Tally) {
    let bytes = fs::read(path).expect("asset bytes");
    let located = match locate(path, &bytes) {
        Ok(Some(located)) => located,
        Ok(None) => {
            tally.no_store += 1;
            return;
        }
        Err(err) => return tally.failures.push(format!("{}: {err}", path.display())),
    };
    let (store, layout) = located;
    tally.stores += 1;
    for entry in store.manifests() {
        match &entry.content {
            Ok(_) => tally.manifests_ok += 1,
            Err(err) => tally
                .failures
                .push(format!("{}: manifest {err}", path.display())),
        }
    }
    let report = block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(bytes),
        layout.as_ref(),
        &RustCryptoBackend,
    ))
    .expect("asset readable");
    eprintln!("  {}: {:?}", path.display(), report.status);
    compare(path, &report, tally);
}

/// The reason the engine must give for `path`, when it is listed in
/// [`STRICTER_THAN_CORPUS`].
fn stricter_reason(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    STRICTER_THAN_CORPUS
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, reason)| *reason)
}

/// Compare `report` with the corpus verdict for `path`.
fn compare(path: &Path, report: &BindingReport, tally: &mut Tally) {
    if let Some(reason) = stricter_reason(path) {
        let expected = BindingStatus::Invalid {
            code: "assertion.dataHash.mismatch",
            reason,
        };
        match report.status == expected {
            true => tally.bindings_stricter += 1,
            false => tally
                .failures
                .push(format!("{}: expected {expected:?}", path.display())),
        }
        return;
    }
    let Some(recorded) = recorded_verdict(path) else {
        tally.bindings_unrecorded += 1;
        return;
    };
    if engine_verdict(report) == Some(recorded) {
        tally.bindings_agree += 1;
    } else {
        tally.failures.push(format!(
            "{}: corpus records match={recorded}, engine reports {:?}",
            path.display(),
            report.status
        ));
    }
}

#[test]
fn every_conformance_asset_is_located_parsed_and_binding_checked() {
    let dir = match corpus_dir() {
        Ok(dir) => dir,
        Err(reason) => {
            eprintln!("SKIPPED conformance parse: {reason}");
            return;
        }
    };
    let files = oracle::corpus_files(&dir, oracle::MAX_CORPUS_FILES).expect("corpus walk");
    let mut tally = Tally::default();
    for file in files
        .iter()
        .filter(|f| has_extension(f, &["jpg", "jpeg", "png", "mp4"]))
    {
        check_file(file, &mut tally);
    }
    eprintln!(
        "conformance parse: {} stores, {} files without a store, {} manifests parsed, {} hard bindings agree with the corpus, {} stricter under C2PA 2.4, {} without a recorded verdict, {} failures",
        tally.stores,
        tally.no_store,
        tally.manifests_ok,
        tally.bindings_agree,
        tally.bindings_stricter,
        tally.bindings_unrecorded,
        tally.failures.len()
    );
    assert!(tally.stores > 0, "no manifest stores found");
    assert_eq!(
        tally.bindings_stricter,
        STRICTER_THAN_CORPUS.len(),
        "every listed deviation was seen"
    );
    assert!(tally.failures.is_empty(), "{:#?}", tally.failures);
}

/// Whether `path` has one of `extensions` (case-insensitive).
fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}
