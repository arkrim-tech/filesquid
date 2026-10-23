//! The folder listing of a collection or folder node from the host's index of registered
//! files (Task DE-1.7; DE-S-018, API-S-011, API-S-021,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)).
//!
//! Every file has its own OCFL object and one home collection, where it sits at its
//! collection logical path. A host that indexes those paths answers a folder of a scope
//! through [`super::ScopeObjects::entries`]: its subfolders, then its files, each named by its
//! last path segment and its file ID. The listing route then reads each file's object for the
//! facts a listing shows (digest, size, the version that last set it), so
//! `GET /api/v1/files/listing?node=` shows every registered file under its collection and
//! folder. The page names no single object: each file entry names its own (`object_id`), and
//! its `path` is the original's logical path inside that object, which the content and
//! details routes take. Folders come first, by name in code-point order, then files by name and
//! identifier; the cursor is opaque and names no path.

use super::error::FileError;
use super::service::{page_size, FileService};
use super::view::{EntryView, FolderPageView, ListingQuery};
use crate::canonical::file_object_id;
use crate::canonical::layout::ORIGINAL_DIR;
use filesquid_core::FileId;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};

/// The prefix of a scope listing cursor, so a cursor of another listing is never mistaken
/// for one.
const CURSOR_PREFIX: &str = "s1.";

/// Most entries of a file object's `original` folder read to find its original (the original
/// and its metadata record, with room to spare).
const MAX_ORIGINAL_ENTRIES: usize = 8;

/// One entry of a folder of a scope, as the host's index answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeEntry {
    /// A folder implied by the collection logical paths of ordinary files below it.
    Folder {
        /// Its name: one segment of a collection logical path.
        name: String,
    },
    /// A registered ordinary file directly in the folder.
    File {
        /// Its name in the collection: the last segment of its collection logical path.
        name: String,
        /// Its file ID; its object is `urn:uuid:<file-id>`.
        file_id: String,
    },
}

/// Where a scope page starts: after this folder (folders come first), or after this file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum ScopeAfter {
    /// After the folder of this name; the files follow the last folder.
    Folder {
        /// The folder's name.
        #[serde(rename = "n")]
        name: String,
    },
    /// After this file, by name then file ID.
    File {
        /// The file's name in the collection.
        #[serde(rename = "n")]
        name: String,
        /// The file's ID.
        #[serde(rename = "i")]
        file_id: String,
    },
}

/// One page of a folder of a scope from the host's index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopePage {
    /// The library the scope belongs to.
    pub library_id: String,
    /// Folders first, by name, then files by name and file ID.
    pub entries: Vec<ScopeEntry>,
}

/// A resolved listing of a scope's folder: the index's entries of one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeListing {
    /// The library.
    pub library_id: String,
    /// The folder: a collection logical path, `""` for the collection's root.
    pub folder: String,
    /// The page's entries.
    pub entries: Vec<ScopeEntry>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<String>,
}

impl ScopeListing {
    /// The listing of `page` of `folder` when the index was asked for `limit` + 1 entries:
    /// at most `limit` kept, and a cursor after the last kept one when more remain. Pure.
    pub fn of(folder: &str, mut page: ScopePage, limit: usize) -> Self {
        let more = page.entries.len() > limit;
        page.entries.truncate(limit);
        let next_cursor = match (more, page.entries.last()) {
            (true, Some(last)) => Some(encode_cursor(&after_entry(last))),
            _ => None,
        };
        Self {
            library_id: page.library_id,
            folder: folder.to_string(),
            entries: page.entries,
            next_cursor,
        }
    }
}

/// Where the page after `entry` starts. Pure.
fn after_entry(entry: &ScopeEntry) -> ScopeAfter {
    match entry {
        ScopeEntry::Folder { name } => ScopeAfter::Folder { name: name.clone() },
        ScopeEntry::File { name, file_id } => ScopeAfter::File {
            name: name.clone(),
            file_id: file_id.clone(),
        },
    }
}

/// The opaque cursor of `after`: the prefix `s1.` and the hexadecimal bytes of its JSON, so
/// it holds no readable name. Pure.
pub fn encode_cursor(after: &ScopeAfter) -> String {
    let json = serde_json::to_vec(after).unwrap_or_default();
    let hex: String = json.iter().map(|b| format!("{b:02x}")).collect();
    format!("{CURSOR_PREFIX}{hex}")
}

/// The position a scope cursor names, or `None` for a cursor that is not one. Pure.
pub fn decode_cursor(cursor: &str) -> Option<ScopeAfter> {
    let hex = cursor.strip_prefix(CURSOR_PREFIX)?;
    if hex.len() % 2 != 0 || !hex.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).ok())
        .collect();
    serde_json::from_slice(&bytes?).ok()
}

/// The page size of a scope listing: as an object listing's (1 to 1,000, 200 by default).
///
/// # Errors
/// `invalid_request` outside the range.
pub fn scope_page_size(limit: Option<usize>) -> Result<usize, FileError> {
    page_size(limit)
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend + 'static,
    R: VersionReservations + 'static,
    S: StorageBackend + 'static,
{
    /// The page of `listing`: its folders, then its files, each read from its own object for
    /// the facts a listing shows. A file whose object is missing is left out (logged with its
    /// error code only), so one stray entry never hides the rest. Storage reads.
    ///
    /// # Errors
    /// `not_found` for a library this host has not opened; `unavailable` when storage fails.
    pub async fn scope_listing(&self, listing: ScopeListing) -> Result<FolderPageView, FileError> {
        let mut entries = Vec::with_capacity(listing.entries.len());
        for entry in listing.entries {
            match entry {
                ScopeEntry::Folder { name } => entries.push(EntryView::Folder {
                    path: join(&listing.folder, &name),
                    name,
                }),
                ScopeEntry::File { name, file_id } => {
                    let found = self.scope_file(&listing.library_id, &file_id, name).await?;
                    entries.extend(found);
                }
            }
        }
        Ok(FolderPageView {
            library_id: listing.library_id,
            object_id: String::new(),
            version: String::new(),
            prefix: listing.folder,
            entries,
            next_cursor: listing.next_cursor,
        })
    }

    /// The listing entry of the file `file_id` named `name` in its collection: its original
    /// at the head of its own object, shown under `name`. `None` (logged) when its object or
    /// original is missing. Storage reads.
    async fn scope_file(
        &self,
        library_id: &str,
        file_id: &str,
        name: String,
    ) -> Result<Option<EntryView>, FileError> {
        let id =
            FileId::new(file_id).map_err(|_| FileError::internal("an indexed file ID is empty"))?;
        let object_id = file_object_id(&id);
        let query = ListingQuery {
            library_id: library_id.to_string(),
            object_id: object_id.clone(),
            version: None,
            prefix: Some(ORIGINAL_DIR.to_string()),
            cursor: None,
            limit: Some(MAX_ORIGINAL_ENTRIES),
        };
        let page = match self.listing(query).await {
            Ok(page) => page,
            Err(err) if err.code().as_str() == "not_found" => return Ok(missing(file_id)),
            Err(err) => return Err(err),
        };
        let original = page.entries.into_iter().find_map(|entry| match entry {
            EntryView::File { .. } => Some(entry),
            EntryView::Folder { .. } => None,
        });
        Ok(original
            .map(|entry| shown_as(entry, name, object_id))
            .or_else(|| missing(file_id)))
    }
}

/// `entry` shown under its collection name `name`, naming its object. Pure.
fn shown_as(entry: EntryView, name: String, object: String) -> EntryView {
    match entry {
        EntryView::File {
            name: stored,
            path,
            digest,
            size,
            modified_version,
            modified,
            type_hint,
            ..
        } => EntryView::File {
            name: stored,
            display_name: name,
            path,
            digest,
            size,
            modified_version,
            modified,
            type_hint,
            object_id: Some(object),
        },
        folder => folder,
    }
}

/// Log an indexed file whose object or original is missing, with its ID only, and answer no
/// entry for it.
fn missing(file_id: &str) -> Option<EntryView> {
    tracing::warn!(
        target: super::LOG_TARGET,
        operation = super::LIST_OPERATION,
        file_id,
        error_code = "object_missing",
        "an indexed file has no readable original; it is left out of the listing"
    );
    None
}

/// `name` beneath the folder `folder` (`""` for the root). Pure.
fn join(folder: &str, name: &str) -> String {
    match folder {
        "" => name.to_string(),
        folder => format!("{folder}/{name}"),
    }
}
