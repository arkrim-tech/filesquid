//! Opaque reference to the file a metadata record describes.
//!
//! A [`FileRef`] is supplied by the caller and handed back to a
//! [`MetadataStore`](crate::store::MetadataStore) or to extraction unchanged.
//! The core never opens it: it only names a file, so the same engine runs over
//! a plain directory, an OCFL object or a caller-owned store, natively and in
//! WebAssembly ([ADR 027](../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)).

use thiserror::Error;

/// Errors from building a [`FileRef`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FileRefError {
    /// The logical path is empty, absolute, or could leave its root.
    #[error("logical path must be relative, non-empty and free of `.`, `..` and `\\`")]
    InvalidLogicalPath,
}

/// Names one file: an object identifier, an optional version and a logical path.
///
/// For the `ocfl` store the object identifier is the object's key prefix in
/// the backend (empty for the backend root) and the version is informational.
/// The `sidecar` store ignores both and resolves the logical path under its root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRef {
    /// Object identifier (empty when the store root is the object).
    object_id: String,
    /// Object version, when the caller pins one.
    version: Option<String>,
    /// Relative, `/`-separated path of the file inside the object.
    logical_path: String,
}

/// Return whether `path` is a relative `/`-separated path without escapes.
fn is_valid_logical_path(path: &str) -> bool {
    if path.is_empty() || path.contains(['\\', '\0']) {
        return false;
    }
    path.split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

impl FileRef {
    /// Build a reference to `logical_path` inside `object_id`.
    ///
    /// # Errors
    ///
    /// [`FileRefError::InvalidLogicalPath`] when the path is empty, absolute,
    /// holds an empty, `.` or `..` segment, or contains `\` or NUL.
    pub fn new(object_id: &str, logical_path: &str) -> Result<Self, FileRefError> {
        if !is_valid_logical_path(logical_path) {
            return Err(FileRefError::InvalidLogicalPath);
        }
        Ok(Self {
            object_id: object_id.to_string(),
            version: None,
            logical_path: logical_path.to_string(),
        })
    }

    /// Pin the object version this reference names.
    #[must_use]
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    /// Object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Pinned object version, if any.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Logical path of the file inside the object.
    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }
}
