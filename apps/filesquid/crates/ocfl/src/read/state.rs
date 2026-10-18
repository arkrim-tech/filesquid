//! Logical state at a caller-pinned version, resolved through the
//! inventory's `state` and `manifest` only (OCFL-S-004, T-039). Pure: no
//! storage is touched.
//!
//! Directory names are never used to find a file: a logical file whose bytes
//! have not changed since an earlier version resolves to the content path in
//! that earlier version's directory, because the manifest says so.

use std::collections::BTreeMap;

use super::error::ReadError;
use crate::digest::DigestAlgorithm;
use crate::inventory::{Inventory, VersionNum};

/// One logical file of a pinned version. With the state's object id and
/// version it carries every field T-039 names for committed logical state
/// (the storage-root-wide enumerator itself is IO-0.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFile {
    /// The logical path, `/`-separated.
    pub path: String,
    /// The content digest, as the inventory writes it.
    pub digest: String,
    /// Where the bytes are: a content path relative to the object root, from
    /// the manifest.
    pub content_path: String,
}

/// The logical state of one object at one pinned version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalState {
    object_id: String,
    version: VersionNum,
    digest_algorithm: DigestAlgorithm,
    files: BTreeMap<String, LogicalFile>,
}

impl LogicalState {
    /// The object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The pinned version.
    pub fn version(&self) -> &VersionNum {
        &self.version
    }

    /// The algorithm of every digest in this state.
    pub fn digest_algorithm(&self) -> DigestAlgorithm {
        self.digest_algorithm
    }

    /// The logical file at `path`, if the version has one.
    pub fn get(&self, path: &str) -> Option<&LogicalFile> {
        self.files.get(path)
    }

    /// Every logical file, in logical-path order.
    pub fn files(&self) -> impl Iterator<Item = &LogicalFile> {
        self.files.values()
    }

    /// Every logical path, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// The number of logical files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the version has no logical files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Resolve the logical state of `version`, which the caller pins (`head` or
/// any named version), through `inventory`'s `state` and `manifest`.
///
/// # Errors
///
/// [`ReadError::VersionNotFound`] when the inventory does not list
/// `version` (an unpublished version above `head` included), or E050 when a
/// state digest has no manifest entry (only an inventory built by hand can
/// have one; [`Inventory::parse`] refuses it).
pub fn resolve_state(
    inventory: &Inventory,
    version: &VersionNum,
) -> Result<LogicalState, ReadError> {
    let block = inventory
        .versions
        .get(version)
        .ok_or_else(|| ReadError::VersionNotFound {
            version: version.to_string(),
        })?;
    let mut files = BTreeMap::new();
    for (digest, paths) in &block.state {
        let content_path = content_path_of(inventory, digest)?;
        for path in paths {
            let file = LogicalFile {
                path: path.clone(),
                digest: digest.clone(),
                content_path: content_path.to_string(),
            };
            files.insert(path.clone(), file);
        }
    }
    Ok(LogicalState {
        object_id: inventory.id.clone(),
        version: version.clone(),
        digest_algorithm: inventory.digest_algorithm,
        files,
    })
}

/// The first content path the manifest lists for `digest`. Every path a
/// digest lists holds the same bytes, so any one of them serves.
fn content_path_of<'i>(inventory: &'i Inventory, digest: &str) -> Result<&'i str, ReadError> {
    inventory
        .manifest
        .get(digest)
        .and_then(|paths| paths.first())
        .map(String::as_str)
        .ok_or_else(|| {
            ReadError::invalid(
                "E050",
                format!("/manifest/{digest}"),
                "a state digest has no manifest entry",
            )
        })
}
