//! Copy a completed snapshot of a storage root (F-001, F-042, OCFL-S-019).
//!
//! The copy holds what the source has committed and nothing else: the storage-root
//! declaration (and `ocfl_layout.json` and `ocfl_1.1.txt` when present) and, per object, its
//! declaration, its root inventory and sidecar, each version inventory present with each
//! sidecar it has (a version may use another digest algorithm than the root), and every
//! content path the manifest lists. Files no inventory lists, such as a stray file or a
//! reservation, are left behind. Every file is written with `IfAbsent` into a destination
//! that MUST be empty, so the copy never replaces anything, and the source is only read
//! ([`super::transfer`] refuses a source file that changes while it is copied). A head that
//! is still being published refuses the copy until it completes. When every file is written,
//! the destination must hold exactly the files copied, and each object of the copy is
//! validated and its findings reported, so the owner gets a checked OCFL root readable with
//! ordinary tools.

use super::error::{is_outage, LocalLibraryError};
use super::root::is_empty;
use super::transfer::copy_file;
use filesquid_ocfl::inventory::{sidecar_algorithm, sidecar_name, INVENTORY_FILE};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE};
use filesquid_ocfl::{validate_object, DigestAlgorithm, ReadError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Optional storage-root files copied when present, besides the declaration.
const ROOT_EXTRAS: [&str; 2] = ["ocfl_layout.json", "ocfl_1.1.txt"];

/// Most keys one version directory listing of `inventory.json.*` reads: one per digest
/// algorithm a sidecar can name, with room to spare.
const MAX_SIDECARS: usize = 16;

/// An object of the copy whose validation found errors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectFindings {
    /// The object identifier.
    pub object_id: String,
    /// The distinct OCFL validation codes of its errors (`E###`), sorted.
    pub error_codes: Vec<String>,
}

/// What a snapshot copy wrote and what its validation found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyReport {
    /// Objects copied.
    pub objects: usize,
    /// Files written to the destination.
    pub files: u64,
    /// Bytes written to the destination.
    pub bytes: u64,
    /// Copied objects whose validation found errors; empty for a clean copy.
    pub invalid_objects: Vec<ObjectFindings>,
}

/// One object to copy: its identifier, root and the keys of its committed files.
struct ObjectFiles {
    id: String,
    root: String,
    keys: BTreeSet<String>,
}

/// The keys written so far and their bytes.
#[derive(Default)]
struct Tally {
    written: BTreeSet<String>,
    bytes: u64,
}

/// Copy the completed snapshot of `source` into the empty `dest` and validate the copy.
///
/// A backend cannot see the folders around it, so the host MUST refuse a destination folder
/// inside the source folder before calling this (OCFL-S-019: a copy never lands inside an
/// OCFL root).
///
/// # Errors
///
/// `destination_not_empty` when `dest` holds anything, or gains a file the copy did not
/// write; `snapshot_incomplete` (retryable) while a head is being published;
/// `snapshot_unreadable` for an object that cannot be opened; the open errors of the source;
/// `unavailable` for a storage outage, including a source file that changed while copied.
pub(super) async fn copy_snapshot<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
) -> Result<CopyReport, LocalLibraryError> {
    if !is_empty(dest).await? {
        return Err(destination_not_empty());
    }
    let storage = StorageRoot::open(source)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let mut root_keys = vec![storage.declaration().file_name()];
    root_keys.extend(ROOT_EXTRAS.iter().map(|name| name.to_string()));
    let objects = plan_objects(source, &storage).await?;
    let mut tally = Tally::default();
    for name in &root_keys {
        if exists(source, name).await? {
            copy_one(source, dest, name, &mut tally).await?;
        }
    }
    for name in objects.iter().flat_map(|object| &object.keys) {
        copy_one(source, dest, name, &mut tally).await?;
    }
    refuse_unplanned(dest, &tally.written).await?;
    Ok(CopyReport {
        objects: objects.len(),
        files: tally.written.len() as u64,
        bytes: tally.bytes,
        invalid_objects: validate_copies(dest, &objects).await?,
    })
}

/// The refusal for a destination that is not empty.
fn destination_not_empty() -> LocalLibraryError {
    LocalLibraryError::refused(
        "destination_not_empty",
        "Choose an empty folder for the copy; a copy never writes over existing files.",
    )
}

/// A storage key from `raw`.
fn key(raw: &str) -> Result<StorageKey, LocalLibraryError> {
    StorageKey::new(raw).map_err(|err| LocalLibraryError::from_storage(&err))
}

/// The committed files of every object of `storage`, refusing an incomplete snapshot.
async fn plan_objects<S: StorageBackend>(
    source: &S,
    storage: &StorageRoot<'_, S>,
) -> Result<Vec<ObjectFiles>, LocalLibraryError> {
    let mut objects = Vec::with_capacity(storage.objects().len());
    for root in storage.objects() {
        let object = storage
            .open_object(root)
            .await
            .map_err(|err| unreadable(&err))?;
        if object.unpublished_head().is_some() {
            return Err(LocalLibraryError::refused(
                "snapshot_incomplete",
                "A change is still being saved. Try the copy again when it completes.",
            ));
        }
        objects.push(object_files(source, &object).await?);
    }
    Ok(objects)
}

/// The error for an object that cannot be opened, logging why to stdout only: a storage
/// outage is `unavailable`; anything else refuses the copy as `snapshot_unreadable`.
fn unreadable(err: &ReadError) -> LocalLibraryError {
    if let ReadError::Storage(storage) = err {
        if is_outage(storage) {
            return LocalLibraryError::from_storage(storage);
        }
    }
    tracing::info!(target: super::LOG_TARGET, error = %err, "snapshot object unreadable");
    LocalLibraryError::refused(
        "snapshot_unreadable",
        "An item in this library cannot be read, so no complete copy can be made. \
         Check the library for problems first.",
    )
}

/// The keys of `object`'s committed files: declaration, root inventory and sidecar, version
/// inventories and their sidecars present, and manifest content paths.
async fn object_files<S: StorageBackend>(
    source: &S,
    object: &Object<'_, S>,
) -> Result<ObjectFiles, LocalLibraryError> {
    let inventory = object.inventory();
    let root = object.root().to_string();
    let mut keys = BTreeSet::new();
    keys.insert(format!("{root}/{}", object.declaration().file_name()));
    keys.insert(format!("{root}/{INVENTORY_FILE}"));
    keys.insert(format!(
        "{root}/{}",
        sidecar_name(inventory.digest_algorithm)
    ));
    for version in inventory.versions.keys() {
        let dir = format!("{root}/{version}");
        let version_inventory = format!("{dir}/{INVENTORY_FILE}");
        if exists(source, &version_inventory).await? {
            keys.insert(version_inventory);
            keys.extend(version_sidecars(source, &dir).await?);
        }
    }
    let content = inventory.manifest.values().flatten();
    keys.extend(content.map(|path| format!("{root}/{path}")));
    Ok(ObjectFiles {
        id: inventory.id.clone(),
        root,
        keys,
    })
}

/// The sidecars in the version directory `dir`: `inventory.json.<algorithm>` for every
/// algorithm the engine knows, whichever the version inventory uses.
async fn version_sidecars<S: StorageBackend>(
    source: &S,
    dir: &str,
) -> Result<Vec<String>, LocalLibraryError> {
    let prefix = format!("{dir}/{INVENTORY_FILE}.");
    let page = source
        .list(&prefix, None, MAX_SIDECARS)
        .await
        .map_err(|err| LocalLibraryError::from_storage(&err))?;
    let named = |raw: &str| {
        let name = raw.rsplit('/').next().unwrap_or(raw);
        sidecar_algorithm(name).and_then(DigestAlgorithm::from_name)
    };
    Ok(page
        .keys
        .iter()
        .map(StorageKey::as_str)
        .filter(|raw| named(raw).is_some())
        .map(String::from)
        .collect())
}

/// Whether `raw` exists on `backend`.
async fn exists<B: StorageBackend>(backend: &B, raw: &str) -> Result<bool, LocalLibraryError> {
    backend
        .exists(&key(raw)?)
        .await
        .map_err(|err| LocalLibraryError::from_storage(&err))
}

/// Copy `raw` and count it.
async fn copy_one<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    raw: &str,
    tally: &mut Tally,
) -> Result<(), LocalLibraryError> {
    let bytes = copy_file(source, dest, &key(raw)?)
        .await
        .map_err(|err| copy_error(&err))?;
    tally.written.insert(raw.to_string());
    tally.bytes += bytes;
    Ok(())
}

/// A storage failure during the copy: a file that appeared at the destination refuses the
/// copy; anything else maps as [`LocalLibraryError::from_storage`].
fn copy_error(err: &StorageError) -> LocalLibraryError {
    match err {
        StorageError::PreconditionFailed { .. } => destination_not_empty(),
        other => LocalLibraryError::from_storage(other),
    }
}

/// Refuse the copy when `dest` holds a key the copy did not write. The listing reads at most
/// one page more than the written keys fill, so a destination with more keys than that is
/// refused without being read to its end.
async fn refuse_unplanned<D: StorageBackend>(
    dest: &D,
    written: &BTreeSet<String>,
) -> Result<(), LocalLibraryError> {
    let max_pages = written.len() / MAX_LIST_PAGE + 2;
    let mut after: Option<StorageKey> = None;
    for _ in 0..max_pages {
        let page = dest
            .list("", after.as_ref(), MAX_LIST_PAGE)
            .await
            .map_err(|err| LocalLibraryError::from_storage(&err))?;
        if page.keys.iter().any(|k| !written.contains(k.as_str())) {
            return Err(destination_not_empty());
        }
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(()),
        }
    }
    Err(destination_not_empty())
}

/// Validate every copied object and keep those with errors.
async fn validate_copies<D: StorageBackend>(
    dest: &D,
    objects: &[ObjectFiles],
) -> Result<Vec<ObjectFindings>, LocalLibraryError> {
    let mut invalid = Vec::new();
    for object in objects {
        let report = validate_object(dest, &object.root)
            .await
            .map_err(|err| LocalLibraryError::from_open(&err))?;
        let codes: Vec<String> = report.error_codes().into_iter().map(String::from).collect();
        if !codes.is_empty() {
            invalid.push(ObjectFindings {
                object_id: object.id.clone(),
                error_codes: codes,
            });
        }
    }
    Ok(invalid)
}
