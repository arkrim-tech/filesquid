//! Pluggable metadata stores ([ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)).
//!
//! The core (statements, register, extraction, `metadata.md` format,
//! interchange) depends on no store. A [`MetadataStore`] loads and saves a
//! [`Record`] by an opaque [`FileRef`]:
//!
//! - [`sidecar`]: `<file>.metadata.md` beside a plain file; the default feature,
//!   native only.
//! - [`ocfl`]: `metadata.md` through `filesquid-ocfl`; the optional `ocfl`
//!   feature, which FileSquid configures as canonical.
//!
//! A caller (for example the browser host) may also implement the trait itself.

use std::error::Error;

use thiserror::Error;

use crate::file_ref::FileRef;
use crate::record::Record;

#[cfg(feature = "ocfl")]
pub mod ocfl;
#[cfg(all(feature = "sidecar", not(target_arch = "wasm32")))]
pub mod sidecar;

/// Canonical file name of a metadata record.
pub const METADATA_MD: &str = "metadata.md";

/// Errors from a [`MetadataStore`].
#[derive(Debug, Error)]
pub enum StoreError {
    /// The [`FileRef`] or object prefix is absolute or could escape the store root.
    #[error("invalid store path")]
    InvalidPath,
    /// The target holds a `.git` directory; Git MUST NOT be the metadata store.
    #[error("refusing to use a Git working tree as the metadata store")]
    Git,
    /// The backend failed; the source says why.
    #[error("metadata store backend failed")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// Loads and saves the metadata record of one file.
///
/// Methods are `async` and runtime-agnostic, like the `filesquid-ocfl`
/// storage backend, so one interface serves native hosts and WebAssembly.
#[allow(async_fn_in_trait)]
pub trait MetadataStore {
    /// Read the record of `file`. A file without a record yields an empty one.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the reference is invalid or the backend fails.
    async fn load(&self, file: &FileRef) -> Result<Record, StoreError>;

    /// Replace the record of `file` with `record`.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the reference is invalid or the backend fails.
    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError>;
}
