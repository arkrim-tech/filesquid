//! Command envelopes on the file write routes (Task API-F2.13; API-S-002, API-S-019,
//! UI-S-012).
//!
//! Every file write route (upload, create folder, rename, move, copy, remove, restore,
//! version labels) takes a `filesquid-core` [`CommandEnvelope`] beside its input and answers
//! a [`CommandOutcome`] for the same operation ID:
//!
//! - **Request.** A JSON route's body is [`FileCommand`], `{"envelope": ..., "input": ...}`,
//!   the body `FileSquidClient.command()` sends; the upload sends the same two members as the
//!   `envelope` and `input` parts of a `multipart/form-data` body beside its `file` part. The
//!   envelope MUST be schema version 1, its operation ID opaque
//!   ([`crate::journal::is_opaque`]) and its scope `library:<library_id>` of the input's
//!   library; anything else is `invalid_request`.
//! - **Expected revision.** The head version of the object the command changes, as the
//!   caller read it (`v1`, `v2`, ...), or [`NEW_FILE_REVISION`] for an upload that makes a
//!   new file. The service compares it under the library's write lock; a stale revision is a
//!   `conflict` with no change.
//! - **Answer.** `200` with [`CommandAnswer`]: the outcome's fields (`operation_id`, `kind`
//!   `committed`, `error` null) and `result`, what the route answered before this task. A
//!   `conflict` is HTTP 409 with [`ConflictAnswer`]: the [`ApiError`] fields beside a
//!   `conflict` outcome of the same operation ID. Every other refusal is the typed
//!   [`ApiError`].
//! - **Receipts.** A repeated operation ID of a command that changed something answers its
//!   first answer again with no second change, before its revision is checked; a command
//!   that changed nothing (creating a folder, or any refusal) stores no receipt
//!   ([`super::receipts`]).
//!
//! Every refusal is journaled once with its code, and every success with the route's entries,
//! each carrying the envelope's operation ID (API-S-017).

use super::error::FileError;
use super::receipts::{Claim, ClaimGuard, Fingerprint};
use super::routes::files;
use super::FileOperations;
use crate::auth::{journal_entry, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::operation::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, OperationId, PROTOCOL_SCHEMA_VERSION,
};
use crate::ApiState;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;

/// The expected revision of an upload that makes a new file: its object does not exist yet.
pub const NEW_FILE_REVISION: &str = "0";

/// Body of a JSON file write route: the envelope and the route's input.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileCommand<T> {
    /// Schema version, operation (idempotency) ID, expected revision and scope.
    pub envelope: CommandEnvelope,
    /// The route's input.
    pub input: T,
}

/// The answer of a file write command: the [`CommandOutcome`] fields and what it made.
#[derive(Clone, Debug, Serialize)]
pub struct CommandAnswer<T> {
    /// The outcome for the envelope's operation ID: `committed`.
    #[serde(flatten)]
    pub outcome: CommandOutcome,
    /// What the command made: the version it published, the copy, or the folder's state.
    pub result: Option<T>,
}

/// The HTTP 409 answer of a file write command: the typed error and a `conflict` outcome of
/// the same operation ID, so both an `ApiError` reader and a `CommandOutcome` reader read it.
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

/// The input of a file write route: it names the library its envelope's scope must name.
pub trait CommandInput: Serialize {
    /// The library the command changes.
    fn library_id(&self) -> &str;
}

/// Implement [`CommandInput`] for input types with a `library_id` field.
macro_rules! library_input {
    ($($input:ty),* $(,)?) => {
        $(impl CommandInput for $input {
            fn library_id(&self) -> &str {
                &self.library_id
            }
        })*
    };
}

library_input!(
    super::view::UploadQuery,
    super::view::FolderRequest,
    super::view::MoveRequest,
    super::view::RemoveRequest,
    super::view::RestoreRequest,
    super::view::LabelRequest,
    super::statements::RenameRequest,
    super::copy::CopyRequest,
);

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

/// What a command that ran answers: the route's view, its journal entries (each without an
/// operation ID; the route adds the envelope's), and whether it changed anything.
#[derive(Debug)]
pub struct Done<V> {
    /// What the route answers as `result`.
    pub view: V,
    /// The journal entries of the success.
    pub entries: Vec<JournalEntry>,
    /// True when the command published a version, so its answer is stored as a receipt.
    pub changed: bool,
}

/// `envelope` and `input` as a checked command, or `invalid_request` for an envelope that is
/// not schema version 1, an operation ID that is not opaque, or a scope other than
/// `library:<input.library_id>`. Pure.
pub fn check<T: CommandInput>(
    envelope: CommandEnvelope,
    input: T,
) -> Result<Checked<T>, FileError> {
    let scope = format!("library:{}", input.library_id());
    let valid = envelope.schema_version() == PROTOCOL_SCHEMA_VERSION
        && crate::journal::is_opaque(envelope.operation_id().as_str())
        && envelope.scope().as_str() == scope;
    if !valid {
        return Err(FileError::invalid(
            "The envelope is not schema version 1, its operation_id is not 1 to 128 letters, \
             digits, `.`, `_`, `-` or `:`, or its scope is not library:<library_id>.",
        ));
    }
    Ok(Checked {
        operation_id: envelope.operation_id().clone(),
        expected: envelope.expected_revision().as_str().to_string(),
        input,
    })
}

/// Run the checked `command` of the route journaled as `operation` once: answer its receipt
/// when the same command already ran, refuse an operation ID of another command or one in
/// flight, else `execute` it, journal the outcome and store the answer of a change.
pub(super) async fn run<T, V, F, Fut>(
    state: &ApiState,
    principal: &Principal,
    operation: &'static str,
    command: Result<Checked<T>, FileError>,
    execute: F,
) -> Response
where
    T: CommandInput,
    V: Serialize,
    F: FnOnce(Arc<dyn FileOperations>, Checked<T>, String) -> Fut,
    Fut: Future<Output = Result<Done<V>, FileError>>,
{
    let command = match command {
        Ok(command) => command,
        Err(err) => return refuse(state, operation, principal.actor(), None, err).await,
    };
    let operation_id = command.operation_id.clone();
    let guard = match claim(state, operation, &command) {
        Ok(guard) => guard,
        Err(answer) => {
            return answer
                .respond(state, operation, principal, &operation_id)
                .await
        }
    };
    let outcome = async {
        let files = files(state)?;
        execute(files, command, principal.actor().to_string()).await
    }
    .await;
    match outcome {
        Ok(done) => committed(state, &operation_id, done, guard).await,
        Err(err) => {
            refuse(
                state,
                operation,
                principal.actor(),
                Some(&operation_id),
                err,
            )
            .await
        }
    }
}

/// What a claim answered instead of a fresh run.
enum Settled {
    /// The stored answer of the same command.
    Replay(serde_json::Value),
    /// The ID names another command.
    Reused,
    /// A run of the ID is in flight.
    Busy,
    /// The input could not be fingerprinted.
    Failed,
}

impl Settled {
    /// Answer a claim that did not run the command; a replay changes nothing and is not
    /// journaled, a refusal is journaled once.
    async fn respond(
        self,
        state: &ApiState,
        operation: &str,
        principal: &Principal,
        operation_id: &OperationId,
    ) -> Response {
        let err = match self {
            Self::Replay(answer) => return Json(answer).into_response(),
            Self::Reused => FileError::invalid(
                "That operation_id already names another command; send a new one.",
            ),
            Self::Busy => FileError::conflict(
                "A command with that operation_id is still running; try again when it finishes.",
            ),
            Self::Failed => FileError::internal("a command input could not be fingerprinted"),
        };
        refuse(state, operation, principal.actor(), Some(operation_id), err).await
    }
}

/// Claim the operation ID of `command` in the host's receipts.
fn claim<T: CommandInput>(
    state: &ApiState,
    operation: &'static str,
    command: &Checked<T>,
) -> Result<ClaimGuard, Settled> {
    let input = serde_json::to_value(&command.input).map_err(|_| Settled::Failed)?;
    let fingerprint = Fingerprint {
        route: operation,
        library_id: command.input.library_id().to_string(),
        input,
    };
    match state
        .file_receipts()
        .claim(command.operation_id.as_str(), fingerprint)
    {
        Claim::Fresh(guard) => Ok(guard),
        Claim::Replay(answer) => Err(Settled::Replay(answer)),
        Claim::Reused => Err(Settled::Reused),
        Claim::Busy => Err(Settled::Busy),
    }
}

/// Journal a command's success under its operation ID, store the answer of a change as its
/// receipt, and answer it.
async fn committed<V: Serialize>(
    state: &ApiState,
    operation_id: &OperationId,
    done: Done<V>,
    guard: ClaimGuard,
) -> Response {
    for mut entry in done.entries {
        if entry.operation_id.is_none() {
            entry.operation_id = Some(operation_id.as_str().to_string());
        }
        journal_entry(state, entry).await;
    }
    let answer = CommandAnswer {
        outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Committed),
        result: Some(done.view),
    };
    let value = match serde_json::to_value(&answer) {
        Ok(value) => value,
        Err(_) => {
            return ApiError::internal("files.command", "an answer did not encode").into_response()
        }
    };
    if done.changed {
        guard.store(value.clone());
    }
    Json(value).into_response()
}

/// Journal a refused command once, with its code and, when known, its operation ID, and
/// answer it: a `conflict` as HTTP 409 with a `conflict` outcome, any other code as the
/// typed error.
pub(super) async fn refuse(
    state: &ApiState,
    operation: &str,
    actor: &str,
    operation_id: Option<&OperationId>,
    err: FileError,
) -> Response {
    let level = match err.code() {
        ErrorCode::Internal | ErrorCode::Unavailable => Level::Error,
        _ => Level::Warn,
    };
    let mut entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    if let Some(id) = operation_id {
        entry = entry.operation_id(id.as_str());
    }
    journal_entry(state, entry).await;
    let conflict = err.code() == ErrorCode::Conflict;
    let error = err.into_api_error(operation);
    match (conflict, operation_id) {
        (true, Some(id)) => conflict_answer(id, error),
        _ => error.into_response(),
    }
}

/// The HTTP 409 answer of `error` for the command `operation_id`.
fn conflict_answer(operation_id: &OperationId, error: ApiError) -> Response {
    let answer = ConflictAnswer {
        outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Conflict),
        result: None,
        error,
    };
    (StatusCode::CONFLICT, Json(answer)).into_response()
}

/// The success entry of a published version: the caller, the library, the object and the
/// version as opaque targets, with its `<object-id>@vN` reference when `with_ref` holds and
/// the journal can keep one (ADR 033 Decision 7).
pub(super) fn version_entry(
    operation: &str,
    principal: &Principal,
    (library_id, object_id, version): (&str, &str, &str),
    with_ref: bool,
) -> JournalEntry {
    let entry = JournalEntry::new(operation, Outcome::Succeeded)
        .actor(principal.actor())
        .target(library_id)
        .target(object_id)
        .target(version);
    let reference = filesquid_core::VersionRef::new(object_id, version)
        .ok()
        .map(|reference| reference.object_ref())
        .filter(|object_ref| crate::journal::is_object_ref(object_ref));
    match (with_ref, reference) {
        (true, Some(object_ref)) => entry.object_ref(object_ref),
        (true, None) => {
            tracing::warn!(
                target: "filesquid::files",
                "a published version was journaled without an object reference"
            );
            entry
        }
        (false, _) => entry,
    }
}
