//! Engine-level errors: the Git prohibition and logical-file write failures.

use thiserror::Error;

use crate::storage::StorageError;

/// Errors from OCFL root inspection.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OcflError {
    /// A `.git` directory was found inside an OCFL object or storage root.
    #[error("a .git directory MUST NOT appear in an OCFL object root or storage root")]
    GitDirectoryForbidden,
}

/// Errors from writing or reading a logical file in an object directory.
#[derive(Debug, Error)]
pub enum WriteError {
    /// Git is forbidden in the object root.
    #[error(transparent)]
    Git(#[from] OcflError),
    /// An I/O error; a missing logical file is `ErrorKind::NotFound`.
    #[error("write failed: {0}")]
    Io(#[from] std::io::Error),
    /// Relative path was empty or tried to escape the object root.
    #[error("logical path must be a relative file name inside the object")]
    InvalidPath,
    /// Any other storage backend failure (limits, conflicts, backend errors).
    #[error(transparent)]
    Storage(StorageError),
}

impl From<StorageError> for WriteError {
    /// Map a backend error onto the logical-file error contract: a missing
    /// object and operating-system failures become [`WriteError::Io`], an
    /// invalid key becomes [`WriteError::InvalidPath`].
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::NotFound { key } => Self::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no logical file at {key}"),
            )),
            StorageError::Io { kind, message, .. } => Self::Io(std::io::Error::new(kind, message)),
            StorageError::InvalidKey { .. } => Self::InvalidPath,
            other => Self::Storage(other),
        }
    }
}
