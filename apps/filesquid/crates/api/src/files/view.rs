//! Request and answer types of the file operation routes, as the contract documents them
//! (`components.schemas`). Answers carry logical paths, versions and digests, never a
//! filesystem path, storage key or OCFL content path (API-S-005).

use filesquid_ocfl::read::VersionInfo;
use filesquid_ocfl::view::{Entry, FileChange, FileDetails, FileHistoryEntry, FolderPage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Query of `GET /api/v1/files/listing`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListingQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version to list (`v1`, `v2`, ...); the head when absent.
    pub version: Option<String>,
    /// The folder, a logical-path prefix without a trailing `/`; the root when absent.
    pub prefix: Option<String>,
    /// The cursor of the previous page.
    pub cursor: Option<String>,
    /// Entries per page, 1 to 1,000; [`super::DEFAULT_PAGE`] when absent.
    pub limit: Option<usize>,
}

/// Query of the ranged content read, `GET` and `HEAD /api/v1/files/content`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version; the head when absent.
    pub version: Option<String>,
    /// The file's logical path.
    pub path: String,
    /// True to offer the file as a download (`Content-Disposition: attachment`); shown in
    /// place (`inline`) otherwise. Either way it is named by its display name.
    pub download: Option<bool>,
}

/// Query naming one file of one version: details.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version; the head when absent.
    pub version: Option<String>,
    /// The file's logical path.
    pub path: String,
}

/// Query of `GET /api/v1/files/history`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The file's logical path.
    pub path: String,
}

/// Query of `GET /api/v1/files/versions`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
}

/// Query of `PUT /api/v1/files/upload` (the body is the file's bytes).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadQuery {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The logical path to add or replace.
    pub path: String,
    /// The version message, 1 to [`super::MAX_MESSAGE_BYTES`] bytes.
    pub message: Option<String>,
}

/// Body of `POST /api/v1/files/folders`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FolderRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The folder's logical path.
    pub path: String,
}

/// Body of `POST /api/v1/files/moves`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoveRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The file or folder to move.
    pub from: String,
    /// Its new logical path.
    pub to: String,
    /// The version message, 1 to [`super::MAX_MESSAGE_BYTES`] bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of `POST /api/v1/files/removals`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The file or folder to remove.
    pub path: String,
    /// The version message, 1 to [`super::MAX_MESSAGE_BYTES`] bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of `POST /api/v1/files/restorations`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The earlier version to restore from.
    pub version: String,
    /// The files or folders to restore, 1 to [`super::MAX_RESTORE_PATHS`].
    pub paths: Vec<String>,
    /// The version message, 1 to [`super::MAX_MESSAGE_BYTES`] bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of `POST /api/v1/content-handles`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MintRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The one version the handle reads (`v1`, `v2`, ...).
    pub version: String,
    /// The file's logical path in that version.
    pub path: String,
}

/// Who made a version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UserView {
    /// A readable name.
    pub name: String,
    /// A URI for the user, when recorded.
    pub address: Option<String>,
}

/// One version of an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VersionView {
    /// The version (`v1`, `v2`, ...).
    pub version: String,
    /// When it was created, RFC 3339 as recorded.
    pub created: String,
    /// Why it was made, when recorded.
    pub message: Option<String>,
    /// Who made it, when recorded.
    pub user: Option<UserView>,
}

impl VersionView {
    /// The view of `info`.
    pub fn of(info: &VersionInfo) -> Self {
        Self {
            version: info.version.to_string(),
            created: info.created.clone(),
            message: info.message.clone(),
            user: info.user.as_ref().map(|u| UserView {
                name: u.name.clone(),
                address: u.address.clone(),
            }),
        }
    }
}

/// One entry of a folder listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryView {
    /// A folder implied by the logical paths below it.
    Folder {
        /// Its last path segment.
        name: String,
        /// Its logical path.
        path: String,
    },
    /// A file.
    File {
        /// Its last path segment: the name it is stored under.
        name: String,
        /// The name users see: the owner's original file name, or the stored name when no
        /// record holds one.
        display_name: String,
        /// Its logical path.
        path: String,
        /// Its content digest.
        digest: String,
        /// Its size in bytes.
        size: u64,
        /// The version that last set its content.
        modified_version: String,
        /// When that version was created.
        modified: String,
        /// The lowercase extension, a display hint only.
        type_hint: Option<String>,
    },
}

/// One page of a folder listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FolderPageView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version listed.
    pub version: String,
    /// The folder listed (`""` for the root).
    pub prefix: String,
    /// Folders first, then files.
    pub entries: Vec<EntryView>,
    /// The cursor of the next page, or `null` on the last page.
    pub next_cursor: Option<String>,
}

impl FolderPageView {
    /// The view of `page` of `library_id`.
    pub fn of(library_id: &str, page: FolderPage) -> Self {
        let entries = page.entries.into_iter().map(entry_view).collect();
        Self {
            library_id: library_id.to_string(),
            object_id: page.object_id,
            version: page.version.to_string(),
            prefix: page.prefix,
            entries,
            next_cursor: page.next,
        }
    }
}

/// The view of one listing entry.
fn entry_view(entry: Entry) -> EntryView {
    match entry {
        Entry::Folder(folder) => EntryView::Folder {
            name: folder.name,
            path: folder.path,
        },
        Entry::File(file) => EntryView::File {
            display_name: file.name.clone(),
            name: file.name,
            path: file.path,
            digest: file.digest,
            size: file.size,
            modified_version: file.modified_version.to_string(),
            modified: file.modified,
            type_hint: file.type_hint,
        },
    }
}

/// Storage facts about one file of one version. The OCFL content path is left out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileDetailsView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version read.
    pub version: String,
    /// The file's logical path.
    pub path: String,
    /// Its last path segment: the name it is stored under.
    pub name: String,
    /// The name users see: the owner's original file name, or the stored name when no
    /// record holds one.
    pub display_name: String,
    /// Its content digest.
    pub digest: String,
    /// The digest algorithm (`sha512` or `sha256`).
    pub digest_algorithm: String,
    /// Its size in bytes.
    pub size: u64,
    /// Fixity values the inventory records, by algorithm.
    pub fixity: BTreeMap<String, Vec<String>>,
    /// The version that last set its content.
    pub last_change: VersionView,
    /// The lowercase extension, a display hint only.
    pub type_hint: Option<String>,
    /// The media type content answers carry.
    pub media_type: String,
}

impl FileDetailsView {
    /// The view of `details` of `library_id`.
    pub fn of(library_id: &str, details: FileDetails) -> Self {
        Self {
            library_id: library_id.to_string(),
            media_type: super::media::media_type(&details.path).to_string(),
            object_id: details.object_id,
            version: details.version.to_string(),
            path: details.path,
            display_name: details.name.clone(),
            name: details.name,
            digest: details.digest,
            digest_algorithm: details.digest_algorithm,
            size: details.size,
            fixity: details.fixity,
            last_change: VersionView::of(&details.last_change),
            type_hint: details.type_hint,
        }
    }
}

/// One version that changed a file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HistoryEntryView {
    /// The version.
    pub version: VersionView,
    /// How it changed the file.
    pub change: FileChangeView,
    /// The file's digest after the change, or `null` when it was removed.
    pub digest: Option<String>,
}

/// How a version changed a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeView {
    /// The path appeared.
    Added,
    /// The content changed.
    Changed,
    /// The path was removed.
    Deleted,
}

impl From<FileChange> for FileChangeView {
    fn from(change: FileChange) -> Self {
        match change {
            FileChange::Added => Self::Added,
            FileChange::Changed => Self::Changed,
            FileChange::Deleted => Self::Deleted,
        }
    }
}

/// The versions that changed one file, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HistoryView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The file's logical path.
    pub path: String,
    /// The versions that changed it.
    pub entries: Vec<HistoryEntryView>,
}

impl HistoryView {
    /// The view of `entries` for `path`.
    pub fn of(
        library_id: &str,
        object_id: &str,
        path: &str,
        entries: Vec<FileHistoryEntry>,
    ) -> Self {
        Self {
            library_id: library_id.to_string(),
            object_id: object_id.to_string(),
            path: path.to_string(),
            entries: entries
                .into_iter()
                .map(|e| HistoryEntryView {
                    version: VersionView::of(&e.version),
                    change: e.change.into(),
                    digest: e.digest,
                })
                .collect(),
        }
    }
}

/// Every version of an object, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VersionsView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The head version.
    pub head: String,
    /// Every version.
    pub versions: Vec<VersionView>,
}

/// A version a write published.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommitView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The new head version.
    pub version: String,
    /// When it was created (RFC 3339 UTC).
    pub created: String,
}

/// The answer to creating a folder: nothing is written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FolderView {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The folder's logical path.
    pub path: String,
    /// True when no file lies below the folder yet, so it exists only in the client until a
    /// file is added beneath it (OCFL stores no empty folder); false when it already exists.
    pub pending: bool,
}

/// The answer to a mint: the handle is answered here and nowhere else. `Debug` output
/// never shows it.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct MintView {
    /// The content handle, 43 base64url characters.
    pub handle: String,
    /// Its relative URL, `/api/v1/content/<handle>`.
    pub url: String,
    /// Its absolute expiry, RFC 3339 UTC.
    pub expires_at: String,
}

impl std::fmt::Debug for MintView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MintView")
            .field("handle", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
