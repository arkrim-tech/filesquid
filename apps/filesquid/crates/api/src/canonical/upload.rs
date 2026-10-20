//! The metadata record of an upload (API-S-026; MD-S-013; ADR 038 Decision 3): the facts the
//! client provides (name and its normalisation form, size, last-modified time, media type),
//! the facts the receiving host knows, and every other fact listed as unavailable. The
//! verified size is the length of the bytes received and hashed; an upload whose declared
//! size differs fails verification, so no record is built and nothing is committed. Pure.

use super::layout::file_object_id;
use super::original::{original_path_for, original_path_of};
use super::CanonicalError;
use filesquid_core::FileId;
use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::file_record::{extension_of, FileRecord};
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, NameForm, UploadFacts,
};
use filesquid_metadata::raw::Raw;
use filesquid_ocfl::DigestAlgorithm;

/// The distribution the API records as the importing software.
const DISTRIBUTION: &str = "filesquid";

/// What one upload provides, besides its bytes.
#[derive(Clone, Copy, Debug)]
pub struct UploadInput<'a> {
    /// The file name the client sent.
    pub name: &'a str,
    /// The normalisation form of that name.
    pub name_normalization: NameForm,
    /// The size the client declared, in bytes.
    pub declared_size: u64,
    /// The media type to record (the client's, or derived from the name).
    pub media_type: &'a str,
    /// The last-modified time the client reported (RFC 3339 UTC).
    pub last_modified: Option<&'a str>,
    /// The operation the upload belongs to, when the caller names one.
    pub operation_id: Option<&'a str>,
    /// When the upload started (RFC 3339 UTC).
    pub started: &'a str,
    /// When the bytes were received and hashed (RFC 3339 UTC).
    pub finished: &'a str,
    /// The OCFL version the upload will publish (`v1` for a new file).
    pub object_version: &'a str,
}

/// The record of the first upload of `file_id`: its original name and extension, the media
/// type and one import entry citing `bytes` at the original's path.
///
/// # Errors
/// [`CanonicalError::Encode`] when the name, media type, a time or the sizes are refused
/// (a declared size that is not the verified size among them).
pub fn new_upload_record(
    file_id: &FileId,
    input: &UploadInput<'_>,
    bytes: &[u8],
) -> Result<FileRecord, CanonicalError> {
    let extension = extension_of(input.name).map(str::to_string);
    let path = original_path_for(file_id, extension.as_deref())?;
    FileRecord::new(
        Raw::from_bytes(input.name.as_bytes()),
        input.name_normalization,
        extension,
        None,
        Some(input.media_type.to_string()),
        upload_entry(file_id, input, bytes, &path),
    )
    .map_err(refuse)
}

/// `previous` with one more import entry: a later upload of new bytes for the same file,
/// stored at the same path. The original name, display name and statements stay.
///
/// # Errors
/// As [`new_upload_record`].
pub fn reupload_record(
    file_id: &FileId,
    previous: &FileRecord,
    input: &UploadInput<'_>,
    bytes: &[u8],
) -> Result<FileRecord, CanonicalError> {
    let path = original_path_of(file_id, previous)?;
    let mut next = previous.clone();
    next.append_import(upload_entry(file_id, input, bytes, &path))
        .map_err(refuse)?;
    Ok(next)
}

/// The import entry of one upload, citing the original stored at `path`.
fn upload_entry(
    file_id: &FileId,
    input: &UploadInput<'_>,
    bytes: &[u8],
    path: &str,
) -> ImportEntry {
    let acquisition = Acquisition {
        recorded_at: input.finished.to_string(),
        started_at: input.started.to_string(),
        finished_at: input.finished.to_string(),
        method: ImportMethod::ApiUpload,
        storage: ByteStorage::Copied,
        job_id: Fact::unavailable(Reason::NotProvidedByUpload),
        operation_id: input.operation_id.map_or_else(
            || Fact::unavailable(Reason::NotProvidedByUpload),
            |id| Fact::value(id.to_string()),
        ),
        object_id: file_object_id(file_id),
        object_version: input.object_version.to_string(),
        logical_path: Some(path.to_string()),
        sha256: DigestAlgorithm::Sha256.digest_hex(bytes),
        size: bytes.len() as u64,
    };
    ImportEntry::for_upload(UploadFacts {
        acquisition,
        name: Raw::from_bytes(input.name.as_bytes()),
        name_normalization: input.name_normalization,
        reported_size: input.declared_size,
        last_modified: input.last_modified.map(str::to_string),
        media_type: Some(input.media_type.to_string()),
        host: receiving_host(),
    })
}

/// The host facts the API records: its operating system family and architecture and the
/// FileSquid version. The host name and the account are not read, so no identity of the
/// machine enters the record.
fn receiving_host() -> Host {
    let not_read = || Fact::unavailable(Reason::NotSupportedBySource);
    Host {
        os_name: Fact::value(std::env::consts::OS.to_string()),
        os_version: not_read(),
        os_build: not_read(),
        architecture: Fact::value(std::env::consts::ARCH.to_string()),
        host_name: not_read(),
        account: not_read(),
        filesquid_version: env!("CARGO_PKG_VERSION").to_string(),
        filesquid_build: "filesquid-api".to_string(),
        distribution: DISTRIBUTION.to_string(),
    }
}

/// The canonical refusal of a record the metadata module refused.
fn refuse(err: filesquid_metadata::file_record::RecordError) -> CanonicalError {
    CanonicalError::Encode(err.to_string())
}

/// The normalisation form of an ASCII name (every form), or `None` for a name whose form
/// the caller has to state.
pub fn ascii_name_form(name: &str) -> Option<NameForm> {
    name.is_ascii().then_some(NameForm::Nfc)
}
