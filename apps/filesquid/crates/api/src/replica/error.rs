//! Errors of the replica (API-S-009, API-S-024): a stable `error_code`, an [`ErrorCode`] for
//! status and retryability, and a message for people that never contains a name, title,
//! path or SQL (API-S-005).

use crate::{ApiError, ErrorCode};

/// Why an entry could not be served from the replica cache: a recovery state, not an empty
/// answer (T-022).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// The replica never held this entry for this identity; fetch it again.
    NotCached,
    /// The cache evicted the entry; fetch it again.
    Evicted,
}

/// Why a replica operation was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplicaError {
    /// The input breaks a rule; `code` names the rule.
    Invalid {
        /// Stable machine-readable rule name, for example `invalid_viewer`.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// The operation is not allowed in the ledger's current state; `code` names the rule.
    Refused {
        /// Stable machine-readable rule name, for example `wrong_state`.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// The operation has to wait for other changes to publish; retry later.
    Blocked {
        /// Stable machine-readable rule name, for example `not_next`.
        code: &'static str,
        /// Message shown to people.
        message: &'static str,
    },
    /// No pending change has that operation ID.
    NotFound,
    /// The entry must be fetched again.
    Recovery(Recovery),
    /// A cached page belongs to another identity and is never served.
    IdentityMismatch,
    /// The local state could not be read or written; the detail went to stdout only.
    Internal,
}

impl ReplicaError {
    /// Shorthand for [`ReplicaError::Invalid`].
    pub const fn invalid(code: &'static str, message: &'static str) -> Self {
        Self::Invalid { code, message }
    }

    /// Shorthand for [`ReplicaError::Refused`].
    pub const fn refused(code: &'static str, message: &'static str) -> Self {
        Self::Refused { code, message }
    }

    /// Shorthand for [`ReplicaError::Blocked`].
    pub const fn blocked(code: &'static str, message: &'static str) -> Self {
        Self::Blocked { code, message }
    }

    /// Stable error code for logs and UIs.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Invalid { code, .. }
            | Self::Refused { code, .. }
            | Self::Blocked { code, .. } => code,
            Self::NotFound => "not_found",
            Self::Recovery(Recovery::NotCached) => "not_cached",
            Self::Recovery(Recovery::Evicted) => "evicted",
            Self::IdentityMismatch => "identity_mismatch",
            Self::Internal => "internal",
        }
    }

    /// The API error code (status and retryability) for this error.
    pub fn api_code(&self) -> ErrorCode {
        match self {
            Self::Invalid { .. } => ErrorCode::InvalidRequest,
            Self::Refused { .. } => ErrorCode::Conflict,
            Self::Blocked { .. } => ErrorCode::Unavailable,
            Self::NotFound => ErrorCode::NotFound,
            Self::Recovery(_) => ErrorCode::Unavailable,
            Self::IdentityMismatch => ErrorCode::Forbidden,
            Self::Internal => ErrorCode::Internal,
        }
    }

    /// The message shown to people. It names no viewer, library, title or path.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Invalid { message, .. }
            | Self::Refused { message, .. }
            | Self::Blocked { message, .. } => message,
            Self::NotFound => "FileSquid could not find that pending change.",
            Self::Recovery(Recovery::NotCached) => {
                "This view is not stored on this device yet. Loading it again."
            }
            Self::Recovery(Recovery::Evicted) => {
                "This device removed the stored view to free space. Loading it again."
            }
            Self::IdentityMismatch => "That stored view belongs to a different access.",
            Self::Internal => "FileSquid could not use the local copy.",
        }
    }
}

impl std::fmt::Display for ReplicaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for ReplicaError {}

impl From<ReplicaError> for ApiError {
    fn from(err: ReplicaError) -> Self {
        ApiError::new(err.api_code(), err.message())
    }
}
