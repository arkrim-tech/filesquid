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
use super::service::{change_model, FileService};
use super::upload::{canonical_error, parse_file_id};
use super::view::UploadView;
use crate::canonical::original::original_path_for;
use crate::canonical::{
    commit_staged_original, file_object_id, file_object_root, read_file_original, OwnerReadable,
};
use filesquid_core::FileId;
use filesquid_metadata::fact::Fact;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::attributes::Attributes;
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, SourceFile, SourceVolume,
};
use filesquid_ocfl::change::{ChangeError, ChangeModel};
use filesquid_ocfl::storage::{StorageBackend, StorageKey};
use filesquid_ocfl::write::VersionReservations;

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
    /// original is staged through [`ChangeModel::stage_file`] (OCFL-S-026): read in bounded
    /// chunks at most twice, once to hash it and once to copy it while hashing again, so no
    /// file is held in memory whatever its size (Task DE-1.9).
    ///
    /// # Errors
    /// `invalid_request` with `size_mismatch` in its message when the source is not the size
    /// it reported, for a malformed file ID or key, or for a record the metadata module
    /// refuses; `conflict` with `source_changed` in its message when the source changed
    /// between its reads; `not_found` for an unknown library; `conflict` when the object
    /// already exists and holds no import of this job; a storage failure otherwise. Nothing
    /// is published on any error, and no message names the source.
    pub async fn import_original<D: StorageBackend>(
        &self,
        import: SourceImport,
        source: &D,
        key: &str,
        actor: &str,
    ) -> Result<UploadView, FileError> {
        let meta = self.meta(actor, None)?;
        let library = self.library(&import.library_id)?;
        let _writing = library.writes.lock().await;
        let backend = library.files.backend.as_ref();
        let file_id = parse_file_id(&import.file_id)?;
        let existing = read_file_original(backend, &OwnerReadable, &file_id)
            .await
            .map_err(canonical_error)?;
        if let Some(original) = existing {
            return replayed(&import, &original.record);
        }
        let root = file_object_root(&file_id).map_err(canonical_error)?;
        let object_id = file_object_id(&file_id);
        let path = original_path_for(&file_id, import.source.extension.as_deref())
            .map_err(canonical_error)?;
        let staging = library.staging(&object_id)?;
        let model = change_model(&library, &root, &object_id, staging);
        let (sha256, size) = stage_source(&model, &path, &import.source, (source, key)).await?;
        let citation = Citation {
            object_id,
            version: "v1".to_string(),
            path,
            sha256,
            size,
        };
        let library_id = import.library_id.clone();
        let entry = import_entry(import, &citation, &self.created());
        let record = import_record(entry)?;
        let content = (citation.sha256.as_str(), citation.size);
        let receipt = commit_staged_original(&model, &file_id, &record, content, meta)
            .await
            .map_err(canonical_error)?;
        library.remember(&citation.object_id, &root);
        Ok(view_of(library_id, &file_id, receipt))
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
    let before = source.head(&checked).await?;
    check_size(reported, before.size)?;
    let digest = model
        .stage_file(path, source, key)
        .await
        .map_err(stage_error)?;
    let after = source.head(&checked).await?;
    match after == before {
        true => Ok((digest, before.size)),
        false => Err(source_changed()),
    }
}

/// The refusal of a source that changed while it was staged. Pure.
fn source_changed() -> FileError {
    FileError::conflict("source_changed: the source changed while it was read; import it again.")
}

/// The error of a failed stage: a source that changed between its reads is
/// [`source_changed`], anything else as the change model's error. Pure.
fn stage_error(err: ChangeError) -> FileError {
    match err {
        ChangeError::SourceChanged { .. } => source_changed(),
        other => other.into(),
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
