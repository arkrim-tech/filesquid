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
//!   example an archived Git repository) and is allowed (owner decision,
//!   2026-10-18). It may hold only ordinary files and non-empty directories:
//!   links and empty directories inside it are refused by the hierarchy check
//!   like anywhere else.
//!
//! An object root is a directory that holds an OCFL object declaration
//! (`0=ocfl_object_*`) and is not itself inside another object.

use std::collections::BTreeSet;

use crate::error::OcflError;
use crate::storage::{
    validate_prefix, ListPage, StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE,
    MAX_WALK_ENTRIES,
};

/// The name Git gives its repository directory or work-tree pointer file.
const GIT_DIR: &str = ".git";

/// Prefix of the NAMASTE file that declares an OCFL object root.
const OBJECT_DECLARATION_PREFIX: &str = "0=ocfl_object_";

/// Most keys, and most listing requests, the storage-root guard reads: the
/// bound every walk of a root shares. Pages may be short, so requests are
/// bounded separately; each must move the continuation forward.
const MAX_GUARD_KEYS: usize = MAX_WALK_ENTRIES;

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
/// `/`; [`OcflError::Storage`] with [`StorageError::InvalidKey`] when `root`
/// is not a valid prefix (such as `/`, `.` or `..`), or a backend failure.
pub async fn reject_git_in_root<B: StorageBackend>(
    backend: &B,
    root: &str,
) -> Result<(), OcflError> {
    validate_prefix(root)?;
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
/// preserved content and passes when it holds only ordinary files and
/// non-empty directories. An object root is a directory holding an
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
    checked_layout(backend).await.map(|_| ())
}

/// Run [`check_storage_root`] and return what its one listing learned, so
/// object discovery (IO-0.4) needs no second walk of the storage root.
///
/// # Errors
///
/// As [`check_storage_root`].
pub(crate) async fn checked_layout<B: StorageBackend>(backend: &B) -> Result<Layout, OcflError> {
    backend.check_hierarchy("").await?;
    let layout = scan_layout(backend).await?;
    match layout.forbidden_git_parent() {
        Some(dir) => Err(git_forbidden(dir)),
        None => Ok(layout),
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
/// Git metadata. At most [`MAX_GUARD_KEYS`] keys and as many requests; every
/// continuation must move past the previous one.
async fn scan_layout<B: StorageBackend>(backend: &B) -> Result<Layout, OcflError> {
    let mut layout = Layout::default();
    let mut start_after: Option<StorageKey> = None;
    let mut seen = 0usize;
    for _ in 0..=MAX_GUARD_KEYS {
        let page = backend
            .list("", start_after.as_ref(), MAX_LIST_PAGE)
            .await?;
        seen = seen.saturating_add(page.keys.len());
        check_key_bound(seen)?;
        page.keys
            .iter()
            .for_each(|key| layout.note_key(key.as_str()));
        let Some(next) = next_start(&page, start_after.as_ref())? else {
            return Ok(layout);
        };
        start_after = Some(next);
    }
    Err(over_key_bound())
}

/// Fail once the scan has read more than [`MAX_GUARD_KEYS`] keys.
fn check_key_bound(seen: usize) -> Result<(), OcflError> {
    if seen > MAX_GUARD_KEYS {
        return Err(over_key_bound());
    }
    Ok(())
}

/// The error for a storage root above [`MAX_GUARD_KEYS`] keys.
fn over_key_bound() -> OcflError {
    StorageError::LimitExceeded {
        what: "keys checked by the storage root guard",
        limit: MAX_GUARD_KEYS as u64,
    }
    .into()
}

/// The continuation after `page`, or `None` on the last page.
///
/// The [`StorageBackend::list`] contract makes the continuation the last key
/// the page returned. Anything else could skip keys, and with them a `.git`,
/// so it is refused rather than trusted.
///
/// # Errors
///
/// [`StorageError::Backend`] when a page with a continuation is empty, when
/// the continuation is not the page's last key, or when it does not move past
/// `previous` (which would otherwise repeat forever).
fn next_start(
    page: &ListPage,
    previous: Option<&StorageKey>,
) -> Result<Option<StorageKey>, OcflError> {
    let Some(next) = page.next.clone() else {
        return Ok(None);
    };
    let Some(last) = page.keys.last() else {
        return Err(listing_fault(format!(
            "an empty listing page carries the continuation {next}"
        )));
    };
    if *last != next {
        return Err(listing_fault(format!(
            "listing continuation {next} is not the page's last key {last}"
        )));
    }
    if previous.is_some_and(|before| next <= *before) {
        return Err(listing_fault(format!(
            "listing continuation {next} made no progress"
        )));
    }
    Ok(Some(next))
}

/// The error for a backend listing that breaks the paging contract.
fn listing_fault(reason: String) -> OcflError {
    StorageError::Backend { reason }.into()
}

/// What the storage-root scan learned about the hierarchy: pure data, no I/O.
#[derive(Debug, Default)]
pub(crate) struct Layout {
    /// Directories (keys without a trailing `/`) that hold an object
    /// declaration.
    object_roots: BTreeSet<String>,
    /// Directories that hold a `.git` entry, outermost per key.
    git_parents: BTreeSet<String>,
    /// Files directly in the storage root whose names start with `0=`: the
    /// candidates for its conformance declaration.
    root_declarations: BTreeSet<String>,
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
        if dirs.is_empty() && name.starts_with("0=") {
            self.root_declarations.insert((*name).to_string());
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

    /// The names of the `0=` files directly in the storage root.
    pub(crate) fn root_declaration_names(&self) -> impl Iterator<Item = &str> {
        self.root_declarations.iter().map(String::as_str)
    }

    /// The outermost object roots, in lexical order: directories holding an
    /// object declaration that are not inside another object. An object
    /// declared at the storage root itself is reported as `""`, alone.
    pub(crate) fn outermost_object_roots(&self) -> Vec<String> {
        self.object_roots
            .iter()
            .filter(|dir| !self.inside_an_object(dir))
            .cloned()
            .collect()
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
    fn only_outermost_object_roots_are_discovered() {
        let found = layout(&[
            "a/obj/0=ocfl_object_1.1",
            "a/obj/v1/content/inner/0=ocfl_object_1.1",
            "b/0=ocfl_object_1.1",
            "0=ocfl_1.1",
        ]);
        assert_eq!(found.outermost_object_roots(), ["a/obj", "b"]);
        let names: Vec<&str> = found.root_declaration_names().collect();
        assert_eq!(names, ["0=ocfl_1.1"]);
        let at_root = layout(&["0=ocfl_object_1.1", "x/0=ocfl_object_1.1"]);
        assert_eq!(at_root.outermost_object_roots(), [""]);
    }

    #[test]
    fn an_object_declared_at_the_storage_root_contains_everything_below() {
        let found = layout(&["0=ocfl_object_1.1", "v1/content/.git/HEAD", ".git"]);
        assert_eq!(found.forbidden_git_parent(), Some(""));
        assert!(found.inside_an_object("v1/content"));
    }
}
