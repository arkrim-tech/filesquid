//! File objects a library's snapshot does not register (Task DE-1.7; DE-S-010, API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decision 6).
//!
//! An earlier build committed uploaded and imported files as objects `files/<file-id>`
//! without registering them in a home collection. A rebuild of a library's index counts such
//! objects in its journal entry and leaves them unregistered, never placing them in a
//! collection the owner did not choose. A tombstone of an expired file (Task API-F2.15) is
//! purged history, not a file, so it is not counted. A file entity of the earlier form, which names no
//! path or object, does not register its object either.

use super::store::LibrarySnapshot;
use super::CanonicalError;
use filesquid_ocfl::read::StorageRoot;
use filesquid_ocfl::storage::StorageBackend;
use std::collections::HashSet;

/// The folder of a storage root that holds one object per file.
pub const FILES_ROOT: &str = "files";

/// How many file objects of the storage root in `backend` the snapshot does not register:
/// every object at `files/<file-id>` whose file has no registered entity in `snapshot`
/// (all of them when there is no snapshot). The storage root is discovered once, within the
/// engine's listing bound.
///
/// # Errors
/// [`CanonicalError`] when the storage root cannot be opened or listed.
pub async fn unregistered_file_objects<B: StorageBackend>(
    backend: &B,
    snapshot: Option<&LibrarySnapshot>,
) -> Result<u64, CanonicalError> {
    let root = StorageRoot::open(backend).await?;
    let registered: HashSet<&str> = snapshot
        .map(|snapshot| {
            snapshot
                .files
                .iter()
                .filter(|file| file.is_registered())
                .map(|file| file.file.id().as_str())
                .collect()
        })
        .unwrap_or_default();
    let prefix = format!("{FILES_ROOT}/");
    let candidates: Vec<&String> = root
        .objects()
        .iter()
        .filter(|object| {
            object
                .strip_prefix(&prefix)
                .is_some_and(|id| !registered.contains(id))
        })
        .collect();
    let mut count: u64 = 0;
    for object in candidates {
        if !super::tombstone::is_tombstone(backend, object).await {
            count += 1;
        }
    }
    Ok(count)
}
