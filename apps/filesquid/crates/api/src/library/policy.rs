//! The hide and delete policy of a library's files
//! ([PRD F-008](../../../../../../resources/prd.md#f-008-deletion-hidden-files-and-duplicate-review)).
//!
//! A file carries a [`FileState`]: a hidden flag and, when recoverably deleted, a
//! [`Deletion`] with its purge-after time. Deletion is recoverable for
//! [`RECOVERABLE_DAYS`] days by default. Permanent deletion needs a [`PurgeConfirmation`]
//! that repeats its exact scope. A [`VisibilityFilter`] picks the ordinary, hidden or deleted
//! view; the ordinary view never shows a hidden or deleted file.

use crate::journal::{format_rfc3339_ms, parse_rfc3339_ms};
use filesquid_core::LibraryId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Default number of days a deleted file stays recoverable: the installation default of
/// 30 days (owner decision 2026-10-22, ADR 041 Decision 10).
pub const RECOVERABLE_DAYS: u32 = 30;

/// Longest retention period a library may set, in days; the shortest is 1 (ADR 041
/// Decision 10, proposed bounds).
pub const MAX_RETENTION_DAYS: u32 = 3_650;

/// Milliseconds in one day.
const DAY_MS: i64 = 86_400_000;

/// How long deleted files stay recoverable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionPolicy {
    /// Days a deleted file stays recoverable. The owner may still purge it earlier with a
    /// [`PurgeConfirmation`].
    pub recoverable_days: u32,
}

impl Default for DeletionPolicy {
    /// The default local policy: [`RECOVERABLE_DAYS`] days.
    fn default() -> Self {
        Self {
            recoverable_days: RECOVERABLE_DAYS,
        }
    }
}

impl DeletionPolicy {
    /// The deletion record of a file deleted at `deleted_at_ms` (Unix milliseconds, UTC).
    pub fn deletion_at(&self, deleted_at_ms: i64) -> Deletion {
        let span = i64::from(self.recoverable_days).saturating_mul(DAY_MS);
        Deletion {
            deleted_at_ms,
            purge_after_ms: deleted_at_ms.saturating_add(span),
        }
    }
}

/// When a file was recoverably deleted and when its recoverability ends. Both times serialize
/// as RFC 3339 UTC with milliseconds (`deleted_at`, `purge_after`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deletion {
    /// Deletion time in Unix milliseconds (UTC).
    #[serde(rename = "deleted_at", with = "rfc3339")]
    pub deleted_at_ms: i64,
    /// End of recoverability in Unix milliseconds (UTC): until then the file can be
    /// restored; after it the file may be purged. An explicit, confirmed purge may remove
    /// the file before this time.
    #[serde(rename = "purge_after", with = "rfc3339")]
    pub purge_after_ms: i64,
}

/// Hide and delete state of one file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    /// Hidden files are left out of ordinary browsing.
    #[serde(default)]
    pub hidden: bool,
    /// Present while the file is recoverably deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Deletion>,
}

/// Which files a listing shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityFilter {
    /// Files that are neither hidden nor deleted (the default for every view).
    #[default]
    Ordinary,
    /// Hidden files that are not deleted.
    Hidden,
    /// Recoverably deleted files, hidden or not.
    Deleted,
}

impl VisibilityFilter {
    /// Whether a file in `state` belongs to this view.
    pub fn admits(self, state: &FileState) -> bool {
        match self {
            Self::Ordinary => !state.hidden && state.deletion.is_none(),
            Self::Hidden => state.hidden && state.deletion.is_none(),
            Self::Deleted => state.deletion.is_some(),
        }
    }
}

/// The explicit scope a permanent deletion repeats: the library and the exact number of
/// files the caller showed the owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeConfirmation {
    /// Library the owner confirmed.
    pub library_id: LibraryId,
    /// Number of files the owner confirmed.
    pub file_count: u32,
}

impl PurgeConfirmation {
    /// Whether this confirmation names exactly `library_id` and `file_count` files.
    pub fn matches(&self, library_id: &LibraryId, file_count: usize) -> bool {
        self.library_id == *library_id
            && usize::try_from(self.file_count).is_ok_and(|n| n == file_count)
    }
}

/// Serde adapter: Unix milliseconds as RFC 3339 UTC text.
mod rfc3339 {
    use super::*;

    /// Write `ms` as RFC 3339 UTC.
    pub fn serialize<S: Serializer>(ms: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_rfc3339_ms(*ms))
    }

    /// Read RFC 3339 text as Unix milliseconds.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let text = String::deserialize(d)?;
        parse_rfc3339_ms(&text).ok_or_else(|| serde::de::Error::custom("invalid RFC 3339 time"))
    }
}
