//! Pluggable metadata stores ([ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)).
//!
//! The core (statements, register, extraction, `metadata.md` format,
//! interchange) depends on no store. A [`MetadataStore`] loads and saves a
//! [`Record`] by an opaque [`FileRef`]:
//!
//! - [`sidecar`]: `<file>.metadata.md` beside a plain file; the default feature,
//!   native only.
//! - [`ocfl`]: `metadata.md` as a logical file of an OCFL object, each edit
//!   committed as a new OCFL version through the `filesquid-ocfl` change
//!   model; the optional `ocfl` feature, which FileSquid configures as
//!   canonical.
//!
//! A caller (for example the browser host) may also implement the trait itself.

use std::error::Error;

use thiserror::Error;

use crate::file_record::RecordError;
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
    /// The edit was made against a version that is no longer the head, or
    /// another writer changed the record first; nothing was published.
    #[error("metadata edit conflict: {reason}")]
    Conflict {
        /// What moved.
        reason: String,
    },
    /// The store holds another object than the caller named.
    #[error("the object identifier is {found:?}, not {expected:?}")]
    WrongObject {
        /// The identifier the caller named.
        expected: String,
        /// The identifier the store holds.
        found: String,
    },
    /// The target holds no object yet and the caller named no identifier to
    /// create it with.
    #[error("no OCFL object here yet; an object identifier is needed to create one")]
    MissingObjectId,
    /// The file's record is missing, invalid, or the save would change what
    /// is immutable (an import entry or the original file name); nothing
    /// was published.
    #[error("file record refused: {0}")]
    Record(#[from] RecordError),
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

    /// Replace the record of `file` with `record`. With the `ocfl` store a
    /// changed record commits a new OCFL version (an unchanged one publishes
    /// nothing), and an earlier version is never rewritten.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the reference is invalid or the backend fails.
    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError>;
}
