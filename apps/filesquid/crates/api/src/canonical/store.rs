//! Writing and reading a library's canonical OCFL state through the `filesquid-ocfl` change
//! model, the only write path (API-S-008). Generic over the host's storage backend, so the
//! API does no filesystem I/O itself; every function here performs storage I/O through it.

use super::entity::{decode, encode, LibraryHead, SnapshotEntity, SnapshotFile, VirtualEntry};
use super::layout::{file_object_root, SnapshotKey, SNAPSHOT_ROOT};
use super::message::{parse_operation_message, OperationRecord};
use super::original::{
    check_new_content, check_new_original, check_same_original, companion_path, decode_record,
    encode_record, original_path_of, FileOriginal,
};
use super::{CanonicalError, ProfileTransform};
use filesquid_core::{Collection, FileId, Folder};
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::store::ocfl::MAX_RECORD_BYTES;
use filesquid_ocfl::change::{ChangeError, ChangeModel, PathChange, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::{LogicalFile, Object, ReadError};
use filesquid_ocfl::storage::{
    read_verified, Precondition, StorageBackend, StorageError, StorageKey,
};
use filesquid_ocfl::write::{BoundContext, CommitReceipt, VersionBoundFiles, VersionReservations};
use filesquid_ocfl::VersionNum;
use filesquid_ocfl::{Declaration, SpecVersion};
use std::collections::{BTreeMap, BTreeSet};
use tracing::Instrument;

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
    let span = super::message::commit_span(meta.message.as_deref());
    match model.commit_with(meta, sealed).instrument(span).await {
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
        let files = self.inner.files(context)?;
        check_journal_files(&files)?;
        for (path, bytes) in files {
            let sealed = self
                .transform
                .seal(&path, bytes)
                .map_err(|e| format!("{path}: {e:?}"))?;
            out.insert(path, sealed);
        }
        Ok(out)
    }
}

/// The journal entry lines a commit wrote into each version of the snapshot object after
/// `after` (every version when `None`), oldest version first, each read from the version
/// that wrote it (a later revert never hides it): what a catalog that missed those commits
/// replays into its outbox. Versions without a journal file are skipped.
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
    read_object_journal(backend, &object, transform, after).await
}

/// The journal entry lines every version of `object` after `after` (every version when
/// `None`) wrote into its `provenance/<version>.journal.jsonl`, oldest version first, each
/// read from the version that wrote it. Versions without a journal file are skipped. Any
/// object a commit recorded through `CommitRecorder` holds them: the snapshot, a file
/// object's version labels, a repaired object's provenance-only version. Storage I/O.
///
/// # Errors
/// As [`read_commit_journal`].
pub async fn read_object_journal<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
    transform: &dyn ProfileTransform,
    after: Option<&VersionNum>,
) -> Result<Vec<String>, CanonicalError> {
    let mut lines = Vec::new();
    for version in object.inventory().versions.keys() {
        if after.is_some_and(|seen| version <= seen) {
            continue;
        }
        lines.extend(read_version_journal(backend, object, transform, version).await?);
    }
    Ok(lines)
}

/// The journal entry lines `version` of `object` wrote into its
/// `provenance/<version>.journal.jsonl`; empty when that version has none. Storage I/O.
///
/// # Errors
/// As [`read_commit_journal`].
pub async fn read_version_journal<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
    transform: &dyn ProfileTransform,
    version: &VersionNum,
) -> Result<Vec<String>, CanonicalError> {
    let path = format!("{HISTORY_PREFIX}{version}{JOURNAL_SUFFIX}");
    let state = object.state(version)?;
    let Some(file) = state.get(&path) else {
        return Ok(Vec::new());
    };
    let stored = read_entity(object, backend, file, MAX_ENTITY_BYTES).await?;
    let plain = transform.open(&path, stored)?;
    journal_lines(&path, &plain)
}

/// Refuse a journal file that a replay could not read back: more than
/// [`MAX_JOURNAL_LINES`] lines or more than [`MAX_ENTITY_BYTES`] bytes. Pure.
fn check_journal_files(files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let journals = files
        .iter()
        .filter(|(path, _)| path.starts_with(HISTORY_PREFIX) && path.ends_with(JOURNAL_SUFFIX));
    for (path, bytes) in journals {
        let lines = bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .count();
        if lines > MAX_JOURNAL_LINES || bytes.len() as u64 > MAX_ENTITY_BYTES {
            return Err(format!("{path}: journal exceeds what a replay reads"));
        }
    }
    Ok(())
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

/// Commit the bytes of the original of `file_id` and its metadata record `record` as one
/// version of the file object `model` writes (one OCFL object per file, ADR 023), with
/// `meta`. The bytes are stored as `original/<file-uuid>.<ext>` and, in the same version, the
/// record (the `filesquid-metadata` [`FileRecord`], API-S-026) as
/// `original/<file-uuid>.metadata.md`. `previous` is the record at the head, if any: the new
/// record must extend it and add the import entries that cite these bytes
/// ([`check_new_original`]). Committing the head's original and record again writes no
/// version and answers `None`, as [`commit_snapshot`] does for an unchanged state.
///
/// # Errors
/// As [`commit_snapshot`]; [`CanonicalError::InvalidIdentifier`] for a file identifier that
/// cannot name a path segment; [`CanonicalError::Encode`] for a record that does not extend
/// `previous` or does not cite these bytes. Nothing is staged on a refusal.
pub async fn commit_file_original<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    transform: &dyn ProfileTransform,
    file_id: &FileId,
    record: &FileRecord,
    previous: Option<&FileRecord>,
    bytes: &[u8],
    meta: VersionMeta,
) -> Result<Option<CommitReceipt>, CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let path = original_path_of(file_id, record)?;
    match previous == Some(record) {
        true => check_same_original(record, bytes)?,
        false => check_new_original(file_id, record, previous, &path, bytes)?,
    }
    let companion = companion_path(file_id)?;
    model.reset().await?;
    let sealed = transform.seal(&path, bytes.to_vec())?;
    model.stage_bytes(&path, &sealed).await?;
    let sealed = transform.seal(&companion, encode_record(record))?;
    model.stage_bytes(&companion, &sealed).await?;
    let span = super::message::commit_span(meta.message.as_deref());
    match model.commit(meta).instrument(span).await {
        Err(ChangeError::NothingToCommit) => Ok(None),
        result => published(result).map(Some),
    }
}

/// Commit the original of the new file object of `file_id`, already staged at its stored path
/// through [`ChangeModel::stage_file`] as content of SHA-256 `digest` and `size` bytes, with
/// its metadata record `record` in the same version, with `meta` (Task DE-1.9, OCFL-S-026):
/// the streamed counterpart of [`commit_file_original`] for an object that has no head yet.
/// The record is stored as [`OwnerReadable`](super::OwnerReadable) content, as the original
/// is: a streamed original cannot be sealed by a profile transform.
///
/// # Errors
/// As [`commit_file_original`]; [`CanonicalError::Encode`] for a record that does not cite
/// the staged content; [`CanonicalError::Conflict`] when committing would change nothing.
pub async fn commit_staged_original<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    file_id: &FileId,
    record: &FileRecord,
    (digest, size): (&str, u64),
    meta: VersionMeta,
) -> Result<CommitReceipt, CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let path = original_path_of(file_id, record)?;
    check_new_content(file_id, record, None, &path, (digest, size))?;
    check_staged(model, &path, digest).await?;
    let companion = companion_path(file_id)?;
    model
        .stage_bytes(&companion, &encode_record(record))
        .await?;
    let span = super::message::commit_span(meta.message.as_deref());
    match model.commit(meta).instrument(span).await {
        Err(ChangeError::NothingToCommit) => Err(CanonicalError::Conflict),
        result => published(result),
    }
}

/// Refuse to commit unless `model` stages exactly one new file, `path`, with the content
/// `digest`, so a record can never cite content its version does not hold. Storage I/O.
///
/// # Errors
/// [`CanonicalError::Encode`] when the staged state is anything else; a read failure.
async fn check_staged<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    path: &str,
    digest: &str,
) -> Result<(), CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let status = model.status().await?;
    let staged = match status.changes.as_slice() {
        [PathChange::Added {
            path: staged,
            digest: content,
            ..
        }] => staged == path && content == digest,
        _ => false,
    };
    match staged {
        true => Ok(()),
        false => Err(CanonicalError::Encode(
            "the original is not staged with the content the record cites".into(),
        )),
    }
}

/// The original of `file_id` at the head of its file object in the storage root `backend`,
/// with its metadata record, or `None` when the object does not exist yet. The record's
/// display name is the name to show and to name an owner download under; the stored name is
/// a storage detail.
///
/// # Errors
/// [`CanonicalError::Decode`] for a metadata record that is not a valid file record, or a
/// head without the record or the original it names; a read failure otherwise.
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
    let bytes = read_entity(&object, backend, file, MAX_RECORD_BYTES).await?;
    let record = decode_record(&companion, &transform.open(&companion, bytes)?)?;
    let path = original_path_of(file_id, &record)?;
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

/// The library the storage root's collection snapshot belongs to: the UUID of its object
/// identifier `urn:uuid:<library-id>`, or `None` when the root holds no snapshot (a new or
/// earlier folder library) or its identifier has another form. Reads the snapshot object's
/// head inventory only, so a copied folder keeps the identity of its library (ADR 041
/// Decision 6).
///
/// # Errors
/// [`CanonicalError`] when the snapshot object exists but cannot be read.
pub async fn snapshot_library_id<B: StorageBackend>(
    backend: &B,
) -> Result<Option<String>, CanonicalError> {
    if !has_snapshot(backend).await? {
        return Ok(None);
    }
    let object = Object::open(backend, SNAPSHOT_ROOT).await?;
    Ok(object.id().strip_prefix("urn:uuid:").map(str::to_string))
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
        let stored = read_entity(object, backend, file, MAX_ENTITY_BYTES).await?;
        let plain = transform.open(&file.path, stored)?;
        entities.push(decode(&file.path, &plain)?);
    }
    Ok(entities)
}

/// The stored bytes of one entity or metadata record, refused before reading when larger
/// than `limit` and verified against its digest.
async fn read_entity<B: StorageBackend>(
    object: &Object<'_, B>,
    backend: &B,
    file: &LogicalFile,
    limit: u64,
) -> Result<Vec<u8>, CanonicalError> {
    let refuse = |reason: &str| CanonicalError::Decode {
        path: file.path.clone(),
        reason: reason.into(),
    };
    let storage = |e| CanonicalError::from(ReadError::Storage(e));
    let key = object.content_key(file)?;
    if backend.head(&key).await.map_err(storage)?.size > limit {
        return Err(refuse("the file is larger than its read bound"));
    }
    let chunk = limit.min(filesquid_ocfl::storage::MAX_CHUNK_BYTES);
    let stored = read_verified(backend, &key, chunk).await.map_err(storage)?;
    if stored.len() as u64 > limit {
        return Err(refuse("the file is larger than its read bound"));
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
