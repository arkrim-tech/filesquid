//! Writing and reading a library's canonical OCFL state through the `filesquid-ocfl` change
//! model, the only write path (API-S-008). Generic over the host's storage backend, so the
//! API does no filesystem I/O itself; every function here performs storage I/O through it.

use super::entity::{decode, encode, LibraryHead, SnapshotEntity, SnapshotFile, VirtualEntry};
use super::layout::{file_object_root, original_path, SnapshotKey, SNAPSHOT_ROOT};
use super::message::{parse_operation_message, OperationRecord};
use super::original::{
    companion_path, decode_companion, encode_companion, stored_name, FileOriginal, OriginalRecord,
};
use super::{CanonicalError, ProfileTransform};
use filesquid_core::{Collection, FileId, Folder};
use filesquid_ocfl::change::{ChangeError, ChangeModel, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::{LogicalFile, Object, ReadError};
use filesquid_ocfl::storage::{
    read_verified, Precondition, StorageBackend, StorageError, StorageKey,
};
use filesquid_ocfl::write::{BoundContext, CommitReceipt, VersionBoundFiles, VersionReservations};
use filesquid_ocfl::VersionNum;
use filesquid_ocfl::{Declaration, SpecVersion};
use std::collections::{BTreeMap, BTreeSet};

/// Most entities one snapshot read accepts (AGENTS.md §22).
pub const MAX_SNAPSHOT_ENTITIES: usize = 1_000_000;

/// Largest logical file of a snapshot entity.
pub const MAX_ENTITY_BYTES: u64 = 1 << 20;

/// Prefix of the version-bound history files (PROV-O record and journal entries) a commit
/// writes beside its entities; they are not entities and a snapshot read skips them.
pub const HISTORY_PREFIX: &str = "provenance/";

/// Suffix of the journal entries a commit writes beside its PROV-O record:
/// `provenance/<version>.journal.jsonl`.
pub const JOURNAL_SUFFIX: &str = ".journal.jsonl";

/// Most journal lines one version's history file may hold (AGENTS.md §22).
pub const MAX_JOURNAL_LINES: usize = 256;

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
    commit_snapshot_with(model, transform, delta, meta, None).await
}

/// [`commit_snapshot`], with `bound` writing version-bound files (the commit's PROV-O record
/// and journal entries, OCFL-S-024) into the same version, sealed by `transform` like the
/// entities. A delta that changes nothing still writes no version.
///
/// # Errors
/// As [`commit_snapshot`]; a `bound` failure publishes nothing.
pub async fn commit_snapshot_with<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    transform: &dyn ProfileTransform,
    delta: &SnapshotDelta,
    meta: VersionMeta,
    bound: Option<&dyn VersionBoundFiles>,
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
    let sealed = bound.map(|inner| Sealed { inner, transform });
    let sealed = sealed.as_ref().map(|s| s as &dyn VersionBoundFiles);
    match model.commit_with(meta, sealed).await {
        Err(ChangeError::NothingToCommit) => Ok(None),
        result => published(result).map(Some),
    }
}

/// Version-bound files sealed by the profile transform, like every entity.
struct Sealed<'a> {
    inner: &'a dyn VersionBoundFiles,
    transform: &'a dyn ProfileTransform,
}

impl VersionBoundFiles for Sealed<'_> {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        let mut out = BTreeMap::new();
        for (path, bytes) in self.inner.files(context)? {
            let sealed = self
                .transform
                .seal(&path, bytes)
                .map_err(|e| e.code().to_string())?;
            out.insert(path, sealed);
        }
        Ok(out)
    }
}

/// The journal entry lines a commit wrote into each version of the snapshot object after
/// `after` (every version when `None`), oldest version first: what a catalog that missed
/// those commits replays into its outbox. Versions without a journal file are skipped.
///
/// # Errors
/// [`CanonicalError::Decode`] for a journal file larger than [`MAX_ENTITY_BYTES`] or with
/// more than [`MAX_JOURNAL_LINES`] lines, or a read failure.
pub async fn read_commit_journal<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
    after: Option<&VersionNum>,
) -> Result<Vec<String>, CanonicalError> {
    if !has_snapshot(backend).await? {
        return Ok(Vec::new());
    }
    let object = Object::open(backend, SNAPSHOT_ROOT).await?;
    let head = object.head().clone();
    let state = object.state(&head)?;
    let mut lines = Vec::new();
    for version in object.inventory().versions.keys() {
        if after.is_some_and(|seen| version <= seen) {
            continue;
        }
        let path = format!("{HISTORY_PREFIX}{version}{JOURNAL_SUFFIX}");
        let Some(file) = state.get(&path) else {
            continue;
        };
        let stored = read_entity(&object, backend, file).await?;
        let plain = transform.open(&path, stored)?;
        lines.extend(journal_lines(&path, &plain)?);
    }
    Ok(lines)
}

/// The non-empty lines of one journal file, bounded by [`MAX_JOURNAL_LINES`]. Pure.
fn journal_lines(path: &str, bytes: &[u8]) -> Result<Vec<String>, CanonicalError> {
    let refuse = |reason: &str| CanonicalError::Decode {
        path: path.to_string(),
        reason: reason.into(),
    };
    let text = std::str::from_utf8(bytes).map_err(|_| refuse("journal is not UTF-8"))?;
    let lines: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    match lines.len() > MAX_JOURNAL_LINES {
        true => Err(refuse("journal holds too many lines")),
        false => Ok(lines),
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

/// Commit the bytes of the original of `file_id`, described by `record` (the owner's file
/// name and the media type), as a version of the file object `model` writes (one OCFL object
/// per file, ADR 023), with `meta`. The bytes are stored as `original/<file-uuid>.<ext>` and,
/// in the same version, the metadata record `original/<file-uuid>.metadata.md` holds
/// `record`. Committing the same original and record again writes no version and answers
/// `None`, as [`commit_snapshot`] does for an unchanged state.
///
/// # Errors
/// As [`commit_snapshot`], and [`CanonicalError::InvalidIdentifier`] for a file identifier
/// that cannot name a path segment.
pub async fn commit_file_original<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    transform: &dyn ProfileTransform,
    file_id: &FileId,
    record: &OriginalRecord,
    bytes: &[u8],
    meta: VersionMeta,
) -> Result<Option<CommitReceipt>, CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let path = original_path(&stored_name(file_id, record.extension())?)?;
    let companion = companion_path(file_id)?;
    let text = encode_companion(record)?;
    model.reset().await?;
    let sealed = transform.seal(&path, bytes.to_vec())?;
    model.stage_bytes(&path, &sealed).await?;
    let sealed = transform.seal(&companion, text)?;
    model.stage_bytes(&companion, &sealed).await?;
    match model.commit(meta).await {
        Err(ChangeError::NothingToCommit) => Ok(None),
        result => published(result).map(Some),
    }
}

/// The original of `file_id` at the head of its file object in the storage root `backend`,
/// with its metadata record, or `None` when the object does not exist yet. The record's
/// original name is the name to display and to name an owner download under; the stored
/// name is a storage detail.
///
/// # Errors
/// [`CanonicalError::Decode`] for a metadata record that cannot be read back, or a head
/// without the record or the original it names; a read failure otherwise.
pub async fn read_file_original<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
    file_id: &FileId,
) -> Result<Option<FileOriginal>, CanonicalError> {
    let root = file_object_root(file_id)?;
    if !has_object(backend, &root).await? {
        return Ok(None);
    }
    let object = Object::open(backend, &root).await?;
    let state = object.state(object.head())?;
    let companion = companion_path(file_id)?;
    let file = state
        .files()
        .find(|file| file.path == companion)
        .ok_or_else(|| not_one_original(&root))?;
    let bytes = read_entity(&object, backend, file).await?;
    let record = decode_companion(&companion, &transform.open(&companion, bytes)?)?;
    let path = original_path(&stored_name(file_id, record.extension())?)?;
    if !state.paths().any(|p| p == path) {
        return Err(not_one_original(&root));
    }
    Ok(Some(FileOriginal { path, record }))
}

/// The refusal of a file object whose head lacks its metadata record or the original it names.
fn not_one_original(path: &str) -> CanonicalError {
    CanonicalError::Decode {
        path: path.to_string(),
        reason: "no original with its metadata record".into(),
    }
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
    has_object(backend, SNAPSHOT_ROOT).await
}

/// Whether the storage root holds an object at `root`: its object declaration exists.
async fn has_object<B: StorageBackend>(backend: &B, root: &str) -> Result<bool, CanonicalError> {
    let declaration = Declaration::object(SpecVersion::V1_1).file_name();
    let key = StorageKey::new(&format!("{root}/{declaration}"))
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
    let is_entity = |file: &&LogicalFile| !file.path.starts_with(HISTORY_PREFIX);
    if state.files().filter(is_entity).count() > MAX_SNAPSHOT_ENTITIES {
        return Err(CanonicalError::Failed(
            "snapshot holds too many entities".into(),
        ));
    }
    let mut entities = Vec::new();
    for file in state.files().filter(is_entity) {
        let stored = read_entity(object, backend, file).await?;
        let plain = transform.open(&file.path, stored)?;
        entities.push(decode(&file.path, &plain)?);
    }
    Ok(entities)
}

/// The stored bytes of one entity or companion file, refused before reading when larger than
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
