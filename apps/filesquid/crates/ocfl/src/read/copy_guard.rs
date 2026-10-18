//! Copies land outside OCFL roots (OCFL-S-019; product vision T-036;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Direct inspection and copy MUST NOT authorize in-place inventory or
//! signed-original edits").
//!
//! [`Object::copy_file`](super::Object::copy_file) writes to a destination
//! the caller chooses, on any backend, the source's own included. Before it
//! writes, [`refuse_copy_into_ocfl_root`] checks every directory above the
//! destination key, from the backend root down, for an OCFL object or
//! storage-root declaration (`0=ocfl_object_<version>`, `0=ocfl_<version>`).
//! A destination below one is refused, so a copy can never replace an
//! inventory, a sidecar or stored content, nor add files to an object. Only
//! the writer ([`crate::write`]) changes an object, and only by adding a
//! version.

use super::error::ReadError;
use crate::namaste::{Declaration, SpecVersion};
use crate::storage::{StorageBackend, StorageError, StorageKey};

/// Most directory levels above a destination key that are checked; a
/// deeper key is refused before any check.
pub(crate) const MAX_DESTINATION_DEPTH: usize = 64;

/// The OCFL versions whose declarations mark a root.
const SPECS: [SpecVersion; 2] = [SpecVersion::V1_0, SpecVersion::V1_1];

/// Refuse `key` on `dest` when a directory above it (the backend root
/// included) is an OCFL object root or storage root.
///
/// # Errors
///
/// [`ReadError::CopyIntoOcflRoot`] naming the declaring directory;
/// [`ReadError::Storage`] for a key deeper than [`MAX_DESTINATION_DEPTH`]
/// levels or a backend failure.
pub(crate) async fn refuse_copy_into_ocfl_root<D: StorageBackend>(
    dest: &D,
    key: &StorageKey,
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
            tracing::warn!(destination = %key, root = %dir, "refused a copy into an OCFL root");
            return Err(ReadError::CopyIntoOcflRoot {
                key: key.to_string(),
                root: dir,
            });
        }
    }
    Ok(())
}

/// Whether the directory `dir` of `dest` (`""` for the backend root)
/// holds an OCFL object or storage-root declaration of a known version.
async fn is_ocfl_root<D: StorageBackend>(dest: &D, dir: &str) -> Result<bool, StorageError> {
    for spec in SPECS {
        for declaration in [Declaration::object(spec), Declaration::storage_root(spec)] {
            let name = declaration.file_name();
            let raw = if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            };
            if dest.exists(&StorageKey::new(&raw)?).await? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
