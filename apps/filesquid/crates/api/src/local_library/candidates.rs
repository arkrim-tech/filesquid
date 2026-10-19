//! Version-inventory repair candidates (API-S-023, OCFL-S-020): the objects whose committed
//! head lacks its version inventory or its sidecar, found by checking each head directory,
//! independently of the scheduled validation (API-F2.7).
//!
//! An object is offered only in a shape the engine repairs: a committed (published) head
//! whose version directory holds no file its inventory does not list besides the version
//! inventory and its sidecar. A head version inventory that differs from the root inventory
//! already stops the object from opening (E064), so it is never offered. A head that holds
//! the version inventory but no sidecar is an interrupted repair, which the command
//! completes. Nothing is written.

use super::error::{is_outage, LocalLibraryError};
use super::repair::{RepairCandidate, RepairCandidates, MAX_CANDIDATES};
use filesquid_ocfl::inventory::{sidecar_name, INVENTORY_FILE};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE};
use filesquid_ocfl::{DigestAlgorithm, ReadError};
use std::collections::BTreeSet;

/// What the head version directory of an object holds of the two repair files.
pub(super) struct HeadFiles {
    inventory: bool,
    sidecar: bool,
}

impl HeadFiles {
    /// Both files are present: nothing to repair.
    pub(super) fn complete(&self) -> bool {
        self.inventory && self.sidecar
    }
}

/// The two repair files of `object`'s head version directory.
///
/// # Errors
///
/// A storage error.
pub(super) async fn head_files<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
) -> Result<HeadFiles, StorageError> {
    let dir = format!("{}/{}", object.root(), object.head());
    let sidecar = sidecar_name(object.inventory().digest_algorithm);
    Ok(HeadFiles {
        inventory: backend
            .exists(&key(&format!("{dir}/{INVENTORY_FILE}"))?)
            .await?,
        sidecar: backend.exists(&key(&format!("{dir}/{sidecar}"))?).await?,
    })
}

/// A storage key from `raw`.
fn key(raw: &str) -> Result<StorageKey, StorageError> {
    StorageKey::new(raw)
}

/// Open the object at `root`, or `None` when it is unreadable (not a storage outage).
///
/// # Errors
///
/// `unavailable` for a storage outage.
pub(super) async fn open_or_skip<'a, B: StorageBackend>(
    storage: &StorageRoot<'a, B>,
    root: &str,
) -> Result<Option<Object<'a, B>>, LocalLibraryError> {
    match storage.open_object(root).await {
        Ok(object) => Ok(Some(object)),
        Err(ReadError::Storage(err)) if is_outage(&err) => {
            Err(LocalLibraryError::from_storage(&err))
        }
        Err(err) => {
            tracing::info!(target: super::LOG_TARGET, error = %err, "object skipped");
            Ok(None)
        }
    }
}

/// The candidates of `backend`'s storage root, at most [`MAX_CANDIDATES`]. Writes nothing.
///
/// # Errors
///
/// The open errors of the storage root; `unavailable` for a storage outage.
pub(super) async fn candidates<B: StorageBackend>(
    backend: &B,
) -> Result<RepairCandidates, LocalLibraryError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let mut found = RepairCandidates {
        candidates: Vec::new(),
        truncated: false,
    };
    for root in storage.objects() {
        let Some(object) = open_or_skip(&storage, root).await? else {
            continue;
        };
        let Some(candidate) = candidate(backend, &object).await? else {
            continue;
        };
        if found.candidates.len() == MAX_CANDIDATES {
            found.truncated = true;
            break;
        }
        found.candidates.push(candidate);
    }
    Ok(found)
}

/// `object` as a candidate, or `None` when its head is complete, not yet published, or holds
/// a file that would make the engine refuse the repair.
async fn candidate<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
) -> Result<Option<RepairCandidate>, LocalLibraryError> {
    if object.unpublished_head().is_some() {
        return Ok(None);
    }
    let storage = |err: StorageError| LocalLibraryError::from_storage(&err);
    let files = head_files(backend, object).await.map_err(storage)?;
    if files.complete() || !head_is_clean(backend, object).await.map_err(storage)? {
        return Ok(None);
    }
    let algorithm = object.inventory().digest_algorithm;
    Ok(Some(RepairCandidate {
        object_id: object.id().to_string(),
        head: object.head().to_string(),
        digest_algorithm: algorithm.name().to_string(),
        interrupted: files.inventory,
        committable_after_repair: algorithm == DigestAlgorithm::Sha256,
    }))
}

/// Whether the head version directory holds only files the head's inventory lists, plus the
/// version inventory and its sidecar. The listing reads at most one page more than those
/// files fill; a directory with more keys than that holds an unlisted file.
async fn head_is_clean<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
) -> Result<bool, StorageError> {
    let root = object.root();
    let head = object.head().to_string();
    let inventory = object.inventory();
    let mut allowed: BTreeSet<String> = inventory
        .manifest
        .values()
        .flatten()
        .filter(|path| path.starts_with(&format!("{head}/")))
        .map(|path| format!("{root}/{path}"))
        .collect();
    allowed.insert(format!("{root}/{head}/{INVENTORY_FILE}"));
    allowed.insert(format!(
        "{root}/{head}/{}",
        sidecar_name(inventory.digest_algorithm)
    ));
    let prefix = format!("{root}/{head}/");
    let mut after: Option<StorageKey> = None;
    for _ in 0..allowed.len() / MAX_LIST_PAGE + 2 {
        let page = backend.list(&prefix, after.as_ref(), MAX_LIST_PAGE).await?;
        if page.keys.iter().any(|k| !allowed.contains(k.as_str())) {
            return Ok(false);
        }
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(true),
        }
    }
    Ok(false)
}
