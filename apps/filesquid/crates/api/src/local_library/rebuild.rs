//! Rebuild from files (F-001, F-042): what every object of a storage root holds, read from
//! the OCFL files alone, so a host can rebuild its catalog without its operational database
//! or a running service.
//!
//! Each object is opened at its committed head and validated in full. Validator errors, such
//! as a digest mismatch left by an external edit, are reported per object and never adopted
//! or repaired; an object whose head has no version inventory (W010) is flagged as a repair
//! candidate, not repaired. An object that cannot be opened at all is reported by its error
//! code only. Nothing is written.

use super::error::{is_outage, LocalLibraryError};
use super::repair::head_files;
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::{validate_object, ReadError};
use serde::{Deserialize, Serialize};

/// One object rebuilt from its files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuiltObject {
    /// The object identifier.
    pub object_id: String,
    /// The committed head version.
    pub head: String,
    /// Logical files in the head version.
    pub files: usize,
    /// The object's digest algorithm (`sha256` or `sha512`).
    pub digest_algorithm: String,
    /// Distinct OCFL validation error codes (`E###`), sorted; empty for a valid object.
    pub error_codes: Vec<String>,
    /// Whether the head lacks its version inventory or its sidecar (W010, or an interrupted
    /// repair): a candidate for the consented repair.
    pub missing_version_inventory: bool,
}

/// An object that could not be opened. Its root is not reported (API-S-005).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadableObject {
    /// The OCFL validation code when an OCFL rule is broken (`E###`), else `unreadable`.
    pub error_code: String,
}

/// Everything a rebuild found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildReport {
    /// Objects read, in object-root order.
    pub objects: Vec<RebuiltObject>,
    /// Objects that could not be opened.
    pub unreadable: Vec<UnreadableObject>,
}

/// Read every object of `backend`'s storage root from its files. Writes nothing.
///
/// # Errors
///
/// The open errors of the storage root; `unavailable` for a storage failure.
pub(super) async fn rebuild<B: StorageBackend>(
    backend: &B,
) -> Result<RebuildReport, LocalLibraryError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let mut report = RebuildReport {
        objects: Vec::with_capacity(storage.objects().len()),
        unreadable: Vec::new(),
    };
    for root in storage.objects() {
        match storage.open_object(root).await {
            Ok(object) => report.objects.push(rebuilt(backend, &object).await?),
            Err(err) => report.unreadable.push(unreadable(&err)?),
        }
    }
    Ok(report)
}

/// The rebuilt view of one opened object.
async fn rebuilt<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
) -> Result<RebuiltObject, LocalLibraryError> {
    let files = object
        .state(object.head())
        .map(|state| state.len())
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let report = validate_object(backend, object.root())
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let head = head_files(backend, object).await?;
    Ok(RebuiltObject {
        object_id: object.id().to_string(),
        head: object.head().to_string(),
        files,
        digest_algorithm: object.inventory().digest_algorithm.name().to_string(),
        error_codes: report.error_codes().into_iter().map(String::from).collect(),
        missing_version_inventory: !head.complete(),
    })
}

/// The report entry of an object that could not be opened; a storage outage stops the
/// rebuild instead.
fn unreadable(err: &ReadError) -> Result<UnreadableObject, LocalLibraryError> {
    if let ReadError::Storage(storage) = err {
        if is_outage(storage) {
            return Err(LocalLibraryError::from_storage(storage));
        }
    }
    tracing::info!(target: super::LOG_TARGET, error = %err, "object unreadable during rebuild");
    Ok(UnreadableObject {
        error_code: err.code().unwrap_or("unreadable").to_string(),
    })
}
