//! Copy a completed snapshot of a storage root (F-001, F-042, OCFL-S-019).
//!
//! The copy holds what the source has committed and nothing else: the storage-root
//! declaration (and `ocfl_layout.json` and `ocfl_1.1.txt` when present) and, per object, its
//! declaration, its root inventory and sidecar, each version inventory and sidecar present,
//! and every content path the manifest lists. Files no inventory lists, such as a stray file
//! or a reservation, are left behind. Every file is written with `IfAbsent` into a
//! destination that MUST be empty, so the copy never replaces anything, and the source is only
//! read. A head that is still being published refuses the copy until it completes. The copy
//! is then validated object by object and its findings reported, so the owner gets a checked
//! OCFL root readable with ordinary tools.

use super::error::LocalLibraryError;
use super::root::is_empty;
use filesquid_ocfl::inventory::{sidecar_name, Inventory, INVENTORY_FILE};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{
    read_verified, ChunkReader, MultipartWriter, Precondition, StorageBackend, StorageError,
    StorageKey, MAX_CHUNK_BYTES, MAX_PART_BYTES, MAX_PUT_BYTES,
};
use filesquid_ocfl::validate_object;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Optional storage-root files copied when present, besides the declaration.
const ROOT_EXTRAS: [&str; 2] = ["ocfl_layout.json", "ocfl_1.1.txt"];

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

/// Files and bytes written so far.
#[derive(Default)]
struct Tally {
    files: u64,
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
/// `destination_not_empty` when `dest` holds anything (or gains a file during the copy);
/// `snapshot_incomplete` (retryable) while a head is being published; `snapshot_unreadable`
/// for an object that cannot be opened; the open errors of the source; `unavailable`.
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
        copy_if_present(source, dest, name, &mut tally).await?;
    }
    for object in &objects {
        for name in &object.keys {
            copy_file(source, dest, &key(name)?, &mut tally).await?;
        }
    }
    Ok(CopyReport {
        objects: objects.len(),
        files: tally.files,
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

/// The refusal for an object that cannot be opened, logging why to stdout only.
fn unreadable(err: &filesquid_ocfl::ReadError) -> LocalLibraryError {
    tracing::info!(target: super::LOG_TARGET, error = %err, "snapshot object unreadable");
    LocalLibraryError::refused(
        "snapshot_unreadable",
        "An item in this library cannot be read, so no complete copy can be made. \
         Check the library for problems first.",
    )
}

/// The keys of `object`'s committed files: declaration, root inventory and sidecar, version
/// inventories and sidecars present, and manifest content paths.
async fn object_files<S: StorageBackend>(
    source: &S,
    object: &Object<'_, S>,
) -> Result<ObjectFiles, LocalLibraryError> {
    let inventory = object.inventory();
    let root = object.root().to_string();
    let sidecar = sidecar_name(inventory.digest_algorithm);
    let mut keys = BTreeSet::new();
    keys.insert(format!("{root}/{}", object.declaration().file_name()));
    keys.insert(format!("{root}/{INVENTORY_FILE}"));
    keys.insert(format!("{root}/{sidecar}"));
    for name in version_files(inventory, &sidecar) {
        let full = format!("{root}/{name}");
        if exists(source, &full).await? {
            keys.insert(full);
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

/// `vN/inventory.json` and `vN/<sidecar>` of every version of `inventory`.
fn version_files(inventory: &Inventory, sidecar: &str) -> Vec<String> {
    inventory
        .versions
        .keys()
        .flat_map(|v| [format!("{v}/{INVENTORY_FILE}"), format!("{v}/{sidecar}")])
        .collect()
}

/// Whether `raw` exists on `backend`.
async fn exists<B: StorageBackend>(backend: &B, raw: &str) -> Result<bool, LocalLibraryError> {
    backend
        .exists(&key(raw)?)
        .await
        .map_err(|err| LocalLibraryError::from_storage(&err))
}

/// Copy `raw` when the source has it.
async fn copy_if_present<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    raw: &str,
    tally: &mut Tally,
) -> Result<(), LocalLibraryError> {
    if !exists(source, raw).await? {
        return Ok(());
    }
    copy_file(source, dest, &key(raw)?, tally).await
}

/// Copy one file, whole when it fits one put and streamed part by part otherwise, never over
/// an existing file.
async fn copy_file<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    key: &StorageKey,
    tally: &mut Tally,
) -> Result<(), LocalLibraryError> {
    let size = source.head(key).await.map_err(|e| copy_error(&e))?.size;
    let copied = if size <= MAX_PUT_BYTES {
        copy_whole(source, dest, key).await
    } else {
        copy_streamed(source, dest, key).await
    };
    copied.map_err(|err| copy_error(&err))?;
    tally.files += 1;
    tally.bytes += size;
    Ok(())
}

/// A storage failure during the copy: a file that appeared at the destination refuses the
/// copy; anything else is `unavailable`.
fn copy_error(err: &StorageError) -> LocalLibraryError {
    match err {
        StorageError::PreconditionFailed { .. } => destination_not_empty(),
        other => LocalLibraryError::from_storage(other),
    }
}

/// Copy a file of at most one put as one consistent read.
async fn copy_whole<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    key: &StorageKey,
) -> Result<(), StorageError> {
    let bytes = read_verified(source, key, MAX_CHUNK_BYTES).await?;
    dest.put_if(key, &bytes, &Precondition::IfAbsent).await?;
    Ok(())
}

/// Copy a large file through a multipart upload, aborting the upload on failure.
async fn copy_streamed<S: StorageBackend, D: StorageBackend>(
    source: &S,
    dest: &D,
    key: &StorageKey,
) -> Result<(), StorageError> {
    let mut writer = MultipartWriter::start(dest, key.clone()).await?;
    let uploaded = upload_parts(source, key, &mut writer).await;
    let finished = match uploaded {
        Ok(()) => writer
            .finish(Some(&Precondition::IfAbsent))
            .await
            .map(|_| ()),
        Err(err) => Err(err),
    };
    if let Err(err) = finished {
        if let Err(abort) = writer.abort().await {
            tracing::warn!(target: super::LOG_TARGET, error = %abort, "could not abort a copy upload");
        }
        return Err(err);
    }
    Ok(())
}

/// Read `key` chunk by chunk and upload parts of up to [`MAX_PART_BYTES`]: a part is sent
/// before one more chunk could take it over the bound. The reader stops
/// at the end of the object, and [`MultipartWriter::write_part`] refuses more parts than the
/// backend bound, so the loop is bounded.
async fn upload_parts<S: StorageBackend, D: StorageBackend>(
    source: &S,
    key: &StorageKey,
    writer: &mut MultipartWriter<'_, D>,
) -> Result<(), StorageError> {
    let mut reader = ChunkReader::new(source, key.clone(), MAX_CHUNK_BYTES)?;
    let mut part = Vec::new();
    while let Some(chunk) = reader.next_chunk().await? {
        part.extend_from_slice(&chunk);
        if part.len() as u64 + MAX_CHUNK_BYTES > MAX_PART_BYTES {
            writer.write_part(&part).await?;
            part.clear();
        }
    }
    if !part.is_empty() {
        writer.write_part(&part).await?;
    }
    Ok(())
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
