//! A file object's original and its companion (owner decision, 2026-10-19; API-S-026).
//!
//! The original is stored under a universally unique name, `original/<file-uuid>.<ext>`: the
//! file identifier (a UUID v4, ADR 021) and the original extension. No part of the owner's
//! file name appears in the path, so sharing and publication stay private by default. That
//! name is a storage detail only. Beside it, `original/<file-uuid>.<ext>.metadata.md` records
//! the owner's original file name exactly as given, so every display name and every owner
//! download or export uses the original name, and a library copied with ordinary tools still
//! shows it.
//!
//! The companion is Markdown with YAML front matter:
//!
//! ```text
//! ---
//! schema: filesquid.original.v1
//! original_name: "Café photo.JPG"
//! ---
//! ```
//!
//! The name is written as a JSON string, which is also a YAML double-quoted scalar, so any
//! name round-trips exactly. Every function here is pure.

use super::layout::{segment, ORIGINAL_DIR};
use super::CanonicalError;
use filesquid_core::FileId;

/// Suffix of a companion file, as for the metadata module's sidecars (ADR 027).
pub const COMPANION_SUFFIX: &str = ".metadata.md";

/// Schema identifier in a companion's front matter.
pub const COMPANION_SCHEMA: &str = "filesquid.original.v1";

/// Longest original file name accepted, in bytes.
pub const MAX_ORIGINAL_NAME_BYTES: usize = 1024;

/// Longest extension kept in a stored name, in bytes.
const MAX_EXTENSION_BYTES: usize = 16;

/// A file object's original as stored: its logical path and the owner's original name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOriginal {
    /// Logical path of the original, `original/<file-uuid>.<ext>` (a storage detail).
    pub path: String,
    /// The owner's file name exactly as given; the name to show and to export under.
    pub original_name: String,
}

/// The universally unique name the original of `file_id` named `original_name` is stored
/// under: the file identifier, then `.` and the original extension when it is 1 to 16 ASCII
/// letters or digits after a dot that does not start the name. Nothing else of the owner's
/// name is kept.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] for an original name that is empty, longer than
/// [`MAX_ORIGINAL_NAME_BYTES`] or holds `/` or NUL, or a name that cannot be one path segment.
pub fn stored_name(file_id: &FileId, original_name: &str) -> Result<String, CanonicalError> {
    check_original_name(original_name)?;
    let name = match extension(original_name) {
        Some(extension) => format!("{}.{extension}", file_id.as_str()),
        None => file_id.as_str().to_string(),
    };
    segment(&name)?;
    companion_name(&name)?;
    Ok(name)
}

/// The logical path of the companion of the original stored as `stored_name`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the companion's name cannot be one path segment.
pub fn companion_path(stored_name: &str) -> Result<String, CanonicalError> {
    Ok(format!("{ORIGINAL_DIR}/{}", companion_name(stored_name)?))
}

/// The bytes of the companion recording `original_name`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] for a name [`stored_name`] refuses;
/// [`CanonicalError::Encode`] when the name cannot be written as a JSON string.
pub fn encode_companion(original_name: &str) -> Result<Vec<u8>, CanonicalError> {
    check_original_name(original_name)?;
    let quoted =
        serde_json::to_string(original_name).map_err(|e| CanonicalError::Encode(e.to_string()))?;
    let text = format!("---\nschema: {COMPANION_SCHEMA}\noriginal_name: {quoted}\n---\n");
    Ok(text.into_bytes())
}

/// The original name a companion at `path` records.
///
/// # Errors
/// [`CanonicalError::Decode`] for bytes that are not a companion of [`COMPANION_SCHEMA`] or
/// whose name [`stored_name`] would refuse.
pub fn decode_companion(path: &str, bytes: &[u8]) -> Result<String, CanonicalError> {
    let refuse = |reason: &str| CanonicalError::Decode {
        path: path.to_string(),
        reason: reason.into(),
    };
    let text = std::str::from_utf8(bytes).map_err(|_| refuse("not a companion file"))?;
    let lines: Vec<&str> = text.lines().take(4).collect();
    let quoted = match lines.as_slice() {
        ["---", schema, name, "---"] if *schema == format!("schema: {COMPANION_SCHEMA}") => {
            name.strip_prefix("original_name: ")
        }
        _ => None,
    }
    .ok_or_else(|| refuse("not a companion file"))?;
    let name: String = serde_json::from_str(quoted).map_err(|_| refuse("not a companion file"))?;
    check_original_name(&name).map_err(|_| refuse("original name is not a file name"))?;
    Ok(name)
}

/// The stored name a companion's logical path names, or `None` for any other path.
pub fn stored_name_of_companion(path: &str) -> Option<&str> {
    path.strip_prefix(ORIGINAL_DIR)?
        .strip_prefix('/')?
        .strip_suffix(COMPANION_SUFFIX)
        .filter(|name| segment(name).is_ok())
}

/// `<stored_name>.metadata.md`, when it is one path segment.
fn companion_name(stored_name: &str) -> Result<String, CanonicalError> {
    let name = format!("{}{COMPANION_SUFFIX}", segment(stored_name)?);
    segment(&name)?;
    Ok(name)
}

/// Refuse an original name that is empty, too long, or holds `/` or NUL.
fn check_original_name(name: &str) -> Result<(), CanonicalError> {
    let ok =
        !name.is_empty() && name.len() <= MAX_ORIGINAL_NAME_BYTES && !name.contains(['/', '\0']);
    ok.then_some(())
        .ok_or_else(|| CanonicalError::InvalidIdentifier("not a file name".into()))
}

/// The extension of `name`, when it has one of 1 to 16 ASCII letters or digits after a dot
/// that does not start the name.
fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.')
        .filter(|(stem, ext)| {
            !stem.is_empty()
                && !ext.is_empty()
                && ext.len() <= MAX_EXTENSION_BYTES
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map(|(_, ext)| ext)
}
