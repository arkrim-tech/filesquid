//! A file object's original and its metadata record (owner decision, 2026-10-19; API-S-026).
//!
//! The original is stored under a universally unique name, `original/<file-uuid>.<ext>`: the
//! file identifier (a UUID v4, ADR 021) and the original extension. No other part of the
//! owner's file name appears in the path, so sharing and publication stay private by default.
//! That name is a storage detail only. Beside it, `original/<file-uuid>.metadata.md` is the
//! file's metadata record: it holds the owner's original file name exactly as given, the
//! extension and the media type, so every display name, owner download and owner
//! archival/original export uses the original name (sharing, publication and privacy-on
//! exports follow the F-046 outgoing filename policy), and a library copied with ordinary
//! tools still shows it.
//!
//! The record is Markdown with YAML front matter:
//!
//! ```text
//! ---
//! schema: filesquid.original.v1
//! original_name: "Café photo.JPG"
//! extension: "JPG"
//! media_type: "image/jpeg"
//! ---
//! ```
//!
//! Each value is written as JSON (`extension` is `null` when the name has none), which YAML
//! reads as the same scalar, so any name round-trips exactly. Every function here is pure.

use super::layout::{segment, ORIGINAL_DIR};
use super::CanonicalError;
use filesquid_core::FileId;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Suffix of a metadata record's file name, as for the metadata module's sidecars (ADR 027).
pub const COMPANION_SUFFIX: &str = ".metadata.md";

/// Schema identifier in a metadata record's front matter.
pub const COMPANION_SCHEMA: &str = "filesquid.original.v1";

/// Longest original file name accepted, in bytes.
pub const MAX_ORIGINAL_NAME_BYTES: usize = 1024;

/// Longest media type accepted, in bytes (RFC 6838: two names of at most 127).
pub const MAX_MEDIA_TYPE_BYTES: usize = 255;

/// Longest extension kept in a stored name, in bytes.
const MAX_EXTENSION_BYTES: usize = 16;

/// What a file's metadata record describes: the owner's name, its extension and the format.
/// Built only by [`OriginalRecord::new`], so every record that is committed can be read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecord {
    /// The owner's file name exactly as given.
    original_name: String,
    /// The extension the stored name keeps, or `None` when the original name has none.
    extension: Option<String>,
    /// The file's format as a media type.
    media_type: String,
}

impl OriginalRecord {
    /// The record of an original the owner named `original_name`, of format `media_type`.
    ///
    /// # Errors
    /// [`CanonicalError::InvalidIdentifier`] for an original name that is empty, longer than
    /// [`MAX_ORIGINAL_NAME_BYTES`] or holds `/` or NUL, or a media type that is not
    /// `type/subtype`.
    pub fn new(original_name: &str, media_type: &str) -> Result<Self, CanonicalError> {
        check_original_name(original_name)?;
        check_media_type(media_type)?;
        Ok(Self {
            original_name: original_name.to_string(),
            extension: extension(original_name).map(str::to_string),
            media_type: media_type.to_string(),
        })
    }

    /// The owner's file name exactly as given; the name to show people.
    pub fn original_name(&self) -> &str {
        &self.original_name
    }

    /// The extension the stored name keeps, or `None` when the original name has none.
    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    /// The file's format as a media type, such as `image/jpeg`.
    pub fn media_type(&self) -> &str {
        &self.media_type
    }
}

/// A file object's original as stored: its logical path and its metadata record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOriginal {
    /// Logical path of the original, `original/<file-uuid>.<ext>` (a storage detail).
    pub path: String,
    /// The original name, extension and media type.
    pub record: OriginalRecord,
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

/// The logical path of the metadata record of `file_id`, `original/<file-uuid>.metadata.md`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the name cannot be one path segment.
pub fn companion_path(file_id: &FileId) -> Result<String, CanonicalError> {
    let name = format!("{}{COMPANION_SUFFIX}", segment(file_id.as_str())?);
    Ok(format!("{ORIGINAL_DIR}/{}", segment(&name)?))
}

/// The bytes of the metadata record of `record`.
///
/// # Errors
/// [`CanonicalError::Encode`] when a value cannot be written as JSON.
pub fn encode_companion(record: &OriginalRecord) -> Result<Vec<u8>, CanonicalError> {
    let text = format!(
        "---\nschema: {COMPANION_SCHEMA}\noriginal_name: {}\nextension: {}\nmedia_type: {}\n---\n",
        to_json(&record.original_name)?,
        to_json(&record.extension)?,
        to_json(&record.media_type)?,
    );
    Ok(text.into_bytes())
}

/// The record a metadata record at `path` holds.
///
/// # Errors
/// [`CanonicalError::Decode`] for bytes that are not a record of [`COMPANION_SCHEMA`], whose
/// values [`OriginalRecord::new`] would refuse, or whose extension is not the original name's.
pub fn decode_companion(path: &str, bytes: &[u8]) -> Result<OriginalRecord, CanonicalError> {
    let refuse = |reason: &str| CanonicalError::Decode {
        path: path.to_string(),
        reason: reason.into(),
    };
    let text = std::str::from_utf8(bytes).map_err(|_| refuse("not a metadata record"))?;
    let lines: Vec<&str> = text.lines().take(6).collect();
    let schema = format!("schema: {COMPANION_SCHEMA}");
    let ["---", head, name, ext, media, "---"] = lines.as_slice() else {
        return Err(refuse("not a metadata record"));
    };
    let name: Option<String> = field(name, "original_name: ");
    let ext: Option<Option<String>> = field(ext, "extension: ");
    let media: Option<String> = field(media, "media_type: ");
    let (true, Some(name), Some(ext), Some(media)) = (*head == schema, name, ext, media) else {
        return Err(refuse("not a metadata record"));
    };
    let record = OriginalRecord::new(&name, &media)
        .map_err(|_| refuse("original name or media type is not valid"))?;
    if record.extension != ext {
        return Err(refuse("extension is not the original name's"));
    }
    Ok(record)
}

/// The JSON value after `key` on `line`, or `None` when the line is not that field.
fn field<T: DeserializeOwned>(line: &str, key: &str) -> Option<T> {
    serde_json::from_str(line.strip_prefix(key)?).ok()
}

/// `value` as one JSON scalar.
fn to_json<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    serde_json::to_string(value).map_err(|e| CanonicalError::Encode(e.to_string()))
}

/// Refuse an original name that is empty, too long, or holds `/` or NUL.
fn check_original_name(name: &str) -> Result<(), CanonicalError> {
    let ok =
        !name.is_empty() && name.len() <= MAX_ORIGINAL_NAME_BYTES && !name.contains(['/', '\0']);
    ok.then_some(())
        .ok_or_else(|| CanonicalError::InvalidIdentifier("not a file name".into()))
}

/// Refuse a media type that is not `type/subtype` of RFC 6838 restricted-name characters.
fn check_media_type(media_type: &str) -> Result<(), CanonicalError> {
    let ok = media_type.len() <= MAX_MEDIA_TYPE_BYTES
        && media_type
            .split_once('/')
            .is_some_and(|(kind, sub)| media_name(kind) && media_name(sub));
    ok.then_some(())
        .ok_or_else(|| CanonicalError::InvalidIdentifier("not a media type".into()))
}

/// Whether `part` is an RFC 6838 restricted name: 1 to 127 of its allowed characters, the
/// first a letter or digit.
fn media_name(part: &str) -> bool {
    let allowed = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(c, '!' | '#' | '$' | '&' | '^' | '_' | '.' | '+' | '-')
    };
    (1..=127).contains(&part.len())
        && part.starts_with(|c: char| c.is_ascii_alphanumeric())
        && part.chars().all(allowed)
}

/// The extension of `name`, when it has one of 1 to 16 ASCII letters or digits after a dot
/// that does not start the name.
fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && is_extension(ext))
        .map(|(_, ext)| ext)
}

/// Whether `ext` is 1 to 16 ASCII letters or digits.
fn is_extension(ext: &str) -> bool {
    (1..=MAX_EXTENSION_BYTES).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
}
