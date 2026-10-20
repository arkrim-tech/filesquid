//! Outgoing file names (API-S-027, product vision C-033): the name a sharing or publication
//! output gives a file. Owner downloads and exports use the display name instead
//! ([`crate::files::names`]); this is the one deliberate exception. Every function here is
//! pure.
//!
//! No sharing or publication route exists yet, so [`outgoing_name`] is the seam those routes
//! call. The saved outgoing policy also decides whether a shared record keeps the original
//! file name; that scrub class belongs to the metadata module (MD-S-013).

use super::original::original_path_of;
use super::CanonicalError;
use filesquid_core::FileId;
use filesquid_metadata::file_record::{safe_display_name, FileRecord};

/// How a sharing or publication output names its files (the saved outgoing policy).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutgoingNames {
    /// FileId plus the real media extension, for example `<file-uuid>.png`: the privacy-on
    /// default, which reveals nothing of the owner's name.
    #[default]
    FileId,
    /// The owner's name for the file (its original name or their rename), chosen per saved
    /// policy with **Use original filename**.
    OriginalName,
}

/// The name a sharing or publication output gives the file `file_id` that `record`
/// describes, under `policy`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the FileId name cannot be one path segment.
pub fn outgoing_name(
    file_id: &FileId,
    record: &FileRecord,
    policy: OutgoingNames,
) -> Result<String, CanonicalError> {
    match policy {
        OutgoingNames::FileId => file_id_name(file_id, record),
        OutgoingNames::OriginalName => Ok(safe_display_name(record.display_name())),
    }
}

/// FileId plus the extension the stored name keeps (1 to 16 ASCII letters or digits).
fn file_id_name(file_id: &FileId, record: &FileRecord) -> Result<String, CanonicalError> {
    let path = original_path_of(file_id, record)?;
    let name = path.rsplit('/').next().unwrap_or(&path);
    Ok(name.to_string())
}
