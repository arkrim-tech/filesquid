//! The UI view API (OCFL-S-012;
//! [ADR 028](../../../../../../resources/architecture/028-filesquid-ui-packages-and-panel-composition.md),
//! [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)):
//! the serializable calls `filesquid-api` routes to every UI to browse and
//! edit an OCFL object like a drive. Every result has a JSON form
//! (`to_json`, [`versions_json`], [`history_json`]).
//!
//! Reads, at a version the caller pins, through an opened
//! [`Object`](crate::read::Object):
//!
//! - [`list_folder`]: the implied folders and files directly below a
//!   logical-path prefix, folders first, with an opaque cursor and at most
//!   [`MAX_PAGE_ENTRIES`] entries per page;
//! - [`file_details`]: object identifier, logical path, digest, size (read
//!   from the backend), content location, fixity values and the version
//!   that last set the content, with its `created`, `user` and `message`;
//! - [`file_history`] and [`object_versions`]: the versions that changed a
//!   file, and every version;
//! - [`read_range`]: at most [`MAX_RANGE_BYTES`] bytes for previews and
//!   downloads; [`check_fixity`]: the manifest digest and fixity values
//!   verified over the stored bytes.
//!
//! Writes, through the IO-0.10 [`ChangeModel`](crate::change::ChangeModel)
//! only (OCFL-S-016): [`add_file`], [`move_path`], [`remove_path`] and
//! [`restore`], each publishing exactly one version. OCFL stores no empty
//! folder: a created folder stays pending in the UI until a file is added
//! beneath its prefix.
//!
//! Storage facts only (C-055): nothing here reads embedded or descriptive
//! metadata; the extension type hint is a display hint, not a format
//! identification.

mod content;
mod details;
mod error;
mod facts;
mod listing;
mod writes;

pub use content::{check_fixity, read_range, FixityCheck, FixityReport};
pub use details::{
    file_details, file_history, history_json, object_versions, FileChange, FileDetails,
    FileHistoryEntry,
};
pub use error::ViewError;
pub use facts::{version_json, versions_json};
pub use listing::{list_folder, Entry, FileEntry, FolderEntry, FolderPage};
pub use writes::{add_file, move_path, remove_path, restore};

use crate::change::index::check_logical_path;
use crate::storage::MAX_CHUNK_BYTES;

/// Most entries one listing page holds.
pub const MAX_PAGE_ENTRIES: usize = 1_000;

/// Most bytes one ranged read returns.
pub const MAX_RANGE_BYTES: u64 = MAX_CHUNK_BYTES;

/// A folder prefix without its trailing `/`: `""` (the root) or a valid
/// logical path.
///
/// # Errors
///
/// [`ViewError::InvalidRequest`] for a prefix that breaks the OCFL
/// logical-path rules.
pub(crate) fn check_prefix(prefix: &str) -> Result<String, ViewError> {
    let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
    if prefix.is_empty() {
        return Ok(String::new());
    }
    check_path(prefix)?;
    Ok(prefix.to_string())
}

/// Refuse a logical path that breaks the OCFL rules (E052, E053).
///
/// # Errors
///
/// [`ViewError::InvalidRequest`].
pub(crate) fn check_path(path: &str) -> Result<(), ViewError> {
    check_logical_path(path).map_err(ViewError::request)
}
