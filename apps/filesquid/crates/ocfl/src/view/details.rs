//! File details and per-file version history (OCFL-S-012): storage facts
//! only, no embedded or descriptive metadata (C-055).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::check_path;
use super::error::ViewError;
use super::facts::{digest_at, file_name, last_change, type_hint, version_info, version_json};
use crate::inventory::{Version, VersionNum};
use crate::read::{Object, ReadError, VersionInfo};
use crate::storage::StorageBackend;

/// Storage facts about one logical file at a pinned version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDetails {
    /// The object identifier.
    pub object_id: String,
    /// The pinned version.
    pub version: VersionNum,
    /// The logical path.
    pub path: String,
    /// The last path segment.
    pub name: String,
    /// The content digest, as the inventory writes it.
    pub digest: String,
    /// The object's digest algorithm (`sha256` or `sha512`).
    pub digest_algorithm: String,
    /// The size in bytes, read from the storage backend.
    pub size: u64,
    /// The content path relative to the object root, from the manifest.
    pub content_path: String,
    /// Every fixity value the inventory records for this content, by
    /// algorithm.
    pub fixity: BTreeMap<String, Vec<String>>,
    /// The version that last set the file's content, with its `created`,
    /// `user` and `message`.
    pub last_change: VersionInfo,
    /// The lowercase extension, a display hint only.
    pub type_hint: Option<String>,
}

impl FileDetails {
    /// The details as JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "objectId": self.object_id,
            "version": self.version.to_string(),
            "path": self.path,
            "name": self.name,
            "digest": self.digest,
            "digestAlgorithm": self.digest_algorithm,
            "size": self.size,
            "contentPath": self.content_path,
            "fixity": self.fixity,
            "lastChange": version_json(&self.last_change),
            "typeHint": self.type_hint,
        })
    }
}

/// Storage facts about the logical file `path` of `version`. The size is
/// read from the backend; nothing else is read from the content.
///
/// # Errors
///
/// [`ViewError::Read`] with `VersionNotFound` or `PathNotFound`, or a
/// backend error.
pub async fn file_details<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
) -> Result<FileDetails, ViewError> {
    check_path(path)?;
    let state = object.state(version)?;
    let file = state.get(path).ok_or_else(|| ReadError::PathNotFound {
        version: version.to_string(),
        path: path.to_string(),
    })?;
    let size = object
        .backend()
        .head(&object.content_key(file)?)
        .await?
        .size;
    let inventory = object.inventory();
    let changed = last_change(inventory, version, path, &file.digest);
    let last_change =
        version_info(inventory, &changed).ok_or_else(|| ReadError::VersionNotFound {
            version: changed.to_string(),
        })?;
    Ok(FileDetails {
        object_id: object.id().to_string(),
        version: version.clone(),
        path: path.to_string(),
        name: file_name(path).to_string(),
        digest: file.digest.clone(),
        digest_algorithm: inventory.digest_algorithm.name().to_string(),
        size,
        content_path: file.content_path.clone(),
        fixity: fixity_values(object, &file.content_path),
        last_change,
        type_hint: type_hint(file_name(path)),
    })
}

/// Every fixity value the inventory lists for `content_path`.
fn fixity_values<B: StorageBackend>(
    object: &Object<'_, B>,
    content_path: &str,
) -> BTreeMap<String, Vec<String>> {
    let mut values = BTreeMap::new();
    for (algorithm, map) in object.inventory().fixity.iter().flatten() {
        let found: Vec<String> = map
            .iter()
            .filter(|(_, paths)| paths.iter().any(|p| p == content_path))
            .map(|(digest, _)| digest.clone())
            .collect();
        if !found.is_empty() {
            values.insert(algorithm.clone(), found);
        }
    }
    values
}

/// How a version changed one logical file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    /// The path appeared (first added, or added again after a delete).
    Added,
    /// The path's content changed.
    Changed,
    /// The path was removed.
    Deleted,
}

impl FileChange {
    /// The change's JSON name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Deleted => "deleted",
        }
    }
}

/// One version that changed a logical file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistoryEntry {
    /// The version and its `created`, `user` and `message`.
    pub version: VersionInfo,
    /// What the version did to the file.
    pub change: FileChange,
    /// The file's digest after the version (`None` once deleted).
    pub digest: Option<String>,
}

/// Every version that added, changed or deleted the logical file `path`,
/// oldest first. Versions that left it as it was are not listed; digests
/// are as the inventory writes them, compared without case. Pure.
///
/// # Errors
///
/// [`ViewError::InvalidRequest`] for a path that breaks the OCFL
/// logical-path rules.
pub fn file_history<B: StorageBackend>(
    object: &Object<'_, B>,
    path: &str,
) -> Result<Vec<FileHistoryEntry>, ViewError> {
    check_path(path)?;
    let inventory = object.inventory();
    let mut history = Vec::new();
    let mut before: Option<&String> = None;
    for (version, block) in &inventory.versions {
        let after = digest_at(block, path);
        if let Some(change) = file_change(before, after) {
            history.push(FileHistoryEntry {
                version: block_info(version, block),
                change,
                digest: after.cloned(),
            });
        }
        before = after;
    }
    Ok(history)
}

/// What a version did to a path that held `before` and now holds `after`.
fn file_change(before: Option<&String>, after: Option<&String>) -> Option<FileChange> {
    match (before, after) {
        (None, Some(_)) => Some(FileChange::Added),
        (Some(_), None) => Some(FileChange::Deleted),
        (Some(old), Some(new)) if !old.eq_ignore_ascii_case(new) => Some(FileChange::Changed),
        _ => None,
    }
}

/// The record of one version block.
fn block_info(version: &VersionNum, block: &Version) -> VersionInfo {
    VersionInfo {
        version: version.clone(),
        created: block.created.clone(),
        message: block.message.clone(),
        user: block.user.clone(),
    }
}

/// A file history as a JSON array.
pub fn history_json(history: &[FileHistoryEntry]) -> Value {
    let entries = history.iter().map(|entry| {
        json!({
            "version": version_json(&entry.version),
            "change": entry.change.name(),
            "digest": entry.digest,
        })
    });
    Value::Array(entries.collect())
}

/// Every committed version of the object, oldest first.
pub fn object_versions<B: StorageBackend>(object: &Object<'_, B>) -> Vec<VersionInfo> {
    object.history()
}
