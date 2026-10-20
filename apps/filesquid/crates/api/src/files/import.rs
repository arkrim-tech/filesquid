//! The import write of a host that reads source files itself (Task DE-1.2; F-002, MD-S-013,
//! ADR 038 Decisions 1 and 3): the same per-file object as an upload (API-S-026), the
//! original at `original/<file-uuid>.<ext>` and its metadata record beside it in one version,
//! with an import entry built from every fact the host's operating system provided about the
//! source, gathered before the host read its bytes.
//!
//! The host supplies the facts ([`SourceImport`]) and the bytes it read; this module adds what
//! only the commit knows (the file's identifier, object, version, logical path, SHA-256 and
//! verified size) and refuses a source whose reported size is not the size of the bytes read,
//! so a file that changed while it was read is never recorded as verified.

use super::error::FileError;
use super::media;
use super::service::{change_model, FileService};
use super::upload::{canonical_error, upload_target};
use super::view::UploadView;
use crate::canonical::original::{original_path_for, original_path_of};
use crate::canonical::{commit_file_original, OwnerReadable};
use filesquid_metadata::fact::Fact;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::attributes::Attributes;
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, SourceFile, SourceVolume,
};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;
use filesquid_ocfl::DigestAlgorithm;

/// What a host gathered about one source file before reading it.
#[derive(Clone, Debug)]
pub struct SourceImport {
    /// The library to import into.
    pub library_id: String,
    /// The existing file whose new content this is (a re-import), or `None` for a new file.
    pub file_id: Option<String>,
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
    /// Commit one import of `bytes`, read by the host from the source `import` describes, by
    /// `actor`: a new file object, or a new version of the object of `import.file_id`.
    ///
    /// # Errors
    /// `invalid_request` with `size_mismatch` in its message when the bytes are not the
    /// size the source reported, or for a record the metadata module refuses; `not_found`
    /// for an unknown library or file; `conflict` when another writer moved the head; a
    /// storage failure otherwise. Nothing is published on any error.
    pub async fn import_original(
        &self,
        import: SourceImport,
        bytes: &[u8],
        actor: &str,
    ) -> Result<UploadView, FileError> {
        check_size(&import.source, bytes)?;
        let meta = self.meta(actor, None)?;
        let library = self.library(&import.library_id)?;
        let _writing = library.writes.lock().await;
        let backend = library.files.backend.as_ref();
        let target = upload_target(backend, import.file_id.as_deref()).await?;
        let path = match &target.previous {
            Some(previous) => original_path_of(&target.file_id, previous),
            None => original_path_for(&target.file_id, stored_extension(&import.source)),
        }
        .map_err(canonical_error)?;
        let citation = Citation {
            object_id: target.object_id.clone(),
            version: target.next_version.clone(),
            path,
            sha256: DigestAlgorithm::Sha256.digest_hex(bytes),
            size: bytes.len() as u64,
        };
        let library_id = import.library_id.clone();
        let entry = import_entry(import, &citation, &self.created());
        let record = import_record(target.previous.as_ref(), entry)?;
        let staging = library.staging(&target.object_id)?;
        let model = change_model(&library, &target.root, &target.object_id, staging);
        let committed = commit_file_original(
            &model,
            &OwnerReadable,
            &target.file_id,
            &record,
            target.previous.as_ref(),
            bytes,
            meta,
        )
        .await
        .map_err(canonical_error)?;
        let receipt = committed.ok_or_else(|| {
            FileError::conflict("The import would change nothing, so no version was made.")
        })?;
        library.remember(&target.object_id, &target.root);
        Ok(UploadView {
            library_id,
            file_id: target.file_id.as_str().to_string(),
            object_id: receipt.object_id,
            version: receipt.version.to_string(),
            created: receipt.created,
        })
    }
}

/// Refuse bytes whose size is not the size the source reported (MD-S-013: the import fails
/// verification and commits nothing). Pure.
fn check_size(source: &SourceFile, bytes: &[u8]) -> Result<(), FileError> {
    match &source.reported_size {
        Fact::Value(reported) if *reported != bytes.len() as u64 => Err(FileError::invalid(
            "size_mismatch: the source changed size while it was read; import it again.",
        )),
        _ => Ok(()),
    }
}

/// The extension the stored original keeps: the source's, when it is 1 to 16 ASCII letters
/// or digits. Pure.
fn stored_extension(source: &SourceFile) -> Option<&str> {
    let extension = source.extension.as_deref()?;
    let plain =
        (1..=16).contains(&extension.len()) && extension.bytes().all(|b| b.is_ascii_alphanumeric());
    plain.then_some(extension)
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

/// The record an import commits: the first record of a new file, or the previous record with
/// one more import entry.
///
/// # Errors
/// `invalid_request` for an entry or record the metadata module refuses.
fn import_record(
    previous: Option<&FileRecord>,
    entry: ImportEntry,
) -> Result<FileRecord, FileError> {
    let refuse = |err: filesquid_metadata::file_record::RecordError| {
        FileError::invalid(format!("The import was refused: {err}."))
    };
    if let Some(previous) = previous {
        let mut next = previous.clone();
        next.append_import(entry).map_err(refuse)?;
        return Ok(next);
    }
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
