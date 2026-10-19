//! Ranged reads for preview and download, and the fixity check of one
//! logical file (OCFL-S-012).

use serde_json::{json, Value};

use super::error::ViewError;
use super::{check_path, MAX_RANGE_BYTES};
use crate::fixity::{FixityAlgorithm, FixityHasher};
use crate::inventory::VersionNum;
use crate::read::{Object, ReadError};
use crate::storage::{ByteRange, ChunkReader, StorageBackend, MAX_CHUNK_BYTES, MAX_OBJECT_BYTES};

/// Most chunks one fixity check reads.
const MAX_CHECK_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Read `len` bytes (1 to [`MAX_RANGE_BYTES`]) of the logical file `path`
/// of `version`, from byte `start`. Fewer bytes come back at the end of
/// the file and none from past it. A range is not checked against the
/// digest (only a whole read can be); [`check_fixity`] does that.
///
/// # Errors
///
/// [`ViewError::InvalidRequest`] for a length outside the bound, a range
/// whose end overflows `u64`, or an invalid logical path;
/// [`ViewError::Read`] for an unknown version or a path the version does
/// not hold; or a backend error.
pub async fn read_range<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
    start: u64,
    len: u64,
) -> Result<Vec<u8>, ViewError> {
    if len == 0 || len > MAX_RANGE_BYTES || start.checked_add(len).is_none() {
        return Err(ViewError::request(format!(
            "a range is 1 to {MAX_RANGE_BYTES} bytes and ends within 2^64"
        )));
    }
    check_path(path)?;
    let state = object.state(version)?;
    let file = state.get(path).ok_or_else(|| ReadError::PathNotFound {
        version: version.to_string(),
        path: path.to_string(),
    })?;
    let range = ByteRange::new(start, len)?;
    let key = object.content_key(file)?;
    Ok(object.backend().get_range(&key, range).await?)
}

/// One digest a fixity check verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixityCheck {
    /// The algorithm's OCFL name.
    pub algorithm: String,
    /// The value the inventory records.
    pub expected: String,
    /// The value computed over the stored bytes.
    pub actual: String,
    /// Whether they match.
    pub ok: bool,
}

/// The result of checking one logical file's stored bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixityReport {
    /// The logical path.
    pub path: String,
    /// The pinned version.
    pub version: VersionNum,
    /// The manifest digest first, then each fixity value the engine can
    /// compute; values for other algorithms are not checked.
    pub checks: Vec<FixityCheck>,
    /// Whether every check matched.
    pub ok: bool,
}

impl FixityReport {
    /// The report as JSON.
    pub fn to_json(&self) -> Value {
        let checks: Vec<Value> = self
            .checks
            .iter()
            .map(|c| {
                json!({
                    "algorithm": c.algorithm,
                    "expected": c.expected,
                    "actual": c.actual,
                    "ok": c.ok,
                })
            })
            .collect();
        json!({
            "path": self.path,
            "version": self.version.to_string(),
            "checks": checks,
            "ok": self.ok,
        })
    }
}

/// Read the stored bytes of the logical file `path` of `version` once and
/// compare them with the manifest digest and every fixity value the
/// inventory records for them. A mismatch is reported, not an error.
///
/// # Errors
///
/// [`ViewError::InvalidRequest`] for an invalid logical path;
/// [`ViewError::Read`] for an unknown version or a path the version does
/// not hold; or a backend error.
pub async fn check_fixity<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
) -> Result<FixityReport, ViewError> {
    check_path(path)?;
    let state = object.state(version)?;
    let file = state.get(path).ok_or_else(|| ReadError::PathNotFound {
        version: version.to_string(),
        path: path.to_string(),
    })?;
    let expected = expected_values(object, &file.digest, &file.content_path);
    let mut hashers: Vec<FixityHasher> = expected.iter().map(|(a, _)| a.hasher()).collect();
    let key = object.content_key(file)?;
    hash_all(object.backend(), key, &mut hashers).await?;
    let checks: Vec<FixityCheck> = expected
        .into_iter()
        .zip(hashers)
        .map(|((algorithm, value), hasher)| check(algorithm, value, hasher))
        .collect();
    Ok(FixityReport {
        path: path.to_string(),
        version: version.clone(),
        ok: checks.iter().all(|c| c.ok),
        checks,
    })
}

/// The manifest digest, then every computable fixity value the inventory
/// lists for `content_path` (all of them, so conflicting values are each
/// checked).
fn expected_values<B: StorageBackend>(
    object: &Object<'_, B>,
    digest: &str,
    content_path: &str,
) -> Vec<(FixityAlgorithm, String)> {
    let inventory = object.inventory();
    let manifest = FixityAlgorithm::from_name(inventory.digest_algorithm.name());
    let mut expected: Vec<(FixityAlgorithm, String)> = manifest
        .map(|a| (a, digest.to_string()))
        .into_iter()
        .collect();
    let blocks = inventory.fixity.iter().flatten();
    for (name, map) in blocks {
        let Some(algorithm) = FixityAlgorithm::from_name(name) else {
            continue;
        };
        let values = map
            .iter()
            .filter(|(_, paths)| paths.iter().any(|p| p == content_path));
        expected.extend(values.map(|(value, _)| (algorithm, value.clone())));
    }
    expected
}

/// Feed every chunk of `key` to each hasher.
async fn hash_all<B: StorageBackend>(
    backend: &B,
    key: crate::storage::StorageKey,
    hashers: &mut [FixityHasher],
) -> Result<(), ViewError> {
    let mut reader = ChunkReader::new(backend, key, MAX_CHUNK_BYTES)?;
    for _ in 0..MAX_CHECK_CHUNKS {
        let Some(chunk) = reader.next_chunk().await? else {
            return Ok(());
        };
        hashers.iter_mut().for_each(|h| h.update(&chunk));
    }
    Err(crate::storage::StorageError::LimitExceeded {
        what: "chunks read by one fixity check",
        limit: MAX_CHECK_CHUNKS,
    }
    .into())
}

/// Compare one finished hasher with its expected value.
fn check(algorithm: FixityAlgorithm, expected: String, hasher: FixityHasher) -> FixityCheck {
    let actual = hasher.finalize_hex();
    FixityCheck {
        algorithm: algorithm.name().to_string(),
        ok: algorithm.matches(&expected, &actual),
        expected,
        actual,
    }
}
