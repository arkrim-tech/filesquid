//! Copy (Task API-F2.2; API-S-011, API-S-026, SRS-F-062): `POST /api/v1/files/copies` makes a
//! new file from the head of a file object, in the new file's own object.
//!
//! The copy gets a new FileId and therefore a new stored name `original/<new-uuid>.<ext>`; it
//! keeps the display name, the original name, the media type and every statement, and its
//! record names the source as `dcterms:source` = `urn:uuid:<source-id>@vN`
//! ([`crate::canonical::copy`]). The source is never changed. One set of bytes never gets
//! two home collections: the copy is a new file, and the host records its home collection in
//! its catalog. The copy is read whole, so a source over [`MAX_COPY_BYTES`] is refused until
//! a streamed copy exists.

use super::error::FileError;
use super::service::{change_model, read, FileService};
use super::statements::metadata_error;
use super::upload::{canonical_error, mint_file_id, parse_file_id};
use crate::canonical::{
    commit_file_original, copy_record, file_object_id, file_object_root, read_file_original,
    source_reference, CanonicalError, CopyInput, OwnerReadable, SOURCE_TERM,
};
use filesquid_core::FileId;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::record::parse_curie_with;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::view;
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};

/// Largest original a copy reads, in bytes: the upload bound, since a copy holds the bytes
/// in memory as an upload does.
pub const MAX_COPY_BYTES: u64 = super::MAX_UPLOAD_BYTES as u64;

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
}

/// The source of one copy: its head record and bytes.
struct Source {
    file_id: FileId,
    version: String,
    record: FileRecord,
    bytes: Vec<u8>,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Copy the file `request.file_id` at its head by `actor` into a new file.
    ///
    /// # Errors
    /// `invalid_request` for a malformed file ID or message, or a source over
    /// [`MAX_COPY_BYTES`]; `not_found` for an unknown library or file; a storage failure
    /// otherwise. Nothing is published on any error, and the source never changes.
    ///
    /// `expected` is the source file object's head the caller read; any other is a
    /// `conflict`, and nothing is copied.
    pub async fn copy(
        &self,
        request: CopyRequest,
        actor: &str,
        expected: &str,
    ) -> Result<CopyView, FileError> {
        let source_id = parse_file_id(&request.file_id)?;
        let meta = self.meta(actor, request.message.as_deref())?;
        let library = self.library(&request.library_id)?;
        let registry = self.registry(&request.library_id).await?;
        let _writing = library.writes.lock().await;
        let backend = library.files.backend.as_ref();
        let source = read_source(backend, source_id).await?;
        if source.version != expected {
            return Err(stale_source());
        }
        let file_id = mint_file_id()?;
        let reference = source_reference(&source.file_id, &source.version);
        let started = self.created();
        let input = CopyInput {
            source: &source.record,
            started: &started,
            finished: &self.created(),
        };
        let mut record = copy_record(&file_id, &input, &source.bytes).map_err(copy_error)?;
        let (namespace, _) = parse_curie_with(&format!("dcterms:{SOURCE_TERM}"), &registry)
            .map_err(metadata_error)?;
        record
            .statements_mut()
            .set_registered(&registry, namespace, SOURCE_TERM, reference.clone())
            .map_err(metadata_error)?;
        let root = file_object_root(&file_id).map_err(copy_error)?;
        let object_id = file_object_id(&file_id);
        let staging = library.staging(&object_id)?;
        let model = change_model(&library, &root, &object_id, staging);
        let committed = commit_file_original(
            &model,
            &OwnerReadable,
            &file_id,
            &record,
            None,
            &source.bytes,
            meta,
        )
        .await
        .map_err(copy_error)?;
        let receipt =
            committed.ok_or_else(|| FileError::internal("a new copy wrote no version"))?;
        library.remember(&object_id, &root);
        Ok(CopyView {
            library_id: request.library_id,
            file_id: file_id.as_str().to_string(),
            object_id: receipt.object_id,
            version: receipt.version.to_string(),
            created: receipt.created,
            source: reference,
        })
    }
}

/// The head record and bytes of the file `file_id`, read whole within [`MAX_COPY_BYTES`].
async fn read_source<B: StorageBackend>(backend: &B, file_id: FileId) -> Result<Source, FileError> {
    let original = read_file_original(backend, &OwnerReadable, &file_id)
        .await
        .map_err(copy_error)?
        .ok_or_else(FileError::not_found)?;
    let root = file_object_root(&file_id).map_err(copy_error)?;
    let object = Object::open(backend, &root).await?;
    let head = object.head().clone();
    let size = view::file_details(&object, &head, &original.path)
        .await?
        .size;
    if size > MAX_COPY_BYTES {
        return Err(FileError::invalid(format!(
            "A copy reads at most {MAX_COPY_BYTES} bytes; this file is larger."
        )));
    }
    let bytes = read(&object, &head, &original.path, 0, size).await?;
    Ok(Source {
        file_id,
        version: head.to_string(),
        record: original.record,
        bytes,
    })
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
