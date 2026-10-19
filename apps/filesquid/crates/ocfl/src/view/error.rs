//! Errors of the UI view API.

use thiserror::Error;

use crate::change::ChangeError;
use crate::read::ReadError;
use crate::storage::StorageError;

/// Why a view call returned no result.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ViewError {
    /// A request parameter is outside its bound (a page size of 0 or above
    /// [`MAX_PAGE_ENTRIES`](super::MAX_PAGE_ENTRIES), an empty range or one
    /// above [`MAX_RANGE_BYTES`](super::MAX_RANGE_BYTES)) or is not a valid
    /// logical path or prefix.
    #[error("invalid request: {reason}")]
    InvalidRequest {
        /// What is wrong with the request.
        reason: String,
    },
    /// The cursor is not one this API returned for this listing.
    #[error("invalid listing cursor")]
    InvalidCursor {
        /// The cursor as given, at most 64 characters.
        cursor: String,
    },
    /// No logical path of the pinned version lies below the prefix. OCFL
    /// stores no empty folder, so a folder exists only while a file does.
    #[error("{prefix:?} is not a folder of {version}")]
    FolderNotFound {
        /// The prefix.
        prefix: String,
        /// The pinned version.
        version: String,
    },
    /// No logical path of the version read is, or lies below, the given
    /// path.
    #[error("{path:?} is not a file or folder of {version}")]
    PathNotFound {
        /// The path.
        path: String,
        /// The version read: the head for a move or remove, the earlier
        /// version for a restore.
        version: String,
    },
    /// The change model's staging area holds other changes; a view write
    /// commits only its own change.
    #[error("the staging area holds other changes; commit or reset them first")]
    StagingNotEmpty,
    /// The object could not be read.
    #[error(transparent)]
    Read(ReadError),
    /// The change model refused or failed the write.
    #[error(transparent)]
    Change(ChangeError),
    /// A storage backend failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl From<ReadError> for ViewError {
    /// Keep a storage failure met while reading as [`ViewError::Storage`].
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::Storage(storage) => Self::Storage(storage),
            other => Self::Read(other),
        }
    }
}

impl From<ChangeError> for ViewError {
    /// Keep a storage failure met by the change model as
    /// [`ViewError::Storage`].
    fn from(err: ChangeError) -> Self {
        match err {
            ChangeError::Storage(storage) => Self::Storage(storage),
            other => Self::Change(other),
        }
    }
}

impl ViewError {
    /// An invalid-request error for `reason`.
    pub(crate) fn request(reason: impl Into<String>) -> Self {
        Self::InvalidRequest {
            reason: reason.into(),
        }
    }
}
