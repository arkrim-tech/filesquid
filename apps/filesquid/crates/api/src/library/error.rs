//! Errors of the library commands and queries (API-S-009, API-S-019).
//!
//! A [`LibraryError`] is what the UI shows: a stable `error_code`, an [`ErrorCode`] for the
//! HTTP status and retryability, and a message for people that never contains a name,
//! title, path or SQL (API-S-005). Store failures are logged to stdout with their detail
//! and reach the caller only as `unavailable` or `internal`.

use super::catalog::CatalogError;
use crate::operation::{CommandOutcome, CommandOutcomeKind, OperationId, PortError};
use crate::{ApiError, ErrorCode};

/// Why a library command or query was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryError {
    /// The request is malformed or breaks a rule; `code` names the rule.
    Invalid {
        /// Stable machine-readable rule name, for example `invalid_name`.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// The named entity (`library`, `collection`, `virtual_collection` or `file`) does not
    /// exist in the named library, or is not visible to the caller.
    NotFound(&'static str),
    /// The expected revision no longer matches the library; reread and decide.
    Conflict,
    /// The injected catalog is not reachable now; retry later.
    Unavailable,
    /// The catalog failed in a way a retry will not fix.
    Internal,
}

impl LibraryError {
    /// Shorthand for [`LibraryError::Invalid`].
    pub const fn invalid(code: &'static str, message: &'static str) -> Self {
        Self::Invalid { code, message }
    }

    /// Stable error code for journals, `PortError`s and UIs.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Invalid { code, .. } => code,
            Self::NotFound(_) => "not_found",
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// The API error code (HTTP status and retryability) for this error.
    pub fn api_code(&self) -> ErrorCode {
        match self {
            Self::Invalid { .. } => ErrorCode::InvalidRequest,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Conflict => ErrorCode::Conflict,
            Self::Unavailable => ErrorCode::Unavailable,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// The message shown to people. It names no entity, title or path.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Invalid { message, .. } => message,
            Self::NotFound(_) => "FileSquid could not find that item in this library.",
            Self::Conflict => "The library changed since it was read. Reload and try again.",
            Self::Unavailable => "The library is not available right now. Try again shortly.",
            Self::Internal => "FileSquid could not complete the request.",
        }
    }

    /// The portable error of this failure, with the same retryability as [`Self::api_code`].
    pub fn port_error(&self) -> PortError {
        let code = self.error_code();
        let retryable = self.api_code().retryable();
        PortError::new(code, retryable).unwrap_or_else(|_| fallback_port_error(retryable))
    }

    /// The command outcome for operation `operation_id`: `conflict` for a revision mismatch,
    /// otherwise `error` carrying [`Self::port_error`].
    pub fn outcome(&self, operation_id: &OperationId) -> CommandOutcome {
        if *self == Self::Conflict {
            return CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Conflict);
        }
        CommandOutcome::failed(operation_id.clone(), self.port_error())
    }
}

/// A port error for the impossible case of an empty code (every code above is non-empty).
fn fallback_port_error(retryable: bool) -> PortError {
    PortError::new("internal", retryable).expect("non-empty code")
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for LibraryError {}

impl From<LibraryError> for ApiError {
    fn from(err: LibraryError) -> Self {
        ApiError::new(err.api_code(), err.message())
    }
}

impl From<CatalogError> for LibraryError {
    /// Map a store failure, logging its detail to stdout only (INFO under the
    /// `filesquid::library` target, which the journal layer does not record). The service journals
    /// the failed command or query once, with its own operation name.
    fn from(err: CatalogError) -> Self {
        match err {
            CatalogError::Conflict => Self::Conflict,
            CatalogError::Unavailable(detail) => {
                tracing::info!(
                    target: super::LOG_TARGET,
                    error_code = "unavailable",
                    detail,
                    "library catalog unavailable"
                );
                Self::Unavailable
            }
            CatalogError::Failed(detail) => {
                tracing::info!(
                    target: super::LOG_TARGET,
                    error_code = "internal",
                    detail,
                    "library catalog failed"
                );
                Self::Internal
            }
        }
    }
}
