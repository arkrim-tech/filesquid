//! Content files against the inventories: every file in a content
//! directory is in the manifest (E023), every manifest path is inside a
//! version's content directory (E042), and every stated digest matches the
//! bytes: manifest digests (E092) and fixity values (E093).

use std::collections::{BTreeMap, BTreeSet};

use super::listing::Listing;
use super::report::ValidationReport;
use crate::digest::DigestAlgorithm;
use crate::fixity::{FixityAlgorithm, FixityHasher};
use crate::inventory::{Inventory, VersionNum};
use crate::read::small::key_in;
use crate::read::ReadError;
use crate::storage::{
    ChunkReader, StorageBackend, StorageError, MAX_CHUNK_BYTES, MAX_OBJECT_BYTES,
};

/// Most chunks read from one content file: its largest possible size in
/// [`MAX_CHUNK_BYTES`] chunks, plus the end-of-stream call.
const MAX_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// A digest a content file must have, and the code for a mismatch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Expectation {
    /// The algorithm the digest is in.
    pub algorithm: FixityAlgorithm,
    /// The stated digest, in lowercase.
    pub digest: String,
    /// E092 for a manifest digest, E093 for a fixity value.
    pub code: &'static str,
}

/// Content paths mapped to the digests their bytes must have.
pub(crate) type Expectations = BTreeMap<String, BTreeSet<Expectation>>;

/// The fixity-table entry of a content-addressing algorithm.
fn as_fixity(algorithm: DigestAlgorithm) -> FixityAlgorithm {
    match algorithm {
        DigestAlgorithm::Sha256 => FixityAlgorithm::Sha256,
        DigestAlgorithm::Sha512 => FixityAlgorithm::Sha512,
    }
}

/// Expect every manifest digest of `inventory` (the root inventory or an
/// earlier version inventory, whose algorithm may differ) at its paths.
pub(crate) fn expect_manifest(expectations: &mut Expectations, inventory: &Inventory) {
    let algorithm = as_fixity(inventory.digest_algorithm);
    for (digest, paths) in &inventory.manifest {
        for path in paths {
            let expectation = Expectation {
                algorithm,
                digest: digest.to_ascii_lowercase(),
                code: "E092",
            };
            expectations
                .entry(path.clone())
                .or_default()
                .insert(expectation);
        }
    }
}

/// Expect every fixity value of `inventory` in an algorithm of the fixity
/// table; values in any other algorithm are ignored (E028).
pub(crate) fn expect_fixity(expectations: &mut Expectations, inventory: &Inventory) {
    let blocks = inventory.fixity.iter().flatten();
    let known = blocks.filter_map(|(name, block)| Some((FixityAlgorithm::from_name(name)?, block)));
    for (algorithm, block) in known {
        for (digest, paths) in block {
            for path in paths {
                let expectation = Expectation {
                    algorithm,
                    digest: digest.to_ascii_lowercase(),
                    code: "E093",
                };
                expectations
                    .entry(path.clone())
                    .or_default()
                    .insert(expectation);
            }
        }
    }
}

/// Every file in a version's content directory must be in `inventory`'s
/// manifest (E023). `label` names the inventory in the message.
pub(crate) fn check_listed(
    listing: &Listing,
    inventory: &Inventory,
    label: &str,
    report: &mut ValidationReport,
) {
    let listed: BTreeSet<&str> = inventory
        .manifest
        .values()
        .flatten()
        .map(String::as_str)
        .collect();
    let content = inventory.content_directory();
    for version in inventory.versions.keys() {
        let dir = format!("{version}/{content}");
        for path in listing.files_under(&dir) {
            if !listed.contains(path) {
                let message = format!("a content file is missing from the manifest of {label}");
                report.add("E023", listing.key(path), message);
            }
        }
    }
}

/// Every manifest path must be inside the content directory of a version
/// the inventory lists (E042).
pub(crate) fn check_manifest_paths(
    listing: &Listing,
    inventory: &Inventory,
    report: &mut ValidationReport,
) {
    for path in inventory.manifest.values().flatten() {
        if !in_content_directory(inventory, path) {
            let message = "a content path must be inside a version's content directory";
            report.add("E042", listing.key(path), message);
        }
    }
}

/// Whether `path` is `vN/<contentDirectory>/...` for a version `vN` of
/// `inventory`.
fn in_content_directory(inventory: &Inventory, path: &str) -> bool {
    let Some((version, rest)) = path.split_once('/') else {
        return false;
    };
    let listed = VersionNum::parse(version).is_ok_and(|v| inventory.versions.contains_key(&v));
    let inside = rest
        .strip_prefix(inventory.content_directory())
        .is_some_and(|below| below.starts_with('/'));
    listed && inside
}

/// Read every expected content file once, hashing it in every algorithm it
/// needs, and report each digest it does not match (E092, E093) and each
/// path that does not exist.
///
/// # Errors
///
/// A backend error other than a missing file.
pub(crate) async fn verify_content<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    expectations: &Expectations,
    report: &mut ValidationReport,
) -> Result<(), ReadError> {
    for (path, expected) in expectations {
        let digests = if listing.contains(path) {
            hash_file(backend, listing, path, expected).await?
        } else {
            None
        };
        match digests {
            Some(digests) => report_mismatches(listing, path, expected, &digests, report),
            None => report_missing(listing, path, expected, report),
        }
    }
    Ok(())
}

/// Stream one content file through a hasher per algorithm in `expected`.
/// `None` when the file disappeared during the read.
///
/// # Errors
///
/// A backend error other than a missing file, or a file above the
/// engine's object bound.
async fn hash_file<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    path: &str,
    expected: &BTreeSet<Expectation>,
) -> Result<Option<BTreeMap<FixityAlgorithm, String>>, ReadError> {
    let algorithms: BTreeSet<FixityAlgorithm> = expected.iter().map(|e| e.algorithm).collect();
    let mut hashers: Vec<(FixityAlgorithm, FixityHasher)> =
        algorithms.into_iter().map(|a| (a, a.hasher())).collect();
    let key = key_in(&listing.key(""), path)?;
    let mut reader = ChunkReader::new(backend, key, MAX_CHUNK_BYTES)?;
    for _ in 0..MAX_CHUNKS {
        let chunk = match reader.next_chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Ok(Some(finish(hashers))),
            Err(StorageError::NotFound { .. }) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        hashers
            .iter_mut()
            .for_each(|(_, hasher)| hasher.update(&chunk));
    }
    Err(StorageError::LimitExceeded {
        what: "chunks read from one content file",
        limit: MAX_CHUNKS,
    }
    .into())
}

/// The final digest of every hasher.
fn finish(hashers: Vec<(FixityAlgorithm, FixityHasher)>) -> BTreeMap<FixityAlgorithm, String> {
    hashers
        .into_iter()
        .map(|(algorithm, hasher)| (algorithm, hasher.finalize_hex()))
        .collect()
}

/// Report each expected digest the computed `digests` do not match.
fn report_mismatches(
    listing: &Listing,
    path: &str,
    expected: &BTreeSet<Expectation>,
    digests: &BTreeMap<FixityAlgorithm, String>,
    report: &mut ValidationReport,
) {
    for expectation in expected {
        let actual = digests.get(&expectation.algorithm).map(String::as_str);
        let algorithm = expectation.algorithm;
        if actual.is_some_and(|actual| algorithm.matches(actual, &expectation.digest)) {
            continue;
        }
        let source = if expectation.code == "E093" {
            "fixity"
        } else {
            "manifest"
        };
        let message = format!(
            "the content does not match its {} {source} digest",
            expectation.algorithm.name()
        );
        report.add(expectation.code, listing.key(path), message);
    }
}

/// Report a content path that does not exist, once for each code that
/// expects it: E092 for the manifest, E093 for the fixity block.
fn report_missing(
    listing: &Listing,
    path: &str,
    expected: &BTreeSet<Expectation>,
    report: &mut ValidationReport,
) {
    let codes: BTreeSet<&'static str> = expected.iter().map(|e| e.code).collect();
    for code in codes {
        let source = if code == "E093" {
            "fixity block"
        } else {
            "manifest"
        };
        let message = format!("the {source} lists a content path that does not exist");
        report.add(code, listing.key(path), message);
    }
}
