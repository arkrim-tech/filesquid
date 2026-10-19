//! Folder listing by logical-path prefix at a pinned version, with cursor
//! paging (OCFL-S-012). OCFL stores files only: a folder is implied by the
//! logical paths below it and exists while one of them does.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::error::ViewError;
use super::facts::{last_change, type_hint, version_info};
use super::{check_prefix, MAX_PAGE_ENTRIES};
use crate::digest::DigestAlgorithm;
use crate::inventory::VersionNum;
use crate::read::{LogicalFile, Object};
use crate::storage::StorageBackend;

/// A folder implied by logical paths below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    /// The folder's name (one path segment).
    pub name: String,
    /// Its logical-path prefix, without a trailing `/`.
    pub path: String,
}

/// A logical file in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// The file's name (the last path segment).
    pub name: String,
    /// Its logical path.
    pub path: String,
    /// Its content digest, as the inventory writes it.
    pub digest: String,
    /// Its size in bytes, read from the storage backend.
    pub size: u64,
    /// The version that last set its content, up to the pinned version.
    pub modified_version: VersionNum,
    /// That version's `created` value.
    pub modified: String,
    /// The lowercase extension, a display hint only.
    pub type_hint: Option<String>,
}

/// One listing entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// An implied folder.
    Folder(FolderEntry),
    /// A logical file.
    File(FileEntry),
}

/// One page of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPage {
    /// The object identifier.
    pub object_id: String,
    /// The pinned version.
    pub version: VersionNum,
    /// The listed prefix (`""` for the root).
    pub prefix: String,
    /// Folders first, then files, each in name order.
    pub entries: Vec<Entry>,
    /// The cursor of the next page; `None` on the last page.
    pub next: Option<String>,
}

impl FolderPage {
    /// The page as JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "objectId": self.object_id,
            "version": self.version.to_string(),
            "prefix": self.prefix,
            "entries": self.entries.iter().map(entry_json).collect::<Vec<_>>(),
            "next": self.next,
        })
    }
}

/// One entry as JSON, tagged by `kind`.
fn entry_json(entry: &Entry) -> Value {
    match entry {
        Entry::Folder(folder) => json!({
            "kind": "folder",
            "name": folder.name,
            "path": folder.path,
        }),
        Entry::File(file) => json!({
            "kind": "file",
            "name": file.name,
            "path": file.path,
            "digest": file.digest,
            "size": file.size,
            "modifiedVersion": file.modified_version.to_string(),
            "modified": file.modified,
            "typeHint": file.type_hint,
        }),
    }
}

/// A listing key: folders sort before files, each by name. The cursor is
/// opaque: a tag of the listing it belongs to (object, version and prefix)
/// and the slot's text form (`d:<name>` or `f:<name>`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    /// A folder name.
    Folder(String),
    /// A file name.
    File(String),
}

impl Slot {
    /// The folder or file name.
    fn name(&self) -> &str {
        match self {
            Self::Folder(name) | Self::File(name) => name,
        }
    }

    /// The cursor naming this slot in the listing tagged `context`.
    fn cursor(&self, context: &str) -> String {
        match self {
            Self::Folder(name) => format!("{context}:d:{name}"),
            Self::File(name) => format!("{context}:f:{name}"),
        }
    }

    /// The slot a cursor of the listing tagged `context` names.
    fn parse(cursor: &str, context: &str) -> Result<Self, ViewError> {
        let slot = cursor
            .strip_prefix(context)
            .and_then(|rest| rest.strip_prefix(':'))
            .and_then(|rest| rest.split_once(':'));
        match slot {
            Some(("d", name)) if !name.is_empty() => Ok(Self::Folder(name.to_string())),
            Some(("f", name)) if !name.is_empty() => Ok(Self::File(name.to_string())),
            _ => Err(ViewError::InvalidCursor {
                cursor: cursor.chars().take(64).collect(),
            }),
        }
    }
}

/// The tag of one listing: the first 16 hex digits of the sha256 of its
/// object identifier, version and prefix, so a cursor from another listing
/// is refused rather than skipping entries.
fn listing_context(object_id: &str, version: &VersionNum, prefix: &str) -> String {
    let text = format!("{object_id}\n{version}\n{prefix}");
    let mut digest = DigestAlgorithm::Sha256.digest_hex(text.as_bytes());
    digest.truncate(16);
    digest
}

/// List the folder `prefix` (`""` for the root) of `version`: the implied
/// folders and files directly below it, at most `limit` entries after
/// `cursor`. File sizes are read from the backend for this page only.
///
/// # Errors
///
/// [`ViewError::InvalidRequest`] for a `limit` outside 1 to
/// [`MAX_PAGE_ENTRIES`] or an invalid prefix; [`ViewError::InvalidCursor`];
/// [`ViewError::FolderNotFound`] when no file lies below a non-root
/// `prefix` (the root of an empty version is an empty page);
/// [`ViewError::Read`] for an unknown version; or a backend error.
pub async fn list_folder<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    prefix: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<FolderPage, ViewError> {
    if limit == 0 || limit > MAX_PAGE_ENTRIES {
        return Err(ViewError::request(format!(
            "page size must be 1 to {MAX_PAGE_ENTRIES}"
        )));
    }
    let prefix = check_prefix(prefix)?;
    let context = listing_context(object.id(), version, &prefix);
    let after = cursor.map(|c| Slot::parse(c, &context)).transpose()?;
    let state = object.state(version)?;
    let slots = children(state.files(), &prefix);
    if slots.is_empty() && !prefix.is_empty() {
        return Err(ViewError::FolderNotFound {
            prefix,
            version: version.to_string(),
        });
    }
    let mut page: Vec<(Slot, Option<&LogicalFile>)> = slots
        .into_iter()
        .filter(|(slot, _)| after.as_ref().is_none_or(|a| slot > a))
        .take(limit + 1)
        .collect();
    let next = (page.len() > limit).then(|| page[limit - 1].0.cursor(&context));
    page.truncate(limit);
    let mut entries = Vec::with_capacity(page.len());
    for (slot, file) in page {
        entries.push(match (slot, file) {
            (Slot::File(name), Some(file)) => {
                Entry::File(file_entry(object, version, &name, file).await?)
            }
            (slot, _) => Entry::Folder(folder_entry(&prefix, slot.name())),
        });
    }
    Ok(FolderPage {
        object_id: object.id().to_string(),
        version: version.clone(),
        prefix,
        entries,
        next,
    })
}

/// The folders and files directly below `prefix`, in listing order.
fn children<'f>(
    files: impl Iterator<Item = &'f LogicalFile>,
    prefix: &str,
) -> BTreeMap<Slot, Option<&'f LogicalFile>> {
    let mut slots = BTreeMap::new();
    for file in files {
        let rest = match prefix {
            "" => Some(file.path.as_str()),
            dir => file
                .path
                .strip_prefix(dir)
                .and_then(|r| r.strip_prefix('/')),
        };
        let Some(rest) = rest else {
            continue;
        };
        match rest.split_once('/') {
            Some((folder, _)) => slots.insert(Slot::Folder(folder.to_string()), None),
            None => slots.insert(Slot::File(rest.to_string()), Some(file)),
        };
    }
    slots
}

/// The folder entry `name` below `prefix`.
fn folder_entry(prefix: &str, name: &str) -> FolderEntry {
    let path = match prefix {
        "" => name.to_string(),
        dir => format!("{dir}/{name}"),
    };
    FolderEntry {
        name: name.to_string(),
        path,
    }
}

/// The file entry for `file`, named `name`; its size is read from the
/// backend.
async fn file_entry<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    name: &str,
    file: &LogicalFile,
) -> Result<FileEntry, ViewError> {
    let size = object
        .backend()
        .head(&object.content_key(file)?)
        .await?
        .size;
    let inventory = object.inventory();
    let modified_version = last_change(inventory, version, &file.path, &file.digest);
    // A version `last_change` returns is listed, so its record exists.
    let modified = version_info(inventory, &modified_version)
        .map(|info| info.created)
        .unwrap_or_default();
    Ok(FileEntry {
        name: name.to_string(),
        path: file.path.clone(),
        digest: file.digest.clone(),
        size,
        modified_version,
        modified,
        type_hint: type_hint(name),
    })
}
