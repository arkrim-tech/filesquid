//! Command envelopes on JSON mutation routes (API-S-002, API-S-019, CORE-S-005): the request
//! shape, the checks every envelope route makes, the claim of its operation ID in the host's
//! receipts, and the answers a command gives.
//!
//! The library routes (Task API-F2.19) are built on this module; the file write routes keep
//! the helpers of [`crate::files`] from Task API-F2.13, which this module generalises so a
//! later task can move them here and give the remaining plain-body mutation routes (the
//! preferences and metadata routes, pending the owner's decision) an envelope with a small
//! change. Nothing here does I/O: the route owns the journal entry of every refusal, and the
//! receipts are the in-memory store of [`crate::files::CommandReceipts`].
//!
//! - **Request.** A route's body is [`Command`], `{"envelope": ..., "input": ...}`, the body
//!   `FileSquidClient.command()` sends. [`check`] accepts an envelope of schema version 1
//!   whose operation ID the journal keeps opaque and whose scope is the one the route derives
//!   from its input (`library:<library_id>`, or `location:<location_id>` for a command that
//!   binds a folder); anything else is the route's `invalid_request`.
//! - **Receipts.** [`claim`] answers a stored receipt of the same route, target and input
//!   before anything else is checked ([`Settled::Replay`]), refuses an operation ID that names
//!   another command ([`Settled::Reused`]) or one still running ([`Settled::Busy`]), and hands
//!   a fresh run its [`ClaimGuard`]; a run that changed something stores its answer, any other
//!   drops the guard and releases the ID.
//! - **Answers.** [`committed`] and [`pending`] are `200` with the [`CommandOutcome`] fields
//!   and `result`; [`conflict`] is HTTP 409 with [`ConflictAnswer`]: the [`ApiError`] fields
//!   beside a `conflict` outcome of the same operation ID.

use crate::error::{ApiError, ErrorCode};
use crate::files::receipts::{Claim, ClaimGuard, CommandReceipts, Fingerprint};
use crate::operation::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, OperationId, PROTOCOL_SCHEMA_VERSION,
};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// The expected revision of a command that makes something new (a library that does not
/// exist yet): the caller has read nothing to be stale about.
pub const NEW_REVISION: &str = "0";

/// Why an envelope was refused, as the message of the route's `invalid_request`.
pub const INVALID_ENVELOPE: &str = "The envelope is not schema version 1, its operation_id is \
     not 1 to 128 letters, digits, `.`, `_`, `-` or `:`, or its scope is not the one the \
     route derives from its input.";

/// Body of a JSON mutation route: the envelope and the route's input.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command<T> {
    /// Schema version, operation (idempotency) ID, expected revision and scope.
    pub envelope: CommandEnvelope,
    /// The route's input.
    pub input: T,
}

/// A command whose envelope was checked.
#[derive(Clone, Debug)]
pub struct Checked<T> {
    /// The operation (idempotency) ID.
    pub operation_id: OperationId,
    /// The revision the caller read.
    pub expected: String,
    /// The route's input.
    pub input: T,
}

/// The `200` answer of a command: the [`CommandOutcome`] fields and what it made.
#[derive(Clone, Debug, Serialize)]
pub struct CommandAnswer<T> {
    /// The outcome for the envelope's operation ID: `committed` or `pending`.
    #[serde(flatten)]
    pub outcome: CommandOutcome,
    /// What the command made, or the job it started.
    pub result: Option<T>,
}

/// The HTTP 409 answer of a command: the typed error and a `conflict` outcome of the same
/// operation ID, so both an `ApiError` reader and a `CommandOutcome` reader read it.
#[derive(Clone, Debug, Serialize)]
pub struct ConflictAnswer {
    /// The outcome: `conflict`, `error` null.
    #[serde(flatten)]
    pub outcome: CommandOutcome,
    /// Always null: a conflict made nothing.
    pub result: Option<()>,
    /// `code` `conflict`, the message for people and `retryable`.
    #[serde(flatten)]
    pub error: ApiError,
}

/// What a claim answered instead of a fresh run.
#[derive(Debug)]
pub enum Settled {
    /// The stored answer of the same command.
    Replay(Value),
    /// The ID names another command.
    Reused,
    /// A run of the ID is in flight.
    Busy,
    /// The input could not be fingerprinted.
    Failed,
}

impl Settled {
    /// The code and message of a claim that refused to run the command. A replay refuses
    /// nothing, so it has none.
    pub fn refusal(&self) -> Option<(ErrorCode, &'static str)> {
        match self {
            Self::Replay(_) => None,
            Self::Reused => Some((
                ErrorCode::InvalidRequest,
                "That operation_id already names another command; send a new one.",
            )),
            Self::Busy => Some((
                ErrorCode::Conflict,
                "A command with that operation_id is still running; its outcome is answered to \
                 the same operation_id once it has finished.",
            )),
            Self::Failed => Some((
                ErrorCode::Internal,
                "FileSquid could not complete the request.",
            )),
        }
    }
}

/// `envelope` and `input` as a checked command, or [`INVALID_ENVELOPE`] for an envelope that
/// is not schema version 1, an operation ID that is not opaque, or a scope other than
/// `scope`. Pure.
pub fn check<T>(
    envelope: CommandEnvelope,
    input: T,
    scope: &str,
) -> Result<Checked<T>, &'static str> {
    let valid = envelope.schema_version() == PROTOCOL_SCHEMA_VERSION
        && crate::journal::is_opaque(envelope.operation_id().as_str())
        && envelope.scope().as_str() == scope;
    if !valid {
        return Err(INVALID_ENVELOPE);
    }
    Ok(Checked {
        operation_id: envelope.operation_id().clone(),
        expected: envelope.expected_revision().as_str().to_string(),
        input,
    })
}

/// Claim `operation_id` in `receipts` for the command of `route` on `target` (the library it
/// names, or the location it binds) with `input`. Pure apart from the receipts' lock.
pub fn claim<T: Serialize>(
    receipts: &Arc<CommandReceipts>,
    route: &'static str,
    target: &str,
    input: &T,
    operation_id: &OperationId,
) -> Result<ClaimGuard, Settled> {
    let input = serde_json::to_value(input).map_err(|_| Settled::Failed)?;
    let fingerprint = Fingerprint {
        route,
        library_id: target.to_string(),
        input,
    };
    match receipts.claim(operation_id.as_str(), fingerprint) {
        Claim::Fresh(guard) => Ok(guard),
        Claim::Replay(answer) => Err(Settled::Replay(answer)),
        Claim::Reused => Err(Settled::Reused),
        Claim::Busy => Err(Settled::Busy),
    }
}

/// The `200` answer of a command that committed, as JSON, or `None` when `result` does not
/// encode. Pure.
pub fn committed<T: Serialize>(operation_id: &OperationId, result: T) -> Option<Value> {
    answer(operation_id, CommandOutcomeKind::Committed, result)
}

/// The `200` answer of a command whose work is durable but not complete, as JSON, or `None`
/// when `result` does not encode. Pure.
pub fn pending<T: Serialize>(operation_id: &OperationId, result: T) -> Option<Value> {
    answer(operation_id, CommandOutcomeKind::Pending, result)
}

/// The answer of `kind` with `result`. Pure.
fn answer<T: Serialize>(
    operation_id: &OperationId,
    kind: CommandOutcomeKind,
    result: T,
) -> Option<Value> {
    let answer = CommandAnswer {
        outcome: CommandOutcome::new(operation_id.clone(), kind),
        result: Some(result),
    };
    serde_json::to_value(&answer).ok()
}

/// The HTTP 409 answer of `error` for the command `operation_id`. Pure.
pub fn conflict(operation_id: &OperationId, error: ApiError) -> Response {
    let answer = ConflictAnswer {
        outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Conflict),
        result: None,
        error,
    };
    (StatusCode::CONFLICT, Json(answer)).into_response()
}

/// Answer `error` for the command `operation_id`: a `conflict` as HTTP 409 with a `conflict`
/// outcome, any other code as the typed error. Pure.
pub fn refusal(operation_id: Option<&OperationId>, error: ApiError) -> Response {
    match (error.code(), operation_id) {
        (ErrorCode::Conflict, Some(id)) => conflict(id, error),
        _ => error.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::{ExpectedRevision, OperationScope};

    fn envelope(schema: u32, operation: &str, scope: &str) -> CommandEnvelope {
        let mut envelope = CommandEnvelope::new(
            OperationId::new(operation).expect("operation"),
            ExpectedRevision::new("1").expect("revision"),
            OperationScope::new(scope).expect("scope"),
        );
        if schema != PROTOCOL_SCHEMA_VERSION {
            let json = serde_json::json!({
                "schema_version": schema,
                "operation_id": operation,
                "expected_revision": "1",
                "scope": scope,
            });
            envelope = serde_json::from_value(json).expect("envelope");
        }
        envelope
    }

    #[test]
    fn a_checked_envelope_keeps_its_operation_id_and_expected_revision() {
        let checked = check(envelope(1, "op-1", "library:lib"), 7u8, "library:lib").expect("ok");
        assert_eq!(checked.operation_id.as_str(), "op-1");
        assert_eq!(checked.expected, "1");
        assert_eq!(checked.input, 7);
    }

    #[test]
    fn another_schema_scope_or_a_non_opaque_operation_id_is_refused() {
        assert!(check(envelope(2, "op-1", "library:lib"), (), "library:lib").is_err());
        assert!(check(envelope(1, "op-1", "library:other"), (), "library:lib").is_err());
        assert!(check(envelope(1, "op 1", "library:lib"), (), "library:lib").is_err());
    }

    #[test]
    fn a_replay_refuses_nothing_and_the_other_settlements_name_their_code() {
        assert!(Settled::Replay(Value::Null).refusal().is_none());
        let codes: Vec<ErrorCode> = [Settled::Reused, Settled::Busy, Settled::Failed]
            .iter()
            .filter_map(|settled| settled.refusal().map(|(code, _)| code))
            .collect();
        assert_eq!(
            codes,
            [
                ErrorCode::InvalidRequest,
                ErrorCode::Conflict,
                ErrorCode::Internal
            ]
        );
    }

    #[test]
    fn committed_and_pending_answers_carry_the_outcome_fields_and_the_result() {
        let id = OperationId::new("op-2").expect("operation");
        let done = committed(&id, serde_json::json!({ "a": 1 })).expect("json");
        assert_eq!(done["operation_id"], "op-2");
        assert_eq!(done["kind"], "committed");
        assert_eq!(done["error"], Value::Null);
        assert_eq!(done["result"]["a"], 1);
        let later = pending(&id, 3u8).expect("json");
        assert_eq!(later["kind"], "pending");
        assert_eq!(later["result"], 3);
    }
}
