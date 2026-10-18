//! Version history and the diff of two versions, from one inventory. Pure:
//! no storage is touched.

use std::collections::BTreeMap;

use super::error::ReadError;
use super::state::{resolve_state, LogicalState};
use crate::inventory::{Inventory, User, VersionNum};

/// What the inventory records about one version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// The version.
    pub version: VersionNum,
    /// When it was created: RFC 3339 text as written.
    pub created: String,
    /// Why it was made, when given.
    pub message: Option<String>,
    /// Who made it, when given.
    pub user: Option<User>,
}

/// Every version of `inventory`, oldest first.
pub fn version_history(inventory: &Inventory) -> Vec<VersionInfo> {
    inventory
        .versions
        .iter()
        .map(|(version, block)| VersionInfo {
            version: version.clone(),
            created: block.created.clone(),
            message: block.message.clone(),
            user: block.user.clone(),
        })
        .collect()
}

/// How one logical path differs between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The path exists only in the later version.
    Added {
        /// The logical path.
        path: String,
        /// Its digest.
        digest: String,
    },
    /// The path exists only in the earlier version.
    Deleted {
        /// The logical path.
        path: String,
        /// Its digest in the earlier version.
        digest: String,
    },
    /// The path exists in both with different content.
    Modified {
        /// The logical path.
        path: String,
        /// Its digest in the earlier version.
        from: String,
        /// Its digest in the later version.
        to: String,
    },
    /// The content of a path that is gone appears at a new path.
    Renamed {
        /// The path in the earlier version.
        from: String,
        /// The path in the later version.
        to: String,
        /// The content digest.
        digest: String,
    },
}

impl Change {
    /// The path the change is listed under: the later path of a rename,
    /// otherwise the changed path.
    pub fn path(&self) -> &str {
        match self {
            Self::Added { path, .. } | Self::Deleted { path, .. } | Self::Modified { path, .. } => {
                path
            }
            Self::Renamed { to, .. } => to,
        }
    }
}

/// The changes from version `from` to version `to`, ordered by
/// [`Change::path`]. A deleted path and an added path with the same digest
/// are a rename; when one digest has several, they pair in path order and
/// the rest stay added or deleted.
///
/// # Errors
///
/// As [`resolve_state`] for either version.
pub fn diff_versions(
    inventory: &Inventory,
    from: &VersionNum,
    to: &VersionNum,
) -> Result<Vec<Change>, ReadError> {
    let before = resolve_state(inventory, from)?;
    let after = resolve_state(inventory, to)?;
    let mut changes = modified(&before, &after);
    let deleted = only_in(&before, &after);
    let added = only_in(&after, &before);
    changes.extend(pair_renames(deleted, added));
    changes.sort_by(|a, b| a.path().cmp(b.path()));
    Ok(changes)
}

/// Paths in both states whose digests differ.
fn modified(before: &LogicalState, after: &LogicalState) -> Vec<Change> {
    before
        .files()
        .filter_map(|old| {
            let new = after.get(&old.path)?;
            (new.digest != old.digest).then(|| Change::Modified {
                path: old.path.clone(),
                from: old.digest.clone(),
                to: new.digest.clone(),
            })
        })
        .collect()
}

/// Paths of `state` missing from `other`, grouped by digest, each group in
/// path order.
fn only_in(state: &LogicalState, other: &LogicalState) -> BTreeMap<String, Vec<String>> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in state.files().filter(|f| other.get(&f.path).is_none()) {
        grouped
            .entry(file.digest.clone())
            .or_default()
            .push(file.path.clone());
    }
    grouped
}

/// Pair deleted and added paths of one digest as renames; the unpaired rest
/// stay deleted or added.
fn pair_renames(
    mut deleted: BTreeMap<String, Vec<String>>,
    added: BTreeMap<String, Vec<String>>,
) -> Vec<Change> {
    let mut changes = Vec::new();
    for (digest, new_paths) in added {
        let old_paths = deleted.remove(&digest).unwrap_or_default();
        let mut old = old_paths.into_iter();
        for to in new_paths {
            changes.push(match old.next() {
                Some(from) => Change::Renamed {
                    from,
                    to,
                    digest: digest.clone(),
                },
                None => Change::Added {
                    path: to,
                    digest: digest.clone(),
                },
            });
        }
        changes.extend(old.map(|path| Change::Deleted {
            path,
            digest: digest.clone(),
        }));
    }
    for (digest, paths) in deleted {
        changes.extend(paths.into_iter().map(|path| Change::Deleted {
            path,
            digest: digest.clone(),
        }));
    }
    changes
}
