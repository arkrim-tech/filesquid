//! The Git guard and plain-hierarchy guard (OCFL-S-001,
//! [ADR 016](../../../resources/architecture/016-rust-ocfl-library.md)
//! layout constraints).
//!
//! Git MUST NOT be the OCFL store, and a `.git` directory MUST NOT appear in
//! an OCFL object root or storage root. Hard links, soft links and empty
//! directories MUST NOT be used under the storage hierarchy.
//!
//! These guards reach storage only through a
//! [`StorageBackend`], so they run on every backend and target, including
//! `wasm32`. Links and empty directories exist only on file systems; the
//! backend reports them through [`StorageBackend::check_hierarchy`].
//!
//! What counts as forbidden Git metadata:
//!
//! - a `.git` entry (a directory, or a file such as a work-tree `gitdir:`
//!   pointer) in the storage root, in any directory of the storage hierarchy
//!   above the objects, or in an object root;
//! - a `.git` entry inside an object's content is preserved content (for
//!   example an archived Git repository) and is allowed.
//!
//! An object root is a directory that holds an OCFL object declaration
//! (`0=ocfl_object_*`) and is not itself inside another object.

use std::collections::BTreeSet;

use crate::error::OcflError;
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE, MAX_WALK_ENTRIES};

/// The name Git gives its repository directory or work-tree pointer file.
const GIT_DIR: &str = ".git";

/// Prefix of the NAMASTE file that declares an OCFL object root.
const OBJECT_DECLARATION_PREFIX: &str = "0=ocfl_object_";

/// Most listing pages the storage-root guard reads: together at most
/// [`MAX_WALK_ENTRIES`] keys, the bound every walk of a root shares.
const MAX_GUARD_PAGES: usize = MAX_WALK_ENTRIES / MAX_LIST_PAGE;

/// Refuse a `.git` entry directly in one root: the storage root (`""`) or an
/// object root (a key prefix, with or without a trailing `/`).
///
/// A `.git` file or any key under `.git/` is refused. An empty `.git`
/// directory holds no keys; [`check_storage_root`] reports it through the
/// backend's hierarchy check as an empty directory.
///
/// # Errors
///
/// [`OcflError::GitDirectoryForbidden`] naming `root` without its trailing
/// `/`; [`OcflError::Storage`] when `root` could escape the storage root or
/// the backend fails.
pub async fn reject_git_in_root<B: StorageBackend>(
    backend: &B,
    root: &str,
) -> Result<(), OcflError> {
    let dir = root.strip_suffix('/').unwrap_or(root);
    let git_key = StorageKey::new(&child(dir, GIT_DIR))?;
    if backend.exists(&git_key).await? {
        return Err(git_forbidden(dir));
    }
    let below = backend.list(&format!("{git_key}/"), None, 1).await?;
    if below.keys.is_empty() {
        return Ok(());
    }
    Err(git_forbidden(dir))
}

/// Check a whole storage root: the backend's hierarchy check first (no soft
/// links, hard links, empty directories or other non-regular entries), then
/// one paged listing of every key for forbidden Git metadata: a `.git`
/// entry in the storage root, in a storage-hierarchy directory above the
/// objects, or in an object root. A `.git` inside an object's content is
/// preserved content and passes. An object root is a directory holding an
/// OCFL object declaration (`0=ocfl_object_*`) that is not itself inside
/// another object.
///
/// The listing reads at most [`MAX_WALK_ENTRIES`] keys; a larger root is
/// reported as over the limit, never passed unchecked.
///
/// # Errors
///
/// [`OcflError::GitDirectoryForbidden`] naming the first directory, in
/// lexical order, that holds forbidden Git metadata; [`OcflError::Storage`]
/// for a hierarchy violation, the key bound or a backend failure.
pub async fn check_storage_root<B: StorageBackend>(backend: &B) -> Result<(), OcflError> {
    backend.check_hierarchy("").await?;
    let layout = scan_layout(backend).await?;
    match layout.forbidden_git_parent() {
        Some(dir) => Err(git_forbidden(dir)),
        None => Ok(()),
    }
}

/// The key of `name` inside the directory `dir` (empty for the root).
fn child(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        return name.to_string();
    }
    format!("{dir}/{name}")
}

/// The Git refusal for the directory `dir`.
fn git_forbidden(dir: &str) -> OcflError {
    OcflError::GitDirectoryForbidden {
        root: dir.to_string(),
    }
}

/// Page through every key of the storage root, recording object roots and
/// Git metadata. Bounded by [`MAX_GUARD_PAGES`].
async fn scan_layout<B: StorageBackend>(backend: &B) -> Result<Layout, OcflError> {
    let mut layout = Layout::default();
    let mut start_after: Option<StorageKey> = None;
    for _ in 0..MAX_GUARD_PAGES {
        let page = backend
            .list("", start_after.as_ref(), MAX_LIST_PAGE)
            .await?;
        page.keys
            .iter()
            .for_each(|key| layout.note_key(key.as_str()));
        if page.next.is_none() {
            return Ok(layout);
        }
        start_after = page.next;
    }
    Err(StorageError::LimitExceeded {
        what: "keys checked by the storage root guard",
        limit: MAX_WALK_ENTRIES as u64,
    }
    .into())
}

/// What the storage-root scan learned about the hierarchy: pure data, no I/O.
#[derive(Debug, Default)]
struct Layout {
    /// Directories (keys without a trailing `/`) that hold an object
    /// declaration.
    object_roots: BTreeSet<String>,
    /// Directories that hold a `.git` entry, outermost per key.
    git_parents: BTreeSet<String>,
}

impl Layout {
    /// Record what one key says: an object declaration marks its directory as
    /// a candidate object root, and its outermost `.git` segment marks the
    /// directory that holds it.
    fn note_key(&mut self, key: &str) {
        let segments: Vec<&str> = key.split('/').collect();
        let Some((name, dirs)) = segments.split_last() else {
            return;
        };
        if name.starts_with(OBJECT_DECLARATION_PREFIX) {
            self.object_roots.insert(dirs.join("/"));
        }
        if let Some(at) = segments.iter().position(|s| *s == GIT_DIR) {
            self.git_parents.insert(segments[..at].join("/"));
        }
    }

    /// The first directory holding `.git` that is not inside an object's
    /// content: the storage root, a storage-hierarchy directory, or an object
    /// root itself.
    fn forbidden_git_parent(&self) -> Option<&str> {
        self.git_parents
            .iter()
            .map(String::as_str)
            .find(|dir| !self.inside_an_object(dir))
    }

    /// Whether `dir` lies strictly below a recorded object root. A
    /// declaration below another object root is content, so only the
    /// outermost roots matter, and they are always proper ancestors here.
    fn inside_an_object(&self, dir: &str) -> bool {
        if dir.is_empty() {
            return false;
        }
        if self.object_roots.contains("") {
            return true;
        }
        dir.match_indices('/')
            .any(|(at, _)| self.object_roots.contains(&dir[..at]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(keys: &[&str]) -> Layout {
        let mut layout = Layout::default();
        keys.iter().for_each(|key| layout.note_key(key));
        layout
    }

    #[test]
    fn the_outermost_git_segment_is_recorded() {
        let found = layout(&["a/.git/modules/b/.git/HEAD"]);
        assert_eq!(found.git_parents.iter().collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn only_proper_ancestors_make_content() {
        let found = layout(&["obj/0=ocfl_object_1.1"]);
        assert!(!found.inside_an_object("obj"));
        assert!(found.inside_an_object("obj/v1/content"));
        assert!(!found.inside_an_object("objx/v1"));
        assert!(!found.inside_an_object(""));
    }

    #[test]
    fn an_object_declared_at_the_storage_root_contains_everything_below() {
        let found = layout(&["0=ocfl_object_1.1", "v1/content/.git/HEAD", ".git"]);
        assert_eq!(found.forbidden_git_parent(), Some(""));
        assert!(found.inside_an_object("v1/content"));
    }
}
