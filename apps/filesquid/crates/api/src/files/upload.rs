//! The upload route's write (Task API-F1.4; API-S-026, MD-S-013, ADR 038 Decision 3): an upload
//! creates the file's own OCFL object `files/<file-id>` with the original at
//! `original/<file-uuid>.<ext>` and its metadata record beside it, in one version, never at a
//! logical path the client chooses. An upload naming an existing file is a new version of
//! that file's object: the new bytes at the same stored path and one more import entry. The
//! verified size is the size of the bytes received; a declared size that differs fails
//! verification and commits nothing.

use super::error::FileError;
use super::service::{change_model, FileService};
use super::view::{UploadQuery, UploadView};
use super::{media, MAX_MESSAGE_BYTES};
use crate::canonical::{
    ascii_name_form, commit_file_original, file_object_id, file_object_root, new_upload_record,
    read_file_original, reupload_record, CanonicalError, OwnerReadable, UploadInput,
};
use crate::ErrorCode;
use filesquid_core::FileId;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::NameForm;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;

/// The file an upload writes: its identifier and object, its record and version at the head,
/// if any, and the version the upload will publish.
struct Target {
    file_id: FileId,
    root: String,
    object_id: String,
    previous: Option<FileRecord>,
    head: Option<String>,
    next_version: String,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Commit one upload of `bytes` by `actor`: a new file object, or a new version of the
    /// object of `query.file_id`.
    ///
    /// # Errors
    /// `invalid_request` for a name, media type, time, size or message that is refused (a
    /// declared size that is not the body's among them); `not_found` for an unknown library
    /// or file; `conflict` when the upload would change nothing or another writer moved the
    /// head; a storage failure otherwise. Nothing is published on any error.
    ///
    /// `expected` is the head of the file object the caller read, or
    /// [`super::NEW_FILE_REVISION`] for a new file; any other is a `conflict`.
    pub async fn upload(
        &self,
        query: UploadQuery,
        bytes: &[u8],
        actor: &str,
        expected: &str,
    ) -> Result<UploadView, FileError> {
        let form = name_form(&query)?;
        let meta = self.meta(actor, query.message.as_deref())?;
        let library = self.library(&query.library_id)?;
        let _writing = library.writes.lock().await;
        let backend = library.files.backend.as_ref();
        let target = match resumable_target(backend, &query).await? {
            Ok(target) => target,
            Err(committed) => return Ok(committed),
        };
        check_target_revision(&target, expected)?;
        let started = self.created();
        let media_type = query
            .media_type
            .clone()
            .unwrap_or_else(|| media::media_type(&query.name).to_string());
        let input = UploadInput {
            name: &query.name,
            name_normalization: form,
            declared_size: query.size,
            media_type: &media_type,
            last_modified: query.last_modified.as_deref(),
            operation_id: None,
            started: &started,
            finished: &self.created(),
            object_version: &target.next_version,
        };
        let record = upload_record(&target, &input, bytes)?;
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
            FileError::conflict("The upload would change nothing, so no version was made.")
        })?;
        library.remember(&target.object_id, &target.root);
        Ok(UploadView {
            library_id: query.library_id,
            file_id: target.file_id.as_str().to_string(),
            object_id: receipt.object_id,
            version: receipt.version.to_string(),
            created: receipt.created,
            collection_id: None,
            path: None,
        })
    }
}

/// The file an upload writes, or, as `Err`, the answer of the new file an earlier, unfinished
/// run of the same operation already committed. Storage I/O.
///
/// # Errors
/// As [`reserved_new_file`], [`committed_before`] and [`upload_target`].
async fn resumable_target<B: StorageBackend>(
    backend: &B,
    query: &UploadQuery,
) -> Result<Result<Target, UploadView>, FileError> {
    let reserved = reserved_new_file(query)?;
    if let Some(view) = committed_before(backend, &query.library_id, reserved.as_ref()).await? {
        return Ok(Err(view));
    }
    let target = upload_target(backend, query.file_id.as_deref(), reserved).await?;
    Ok(Ok(target))
}

/// The file ID the upload's operation reserved for a new file, if any; an upload of new
/// content for an existing file reserves none. Pure.
///
/// # Errors
/// `invalid_request` for a reserved ID that is not a UUID v4.
fn reserved_new_file(query: &UploadQuery) -> Result<Option<FileId>, FileError> {
    match (&query.file_id, &query.reserved_file_id) {
        (None, Some(id)) => parse_file_id(id).map(Some),
        _ => Ok(None),
    }
}

/// The answer of a new file an earlier, unfinished run of the same operation already
/// committed under its reserved ID (a crash between the object and its registration), so the
/// retry completes it once instead of writing it again; `None` when nothing is committed under
/// that ID yet. Storage I/O.
///
/// # Errors
/// A storage failure reading the object; `internal` for an object that does not decode as a
/// file object or has no version.
pub(super) async fn committed_before<B: StorageBackend>(
    backend: &B,
    library_id: &str,
    reserved: Option<&FileId>,
) -> Result<Option<UploadView>, FileError> {
    let Some(file_id) = reserved else {
        return Ok(None);
    };
    let root = file_object_root(file_id).map_err(canonical_error)?;
    let existing = read_file_original(backend, &OwnerReadable, file_id)
        .await
        .map_err(canonical_error)?;
    if existing.is_none() {
        return Ok(None);
    }
    let object = Object::open(backend, &root).await?;
    let head = filesquid_ocfl::read::version_history(object.inventory())
        .into_iter()
        .last()
        .ok_or_else(|| FileError::internal("a committed object has no version"))?;
    Ok(Some(UploadView {
        library_id: library_id.to_string(),
        file_id: file_id.as_str().to_string(),
        object_id: file_object_id(file_id),
        version: head.version.to_string(),
        created: head.created,
        collection_id: None,
        path: None,
    }))
}

/// The normalisation form of the uploaded name: the one the client stated, or NFC for an
/// ASCII name.
fn name_form(query: &UploadQuery) -> Result<NameForm, FileError> {
    if query
        .message
        .as_ref()
        .is_some_and(|m| m.len() > MAX_MESSAGE_BYTES)
    {
        return Err(FileError::invalid(format!(
            "A version message is 1 to {MAX_MESSAGE_BYTES} bytes and not blank."
        )));
    }
    query
        .name_normalization
        .or_else(|| ascii_name_form(&query.name))
        .ok_or_else(|| FileError::invalid("A name that is not ASCII needs its name_normalization."))
}

/// The file an upload writes: a new file, or the existing file `file_id` with its record and
/// the version the upload will publish.
async fn upload_target<B: StorageBackend>(
    backend: &B,
    file_id: Option<&str>,
    reserved: Option<FileId>,
) -> Result<Target, FileError> {
    let (file_id, existing) = match (file_id, reserved) {
        (Some(id), _) => (parse_file_id(id)?, true),
        (None, Some(reserved)) => (reserved, false),
        (None, None) => (mint_file_id()?, false),
    };
    let root = file_object_root(&file_id).map_err(canonical_error)?;
    let object_id = file_object_id(&file_id);
    let original = read_file_original(backend, &OwnerReadable, &file_id)
        .await
        .map_err(canonical_error)?;
    let previous = match (existing, original) {
        (true, Some(original)) => Some(original.record),
        (true, None) => return Err(FileError::not_found()),
        (false, Some(_)) => return Err(FileError::conflict("That file already exists.")),
        (false, None) => None,
    };
    let (head, next_version) = match &previous {
        Some(_) => {
            let (head, next) = versions(backend, &root).await?;
            (Some(head), next)
        }
        None => (None, "v1".to_string()),
    };
    Ok(Target {
        file_id,
        root,
        object_id,
        previous,
        head,
        next_version,
    })
}

/// Refuse an upload as a `conflict` unless `expected` is the head of the file it writes, or
/// [`super::NEW_FILE_REVISION`] for a new file. Pure.
fn check_target_revision(target: &Target, expected: &str) -> Result<(), FileError> {
    let current = target.head.as_deref().unwrap_or(super::NEW_FILE_REVISION);
    match current == expected {
        true => Ok(()),
        false => Err(FileError::conflict(
            "The file changed since it was read, so nothing was uploaded; reload and try again.",
        )),
    }
}

/// The record an upload commits: the first record of a new file, or the previous record
/// with one more import entry.
fn upload_record(
    target: &Target,
    input: &UploadInput<'_>,
    bytes: &[u8],
) -> Result<FileRecord, FileError> {
    let record = match &target.previous {
        Some(previous) => reupload_record(&target.file_id, previous, input, bytes),
        None => new_upload_record(&target.file_id, input, bytes),
    };
    record.map_err(canonical_error)
}

/// The head of the object at `root` and the version after it.
async fn versions<B: StorageBackend>(
    backend: &B,
    root: &str,
) -> Result<(String, String), FileError> {
    let object = Object::open(backend, root).await?;
    let next = object.head().next().ok_or_else(|| {
        FileError::conflict("The file has reached the last version its numbering allows.")
    })?;
    Ok((object.head().to_string(), next.to_string()))
}

/// The file identifier `id` names.
pub(super) fn parse_file_id(id: &str) -> Result<FileId, FileError> {
    match crate::library::is_uuid_v4(id) {
        true => FileId::new(id).map_err(|_| FileError::invalid("A file_id is a UUID v4.")),
        false => Err(FileError::invalid("A file_id is a UUID v4.")),
    }
}

/// A new file identifier (UUID v4, ADR 021).
pub(super) fn mint_file_id() -> Result<FileId, FileError> {
    let id = crate::library::mint_uuid_v4()
        .map_err(|_| FileError::unavailable("the random source is unavailable"))?;
    FileId::new(&id).map_err(|_| FileError::internal("a minted file identifier was refused"))
}

/// The file error of a canonical refusal or failure.
pub(super) fn canonical_error(err: CanonicalError) -> FileError {
    match err {
        CanonicalError::Encode(reason) | CanonicalError::InvalidIdentifier(reason) => {
            FileError::invalid(format!("The upload was refused: {reason}."))
        }
        CanonicalError::Conflict => {
            FileError::conflict("Another change reached the file first; try again.")
        }
        CanonicalError::Unavailable(detail) => FileError::unavailable(detail),
        CanonicalError::HeadWithoutInventory => FileError::new(
            ErrorCode::Conflict,
            "The file's last version has no version inventory; repair the library first.",
        ),
        CanonicalError::Decode { .. } | CanonicalError::Failed(_) => {
            FileError::internal("the file object could not be read or written")
        }
    }
}
