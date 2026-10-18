//! Create and open a library's OCFL storage root (F-001, F-042).
//!
//! Creating writes exactly one file, the OCFL 1.1 storage-root declaration `0=ocfl_1.1`,
//! into an empty folder, with `IfAbsent` so it never replaces anything. Opening runs the
//! engine's storage-root guard and discovery ([`StorageRoot::open`]) and writes nothing.

use super::error::LocalLibraryError;
use filesquid_ocfl::read::StorageRoot;
use filesquid_ocfl::storage::{Precondition, StorageBackend, StorageError, StorageKey};
use filesquid_ocfl::{Declaration, SpecVersion};
use serde::{Deserialize, Serialize};

/// What an opened (or created) storage root holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSummary {
    /// The OCFL version the storage root declares (`1.1`).
    pub spec_version: String,
    /// The objects discovered in it.
    pub objects: usize,
}

/// The refusal for a folder that already holds something.
fn not_empty() -> LocalLibraryError {
    LocalLibraryError::refused(
        "root_not_empty",
        "Choose an empty folder for a new library; this one already holds files.",
    )
}

/// Whether `backend` holds no key at all.
///
/// # Errors
///
/// `unavailable` when the listing fails.
pub(super) async fn is_empty<B: StorageBackend>(backend: &B) -> Result<bool, LocalLibraryError> {
    let page = backend
        .list("", None, 1)
        .await
        .map_err(|err| LocalLibraryError::from_storage(&err))?;
    Ok(page.keys.is_empty())
}

/// Write the OCFL 1.1 storage-root declaration into the empty `backend`, then open it.
///
/// # Errors
///
/// `root_not_empty` when the folder holds any file (nothing is written); `unavailable` for a
/// storage failure; the errors of [`open_root`].
pub(super) async fn create_root<B: StorageBackend>(
    backend: &B,
) -> Result<RootSummary, LocalLibraryError> {
    if !is_empty(backend).await? {
        return Err(not_empty());
    }
    let declaration = Declaration::storage_root(SpecVersion::V1_1);
    let key = StorageKey::new(&declaration.file_name())
        .map_err(|err| LocalLibraryError::from_storage(&err))?;
    let written = backend
        .put_if(
            &key,
            declaration.contents().as_bytes(),
            &Precondition::IfAbsent,
        )
        .await;
    match written {
        Ok(_) => open_root(backend).await,
        Err(StorageError::PreconditionFailed { .. }) => Err(not_empty()),
        Err(err) => Err(LocalLibraryError::from_storage(&err)),
    }
}

/// Open the storage root of `backend` and count its objects. Writes nothing.
///
/// # Errors
///
/// `not_a_storage_root`, `git_in_root` or `unavailable` (see
/// [`LocalLibraryError::from_open`]).
pub(super) async fn open_root<B: StorageBackend>(
    backend: &B,
) -> Result<RootSummary, LocalLibraryError> {
    let root = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    Ok(RootSummary {
        spec_version: root.declaration().spec.number().to_string(),
        objects: root.objects().len(),
    })
}
