//! The metadata record of a copy (Task API-F2.2; API-S-011, API-S-026, SRS-F-062): a copy is a
//! new file with a new FileId and a new stored name, holding the same bytes as its source.
//!
//! Its record keeps what the owner sees and described: the original name and its form, the
//! display name, the extension, format and media type, and every statement. Its one import
//! entry is the source's latest entry rebound to the copy: the acquisition cites the copy's
//! object, version, stored path, digest and size, and records when the copy was made, while
//! the source file, volume, attributes and host facts stay as the source's acquisition
//! recorded them. The source itself is named by the asserted statement `dcterms:source`
//! ([`SOURCE_TERM`]), whose value is the source's object version `urn:uuid:<file-id>@vN`.
//! Pure.

use super::layout::file_object_id;
use super::original::original_path_of;
use super::CanonicalError;
use filesquid_core::FileId;
use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::{Acquisition, ByteStorage, ImportEntry};
use filesquid_ocfl::DigestAlgorithm;

/// The Dublin Core term naming the resource a copy derives from.
pub const SOURCE_TERM: &str = "source";

/// What one copy provides, besides the bytes.
#[derive(Clone, Copy, Debug)]
pub struct CopyInput<'a> {
    /// The source file's record at the version copied.
    pub source: &'a FileRecord,
    /// When the copy started (RFC 3339 UTC).
    pub started: &'a str,
    /// When the bytes were read and hashed (RFC 3339 UTC).
    pub finished: &'a str,
}

/// The record of `file_id`, a copy of the file `input.source` describes, whose bytes are
/// `bytes`. The `dcterms:source` statement is added by the caller, which holds the library's
/// namespace registry.
///
/// # Errors
/// [`CanonicalError::Encode`] when the source record has no import entry or the metadata
/// module refuses the new record.
pub fn copy_record(
    file_id: &FileId,
    input: &CopyInput<'_>,
    bytes: &[u8],
) -> Result<FileRecord, CanonicalError> {
    let source = input.source;
    let path = original_path_of(file_id, source)?;
    let entry = copy_entry(file_id, input, bytes, &path)?;
    let mut record = FileRecord::new(
        source.original_name().clone(),
        source.name_normalization(),
        source.extension().map(str::to_string),
        source.format().map(str::to_string),
        source.media_type().map(str::to_string),
        entry,
    )
    .map_err(refuse)?
    .with_statements(source.statements().clone());
    record
        .set_display_name(source.display_name())
        .map_err(refuse)?;
    Ok(record)
}

/// The value of a copy's `dcterms:source`: the source's object version
/// `urn:uuid:<file-id>@<version>`.
pub fn source_reference(source: &FileId, version: &str) -> String {
    format!("{}@{version}", file_object_id(source))
}

/// The source's latest import entry, rebound to the copy stored at `path`.
fn copy_entry(
    file_id: &FileId,
    input: &CopyInput<'_>,
    bytes: &[u8],
    path: &str,
) -> Result<ImportEntry, CanonicalError> {
    let mut entry =
        input.source.imports().last().cloned().ok_or_else(|| {
            CanonicalError::Encode("the source record has no import entry".into())
        })?;
    let not_a_job = || Fact::unavailable(Reason::NotSupportedBySource);
    entry.acquisition = Acquisition {
        recorded_at: input.finished.to_string(),
        started_at: input.started.to_string(),
        finished_at: input.finished.to_string(),
        method: entry.acquisition.method,
        storage: ByteStorage::Copied,
        job_id: not_a_job(),
        operation_id: not_a_job(),
        object_id: file_object_id(file_id),
        object_version: "v1".to_string(),
        logical_path: Some(path.to_string()),
        sha256: DigestAlgorithm::Sha256.digest_hex(bytes),
        size: bytes.len() as u64,
    };
    Ok(entry)
}

/// The canonical refusal of a record the metadata module refused.
fn refuse(err: filesquid_metadata::file_record::RecordError) -> CanonicalError {
    CanonicalError::Encode(err.to_string())
}
