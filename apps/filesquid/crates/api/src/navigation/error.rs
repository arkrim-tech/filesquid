//! Errors of the navigation listing (API-S-009, API-S-021).

use super::LOG_TARGET;
use crate::library::CatalogError;
use crate::{ApiError, ErrorCode};

/// Why a navigation request was refused or failed. Messages name no node, name or path, and
/// an unknown node and one the caller may not read share one variant and one message, so the
/// answer cannot tell them apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationError {
    /// A cursor or limit is unusable, or a parameter is unknown or malformed.
    Invalid(&'static str),
    /// The node does not exist, is malformed, or the caller may not read it.
    NotFound,
    /// The catalog or the folder index is not reachable now; retry later.
    Unavailable,
    /// Something failed that a retry will not fix.
    Internal,
}

impl NavigationError {
    /// A refusal of an unusable request, with the message shown to people.
    pub const fn invalid(message: &'static str) -> Self {
        Self::Invalid(message)
    }

    /// The folder index or catalog is not reachable now.
    pub const fn unavailable() -> Self {
        Self::Unavailable
    }

    /// An internal failure.
    pub const fn internal() -> Self {
        Self::Internal
    }

    /// Stable error code for journals and UIs.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid_request",
            Self::NotFound => "not_found",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// The API error code (HTTP status and retryability).
    pub fn api_code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::InvalidRequest,
            Self::NotFound => ErrorCode::NotFound,
            Self::Unavailable => ErrorCode::Unavailable,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// The message shown to people.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Invalid(message) => message,
            Self::NotFound => "That navigation node does not exist or you may not open it.",
            Self::Unavailable => {
                "The navigation tree is not available right now. Try again shortly."
            }
            Self::Internal => "FileSquid could not list the navigation tree.",
        }
    }
}

impl std::fmt::Display for NavigationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for NavigationError {}

impl From<NavigationError> for ApiError {
    fn from(err: NavigationError) -> Self {
        ApiError::new(err.api_code(), err.message())
    }
}

impl From<CatalogError> for NavigationError {
    /// A transient catalog failure is `unavailable`, any other `internal`; the detail goes to
    /// stdout only.
    fn from(err: CatalogError) -> Self {
        match err {
            CatalogError::Unavailable(detail) => {
                tracing::info!(target: LOG_TARGET, detail, "library catalog unavailable");
                Self::Unavailable
            }
            CatalogError::Conflict | CatalogError::Failed(_) => {
                tracing::info!(target: LOG_TARGET, "library catalog failed");
                Self::Internal
            }
        }
    }
}
