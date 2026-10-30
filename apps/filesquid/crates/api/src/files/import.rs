//! The import write of a host that reads source files itself (Task DE-1.2; F-002, MD-S-013,
//! ADR 038 Decisions 1 and 3): the same per-file object as an upload (API-S-026), the
//! original at `original/<file-uuid>.<ext>` and its metadata record beside it in one version,
//! with an import entry built from every fact the host's operating system provided about the
//! source, gathered before the host read its bytes.
//!
//! The host supplies the facts ([`SourceImport`]), the new file's identifier and a read-only
//! storage view of the source, which is staged through the streamed
//! `ChangeModel::stage_file` (OCFL-S-026, Task DE-1.9); this module adds what only the commit
//! knows (the object, version, logical path, SHA-256 and verified size) and refuses a source
//! whose reported size is not its size, or which changed while it was staged, so a file that
//! changed while it was read is never recorded as verified.
//!
//! An import always creates a new file object, under the identifier the host minted when it
//! planned the import. Repeating the import of the same job (a run that crashed between the
//! commit and its checkpoint) answers the object that job already committed instead of
//! creating a second one; an object of another job or an upload under that identifier is a
//! conflict.

use super::error::FileError;
use super::media;
use super::provenance::{Caller, Committed, Prepared, Write};
use super::service::{change_model, FileService, Library};
use super::upload::{canonical_error, parse_file_id};
use super::view::UploadView;
use crate::canonical::original::original_path_for;
use crate::canonical::{
    commit_staged_original, file_object_id, file_object_root, read_file_original, BoundMeta,
    OwnerReadable,
};
use filesquid_core::{FileId, OperationId, OperationKind};
use filesquid_metadata::fact::Fact;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::attributes::Attributes;
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, SourceFile, SourceVolume,
};
use filesquid_ocfl::change::{ChangeError, ChangeModel};
use filesquid_ocfl::storage::{StorageBackend, StorageError, StorageKey};
use filesquid_ocfl::write::VersionReservations;
use std::collections::BTreeMap;

/// What a host gathered about one source file before reading it.
#[derive(Clone, Debug)]
pub struct SourceImport {
    /// The library to import into.
    pub library_id: String,
    /// The new file's identifier (a UUID v4 the host minted when it planned the import), so
    /// a repeated import of the same job finds the object it already committed.
    pub file_id: String,
    /// How the file reached FileSquid.
    pub method: ImportMethod,
    /// The import job, when one runs the import.
    pub job_id: Option<String>,
    /// The operation the import belongs to.
    pub operation_id: Option<String>,
    /// When the host started importing this file (RFC 3339 UTC).
    pub started_at: String,
    /// The source file's facts; its `verified_size` is set here from the bytes.
    pub source: SourceFile,
    /// The source volume's facts.
    pub volume: SourceVolume,
    /// The source file's extended attributes, or why they are unavailable.
    pub attributes: Fact<Attributes>,
    /// The importing host.
    pub host: Host,
}

/// The citation of an import: the file's object, version, stored path and the content.
struct Citation {
    object_id: String,
    version: String,
    path: String,
    sha256: String,
    size: u64,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Commit one import of the file at `key` of `source`, the host's read-only view of the
    /// source `import` describes, by `actor`, as the new file object of `import.file_id`; when
    /// that object already holds an import of the same job, answer it without writing. The
    /// original is staged through [`ChangeModel::stage_file`] (OCFL-S-026): read in chunks of
    /// at most 8 MiB, at most twice, once to hash it and once to copy it while hashing again.
    /// The stage's own buffers are bounded whatever the file's size; the staged blob lives in
    /// the library's staging backend, which a host keeps on disk (`FsBackend`) or in a remote
    /// store for large files, since a `MemoryBackend` staging area holds it in memory (Task
    /// DE-1.9). The stage runs in the new object's own staging area outside the library's
    /// write lock, which is taken for the commit only; whatever an attempt staged is cleared
    /// when it fails.
    ///
    /// The commit is linked to the journal like every file write ([`super::Committed`]): kind
    /// `import` under the import job's operation ID, with the commit's entries (`ocfl.commit`)
    /// for the job to journal beside its own `import.file` entry. An import an earlier run
    /// already committed answers no entries (API-S-036, API-S-028).
    ///
    /// # Errors
    /// `invalid_request` with `size_mismatch` in its message when the source is not the size
    /// it reported, for a malformed file ID or key, or for a record the metadata module
    /// refuses; `conflict` with `source_changed` in its message when the source changed
    /// between its reads, or `source_unreadable` when it could not be read; `not_found` for an
    /// unknown library; `conflict` when the object already exists and holds no import of this
    /// job; a storage failure otherwise. Nothing is published on any error, and no message
    /// names the source.
    pub async fn import_original<D: StorageBackend>(
        &self,
        import: SourceImport,
        source: &D,
        key: &str,
        actor: &str,
    ) -> Result<Committed<UploadView>, FileError> {
        let caller = import_caller(actor, &import);
        let library = self.library(&import.library_id)?;
        let file_id = parse_file_id(&import.file_id)?;
        if let Some(answer) = imported_before(&library, &import, &file_id).await? {
            return Ok(unjournaled(answer));
        }
        let root = file_object_root(&file_id).map_err(canonical_error)?;
        let object_id = file_object_id(&file_id);
        let path = original_path_for(&file_id, import.source.extension.as_deref())
            .map_err(canonical_error)?;
        let staging = library.staging(&object_id)?;
        let model = change_model(&library, &root, &object_id, staging);
        let imported = async {
            let staged = stage_source(&model, &path, &import.source, (source, key)).await?;
            let created = self.created();
            let _writing = library.writes.lock().await;
            if let Some(answer) = imported_before(&library, &import, &file_id).await? {
                clear_staged(&model).await;
                return Ok(unjournaled(answer));
            }
            let prepared = self.prepare(&Write {
                caller: &caller,
                kind: OperationKind::Import,
                label: None,
                operation: None,
                library_id: &import.library_id,
                object_id: &object_id,
                metadata: false,
                moves: BTreeMap::new(),
            })?;
            let citation = Citation {
                object_id: object_id.clone(),
                version: "v1".to_string(),
                path: path.clone(),
                sha256: staged.0,
                size: staged.1,
            };
            let committed =
                commit_import(&model, import, &file_id, &citation, (&created, prepared));
            let answer = committed.await?;
            library.remember(&object_id, &root);
            Ok(answer)
        }
        .await;
        if imported.is_err() {
            clear_staged(&model).await;
        }
        imported
    }
}

/// The caller of an import by `actor`: the import job's operation ID when it is a usable
/// one, else none, so the commit gets a new operation ID. Pure.
fn import_caller(actor: &str, import: &SourceImport) -> Caller {
    let operation = import
        .operation_id
        .as_deref()
        .and_then(|id| OperationId::new(id).ok());
    match operation {
        Some(id) => Caller::new(actor).with_operation(id),
        None => Caller::new(actor),
    }
}

/// The answer of an import an earlier run already committed: nothing new is committed, so
/// there is nothing to journal for the commit. Pure.
fn unjournaled(view: UploadView) -> Committed<UploadView> {
    Committed {
        view,
        entries: Vec::new(),
    }
}

/// The answer of an import whose file object already exists in `library`: see [`replayed`].
/// `None` when it does not exist yet. Storage I/O.
///
/// # Errors
/// As [`replayed`]; a read failure.
async fn imported_before<B, R, S>(
    library: &Library<B, R, S>,
    import: &SourceImport,
    file_id: &FileId,
) -> Result<Option<UploadView>, FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let backend = library.files.backend.as_ref();
    let existing = read_file_original(backend, &OwnerReadable, file_id)
        .await
        .map_err(canonical_error)?;
    existing
        .map(|original| replayed(import, &original.record))
        .transpose()
}

/// Build the record of `import` citing the staged original `citation`, recorded at `created`,
/// and commit it with the original as the new object of `file_id`, with `meta`. Storage I/O.
///
/// # Errors
/// A record the metadata module refuses; as [`commit_staged_original`].
async fn commit_import<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    import: SourceImport,
    file_id: &FileId,
    citation: &Citation,
    (created, prepared): (&str, Prepared),
) -> Result<Committed<UploadView>, FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let library_id = import.library_id.clone();
    let record = import_record(import_entry(import, citation, created))?;
    let content = (citation.sha256.as_str(), citation.size);
    let meta = BoundMeta {
        meta: prepared.meta(),
        bound: prepared.bound(),
    };
    let receipt = commit_staged_original(model, file_id, &record, content, meta)
        .await
        .map_err(canonical_error)?;
    let view = view_of(library_id, file_id, receipt.clone());
    Ok(prepared.finish(view, &receipt))
}

/// Clear what a failed import left in its object's staging area, so a quarantined import
/// keeps no copy of its source; a failure to clear is logged (the next attempt clears it
/// first). Storage I/O.
async fn clear_staged<B, R, S>(model: &ChangeModel<'_, B, R, S>)
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    if let Err(err) = model.reset().await {
        tracing::warn!(error = %err, "a failed import's staging area could not be cleared");
    }
}

/// Stage the file at `key` of `source` as the logical file `path` of `model`'s object, after
/// clearing what an earlier attempt left staged, and answer its SHA-256 and size: the size
/// the source has before and after the stage, which must be the size it `reported`. Storage
/// I/O.
///
/// # Errors
/// `size_mismatch` or `source_changed` (see [`FileService::import_original`]); a storage
/// failure otherwise.
async fn stage_source<B, R, S, D>(
    model: &ChangeModel<'_, B, R, S>,
    path: &str,
    reported: &SourceFile,
    (source, key): (&D, &str),
) -> Result<(String, u64), FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
    D: StorageBackend,
{
    let checked =
        StorageKey::new(key).map_err(|_| FileError::invalid("The source key is not valid."))?;
    model.reset().await?;
    let before = source.head(&checked).await.map_err(source_unreadable)?;
    check_size(reported, before.size)?;
    let digest = model
        .stage_file(path, source, key)
        .await
        .map_err(|err| stage_error(err, &checked))?;
    let after = source.head(&checked).await.map_err(source_unreadable)?;
    match after == before {
        true => Ok((digest, before.size)),
        false => Err(source_changed()),
    }
}

/// The refusal of a source that changed while it was staged. Pure.
fn source_changed() -> FileError {
    FileError::conflict("source_changed: the source changed while it was read; import it again.")
}

/// The refusal of a source that could not be read (gone, or unreadable), whose retry may
/// succeed; the storage error is logged, never answered, since it names the key only. Pure
/// but for the log.
fn source_unreadable(err: StorageError) -> FileError {
    tracing::debug!(error = %err, "an import source could not be read");
    FileError::conflict("source_unreadable: the source could not be read; import it again.")
}

/// The error of a failed stage: a source that changed between its reads is
/// [`source_changed`], a failed read of the source `source_key` is [`source_unreadable`],
/// anything else as the change model's error. Pure but for the log.
fn stage_error(err: ChangeError, source_key: &StorageKey) -> FileError {
    match err {
        ChangeError::SourceChanged { .. } => source_changed(),
        ChangeError::Storage(storage) if names_key(&storage, source_key) => {
            source_unreadable(storage)
        }
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

/// The answer of a committed import. Pure.
fn view_of(
    library_id: String,
    file_id: &FileId,
    receipt: filesquid_ocfl::write::CommitReceipt,
) -> UploadView {
    UploadView {
        library_id,
        file_id: file_id.as_str().to_string(),
        object_id: receipt.object_id,
        version: receipt.version.to_string(),
        created: receipt.created,
    }
}

/// The answer of an import whose object already exists: the version its import entry of
/// `import`'s job cites, or a conflict when no entry of that job is in `record`. Pure.
fn replayed(import: &SourceImport, record: &FileRecord) -> Result<UploadView, FileError> {
    let job = import
        .job_id
        .clone()
        .ok_or_else(|| FileError::conflict("That file already exists."))?;
    let entry = record
        .imports()
        .iter()
        .find(|entry| entry.acquisition.job_id == Fact::value(job.clone()))
        .ok_or_else(|| FileError::conflict("That file already exists."))?;
    Ok(UploadView {
        library_id: import.library_id.clone(),
        file_id: import.file_id.clone(),
        object_id: entry.acquisition.object_id.clone(),
        version: entry.acquisition.object_version.clone(),
        created: entry.acquisition.recorded_at.clone(),
    })
}

/// Refuse a source of `size` bytes when that is not the size it reported (MD-S-013: the import
/// fails verification and commits nothing). Pure.
fn check_size(source: &SourceFile, size: u64) -> Result<(), FileError> {
    match &source.reported_size {
        Fact::Value(reported) if *reported != size => Err(FileError::invalid(
            "size_mismatch: the source changed size while it was read; import it again.",
        )),
        _ => Ok(()),
    }
}

/// The import entry of `import` citing `citation`, recorded at `now`. Pure.
fn import_entry(import: SourceImport, citation: &Citation, now: &str) -> ImportEntry {
    let optional = |value: Option<String>| match value {
        Some(text) => Fact::value(text),
        None => Fact::unavailable_because(
            filesquid_metadata::fact::Reason::NotSupportedBySource,
            "not run as a job",
        ),
    };
    let mut source = import.source;
    source.verified_size = citation.size;
    ImportEntry {
        acquisition: Acquisition {
            recorded_at: now.to_string(),
            started_at: import.started_at,
            finished_at: now.to_string(),
            method: import.method,
            storage: ByteStorage::Copied,
            job_id: optional(import.job_id),
            operation_id: optional(import.operation_id),
            object_id: citation.object_id.clone(),
            object_version: citation.version.clone(),
            logical_path: Some(citation.path.clone()),
            sha256: citation.sha256.clone(),
            size: citation.size,
        },
        source,
        volume: import.volume,
        attributes: import.attributes,
        host: import.host,
    }
}

/// The first record of the new file an import creates.
///
/// # Errors
/// `invalid_request` for an entry or record the metadata module refuses.
fn import_record(entry: ImportEntry) -> Result<FileRecord, FileError> {
    let refuse = |err: filesquid_metadata::file_record::RecordError| {
        FileError::invalid(format!("The import was refused: {err}."))
    };
    let source = &entry.source;
    let name = String::from_utf8_lossy(source.original_name.as_bytes()).into_owned();
    FileRecord::new(
        source.original_name.clone(),
        source.name_normalization,
        source.extension.clone(),
        None,
        Some(media::media_type(&name).to_string()),
        entry,
    )
    .map_err(refuse)
}
