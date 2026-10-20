//! A file object's original and its metadata record (owner decisions, 2026-10-19 and
//! 2026-10-20; API-S-026, [ADR 038](../../../../../../resources/architecture/038-import-file-record.md)
//! Decision 8).
//!
//! The original is stored under a universally unique name, `original/<file-uuid>.<ext>`: the
//! file identifier (a UUID v4, ADR 021) and the original extension when it is 1 to 16 ASCII
//! letters or digits. No other part of the owner's file name appears in the path, so sharing
//! and publication stay private by default. That name is a storage detail only.
//!
//! Beside it, `original/<file-uuid>.metadata.md` is the file's metadata record: the
//! `filesquid-metadata` [`FileRecord`] (MD-S-008, MD-S-013), read and written only through
//! that crate's codec ([`FileRecord::to_markdown`], [`FileRecord::from_markdown`]). It holds
//! the owner's original file name exactly with its normalisation form, the display name, the
//! full extension, the format and media type, the descriptive statements and the append-only
//! `import` list. Every display name, owner download and owner archival/original export uses
//! the record's display name; a library copied with ordinary tools still shows it. Every
//! function here is pure.

use super::layout::{segment, ORIGINAL_DIR};
use super::CanonicalError;
use filesquid_core::FileId;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::import::ByteStorage;
use filesquid_ocfl::DigestAlgorithm;

/// Suffix of a metadata record's file name, as for the metadata module's sidecars (ADR 027).
pub const COMPANION_SUFFIX: &str = ".metadata.md";

/// Longest extension kept in a stored name, in bytes.
const MAX_EXTENSION_BYTES: usize = 16;

/// A file object's original as stored: its logical path and its metadata record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOriginal {
    /// Logical path of the original, `original/<file-uuid>.<ext>` (a storage detail).
    pub path: String,
    /// The file's metadata record.
    pub record: FileRecord,
}

/// The universally unique name the original of `file_id` is stored under: the file
/// identifier, then `.` and `extension` when there is one. Nothing of the owner's name but the
/// extension is kept, and an extension never holds a dot, so the name cannot be the metadata
/// record's.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] for an extension that is not 1 to 16 ASCII letters or
/// digits, or a name that cannot be one path segment.
pub fn stored_name(file_id: &FileId, extension: Option<&str>) -> Result<String, CanonicalError> {
    if !extension.is_none_or(is_extension) {
        return Err(CanonicalError::InvalidIdentifier("not an extension".into()));
    }
    let name = match extension {
        Some(extension) => format!("{}.{extension}", file_id.as_str()),
        None => file_id.as_str().to_string(),
    };
    segment(&name)?;
    Ok(name)
}

/// The logical path of the original of `file_id` that `record` describes.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the name cannot be one path segment.
pub fn original_path_of(file_id: &FileId, record: &FileRecord) -> Result<String, CanonicalError> {
    original_path_for(file_id, record.extension())
}

/// The logical path of the original of `file_id` whose full extension is `extension`: the
/// stored name keeps it only when it is 1 to 16 ASCII letters or digits.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the name cannot be one path segment.
pub fn original_path_for(
    file_id: &FileId,
    extension: Option<&str>,
) -> Result<String, CanonicalError> {
    let kept = extension.filter(|ext| is_extension(ext));
    let name = stored_name(file_id, kept)?;
    Ok(format!("{ORIGINAL_DIR}/{name}"))
}

/// The logical path of the metadata record of `file_id`, `original/<file-uuid>.metadata.md`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the name cannot be one path segment.
pub fn companion_path(file_id: &FileId) -> Result<String, CanonicalError> {
    let name = format!("{}{COMPANION_SUFFIX}", segment(file_id.as_str())?);
    Ok(format!("{ORIGINAL_DIR}/{}", segment(&name)?))
}

/// The bytes of `record` as a metadata record.
pub fn encode_record(record: &FileRecord) -> Vec<u8> {
    record.to_markdown().into_bytes()
}

/// The record a metadata record at `path` holds.
///
/// # Errors
/// [`CanonicalError::Decode`] for bytes that are not a valid `filesquid.original.v1` file
/// record.
pub fn decode_record(path: &str, bytes: &[u8]) -> Result<FileRecord, CanonicalError> {
    let refuse = |reason: String| CanonicalError::Decode {
        path: path.to_string(),
        reason,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| refuse("not UTF-8".into()))?;
    FileRecord::from_markdown(text).map_err(|e| refuse(e.to_string()))
}

/// Check that `record` may be committed over `previous` (the record at the head, if any)
/// beside `bytes` stored at `path` in the object of `file_id`: it extends the previous record
/// (MD-S-013), adds at least one import entry, and every new entry is a copied import that
/// cites that object, that path, the SHA-256 of `bytes` and their length.
///
/// # Errors
/// [`CanonicalError::Encode`] naming the first rule the record breaks.
pub fn check_new_original(
    file_id: &FileId,
    record: &FileRecord,
    previous: Option<&FileRecord>,
    path: &str,
    bytes: &[u8],
) -> Result<(), CanonicalError> {
    let refuse = |reason: String| CanonicalError::Encode(reason);
    let first_new = match previous {
        Some(previous) => {
            record
                .check_extends(previous)
                .map_err(|e| refuse(e.to_string()))?;
            previous.imports().len()
        }
        None => 0,
    };
    if record.imports().len() <= first_new {
        return Err(refuse("a new original adds one import entry".into()));
    }
    let object_id = super::layout::file_object_id(file_id);
    let companion = companion_path(file_id)?;
    record
        .check_binding(first_new, &object_id, path, &companion)
        .map_err(|e| refuse(e.to_string()))?;
    let copied = record.imports()[first_new..]
        .iter()
        .all(|entry| entry.acquisition.storage == ByteStorage::Copied);
    if !copied {
        return Err(refuse("an original's import entry stores its bytes".into()));
    }
    let digest = DigestAlgorithm::Sha256.digest_hex(bytes);
    let cites_bytes = record.imports()[first_new..].iter().all(|entry| {
        entry.acquisition.sha256 == digest && entry.acquisition.size == bytes.len() as u64
    });
    match cites_bytes {
        true => Ok(()),
        false => Err(refuse(
            "an import entry cites other content than the original".into(),
        )),
    }
}

/// Check that `bytes` are the content the last import entry of an unchanged `record` cites,
/// so committing the head's record again can never store other bytes under it.
///
/// # Errors
/// [`CanonicalError::Encode`] when the bytes are not the cited content.
pub fn check_same_original(record: &FileRecord, bytes: &[u8]) -> Result<(), CanonicalError> {
    let digest = DigestAlgorithm::Sha256.digest_hex(bytes);
    let cited = record.imports().last().is_some_and(|entry| {
        entry.acquisition.sha256 == digest && entry.acquisition.size == bytes.len() as u64
    });
    match cited {
        true => Ok(()),
        false => Err(CanonicalError::Encode(
            "the bytes are not the content the record cites".into(),
        )),
    }
}

/// Whether `ext` is 1 to 16 ASCII letters or digits.
fn is_extension(ext: &str) -> bool {
    (1..=MAX_EXTENSION_BYTES).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
}
