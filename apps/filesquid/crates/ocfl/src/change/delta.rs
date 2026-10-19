//! Delta computation (OCFL-S-014): classify every logical path of a working
//! state against a base version by digest. Pure: no storage is touched.

use std::collections::{BTreeMap, BTreeSet};

use super::error::ChangeError;
use crate::inventory::{Inventory, VersionNum};
use crate::read::ReadError;

/// A logical state as logical path → content digest (lowercase hex).
pub type WorkingState = BTreeMap<String, String>;

/// How one logical path of a working state differs from its base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathChange {
    /// The path is new and its content is not the content of a path the
    /// base had or the object held before.
    Added {
        /// The logical path.
        path: String,
        /// Its digest.
        digest: String,
        /// Whether the bytes are new to the object's `manifest` and will be
        /// written; `false` for a copy of content the object already holds.
        new_bytes: bool,
    },
    /// The path exists in the base with other content.
    Changed {
        /// The logical path.
        path: String,
        /// Its digest in the base.
        from: String,
        /// Its new digest.
        to: String,
        /// Whether the bytes are new to the `manifest` and will be written.
        new_bytes: bool,
    },
    /// The content of a base path that is gone appears at a new path. No
    /// bytes are written.
    Renamed {
        /// The path in the base.
        from: String,
        /// The new path.
        to: String,
        /// The content digest.
        digest: String,
    },
    /// The path is gone. Its bytes stay in the object for history.
    Deleted {
        /// The logical path.
        path: String,
        /// Its digest in the base.
        digest: String,
    },
    /// The path is new to the base and holds content that an earlier
    /// version held but the base does not. No bytes are written.
    Reinstated {
        /// The logical path.
        path: String,
        /// The content digest, already in the `manifest`.
        digest: String,
    },
}

impl PathChange {
    /// The path the change is listed under: the new path of a rename,
    /// otherwise the changed path.
    pub fn path(&self) -> &str {
        match self {
            Self::Added { path, .. }
            | Self::Changed { path, .. }
            | Self::Deleted { path, .. }
            | Self::Reinstated { path, .. } => path,
            Self::Renamed { to, .. } => to,
        }
    }

    /// Whether committing the change writes bytes into the object.
    pub fn writes_bytes(&self) -> bool {
        match self {
            Self::Added { new_bytes, .. } | Self::Changed { new_bytes, .. } => *new_bytes,
            _ => false,
        }
    }
}

/// The change set from version `base` of `inventory` to `working`, by
/// digest (OCFL-S-014). `None` for `inventory` or `base` means an empty
/// base (a new object): `base` is not read without an inventory. Digests are compared without case; content counts
/// as known (not new bytes) when the object's `manifest` holds it.
///
/// Changes are ordered by [`PathChange::path`]. A deleted path and a new
/// path with the same digest are a rename; when one digest has several,
/// they pair in path order and the rest stay added or deleted.
///
/// # Errors
///
/// [`ChangeError::Read`] with [`ReadError::VersionNotFound`] when `base` is
/// not a version of `inventory`.
pub fn compute_delta(
    inventory: Option<&Inventory>,
    base: Option<&VersionNum>,
    working: &WorkingState,
) -> Result<Vec<PathChange>, ChangeError> {
    let before = match (inventory, base) {
        (Some(inventory), Some(version)) => state_digests(Some(inventory), version)?,
        _ => WorkingState::new(),
    };
    let known = inventory.map(manifest_digests).unwrap_or_default();
    Ok(classify(&before, &lowercase(working), &known))
}

/// The changes from version `from` to version `to`. Content counts as
/// known when a version before `to` held it, or, for a reverse diff (`to`
/// before `from`), when any version up to `from` held it, `to` included.
///
/// # Errors
///
/// [`ReadError::VersionNotFound`] for a version the inventory lacks.
pub(crate) fn diff_versions(
    inventory: &Inventory,
    from: &VersionNum,
    to: &VersionNum,
) -> Result<Vec<PathChange>, ChangeError> {
    let before = state_digests(Some(inventory), from)?;
    let after = state_digests(Some(inventory), to)?;
    let held = if to < from {
        inventory.versions.range(..=from)
    } else {
        inventory.versions.range(..to)
    };
    let known = held
        .flat_map(|(_, block)| block.state.keys())
        .map(|digest| digest.to_ascii_lowercase())
        .collect();
    Ok(classify(&before, &after, &known))
}

/// The logical state of `version` as path → lowercase digest.
///
/// # Errors
///
/// [`ReadError::VersionNotFound`] when the inventory lacks `version`.
pub(crate) fn state_digests(
    inventory: Option<&Inventory>,
    version: &VersionNum,
) -> Result<WorkingState, ChangeError> {
    let block = inventory
        .and_then(|inv| inv.versions.get(version))
        .ok_or_else(|| ReadError::VersionNotFound {
            version: version.to_string(),
        })?;
    let mut state = WorkingState::new();
    for (digest, paths) in &block.state {
        for path in paths {
            state.insert(path.clone(), digest.to_ascii_lowercase());
        }
    }
    Ok(state)
}

/// Every digest of the `manifest`, lowercase.
pub(crate) fn manifest_digests(inventory: &Inventory) -> BTreeSet<String> {
    inventory
        .manifest
        .keys()
        .map(|digest| digest.to_ascii_lowercase())
        .collect()
}

/// `state` with lowercase digests.
fn lowercase(state: &WorkingState) -> WorkingState {
    state
        .iter()
        .map(|(path, digest)| (path.clone(), digest.to_ascii_lowercase()))
        .collect()
}

/// Classify `after` against `before`; `known` digests are not new bytes.
pub(crate) fn classify(
    before: &WorkingState,
    after: &WorkingState,
    known: &BTreeSet<String>,
) -> Vec<PathChange> {
    let mut changes = changed(before, after, known);
    let deleted = only_in(before, after);
    let added = only_in(after, before);
    let in_base: BTreeSet<&String> = before.values().collect();
    changes.extend(pair_added(deleted, added, known, &in_base));
    changes.sort_by(|a, b| a.path().cmp(b.path()));
    changes
}

/// Paths in both states whose digests differ.
fn changed(
    before: &WorkingState,
    after: &WorkingState,
    known: &BTreeSet<String>,
) -> Vec<PathChange> {
    after
        .iter()
        .filter_map(|(path, to)| {
            let from = before.get(path)?;
            (from != to).then(|| PathChange::Changed {
                path: path.clone(),
                from: from.clone(),
                to: to.clone(),
                new_bytes: !known.contains(to),
            })
        })
        .collect()
}

/// Paths of `state` missing from `other`, grouped by digest, each group in
/// path order.
fn only_in(state: &WorkingState, other: &WorkingState) -> BTreeMap<String, Vec<String>> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, digest) in state.iter().filter(|(p, _)| !other.contains_key(*p)) {
        grouped
            .entry(digest.clone())
            .or_default()
            .push(path.clone());
    }
    grouped
}

/// Pair deleted and new paths of one digest as renames; classify the
/// unpaired new paths as reinstated or added, and the rest as deleted.
fn pair_added(
    mut deleted: BTreeMap<String, Vec<String>>,
    added: BTreeMap<String, Vec<String>>,
    known: &BTreeSet<String>,
    in_base: &BTreeSet<&String>,
) -> Vec<PathChange> {
    let mut changes = Vec::new();
    for (digest, new_paths) in added {
        let mut old = deleted.remove(&digest).unwrap_or_default().into_iter();
        for to in new_paths {
            changes.push(match old.next() {
                Some(from) => PathChange::Renamed {
                    from,
                    to,
                    digest: digest.clone(),
                },
                None => new_path(to, &digest, known, in_base),
            });
        }
        changes.extend(old.map(|path| PathChange::Deleted {
            path,
            digest: digest.clone(),
        }));
    }
    for (digest, paths) in deleted {
        changes.extend(paths.into_iter().map(|path| PathChange::Deleted {
            path,
            digest: digest.clone(),
        }));
    }
    changes
}

/// A new path that is not a rename: reinstated when the object holds its
/// content but the base does not, otherwise added.
fn new_path(
    path: String,
    digest: &str,
    known: &BTreeSet<String>,
    in_base: &BTreeSet<&String>,
) -> PathChange {
    let digest = digest.to_string();
    let is_known = known.contains(&digest);
    if is_known && !in_base.contains(&digest) {
        return PathChange::Reinstated { path, digest };
    }
    PathChange::Added {
        path,
        digest,
        new_bytes: !is_known,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state from `(path, digest)` pairs.
    fn state(pairs: &[(&str, &str)]) -> WorkingState {
        pairs
            .iter()
            .map(|(p, d)| (p.to_string(), d.to_string()))
            .collect()
    }

    /// A digest set.
    fn set(digests: &[&str]) -> BTreeSet<String> {
        digests.iter().map(|d| d.to_string()).collect()
    }

    #[test]
    fn several_paths_of_one_digest_pair_in_path_order() {
        let before = state(&[("a", "x"), ("b", "x")]);
        let after = state(&[("c", "x"), ("d", "x"), ("e", "x")]);
        let changes = classify(&before, &after, &set(&["x"]));
        assert_eq!(
            changes,
            [
                PathChange::Renamed {
                    from: "a".into(),
                    to: "c".into(),
                    digest: "x".into()
                },
                PathChange::Renamed {
                    from: "b".into(),
                    to: "d".into(),
                    digest: "x".into()
                },
                PathChange::Added {
                    path: "e".into(),
                    digest: "x".into(),
                    new_bytes: false
                },
            ]
        );
    }

    #[test]
    fn a_change_back_to_older_content_writes_no_bytes() {
        let before = state(&[("a", "new")]);
        let after = state(&[("a", "old")]);
        let changes = classify(&before, &after, &set(&["old", "new"]));
        assert!(!changes[0].writes_bytes());
        assert_eq!(changes[0].path(), "a");
    }

    #[test]
    fn digests_compare_without_case() {
        let upper = "AB".repeat(32);
        let before = state(&[("a", &upper.to_ascii_lowercase())]);
        let working = state(&[("a", &upper)]);
        assert!(classify(&before, &lowercase(&working), &set(&[])).is_empty());
    }

    #[test]
    fn an_unchanged_state_has_no_changes() {
        let both = state(&[("a", "x"), ("b", "y")]);
        assert!(classify(&both, &both, &set(&["x", "y"])).is_empty());
    }
}
