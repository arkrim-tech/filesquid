//! Errors of the OCFL validation service and routes (API-S-009, API-S-022).

use super::store::StoreError;
use super::LOG_TARGET;
use crate::{ApiError, ErrorCode};

/// Why a validation request was refused or failed. Messages name no library, object, path or
/// file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// The library is not open on this host.
    NotFound,
    /// A run of the library is already going; it runs at most once at a time.
    AlreadyRunning,
    /// A configuration or request value is out of bounds; `code` names the rule.
    Invalid {
        /// Stable machine-readable rule name.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// The findings store or the library storage is not reachable now; retry.
    Unavailable,
    /// Something failed that a retry will not fix.
    Internal,
}

impl ValidationError {
    /// Stable error code for journals and UIs.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::AlreadyRunning => "already_running",
            Self::Invalid { code, .. } => code,
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// The API error code (HTTP status and retryability).
    pub fn api_code(&self) -> ErrorCode {
        match self {
            Self::NotFound => ErrorCode::NotFound,
            Self::AlreadyRunning => ErrorCode::Conflict,
            Self::Invalid { .. } => ErrorCode::InvalidRequest,
            Self::Unavailable => ErrorCode::Unavailable,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// The message shown to people.
    pub fn message(&self) -> &'static str {
        match self {
            Self::NotFound => "That library is not open on this host.",
            Self::AlreadyRunning => "A check of this library is already running.",
            Self::Invalid { message, .. } => message,
            Self::Unavailable => {
                "The OCFL checks or their findings are not available right now. Try again shortly."
            }
            Self::Internal => "FileSquid could not complete the check.",
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for ValidationError {}

impl From<ValidationError> for ApiError {
    fn from(err: ValidationError) -> Self {
        ApiError::new(err.api_code(), err.message())
    }
}

impl From<StoreError> for ValidationError {
    /// A transient store failure is `unavailable`, any other `internal`; the detail goes to
    /// stdout only.
    fn from(err: StoreError) -> Self {
        tracing::info!(target: LOG_TARGET, error = %err, "findings store failed");
        if err.transient {
            Self::Unavailable
        } else {
            Self::Internal
        }
    }
}
