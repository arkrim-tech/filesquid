//! Engine-level errors: the Git guard and logical-file write failures.

use thiserror::Error;

use crate::storage::StorageError;

/// Errors from the OCFL root guard (OCFL-S-001, ADR 016 layout constraints).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OcflError {
    /// A `.git` entry (directory, file or link) was found in an OCFL object
    /// root, the storage root, or a directory of the storage hierarchy above
    /// the objects.
    #[error("a .git directory MUST NOT appear in an OCFL object root or storage root (found in {root:?})")]
    GitDirectoryForbidden {
        /// The root that holds `.git`: a key prefix without a trailing `/`
        /// (empty for the storage root) for the backend guards, or the
        /// directory path for the path-based `reject_git_in_ocfl_root`.
        root: String,
    },
    /// The storage hierarchy holds a soft link, a hard link, an empty
    /// directory or another entry that is not a plain file, or the backend
    /// could not be inspected.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Errors from writing or reading a logical file in an object directory.
#[derive(Debug, Error)]
pub enum WriteError {
    /// The Git guard refused the object root.
    #[error(transparent)]
    Git(OcflError),
    /// An I/O error; a missing logical file is `ErrorKind::NotFound`.
    #[error("logical-file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// Relative path was empty or tried to escape the object root.
    #[error("logical path must be a relative file name inside the object")]
    InvalidPath,
    /// Any other storage backend failure (limits, conflicts, backend errors).
    #[error(transparent)]
    Storage(StorageError),
}

impl From<OcflError> for WriteError {
    /// Keep a Git refusal as [`WriteError::Git`]; a storage failure met while
    /// inspecting the root takes the same mapping as any other storage error.
    fn from(err: OcflError) -> Self {
        match err {
            OcflError::Storage(storage) => storage.into(),
            git => Self::Git(git),
        }
    }
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
