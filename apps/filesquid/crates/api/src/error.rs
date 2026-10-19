//! Typed API errors (API-S-009, [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)
//! Decision 6).
//!
//! Every failed request answers with an [`ApiError`] JSON body (`code`, `message`,
//! `retryable`, and `reason` for an authorisation denial) documented in the contract as
//! `components.schemas.ApiError`, so every UI can show the failure and decide whether to offer
//! a retry or a sign-in. Messages are written for people and
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
    /// The path exists but does not accept this HTTP method (HTTP 405).
    MethodNotAllowed,
    /// The expected revision no longer matches; reread and decide (HTTP 409).
    Conflict,
    /// The requested byte range starts past the end of the file (HTTP 416); the answer
    /// carries `Content-Range: bytes */<length>`.
    RangeNotSatisfiable,
    /// The host failed while handling the request (HTTP 500).
    Internal,
    /// The host or module is not ready or not enabled; try again later (HTTP 503).
    Unavailable,
}

impl ErrorCode {
    /// Every error code, in contract order.
    pub const ALL: [ErrorCode; 9] = [
        ErrorCode::InvalidRequest,
        ErrorCode::Unauthorized,
        ErrorCode::Forbidden,
        ErrorCode::NotFound,
        ErrorCode::MethodNotAllowed,
        ErrorCode::Conflict,
        ErrorCode::RangeNotSatisfiable,
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
            ErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::RangeNotSatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Wire name (`snake_case`), also used as the journal error code.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidRequest => "invalid_request",
            ErrorCode::Unauthorized => "unauthorized",
            ErrorCode::Forbidden => "forbidden",
            ErrorCode::NotFound => "not_found",
            ErrorCode::MethodNotAllowed => "method_not_allowed",
            ErrorCode::Conflict => "conflict",
            ErrorCode::RangeNotSatisfiable => "range_not_satisfiable",
            ErrorCode::Internal => "internal",
            ErrorCode::Unavailable => "unavailable",
        }
    }

    /// Whether repeating the same request unchanged may succeed. Only `unavailable` is
    /// retryable; a `conflict` needs a reread first, so it is not.
    pub fn retryable(self) -> bool {
        matches!(self, ErrorCode::Unavailable)
    }
}

/// Why the authorisation layer refused a call (API-S-004). Serialized in `snake_case` as the
/// optional `reason` member of [`ApiError`]; documented as `components.schemas.DenialReason`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenialReason {
    /// The call carried no credential (`unauthorized`, HTTP 401).
    MissingCredentials,
    /// The credential is not one this host accepts (`forbidden`, HTTP 403).
    InvalidCredentials,
    /// The caller is known but the policy does not permit this action (`forbidden`, HTTP 403).
    NotPermitted,
}

impl DenialReason {
    /// Every denial reason, in contract order.
    pub const ALL: [DenialReason; 3] = [
        DenialReason::MissingCredentials,
        DenialReason::InvalidCredentials,
        DenialReason::NotPermitted,
    ];

    /// Wire name, also used as the journal error code.
    pub fn as_str(self) -> &'static str {
        match self {
            DenialReason::MissingCredentials => "missing_credentials",
            DenialReason::InvalidCredentials => "invalid_credentials",
            DenialReason::NotPermitted => "not_permitted",
        }
    }

    /// The error code answered for this reason.
    pub fn code(self) -> ErrorCode {
        match self {
            DenialReason::MissingCredentials => ErrorCode::Unauthorized,
            DenialReason::InvalidCredentials | DenialReason::NotPermitted => ErrorCode::Forbidden,
        }
    }

    /// The message shown to people. It never repeats the presented credential.
    fn message(self) -> &'static str {
        match self {
            DenialReason::MissingCredentials => {
                "FileSquid needs credentials for this request. Sign in or reopen the app."
            }
            DenialReason::InvalidCredentials => {
                "These credentials are not valid for this FileSquid host. Sign in again."
            }
            DenialReason::NotPermitted => "You do not have permission to do this.",
        }
    }
}

/// Error body of every failed API request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    code: ErrorCode,
    message: String,
    retryable: bool,
    /// Present only on authorisation denials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<DenialReason>,
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
            reason: None,
        }
    }

    /// The typed denial for `reason`: `unauthorized` or `forbidden`, never retryable, with a
    /// message the UI can show (API-S-004, API-S-009).
    pub fn denied(reason: DenialReason) -> Self {
        Self {
            reason: Some(reason),
            ..Self::new(reason.code(), reason.message())
        }
    }

    /// Why the call was refused, for authorisation denials.
    pub fn reason(&self) -> Option<DenialReason> {
        self.reason
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
