//! Writing and reading a library's canonical OCFL state through the `filesquid-ocfl` change
//! model, the only write path (API-S-008). Generic over the host's storage backend, so the
//! API does no filesystem I/O itself; every function here performs storage I/O through it.

use super::entity::{decode, encode, LibraryHead, SnapshotEntity, SnapshotFile, VirtualEntry};
use super::layout::{original_path, SnapshotKey, SNAPSHOT_ROOT};
use super::message::{parse_operation_message, OperationRecord};
use super::{CanonicalError, ProfileTransform};
use filesquid_core::{Collection, Folder};
use filesquid_ocfl::change::{ChangeError, ChangeModel, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::{LogicalFile, Object, ReadError};
use filesquid_ocfl::storage::{
    read_verified, Precondition, StorageBackend, StorageError, StorageKey,
};
use filesquid_ocfl::write::{CommitReceipt, VersionReservations};
use filesquid_ocfl::VersionNum;
use filesquid_ocfl::{Declaration, SpecVersion};
use std::collections::{BTreeMap, BTreeSet};

/// Most entities one snapshot read accepts (AGENTS.md §22).
pub const MAX_SNAPSHOT_ENTITIES: usize = 1_000_000;

/// Largest logical file of a snapshot entity.
pub const MAX_ENTITY_BYTES: u64 = 1 << 20;

/// The entity files one commit writes and removes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDelta {
    /// Logical path → plain bytes to write.
    pub puts: BTreeMap<String, Vec<u8>>,
    /// Logical paths to remove.
    pub removes: BTreeSet<String>,
}

impl SnapshotDelta {
    /// Write `entity` (replacing any earlier write or removal of its path). Pure.
    ///
    /// # Errors
    /// As [`encode`] and [`SnapshotKey::path`].
    pub fn put(&mut self, entity: &SnapshotEntity) -> Result<(), CanonicalError> {
        let path = entity.key().path()?;
        self.removes.remove(&path);
        self.puts.insert(path, encode(entity)?);
        Ok(())
    }

    /// Remove the entity `key` (replacing any earlier write of its path). Pure.
    ///
    /// # Errors
    /// As [`SnapshotKey::path`].
    pub fn remove(&mut self, key: &SnapshotKey) -> Result<(), CanonicalError> {
        let path = key.path()?;
        self.puts.remove(&path);
        self.removes.insert(path);
        Ok(())
    }

    /// Whether the delta writes and removes nothing. Pure.
    pub fn is_empty(&self) -> bool {
        self.puts.is_empty() && self.removes.is_empty()
    }
}

/// A library's collection snapshot at one version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibrarySnapshot {
    /// The version read.
    pub version: VersionNum,
    /// The library entity.
    pub library: LibraryHead,
    /// Every collection.
    pub collections: Vec<Collection>,
    /// Every virtual collection with its members.
    pub virtual_collections: Vec<VirtualEntry>,
    /// Every folder.
    pub folders: Vec<Folder>,
    /// Every file's place and state.
    pub files: Vec<SnapshotFile>,
    /// The operation every version up to `version` published, oldest first.
    pub operations: Vec<OperationRecord>,
}

/// Write the OCFL 1.1 storage-root declaration into `backend`, which must hold no file.
///
/// # Errors
/// [`CanonicalError::Failed`] with `root_not_empty` when the backend already holds a file
/// (nothing is written); [`CanonicalError::Unavailable`] for storage trouble.
pub async fn create_storage_root<B: StorageBackend>(backend: &B) -> Result<(), CanonicalError> {
    let storage = |e| CanonicalError::from(ReadError::Storage(e));
    let page = backend.list("", None, 1).await.map_err(storage)?;
    if !page.keys.is_empty() {
        return Err(CanonicalError::Failed("root_not_empty".into()));
    }
    let declaration = Declaration::storage_root(SpecVersion::V1_1);
    let key = StorageKey::new(&declaration.file_name()).map_err(storage)?;
    match backend
        .put_if(
            &key,
            declaration.contents().as_bytes(),
            &Precondition::IfAbsent,
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(StorageError::PreconditionFailed { .. }) => {
            Err(CanonicalError::Failed("root_not_empty".into()))
        }
        Err(err) => Err(storage(err)),
    }
}

/// Commit `delta` as one version of the object `model` writes, with `meta`, and answer its
/// receipt, or `None` when the delta leaves the head's state as it is (no version is
/// written). The staging area is reset first, so a crash's leftovers are never published;
/// removals of paths the head does not hold are skipped. A version published whose staging
/// area could not be cleared still answers its receipt.
///
/// # Errors
/// [`CanonicalError::Conflict`] when the head moved under a conflicting change,
/// [`CanonicalError::HeadWithoutInventory`] for a W010 head (never repaired here),
/// [`CanonicalError::Unavailable`] for storage or reservation trouble; nothing is published.
pub async fn commit_snapshot<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    transform: &dyn ProfileTransform,
    delta: &SnapshotDelta,
    meta: VersionMeta,
) -> Result<Option<CommitReceipt>, CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    model.reset().await?;
    for (path, bytes) in &delta.puts {
        let sealed = transform.seal(path, bytes.clone())?;
        model.stage_bytes(path, &sealed).await?;
    }
    for path in &delta.removes {
        match model.stage_remove(path).await {
            Ok(()) | Err(ChangeError::PathNotFound { .. }) => {}
            Err(err) => return Err(err.into()),
        }
    }
    match model.commit(meta).await {
        Err(ChangeError::NothingToCommit) => Ok(None),
        result => published(result).map(Some),
    }
}

/// The receipt of a commit, also when only clearing the staging area failed after it was
/// published.
fn published(result: Result<CommitReceipt, ChangeError>) -> Result<CommitReceipt, CanonicalError> {
    match result {
        Ok(receipt) => Ok(receipt),
        Err(ChangeError::StagingNotCleared { receipt, .. }) => Ok(*receipt),
        Err(err) => Err(err.into()),
    }
}

/// Commit the bytes of a file's original named `name` as a version of the file object
/// `model` writes (one OCFL object per file, ADR 023), with `meta`. `name` must already be
/// one logical path segment (`[A-Za-z0-9._-]`, see [`super::layout::segment`]): the caller
/// (the import of DE-1.2) chooses that name, for example from the file identifier and a
/// normalised extension, and records the owner's original file name in the file record.
///
/// # Errors
/// As [`commit_snapshot`], and [`CanonicalError::InvalidIdentifier`] for a `name` that is
/// not one path segment.
pub async fn commit_file_original<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    transform: &dyn ProfileTransform,
    name: &str,
    bytes: &[u8],
    meta: VersionMeta,
) -> Result<CommitReceipt, CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let path = original_path(name)?;
    model.reset().await?;
    let sealed = transform.seal(&path, bytes.to_vec())?;
    model.stage_bytes(&path, &sealed).await?;
    published(model.commit(meta).await)
}

/// The collection snapshot of the storage root in `backend` at its head, or `None` when the
/// root holds no snapshot object yet.
///
/// # Errors
/// [`CanonicalError::Decode`] for an entity that cannot be read back,
/// [`CanonicalError::Failed`] for a snapshot without its library entity or larger than
/// [`MAX_SNAPSHOT_ENTITIES`], or a read failure.
pub async fn read_snapshot<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
) -> Result<Option<LibrarySnapshot>, CanonicalError> {
    if !has_snapshot(backend).await? {
        return Ok(None);
    }
    let object = Object::open(backend, SNAPSHOT_ROOT).await?;
    let version = object.head().clone();
    let entities = read_entities(&object, backend, transform, &version).await?;
    let operations = operations(&object)?;
    assemble(version, entities, operations).map(Some)
}

/// The operation every version of `object` published, oldest first. Versions without an
/// operation message (made by other tools) are skipped.
///
/// # Errors
/// [`CanonicalError::Decode`] for an operation message that cannot be read, so a receipt is
/// never silently lost.
fn operations<B: StorageBackend>(
    object: &Object<'_, B>,
) -> Result<Vec<OperationRecord>, CanonicalError> {
    let mut records = Vec::new();
    for version in object.history() {
        let message = version.message.as_deref().unwrap_or_default();
        let parsed = parse_operation_message(message).map_err(|reason| CanonicalError::Decode {
            path: format!("{}#message", version.version),
            reason,
        })?;
        records.extend(parsed);
    }
    Ok(records)
}

/// Whether the storage root holds a snapshot object: its object declaration exists.
async fn has_snapshot<B: StorageBackend>(backend: &B) -> Result<bool, CanonicalError> {
    let declaration = Declaration::object(SpecVersion::V1_1).file_name();
    let key = StorageKey::new(&format!("{SNAPSHOT_ROOT}/{declaration}"))
        .map_err(|e| CanonicalError::from(ReadError::Storage(e)))?;
    backend
        .exists(&key)
        .await
        .map_err(|e| CanonicalError::from(ReadError::Storage(e)))
}

/// Every entity of `object` at `version`, read, verified against its digest and decoded.
async fn read_entities<B: StorageBackend>(
    object: &Object<'_, B>,
    backend: &B,
    transform: &dyn ProfileTransform,
    version: &VersionNum,
) -> Result<Vec<SnapshotEntity>, CanonicalError> {
    let state = object.state(version)?;
    if state.files().count() > MAX_SNAPSHOT_ENTITIES {
        return Err(CanonicalError::Failed(
            "snapshot holds too many entities".into(),
        ));
    }
    let mut entities = Vec::new();
    for file in state.files() {
        let stored = read_entity(object, backend, file).await?;
        let plain = transform.open(&file.path, stored)?;
        entities.push(decode(&file.path, &plain)?);
    }
    Ok(entities)
}

/// The stored bytes of one entity file, refused before reading when larger than
/// [`MAX_ENTITY_BYTES`] and verified against its digest.
async fn read_entity<B: StorageBackend>(
    object: &Object<'_, B>,
    backend: &B,
    file: &LogicalFile,
) -> Result<Vec<u8>, CanonicalError> {
    let refuse = |reason: &str| CanonicalError::Decode {
        path: file.path.clone(),
        reason: reason.into(),
    };
    let storage = |e| CanonicalError::from(ReadError::Storage(e));
    let key = object.content_key(file)?;
    if backend.head(&key).await.map_err(storage)?.size > MAX_ENTITY_BYTES {
        return Err(refuse("entity is larger than 1 MiB"));
    }
    let stored = read_verified(backend, &key, MAX_ENTITY_BYTES)
        .await
        .map_err(storage)?;
    if stored.len() as u64 > MAX_ENTITY_BYTES {
        return Err(refuse("entity is larger than 1 MiB"));
    }
    let digest = DigestAlgorithm::ENGINE.digest_hex(&stored);
    if !DigestAlgorithm::ENGINE.matches(&digest, &file.digest) {
        return Err(refuse("content does not match its digest"));
    }
    Ok(stored)
}

/// The snapshot made of `entities`. Pure.
fn assemble(
    version: VersionNum,
    entities: Vec<SnapshotEntity>,
    operations: Vec<OperationRecord>,
) -> Result<LibrarySnapshot, CanonicalError> {
    if entities.len() > MAX_SNAPSHOT_ENTITIES {
        return Err(CanonicalError::Failed(
            "snapshot holds too many entities".into(),
        ));
    }
    let mut library = None;
    let (mut collections, mut virtual_collections) = (Vec::new(), Vec::new());
    let (mut folders, mut files) = (Vec::new(), Vec::new());
    for entity in entities {
        match entity {
            SnapshotEntity::Library(head) => library = Some(head),
            SnapshotEntity::Collection(c) => collections.push(c),
            SnapshotEntity::VirtualCollection(v) => virtual_collections.push(v),
            SnapshotEntity::Folder(f) => folders.push(f),
            SnapshotEntity::File(f) => files.push(f),
        }
    }
    let library =
        library.ok_or_else(|| CanonicalError::Failed("snapshot has no library entity".into()))?;
    Ok(LibrarySnapshot {
        version,
        library,
        collections,
        virtual_collections,
        folders,
        files,
        operations,
    })
}
