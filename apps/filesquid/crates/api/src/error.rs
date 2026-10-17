//! Typed API errors (API-S-009, [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)
//! Decision 6).
//!
//! Every failed request answers with an [`ApiError`] JSON body (`code`, `message`,
//! `retryable`) documented in the contract as `components.schemas.ApiError`, so every UI can
//! show the failure and decide whether to offer a retry. Messages are written for people and
//! never contain filesystem paths, SQL or internal error text (API-S-005). Internal details
//! go to stdout only; the activity journal receives a redacted error record (the whitelisted
//! `operation` and `error_code` fields, never the detail).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

/// Stable, machine-readable error code. Serialized in `snake_case`; each code has one HTTP
/// status and one retryability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request was malformed or a parameter was invalid (HTTP 400).
    InvalidRequest,
    /// No credentials were presented (HTTP 401).
    Unauthorized,
    /// The credentials do not grant this operation (HTTP 403).
    Forbidden,
    /// The resource does not exist or is not visible to the caller (HTTP 404).
    NotFound,
    /// The expected revision no longer matches; reread and decide (HTTP 409).
    Conflict,
    /// The host failed while handling the request (HTTP 500).
    Internal,
    /// The host or module is not ready or not enabled; try again later (HTTP 503).
    Unavailable,
}

impl ErrorCode {
    /// Every error code, in contract order.
    pub const ALL: [ErrorCode; 7] = [
        ErrorCode::InvalidRequest,
        ErrorCode::Unauthorized,
        ErrorCode::Forbidden,
        ErrorCode::NotFound,
        ErrorCode::Conflict,
        ErrorCode::Internal,
        ErrorCode::Unavailable,
    ];

    /// HTTP status answered for this code.
    pub fn status(self) -> StatusCode {
        match self {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Whether repeating the same request unchanged may succeed. Only `unavailable` is
    /// retryable; a `conflict` needs a reread first, so it is not.
    pub fn retryable(self) -> bool {
        matches!(self, ErrorCode::Unavailable)
    }
}

/// Error body of every failed API request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    code: ErrorCode,
    message: String,
    retryable: bool,
}

impl ApiError {
    /// Build an error; `retryable` is taken from `code`.
    ///
    /// `message` is shown to people: it MUST NOT contain paths, SQL or internal error text.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.retryable(),
        }
    }

    /// The error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// The human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether the caller may repeat the request unchanged.
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// Log `detail` to stdout and return a generic `internal` error that does not reveal it.
    /// The journal layer records only the whitelisted `operation` and `error_code` fields of
    /// this event, so the activity journal gets a redacted error record without the detail.
    /// `operation` names what failed, for example `api.admin.journal`.
    pub fn internal(operation: &str, detail: &str) -> Self {
        tracing::error!(
            operation,
            error_code = "internal",
            detail,
            "API request failed"
        );
        Self::new(
            ErrorCode::Internal,
            "FileSquid could not complete the request.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.code.status(), Json(self)).into_response()
    }
}
