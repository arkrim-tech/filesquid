//! Why a file operation was refused or failed, with the API error code each answer gets.
//! Messages are written for people and never carry a filesystem path, storage key, OCFL
//! content path or engine detail (API-S-005); engine details go to stdout only.

use crate::error::{ApiError, ErrorCode};
use filesquid_ocfl::change::ChangeError;
use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::StorageError;
use filesquid_ocfl::view::ViewError;
use filesquid_ocfl::write::{CommitError, ReservationError};

/// A refused or failed file operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    code: ErrorCode,
    message: String,
    /// What failed inside the engine, for stdout only (`internal` answers).
    detail: Option<String>,
}

impl FileError {
    /// A refusal with `code` and a `message` for people.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// `invalid_request` with `message`.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// `not_found`: the library, object, version, folder or file does not exist. One message
    /// for all of them, so a caller learns nothing about which part is missing.
    pub fn not_found() -> Self {
        let message = "That library, object, version, folder or file does not exist.";
        Self::new(ErrorCode::NotFound, message)
    }

    /// `conflict` with `message`.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    /// `unavailable`: the storage could not be reached; a retry may succeed. `detail` goes
    /// to stdout only.
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            detail: Some(detail.into()),
            ..Self::new(
                ErrorCode::Unavailable,
                "The library's storage is not available right now; try again shortly.",
            )
        }
    }

    /// `internal`: the object could not be read or written as stored. `detail` goes to
    /// stdout only.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            detail: Some(detail.into()),
            ..Self::new(
                ErrorCode::Internal,
                "FileSquid could not complete the request.",
            )
        }
    }

    /// The error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// The code's wire name, as the journal records it.
    pub fn error_code(&self) -> &'static str {
        self.code.as_str()
    }

    /// The typed answer for `operation`. An `internal` failure logs its detail to stdout
    /// (never to the journal) through [`ApiError::internal`]; an `unavailable` one logs it
    /// at INFO under a target outside the journal's, so it reaches stdout only: the routes
    /// journal the refusal itself once, at level `error`, and a WARN here would journal it a
    /// second time.
    pub fn into_api_error(self, operation: &str) -> ApiError {
        match (self.code, self.detail) {
            (ErrorCode::Internal, Some(detail)) => ApiError::internal(operation, &detail),
            (code, detail) => {
                if let Some(detail) = detail {
                    tracing::info!(target: super::LOG_TARGET, operation, detail, "storage unavailable");
                }
                ApiError::new(code, self.message)
            }
        }
    }
}

impl From<StorageError> for FileError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::Io { .. } | StorageError::Backend { .. } => {
                Self::unavailable(err.to_string())
            }
            other => Self::internal(other.to_string()),
        }
    }
}

impl From<ReadError> for FileError {
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::VersionNotFound { .. } | ReadError::PathNotFound { .. } => Self::not_found(),
            ReadError::Storage(storage) => storage.into(),
            other => Self::internal(other.to_string()),
        }
    }
}

impl From<ViewError> for FileError {
    fn from(err: ViewError) -> Self {
        match err {
            ViewError::InvalidRequest { .. } => Self::invalid(
                "The request names an invalid logical path or prefix, page size or range.",
            ),
            ViewError::InvalidCursor { .. } => {
                Self::invalid("The cursor is not one this listing answered.")
            }
            ViewError::FolderNotFound { .. } | ViewError::PathNotFound { .. } => Self::not_found(),
            ViewError::StagingNotEmpty => Self::conflict(
                "Another change to this object is in progress; try again when it has finished.",
            ),
            ViewError::Read(read) => read.into(),
            ViewError::Change(change) => change.into(),
            ViewError::Storage(storage) => storage.into(),
        }
    }
}

impl From<ChangeError> for FileError {
    fn from(err: ChangeError) -> Self {
        match err {
            ChangeError::NothingToCommit => {
                Self::conflict("Nothing would change, so no version was made.")
            }
            ChangeError::Conflict { .. } => Self::conflict(
                "The object changed at the same paths meanwhile; reread it and try again.",
            ),
            ChangeError::PathNotFound { .. } => Self::not_found(),
            ChangeError::InvalidPath { .. } => Self::invalid(
                "That logical path is not valid in OCFL, or another file already has it.",
            ),
            ChangeError::TooManyRebases { .. } => Self::conflict(
                "The object kept changing while this was saved; reread it and try again.",
            ),
            ChangeError::Commit(commit) => commit.into(),
            ChangeError::Read(read) => read.into(),
            ChangeError::Storage(storage) => storage.into(),
            other => Self::internal(other.to_string()),
        }
    }
}

impl From<CommitError> for FileError {
    fn from(err: CommitError) -> Self {
        match err {
            CommitError::MissingVersionInventory { .. }
            | CommitError::VersionInventoryMismatch { .. } => Self::conflict(
                "This object's latest version is incomplete; repair it before changing files.",
            ),
            CommitError::HeadMoved { .. } => {
                Self::conflict("The object changed meanwhile; reread it and try again.")
            }
            CommitError::Reservation(ReservationError::Held { .. }) => {
                Self::unavailable("another writer holds the object's next version")
            }
            CommitError::Reservation(ReservationError::Lost { .. }) => {
                Self::unavailable("the reservation expired or passed to another writer")
            }
            CommitError::Reservation(ReservationError::ClockUnavailable) => {
                Self::unavailable("the reservation clock cannot tell the time")
            }
            CommitError::Storage(storage) => storage.into(),
            CommitError::Read(read) => read.into(),
            other => Self::internal(other.to_string()),
        }
    }
}
