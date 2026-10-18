//! The error for reading OCFL storage roots and objects.

use thiserror::Error;

use crate::error::OcflError;
use crate::inventory::FormatError;
use crate::storage::StorageError;

/// Why a storage root, object, version or logical file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReadError {
    /// An OCFL file breaks an OCFL 1.1 rule (with its validation code), or
    /// is larger than the engine's parsing bound (no code).
    #[error(transparent)]
    Format(#[from] FormatError),
    /// The Git guard refused the storage root or object root (OCFL-S-001).
    #[error(transparent)]
    Guard(OcflError),
    /// The storage backend failed, refused a link or empty directory, or a
    /// bound such as the key limit was reached.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The caller pinned a version the root inventory does not list. A
    /// version above the root `head` is unpublished and lands here too.
    #[error("version {version} is not a committed version of this object")]
    VersionNotFound {
        /// The pinned version name.
        version: String,
    },
    /// The logical path is not in the pinned version's state.
    #[error("{path:?} is not a logical path of version {version}")]
    PathNotFound {
        /// The pinned version name.
        version: String,
        /// The logical path asked for.
        path: String,
    },
    /// The root declares an OCFL version other than 1.1. This reader reads
    /// OCFL 1.1 only (OCFL-S-011); this is the engine's scope, not an OCFL
    /// rule, so it has no validation code.
    #[error("{declaration} declares an OCFL version this reader does not read (only 1.1)")]
    UnsupportedVersion {
        /// The declaration file's key.
        declaration: String,
    },
    /// The storage root's own declaration names it as an object root too.
    #[error("the storage root is also declared as an OCFL object root")]
    ObjectAtStorageRoot,
    /// A copy's destination lies inside an OCFL object root or storage root
    /// on the destination backend. Inspection and copy never edit an object
    /// (OCFL-S-019); only the writer adds versions. This is the engine's
    /// rule, not an OCFL validation code.
    #[error("{key} is inside the OCFL root {root:?}; a copy never writes into an OCFL root")]
    CopyIntoOcflRoot {
        /// The refused destination key.
        key: String,
        /// The directory that holds the OCFL declaration (`""` for the
        /// backend root).
        root: String,
    },
}

impl ReadError {
    /// The OCFL 1.1 validation code (`E###`) when an OCFL rule is broken,
    /// otherwise `None`.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Format(err) => err.code(),
            _ => None,
        }
    }

    /// A broken OCFL rule with `code` at `at`.
    pub(crate) fn invalid(
        code: &'static str,
        at: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Format(FormatError::invalid(code, at, reason))
    }
}

impl From<OcflError> for ReadError {
    /// Keep a Git refusal as [`ReadError::Guard`]; a storage failure met by
    /// the guard is a [`ReadError::Storage`].
    fn from(err: OcflError) -> Self {
        match err {
            OcflError::Storage(storage) => Self::Storage(storage),
            git => Self::Guard(git),
        }
    }
}
