//! Pending changes of a client that works from a local replica (API-S-024, ADR 025).
//!
//! A change a browser makes is first only *pending*: it is kept with its command unchanged
//! and shown as pending. It becomes a canonical OCFL commit, `committed`, only when its fenced
//! head publication succeeds ([`PendingLedger::commit_published`]). A writer that loses the
//! race re-reads the head and rebases or reports the conflict; it never overwrites. Changes
//! publish in queue order per library, and every loop and queue here is bounded.
//!
//! The ledger is a pure state machine. The host stores [`PendingLedger::snapshot`] in its
//! own local storage and decides when to publish; [`HeadPublication`] is the minimal proof of
//! a successful publication, which Task CL-0.4 replaces with the frozen commit receipt.

use super::{ReplicaError, LOG_TARGET};
use crate::library::CommandRequest;
use crate::operation::{CommandEnvelope, ExpectedRevision, OperationId};
use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Most changes (pending or not yet acknowledged) one library may hold in the ledger.
pub const MAX_PENDING_PER_LIBRARY: usize = 1_000;

/// Most libraries the ledger may hold changes for.
pub const MAX_LIBRARIES: usize = 64;

/// Publication attempts after which a change stalls for the owner to decide.
pub const MAX_ATTEMPTS: u32 = 5;

/// Where a pending change stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingState {
    /// Waiting for its turn to publish.
    Queued,
    /// A fenced head publication is in flight.
    Publishing,
    /// The last attempt failed in a way a retry may fix; it publishes again on the next try.
    RetryWait,
    /// The head moved: the owner rebases the change or discards it.
    Conflict,
    /// [`MAX_ATTEMPTS`] attempts failed: the owner rebases the change or discards it.
    Stalled,
    /// The head publication succeeded: the change is a canonical OCFL commit.
    Committed,
}

/// Proof that a fenced head publication succeeded. Only this moves a change to `committed`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadPublication {
    revision: String,
}

impl HeadPublication {
    /// A publication that left the library at `revision`.
    ///
    /// # Errors
    ///
    /// Returns `invalid_publication` when `revision` is blank.
    pub fn new(revision: impl Into<String>) -> Result<Self, ReplicaError> {
        let revision = revision.into();
        if revision.trim().is_empty() {
            return Err(ReplicaError::invalid(
                "invalid_publication",
                "A published change must name the revision it produced.",
            ));
        }
        Ok(Self { revision })
    }
}

/// One change waiting to become canonical.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingChange {
    /// The library the command changes.
    pub library_id: LibraryId,
    /// The command, unchanged; its operation ID is the change's identity.
    pub request: CommandRequest,
    /// When the change was queued (RFC 3339, UTC).
    pub queued_at: String,
    /// Where the change stands.
    pub state: PendingState,
    /// Publication attempts begun so far.
    pub attempts: u32,
    /// The revision the publication produced; present exactly when `committed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_revision: Option<String>,
}

impl PendingChange {
    /// The change's operation ID.
    fn operation_id(&self) -> &OperationId {
        self.request.envelope.operation_id()
    }
}

/// The ledger of pending changes, in queue order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingLedger {
    changes: Vec<PendingChange>,
}

impl PendingLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue `request` as a pending change at `queued_at` (RFC 3339, UTC, ending in `Z`). A
    /// repeat of the same request answers the existing change.
    ///
    /// # Errors
    ///
    /// Returns `invalid_time` for a bad time, `pending_needs_library` for a command that
    /// names no library, `operation_reused` when the operation ID belongs to a different
    /// request, and `pending_full` at the per-library or per-ledger bound.
    pub fn enqueue(
        &mut self,
        request: CommandRequest,
        queued_at: &str,
    ) -> Result<PendingChange, ReplicaError> {
        check_utc(queued_at)?;
        let library_id = request.command.library_id().cloned().ok_or_else(|| {
            ReplicaError::invalid(
                "pending_needs_library",
                "Only a change to an existing library can wait as pending.",
            )
        })?;
        if let Some(existing) = self.get(request.envelope.operation_id()) {
            return same_request(existing, &request);
        }
        self.check_room(&library_id)?;
        let change = PendingChange {
            library_id,
            request,
            queued_at: queued_at.to_string(),
            state: PendingState::Queued,
            attempts: 0,
            published_revision: None,
        };
        self.changes.push(change.clone());
        Ok(change)
    }

    /// The change with operation ID `operation_id`.
    pub fn get(&self, operation_id: &OperationId) -> Option<&PendingChange> {
        self.changes
            .iter()
            .find(|c| c.operation_id() == operation_id)
    }

    /// The changes of `library_id`, in queue order, including committed ones not yet
    /// acknowledged.
    pub fn list(&self, library_id: &LibraryId) -> Vec<&PendingChange> {
        self.changes
            .iter()
            .filter(|c| &c.library_id == library_id)
            .collect()
    }

    /// How many changes of `library_id` are not yet canonical.
    pub fn pending_count(&self, library_id: &LibraryId) -> usize {
        self.list(library_id)
            .into_iter()
            .filter(|c| c.state != PendingState::Committed)
            .count()
    }

    /// The change that may begin publishing now: the oldest unfinished change of the library,
    /// when it is `queued` or `retry_wait`. A publishing, conflicting or stalled head blocks
    /// the queue.
    pub fn next_to_publish(&self, library_id: &LibraryId) -> Option<&PendingChange> {
        let head = self.head_of(library_id)?;
        matches!(head.state, PendingState::Queued | PendingState::RetryWait).then_some(head)
    }

    /// Begin a fenced head publication of `operation_id`, counting the attempt.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, `wrong_state` unless the change is `queued` or `retry_wait`, and
    /// `not_next` unless it is the oldest unfinished change of its library.
    pub fn begin_publish(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<PendingChange, ReplicaError> {
        let at = self.index_of(operation_id)?;
        let change = &self.changes[at];
        expect_state(change, &[PendingState::Queued, PendingState::RetryWait])?;
        if self
            .next_to_publish(&change.library_id)
            .map(PendingChange::operation_id)
            != Some(operation_id)
        {
            return Err(ReplicaError::refused(
                "not_next",
                "An earlier change in this library has to publish first.",
            ));
        }
        let change = &mut self.changes[at];
        change.attempts += 1;
        change.state = PendingState::Publishing;
        Ok(change.clone())
    }

    /// Record that the fenced head publication of `operation_id` succeeded: the change is now
    /// a canonical OCFL commit. This is the only way to reach `committed`.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` unless the change is `publishing`.
    pub fn commit_published(
        &mut self,
        operation_id: &OperationId,
        head: HeadPublication,
    ) -> Result<PendingChange, ReplicaError> {
        let change = self.publishing_mut(operation_id)?;
        change.state = PendingState::Committed;
        change.published_revision = Some(head.revision);
        Ok(change.clone())
    }

    /// Record that the publication of `operation_id` lost the race: the head moved.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` unless the change is `publishing`.
    pub fn report_conflict(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<PendingChange, ReplicaError> {
        let change = self.publishing_mut(operation_id)?;
        change.state = PendingState::Conflict;
        Ok(change.clone())
    }

    /// Record a retryable failure of `operation_id`: `retry_wait`, or `stalled` once
    /// [`MAX_ATTEMPTS`] attempts have been made.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` unless the change is `publishing`.
    pub fn report_retry(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<PendingChange, ReplicaError> {
        let change = self.publishing_mut(operation_id)?;
        change.state = if change.attempts >= MAX_ATTEMPTS {
            PendingState::Stalled
        } else {
            PendingState::RetryWait
        };
        Ok(change.clone())
    }

    /// Rebase a `conflict` or `stalled` change onto the revision the owner re-read: it is
    /// `queued` again with no attempts.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` unless the change is `conflict` or `stalled`.
    pub fn rebase(
        &mut self,
        operation_id: &OperationId,
        expected_revision: ExpectedRevision,
    ) -> Result<PendingChange, ReplicaError> {
        let at = self.index_of(operation_id)?;
        expect_state(
            &self.changes[at],
            &[PendingState::Conflict, PendingState::Stalled],
        )?;
        let change = &mut self.changes[at];
        let envelope = &change.request.envelope;
        change.request.envelope = CommandEnvelope::new(
            envelope.operation_id().clone(),
            expected_revision,
            envelope.scope().clone(),
        );
        change.state = PendingState::Queued;
        change.attempts = 0;
        Ok(change.clone())
    }

    /// Drop a change that is not publishing and not canonical.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` for a `publishing` or `committed` change.
    pub fn discard(&mut self, operation_id: &OperationId) -> Result<PendingChange, ReplicaError> {
        let at = self.index_of(operation_id)?;
        let open = [
            PendingState::Queued,
            PendingState::RetryWait,
            PendingState::Conflict,
            PendingState::Stalled,
        ];
        expect_state(&self.changes[at], &open)?;
        Ok(self.changes.remove(at))
    }

    /// Remove a `committed` change from the ledger once the UI has shown it.
    ///
    /// # Errors
    ///
    /// Returns `not_found`, or `wrong_state` unless the change is `committed`.
    pub fn acknowledge(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<PendingChange, ReplicaError> {
        let at = self.index_of(operation_id)?;
        expect_state(&self.changes[at], &[PendingState::Committed])?;
        Ok(self.changes.remove(at))
    }

    /// The ledger as JSON for the host's local storage. It holds commands and IDs only.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] if encoding fails (the detail goes to stdout only).
    pub fn snapshot(&self) -> Result<String, ReplicaError> {
        serde_json::to_string(&self.changes).map_err(|err| {
            tracing::info!(target: LOG_TARGET, error_code = "internal", detail = %err, "ledger could not be encoded");
            ReplicaError::Internal
        })
    }

    /// Rebuild a ledger from a [`Self::snapshot`]. A change that was `publishing` when the
    /// snapshot was taken was interrupted and comes back as `retry_wait`.
    ///
    /// # Errors
    ///
    /// Returns `invalid_snapshot` for malformed JSON, a duplicate operation ID, a change whose
    /// library differs from its command's, a bad time, a committed change without a revision,
    /// a revision on an uncommitted one, too many attempts, or input over the bounds.
    pub fn restore(snapshot: &str) -> Result<Self, ReplicaError> {
        let mut changes: Vec<PendingChange> =
            serde_json::from_str(snapshot).map_err(|_| invalid_snapshot())?;
        if changes.len() > MAX_LIBRARIES * MAX_PENDING_PER_LIBRARY {
            return Err(invalid_snapshot());
        }
        let mut ledger = Self::new();
        let mut seen = HashSet::new();
        for change in &mut changes {
            check_restored(change, &mut seen)?;
            ledger
                .check_room(&change.library_id)
                .map_err(|_| invalid_snapshot())?;
            ledger.changes.push(change.clone());
        }
        Ok(ledger)
    }

    /// The oldest change of `library_id` that is not committed.
    fn head_of(&self, library_id: &LibraryId) -> Option<&PendingChange> {
        self.changes
            .iter()
            .find(|c| &c.library_id == library_id && c.state != PendingState::Committed)
    }

    /// Position of the change with `operation_id`.
    fn index_of(&self, operation_id: &OperationId) -> Result<usize, ReplicaError> {
        self.changes
            .iter()
            .position(|c| c.operation_id() == operation_id)
            .ok_or(ReplicaError::NotFound)
    }

    /// The `publishing` change with `operation_id`, for a transition out of publishing.
    fn publishing_mut(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<&mut PendingChange, ReplicaError> {
        let at = self.index_of(operation_id)?;
        expect_state(&self.changes[at], &[PendingState::Publishing])?;
        Ok(&mut self.changes[at])
    }

    /// Whether one more change for `library_id` fits the per-library and per-ledger bounds.
    fn check_room(&self, library_id: &LibraryId) -> Result<(), ReplicaError> {
        let in_library = self
            .changes
            .iter()
            .filter(|c| &c.library_id == library_id)
            .count();
        let libraries: HashSet<&LibraryId> = self.changes.iter().map(|c| &c.library_id).collect();
        let new_library = in_library == 0 && libraries.len() >= MAX_LIBRARIES;
        if in_library >= MAX_PENDING_PER_LIBRARY || new_library {
            return Err(ReplicaError::refused(
                "pending_full",
                "Too many changes are waiting on this device. Let them publish first.",
            ));
        }
        Ok(())
    }
}

/// The existing change when `request` repeats it, else `operation_reused`.
fn same_request(
    existing: &PendingChange,
    request: &CommandRequest,
) -> Result<PendingChange, ReplicaError> {
    if &existing.request == request {
        return Ok(existing.clone());
    }
    Err(ReplicaError::refused(
        "operation_reused",
        "That operation identifier already belongs to a different change.",
    ))
}

/// `wrong_state` unless `change` is in one of the `allowed` states.
fn expect_state(change: &PendingChange, allowed: &[PendingState]) -> Result<(), ReplicaError> {
    if allowed.contains(&change.state) {
        return Ok(());
    }
    Err(ReplicaError::refused(
        "wrong_state",
        "That change cannot do this in its current state.",
    ))
}

/// `invalid_time` unless `text` is an RFC 3339 time in UTC written with a `Z` suffix.
fn check_utc(text: &str) -> Result<(), ReplicaError> {
    let ok = text.ends_with('Z') && OffsetDateTime::parse(text, &Rfc3339).is_ok();
    if !ok {
        return Err(ReplicaError::invalid(
            "invalid_time",
            "The time must be RFC 3339 in UTC, for example 2026-10-18T00:00:00Z.",
        ));
    }
    Ok(())
}

/// The error for a snapshot the ledger cannot trust.
fn invalid_snapshot() -> ReplicaError {
    ReplicaError::invalid(
        "invalid_snapshot",
        "The saved list of waiting changes is damaged and was not used.",
    )
}

/// Validate one restored change and turn an interrupted publication into `retry_wait`.
fn check_restored(
    change: &mut PendingChange,
    seen: &mut HashSet<OperationId>,
) -> Result<(), ReplicaError> {
    let names_own_library = change.request.command.library_id() == Some(&change.library_id);
    let committed = change.state == PendingState::Committed;
    let revision_ok = change
        .published_revision
        .as_deref()
        .is_some_and(|r| !r.trim().is_empty())
        == committed;
    let fresh = seen.insert(change.operation_id().clone());
    let valid = names_own_library
        && revision_ok
        && fresh
        && change.attempts <= MAX_ATTEMPTS
        && check_utc(&change.queued_at).is_ok();
    if !valid {
        return Err(invalid_snapshot());
    }
    if change.state == PendingState::Publishing {
        change.state = PendingState::RetryWait;
    }
    Ok(())
}
