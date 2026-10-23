//! Copies land outside OCFL roots (OCFL-S-019; product vision T-036;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Direct inspection and copy MUST NOT authorize in-place inventory or
//! signed-original edits").
//!
//! [`Object::copy_file`](super::Object::copy_file) writes to a destination
//! the caller chooses, on any backend, the source's own included. Before it
//! writes, [`refuse_copy_into_ocfl_root`] checks every directory above the
//! destination key, from the backend root down, for an OCFL object or
//! storage-root declaration (`0=ocfl_object_<version>`, `0=ocfl_<version>`)
//! of any version, known to this engine or not. A destination below one is
//! refused, so a copy can never replace an inventory, a sidecar or stored
//! content, nor add files to an object. Only the writer ([`crate::write`])
//! changes an object, and only by adding a version.
//!
//! Each directory is asked for its declarations with one listing of the
//! keys that start with `<dir>/0=ocfl_`, at most [`MAX_LIST_PAGE`] keys.
//! Backends list only what such a prefix can match: the filesystem backend
//! reads that directory's entries and enters only subdirectories whose
//! names start with `0=ocfl_`, pruning every other subtree unread, so the
//! check never walks a whole root.
//!
//! A refusal warns with the destination, the declaring root and the
//! [`CopyConcern`]: the identifier of the object the copy concerns and, when
//! the call knows it, its version, as the `object` and `version` fields a
//! host journals as the object reference (OCFL-S-025; API-S-031).

use super::error::ReadError;
use crate::inventory::VersionNum;
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE};

/// Most directory levels above a destination key that are checked; a
/// deeper key is refused before any check.
pub(crate) const MAX_DESTINATION_DEPTH: usize = 64;

/// The file-name prefix shared by every OCFL object declaration
/// (`0=ocfl_object_<version>`) and storage-root declaration
/// (`0=ocfl_<version>`), whatever the version.
const DECLARATION_PREFIX: &str = "0=ocfl_";

/// The object a guarded copy concerns, named in the guard's warning
/// (OCFL-S-025): the source object of a copy or checkout with the version it
/// reads, or the object a staging area belongs to, which has no version.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CopyConcern<'c> {
    /// The object's identifier (the inventory `id`).
    pub(crate) object: &'c str,
    /// The version the copy reads, when the call knows one.
    pub(crate) version: Option<&'c VersionNum>,
}

/// Refuse `key` on `dest` when a directory above it (the backend root
/// included) is an OCFL object root or storage root of any version, and
/// warn naming `concern`.
///
/// # Errors
///
/// [`ReadError::CopyIntoOcflRoot`] naming the declaring directory;
/// [`ReadError::Storage`] for a key deeper than [`MAX_DESTINATION_DEPTH`]
/// levels, a directory whose declaration listing overflows one page, or a
/// backend failure.
pub(crate) async fn refuse_copy_into_ocfl_root<D: StorageBackend>(
    dest: &D,
    key: &StorageKey,
    concern: CopyConcern<'_>,
) -> Result<(), ReadError> {
    let segments: Vec<&str> = key.segments().collect();
    if segments.len() > MAX_DESTINATION_DEPTH {
        return Err(StorageError::LimitExceeded {
            what: "directory levels above a copy destination",
            limit: MAX_DESTINATION_DEPTH as u64,
        }
        .into());
    }
    for depth in 0..segments.len() {
        let dir = segments[..depth].join("/");
        if is_ocfl_root(dest, &dir).await? {
            warn_refused(key, &dir, concern);
            return Err(ReadError::CopyIntoOcflRoot {
                key: key.to_string(),
                root: dir,
            });
        }
    }
    Ok(())
}

/// Warn that the copy to `key` was refused because `root` declares an OCFL
/// root, naming the object and, when known, the version of `concern`.
fn warn_refused(key: &StorageKey, root: &str, concern: CopyConcern<'_>) {
    let object = concern.object;
    match concern.version {
        Some(version) => tracing::warn!(
            destination = %key, root, object, version = %version,
            "refused a copy into an OCFL root"
        ),
        None => tracing::warn!(
            destination = %key, root, object,
            "refused a copy into an OCFL root"
        ),
    }
}

/// Whether the directory `dir` of `dest` (`""` for the backend root)
/// directly holds a file named like an OCFL object or storage-root
/// declaration of any version. Keys below a subdirectory that is merely
/// named like one are not declarations.
///
/// # Errors
///
/// [`StorageError::LimitExceeded`] when one page of [`MAX_LIST_PAGE`] keys
/// under the declaration prefix holds no declaration but more keys follow,
/// so the answer is never guessed; or a backend failure.
async fn is_ocfl_root<D: StorageBackend>(dest: &D, dir: &str) -> Result<bool, StorageError> {
    let parent = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let prefix = format!("{parent}{DECLARATION_PREFIX}");
    let page = dest.list(&prefix, None, MAX_LIST_PAGE).await?;
    let declared = page
        .keys
        .iter()
        .filter_map(|found| found.as_str().strip_prefix(&prefix))
        .any(|rest| !rest.contains('/'));
    if declared || page.next.is_none() {
        return Ok(declared);
    }
    Err(StorageError::LimitExceeded {
        what: "keys named like an OCFL declaration in one directory",
        limit: MAX_LIST_PAGE as u64,
    })
}
