//! Copy (Task API-F2.2; API-S-011, API-S-026, SRS-F-062): `POST /api/v1/files/copies` makes a
//! new file from the head of a file object, in the new file's own object.
//!
//! The copy gets a new FileId and therefore a new stored name `original/<new-uuid>.<ext>`; it
//! keeps the display name, the original name, the media type and every statement, and its
//! record names the source as `dcterms:source` = `urn:uuid:<source-id>@vN`
//! ([`crate::canonical::copy`]). The source is never changed. One set of bytes never gets
//! two home collections: the copy is a new file, and the host records its home collection in
//! its catalog.
//!
//! A copy of any size is streamed (Task API-F2.26; API-S-011, NFR-003, owner decision
//! 2026-10-31, verbatim: "build streamed copies"): the source's stored original is staged
//! into the new object's staging area through `ChangeModel::stage_file` (OCFL-S-026), as the
//! import stages a source (`FileService::import_original`), read in ranges of at most 8 MiB and written in
//! multipart parts of at most 64 MiB, so the file is never held in memory. The staged bytes
//! live in the library's staging backend, which a host keeps on disk; an in-memory staging
//! backend holds what it stages. The stage runs outside the library's write lock, which is
//! taken for the commit only; under it the copy rechecks that no earlier run of its operation
//! committed and that the source is still at the revision the caller read. Whatever a failed
//! attempt staged is cleared.

use super::error::FileError;
use super::provenance::{label_of, Caller, Committed, Write};
use super::service::{change_model, FileService, Library};
use super::statements::metadata_error;
use super::upload::{canonical_error, committed_before, mint_file_id, parse_file_id};
use super::view::UploadView;
use super::COPY_OPERATION;
use crate::canonical::{
    commit_staged_original, copy_record, file_object_id, file_object_root, read_file_original,
    source_reference, BoundMeta, CanonicalError, CopyInput, OwnerReadable, SOURCE_TERM,
};
use filesquid_core::{FileId, OperationKind};
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::namespace::NamespaceRegistry;
use filesquid_metadata::record::parse_curie_with;
use filesquid_ocfl::change::{ChangeError, ChangeModel};
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{StorageBackend, StorageError, StorageKey};
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Body of `POST /api/v1/files/copies`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CopyRequest {
    /// The library.
    pub library_id: String,
    /// The file to copy, at its head.
    pub file_id: String,
    /// Optional version message of the copy's first version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The destination collection, a collection of the library (API-S-011 **Destination**).
    /// Required: a copy without it is refused (`destination_required`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// The destination folder inside the collection, a collection logical path; the
    /// collection's root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// The file ID the copy's operation reserved, so a retry after a crash completes the same
    /// copy once. Set by the route, never read from or written to the wire.
    #[serde(skip)]
    pub reserved_file_id: Option<String>,
}

/// The copy a copy request made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CopyView {
    /// The library.
    pub library_id: String,
    /// The new file.
    pub file_id: String,
    /// The new file's object, `urn:uuid:<file-id>`.
    pub object_id: String,
    /// The new file's first version.
    pub version: String,
    /// When that version was created (RFC 3339 UTC).
    pub created: String,
    /// The source's object version, `urn:uuid:<file-id>@vN`: the record's `dcterms:source`.
    pub source: String,
    /// The copy's home collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// The copy's collection logical path: the one asked for, or the numbered path beside a
    /// taken one that it got (API-S-011 **Same logical path**).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The source of one copy: its head, its record there and where its original is stored.
struct Source {
    file_id: FileId,
    version: String,
    record: FileRecord,
    /// The storage key of the original's content in the library's storage root.
    key: StorageKey,
}

/// The staged content of one copy: its SHA-256 (lowercase hexadecimal) and size in bytes.
struct StagedContent {
    sha256: String,
    size: u64,
}

/// One copy on its way: what was asked, by whom, of which source, into which new file.
struct Attempt<'a> {
    request: &'a CopyRequest,
    caller: &'a Caller,
    label: Option<String>,
    registry: &'a NamespaceRegistry,
    source: &'a Source,
    file_id: &'a FileId,
    /// The revision of the source the caller read.
    expected: &'a str,
    /// When the copy started (RFC 3339 UTC).
    started: String,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// The display name a copy of the file `file_id` of `library_id` takes: its head
    /// record's. Storage I/O.
    ///
    /// # Errors
    /// `invalid_request` for a malformed file ID; `not_found` for an unknown library or file;
    /// a storage failure otherwise.
    pub async fn copy_name(&self, library_id: &str, file_id: &str) -> Result<String, FileError> {
        let file_id = parse_file_id(file_id)?;
        let library = self.library(library_id)?;
        let backend = library.files.backend.as_ref();
        let original = read_file_original(backend, &OwnerReadable, &file_id)
            .await
            .map_err(copy_error)?
            .ok_or_else(FileError::not_found)?;
        Ok(original.record.display_name().to_string())
    }

    /// Copy the file `request.file_id` at its head by `caller` into a new file, in one commit
    /// linked to the journal ([`super::Committed`]). The original is streamed whatever its
    /// size: staged through [`ChangeModel::stage_file`] (OCFL-S-026) outside the library's
    /// write lock, which is held for the commit only, and never held in memory (NFR-003).
    /// Storage I/O.
    ///
    /// # Errors
    /// `invalid_request` for a malformed file ID or message; `not_found` for an unknown
    /// library or file; `conflict` with `source_changed` in its message when the stored
    /// original changed while it was read, or `source_unreadable` when it could not be read;
    /// a storage failure otherwise. Nothing is published on any error, whatever the attempt
    /// staged is cleared, and the source never changes.
    ///
    /// `expected` is the source file object's head the caller read; any other, before the
    /// stage or at the commit, is a `conflict`, and nothing is copied.
    pub async fn copy(
        &self,
        request: CopyRequest,
        caller: impl Into<Caller>,
        expected: &str,
    ) -> Result<Committed<CopyView>, FileError> {
        let caller = caller.into();
        let source_id = parse_file_id(&request.file_id)?;
        let label = label_of(request.message.as_deref())?;
        let library = self.library(&request.library_id)?;
        let registry = self.registry(&request.library_id).await?;
        let backend = library.files.backend.as_ref();
        let file_id = match copy_target(backend, &request).await? {
            CopyTarget::New(file_id) => file_id,
            CopyTarget::Committed(view) => return Ok(resumed(view)),
        };
        let source = read_source(backend, source_id, expected).await?;
        let root = file_object_root(&file_id).map_err(copy_error)?;
        let object_id = file_object_id(&file_id);
        let staging = library.staging(&object_id)?;
        let model = change_model(&library, &root, &object_id, staging);
        let attempt = Attempt {
            request: &request,
            caller: &caller,
            label,
            registry: &registry,
            source: &source,
            file_id: &file_id,
            expected,
            started: self.created(),
        };
        let copied = self.stage_and_commit(&library, &model, &attempt).await;
        match &copied {
            Ok(_) => library.remember(&object_id, &root),
            Err(_) => clear_staged(&model).await,
        }
        copied
    }

    /// Stage the source of `attempt` into `model`'s staging area, then commit it under the
    /// write lock of `library`, once no earlier run committed the copy and the source is
    /// still at the revision the caller read. Storage I/O.
    ///
    /// # Errors
    /// As [`FileService::copy`]; the caller clears the staging area.
    async fn stage_and_commit(
        &self,
        library: &Library<B, R, S>,
        model: &ChangeModel<'_, B, R, S>,
        attempt: &Attempt<'_>,
    ) -> Result<Committed<CopyView>, FileError> {
        let backend = library.files.backend.as_ref();
        let source = attempt.source;
        let path = crate::canonical::original::original_path_of(attempt.file_id, &source.record)
            .map_err(copy_error)?;
        let staged = stage_source(model, &path, backend, &source.key).await?;
        let _writing = library.writes.lock().await;
        if let CopyTarget::Committed(view) = copy_target(backend, attempt.request).await? {
            clear_staged(model).await;
            return Ok(resumed(view));
        }
        check_source_head(backend, &source.file_id, attempt.expected).await?;
        self.commit_copy(model, attempt, &staged).await
    }

    /// Commit the staged content `staged` of `attempt` with the copy's record as the first
    /// version of the new file object `model` writes. Storage I/O.
    ///
    /// # Errors
    /// A record the metadata module refuses; as [`commit_staged_original`].
    async fn commit_copy(
        &self,
        model: &ChangeModel<'_, B, R, S>,
        attempt: &Attempt<'_>,
        staged: &StagedContent,
    ) -> Result<Committed<CopyView>, FileError> {
        let (request, source, file_id) = (attempt.request, attempt.source, attempt.file_id);
        let reference = source_reference(&source.file_id, &source.version);
        let record = self.copied_record(attempt, staged, &reference)?;
        let object_id = file_object_id(file_id);
        let prepared = self.prepare(&Write {
            caller: attempt.caller,
            kind: OperationKind::Copy,
            label: attempt.label.clone(),
            operation: Some(COPY_OPERATION),
            library_id: &request.library_id,
            object_id: &object_id,
            metadata: false,
            moves: BTreeMap::new(),
        })?;
        let meta = BoundMeta {
            meta: prepared.meta(),
            bound: prepared.bound(),
        };
        let content = (staged.sha256.as_str(), staged.size);
        let receipt = commit_staged_original(model, file_id, &record, content, meta)
            .await
            .map_err(copy_error)?;
        let view = CopyView {
            library_id: request.library_id.clone(),
            file_id: file_id.as_str().to_string(),
            object_id: receipt.object_id.clone(),
            version: receipt.version.to_string(),
            created: receipt.created.clone(),
            source: reference,
            collection_id: None,
            path: None,
        };
        Ok(prepared.finish(view, &receipt))
    }

    /// The record of the copy `attempt` makes, citing the staged content `staged`: the
    /// source's record rebound to the copy, naming the source `reference` as its asserted
    /// `dcterms:source`, finished at this service's clock. Pure but for the clock.
    fn copied_record(
        &self,
        attempt: &Attempt<'_>,
        staged: &StagedContent,
        reference: &str,
    ) -> Result<FileRecord, FileError> {
        let registry = attempt.registry;
        let input = CopyInput {
            source: &attempt.source.record,
            started: &attempt.started,
            finished: &self.created(),
        };
        let content = (staged.sha256.as_str(), staged.size);
        let mut record = copy_record(attempt.file_id, &input, content).map_err(copy_error)?;
        let (namespace, _) = parse_curie_with(&format!("dcterms:{SOURCE_TERM}"), registry)
            .map_err(metadata_error)?;
        record
            .statements_mut()
            .set_registered(registry, namespace, SOURCE_TERM, reference.to_string())
            .map_err(metadata_error)?;
        Ok(record)
    }
}

/// The answer of a copy an earlier run of the same operation committed: nothing is written,
/// so it has no journal entries of its own. Pure.
fn resumed(view: CopyView) -> Committed<CopyView> {
    Committed {
        view,
        entries: Vec::new(),
    }
}

/// The head record of the file `file_id` and the storage key of its original's content, once
/// its head is `expected`: a stale revision is refused before any byte is read. Storage I/O.
///
/// # Errors
/// `not_found` for an unknown file; `conflict` for a head that is not `expected`; a storage
/// failure otherwise.
async fn read_source<B: StorageBackend>(
    backend: &B,
    file_id: FileId,
    expected: &str,
) -> Result<Source, FileError> {
    let original = read_file_original(backend, &OwnerReadable, &file_id)
        .await
        .map_err(copy_error)?
        .ok_or_else(FileError::not_found)?;
    let root = file_object_root(&file_id).map_err(copy_error)?;
    let object = Object::open(backend, &root).await?;
    let head = object.head().clone();
    if head.to_string() != expected {
        return Err(stale_source());
    }
    let state = object.state(&head)?;
    let file = state
        .get(&original.path)
        .ok_or_else(|| ReadError::PathNotFound {
            version: head.to_string(),
            path: original.path.clone(),
        })?;
    let key = object.content_key(file)?;
    Ok(Source {
        file_id,
        version: head.to_string(),
        record: original.record,
        key,
    })
}

/// Refuse unless the head of the file object of `file_id` is still `expected`: a source that
/// moved on while its copy was staged is not copied. Storage I/O.
///
/// # Errors
/// `conflict` for another head; a storage failure otherwise.
async fn check_source_head<B: StorageBackend>(
    backend: &B,
    file_id: &FileId,
    expected: &str,
) -> Result<(), FileError> {
    let root = file_object_root(file_id).map_err(copy_error)?;
    let object = Object::open(backend, &root).await?;
    match object.head().to_string() == expected {
        true => Ok(()),
        false => Err(stale_source()),
    }
}

/// Stage the stored content at `key` of `source` as the logical file `path` of `model`'s
/// object, after clearing what an earlier attempt left staged, and answer its SHA-256 and
/// size: the size the content has before and after the stage. The content is read in ranges
/// of at most 8 MiB, at most twice, and written in multipart parts of at most 64 MiB
/// (OCFL-S-026). Storage I/O.
///
/// # Errors
/// `conflict` with `source_changed` or `source_unreadable` in its message (see
/// [`FileService::copy`]); a storage failure otherwise.
async fn stage_source<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    path: &str,
    source: &B,
    key: &StorageKey,
) -> Result<StagedContent, FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    model.reset().await?;
    let before = source.head(key).await.map_err(source_unreadable)?;
    let sha256 = model
        .stage_file(path, source, key.as_str())
        .await
        .map_err(|err| stage_error(err, key))?;
    let after = source.head(key).await.map_err(source_unreadable)?;
    match after == before {
        true => Ok(StagedContent {
            sha256,
            size: before.size,
        }),
        false => Err(source_changed()),
    }
}

/// Clear what a copy that failed, or that an earlier run had already committed, left in its
/// object's staging area; a failure to clear is logged (the next attempt clears it first).
/// Storage I/O.
async fn clear_staged<B, R, S>(model: &ChangeModel<'_, B, R, S>)
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    if let Err(err) = model.reset().await {
        tracing::warn!(error = %err, "a copy's staging area could not be cleared");
    }
}

/// The refusal of a copy whose stored original changed while it was read. Pure.
fn source_changed() -> FileError {
    FileError::conflict(
        "source_changed: the file changed while it was read, so nothing was copied; try again.",
    )
}

/// The refusal of a copy whose stored original could not be read; the storage error is
/// logged, never answered, since it names a storage key. Pure but for the log.
fn source_unreadable(err: StorageError) -> FileError {
    tracing::warn!(error = %err, "the stored original of a copy's source could not be read");
    FileError::conflict(
        "source_unreadable: the file could not be read, so nothing was copied; try again.",
    )
}

/// The error of a failed stage: a source that changed between its reads is
/// [`source_changed`], a failed read of the source at `key` is [`source_unreadable`], anything
/// else (the staging area's own failures) as the change model's error. Pure but for the log.
fn stage_error(err: ChangeError, key: &StorageKey) -> FileError {
    match err {
        ChangeError::SourceChanged { .. } => source_changed(),
        ChangeError::Storage(storage) if names_key(&storage, key) => source_unreadable(storage),
        other => other.into(),
    }
}

/// Whether `err` is about `key` (a read of the source, rather than of the staging area, whose
/// keys are the area's own). Pure.
fn names_key(err: &StorageError, key: &StorageKey) -> bool {
    match err {
        StorageError::NotFound { key: named }
        | StorageError::Io { key: named, .. }
        | StorageError::InvalidKey { key: named, .. } => named == key.as_str(),
        _ => false,
    }
}

/// The refusal of a copy whose source moved past the revision the caller read.
fn stale_source() -> FileError {
    FileError::conflict(
        "The file changed since it was read, so nothing was copied; reload and try again.",
    )
}

/// The file error of a canonical refusal or failure of a copy.
fn copy_error(err: CanonicalError) -> FileError {
    match err {
        CanonicalError::Encode(reason) | CanonicalError::InvalidIdentifier(reason) => {
            FileError::invalid(format!("The copy was refused: {reason}."))
        }
        other => canonical_error(other),
    }
}

/// The answer of a copy an earlier, unfinished run of the same operation already committed:
/// its object's head, citing the source its record names (`dcterms:source`). Storage I/O.
///
/// # Errors
/// `internal` for a committed copy whose record names no source; a storage failure.
async fn resumed_copy<B: StorageBackend>(
    backend: &B,
    done: UploadView,
) -> Result<CopyView, FileError> {
    let file_id = parse_file_id(&done.file_id)?;
    let original = read_file_original(backend, &OwnerReadable, &file_id)
        .await
        .map_err(copy_error)?
        .ok_or_else(|| FileError::internal("a committed copy has no record"))?;
    let source = original
        .record
        .statements()
        .get(&format!("dcterms:{SOURCE_TERM}"))
        .map(|statement| statement.value.clone())
        .ok_or_else(|| FileError::internal("a committed copy names no source"))?;
    Ok(CopyView {
        library_id: done.library_id,
        file_id: done.file_id,
        object_id: done.object_id,
        version: done.version,
        created: done.created,
        source,
        collection_id: None,
        path: None,
    })
}

/// The file a copy writes.
enum CopyTarget {
    /// A file not committed yet: its reserved ID, or a freshly minted one.
    New(FileId),
    /// The copy an earlier, unfinished run of the same operation already committed, answered
    /// as it was made (its source is the one its record names), so a retry completes it even
    /// when the source changed since.
    Committed(CopyView),
}

/// The file `request` writes: what its operation's reserved ID already committed, else that
/// ID, else a freshly minted one. Storage I/O.
///
/// # Errors
/// `invalid_request` for a reserved ID that is not a UUID v4; a storage failure.
async fn copy_target<B: StorageBackend>(
    backend: &B,
    request: &CopyRequest,
) -> Result<CopyTarget, FileError> {
    let reserved = request
        .reserved_file_id
        .as_deref()
        .map(parse_file_id)
        .transpose()?;
    let committed = committed_before(backend, &request.library_id, reserved.as_ref()).await?;
    match (committed, reserved) {
        (Some(done), _) => resumed_copy(backend, done).await.map(CopyTarget::Committed),
        (None, Some(id)) => Ok(CopyTarget::New(id)),
        (None, None) => mint_file_id().map(CopyTarget::New),
    }
}
