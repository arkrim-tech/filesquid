//! Errors of the local library operations (API-S-005, API-S-009, API-S-023).
//!
//! A [`LocalLibraryError`] carries a stable `error_code` for journals and UIs, an
//! [`ErrorCode`] for the HTTP status, and a message for people that never names a path,
//! storage key, object root or file. Engine and storage details go to stdout only.

use super::LOG_TARGET;
use crate::{ApiError, ErrorCode};
use filesquid_ocfl::storage::StorageError;
use filesquid_ocfl::ReadError;

/// Why a local library operation was refused or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalLibraryError {
    /// The request is malformed; `code` names the rule (for example `invalid_objects`).
    Invalid {
        /// Stable machine-readable rule name.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// The caller may not run this operation (the repair is the desktop owner's or the
    /// library owner's only).
    Forbidden,
    /// The operation is refused by a rule about the storage root, the destination or the
    /// consent; `code` names the rule (for example `root_not_empty`, `consent_required`).
    Refused {
        /// Stable machine-readable rule name.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// Storage, the journal ledger or the reservation store is not reachable now; retry.
    Unavailable,
    /// Something failed that a retry will not fix.
    Internal,
}

/// Refusal codes a retry may clear once the storage root settles.
const RETRYABLE_REFUSALS: [&str; 1] = ["snapshot_incomplete"];

impl LocalLibraryError {
    /// Shorthand for [`LocalLibraryError::Invalid`].
    pub const fn invalid(code: &'static str, message: &'static str) -> Self {
        Self::Invalid { code, message }
    }

    /// Shorthand for [`LocalLibraryError::Refused`].
    pub const fn refused(code: &'static str, message: &'static str) -> Self {
        Self::Refused { code, message }
    }

    /// Stable error code for journals and UIs.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Invalid { code, .. } | Self::Refused { code, .. } => code,
            Self::Forbidden => "forbidden",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// The API error code (HTTP status) for this error.
    pub fn api_code(&self) -> ErrorCode {
        match self {
            Self::Invalid { .. } => ErrorCode::InvalidRequest,
            Self::Forbidden => ErrorCode::Forbidden,
            Self::Refused { .. } => ErrorCode::Conflict,
            Self::Unavailable => ErrorCode::Unavailable,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// Whether the same request may succeed later without a change by the caller.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Unavailable => true,
            Self::Refused { code, .. } => RETRYABLE_REFUSALS.contains(code),
            _ => false,
        }
    }

    /// The message shown to people. It names no path, key, object root or file.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Invalid { message, .. } | Self::Refused { message, .. } => message,
            Self::Forbidden => "Only the owner of this library can do this.",
            Self::Unavailable => "The library folder is not available right now. Try again.",
            Self::Internal => "FileSquid could not complete the request.",
        }
    }

    /// Map a storage failure, logging its detail to stdout only.
    pub(super) fn from_storage(err: &StorageError) -> Self {
        tracing::info!(target: LOG_TARGET, error = %err, "library storage failed");
        Self::Unavailable
    }

    /// Map a failure to open a storage root or object: Git metadata is `git_in_root`, a
    /// storage outage `unavailable`, and any other broken rule (declaration, OCFL version,
    /// link, empty directory, inventory) `not_a_storage_root`.
    pub(super) fn from_open(err: &ReadError) -> Self {
        tracing::info!(target: LOG_TARGET, error = %err, "storage root refused");
        match err {
            ReadError::Guard(_) => Self::refused(
                "git_in_root",
                "This folder holds Git metadata, which an OCFL storage root must not contain.",
            ),
            ReadError::Storage(storage) if is_outage(storage) => Self::Unavailable,
            _ => Self::refused(
                "not_a_storage_root",
                "This folder is not a FileSquid library (an OCFL 1.1 storage root).",
            ),
        }
    }
}

/// Whether `err` is an outage of the storage itself (I/O or backend failure) rather than a
/// broken rule of the root, such as a link, an empty directory or a missing file.
pub(super) fn is_outage(err: &StorageError) -> bool {
    matches!(err, StorageError::Io { .. } | StorageError::Backend { .. })
}

impl std::fmt::Display for LocalLibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for LocalLibraryError {}

impl From<LocalLibraryError> for ApiError {
    fn from(err: LocalLibraryError) -> Self {
        ApiError::new(err.api_code(), err.message())
    }
}
