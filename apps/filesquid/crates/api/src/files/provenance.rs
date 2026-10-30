//! Journal and commit linkage of every file and metadata write (Task API-F2.8; API-S-028,
//! API-S-017; ADR 040 Decision 6).
//!
//! Each write route (upload, move, remove, copy, rename, restore, label, and the metadata
//! routes) prepares its one commit here:
//!
//! - **Version metadata.** The `filesquid.operation.v1` message carries the operation ID (the
//!   command envelope's, or a new one for a write without an envelope), the operation kind
//!   and the caller's optional message as the version `label`. With provenance installed
//!   ([`FileService::install_provenance`]) the version `user` is the installation (name
//!   `FileSquid installation`, address `urn:uuid:<installation-uuid>`), never a person; a
//!   service without it names the caller, as before.
//! - **Version-bound files.** With provenance installed, a [`CommitRecorder`] writes the
//!   PROV-O record and the commit's journal entries into the same version, under journal
//!   IDs allocated before the commit.
//! - **Journal entries.** [`Prepared::finish`] answers the entries to append once the commit
//!   is published: exactly one `ocfl.commit` naming the operation ID and the object version,
//!   the route's own entry, and `metadata.edit` for a write of a metadata record. Each
//!   carries the operation ID and the object reference, and is redacted. A refused or failed
//!   attempt publishes nothing, so it is journaled by its route as before and never reaches
//!   the version history.

use super::error::FileError;
use super::labels::check_label;
use super::service::{commit_view, FileService};
use super::view::CommitView;
use super::METADATA_EDIT_OPERATION;
use crate::history::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use crate::journal::commit::{ocfl_commit_entries, CommitFacts};
use crate::journal::{JournalEntry, JournalStore, Outcome};
use crate::library::mint_uuid_v4;
use filesquid_core::{OperationId, OperationKind, VersionRef};
use filesquid_metadata::store::ocfl::CommitOptions;
use filesquid_ocfl::change::{ChangeError, ChangeModel, VersionMeta};
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::{CommitReceipt, VersionBoundFiles, VersionReservations};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::Instrument;

/// Who asks for a write, and the operation it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    /// The journal actor: an opaque principal name.
    pub actor: String,
    /// The command envelope's operation ID; `None` for a write without an envelope, which is
    /// given a new one.
    pub operation_id: Option<OperationId>,
}

impl Caller {
    /// `actor`, with no operation ID yet.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            operation_id: None,
        }
    }

    /// The same caller writing for the operation `operation_id`.
    #[must_use]
    pub fn with_operation(mut self, operation_id: OperationId) -> Self {
        self.operation_id = Some(operation_id);
        self
    }
}

impl From<&str> for Caller {
    /// `actor` with no operation ID.
    fn from(actor: &str) -> Self {
        Self::new(actor)
    }
}

impl From<String> for Caller {
    /// `actor` with no operation ID.
    fn from(actor: String) -> Self {
        Self::new(actor)
    }
}

/// What a file service records provenance with: who commits, and the journal store that
/// hands out each commit's entry IDs before it.
#[derive(Clone)]
pub struct FileProvenance {
    context: ProvenanceContext,
    ids: Arc<dyn JournalStore>,
}

/// A published version and the journal entries of its commit, to append after it, each once
/// under its ID (allocated before the commit when provenance is installed).
#[derive(Clone, Debug, PartialEq)]
pub struct Committed<V = CommitView> {
    /// What the route answers about the version.
    pub view: V,
    /// The commit's journal entries: `ocfl.commit` first, then the operation's own.
    pub entries: Vec<JournalEntry>,
}

/// Which write one commit is, for its message, record and journal entries.
pub(super) struct Write<'a> {
    /// Who writes, for which operation.
    pub caller: &'a Caller,
    /// The operation kind of the message and the record.
    pub kind: OperationKind,
    /// The caller's version label, already checked with [`label_of`].
    pub label: Option<String>,
    /// The route's journal operation, such as `files.move`.
    pub operation: &'static str,
    /// The library written.
    pub library_id: &'a str,
    /// The object written.
    pub object_id: &'a str,
    /// Whether the commit writes a metadata record (journaled as `metadata.edit` too).
    pub metadata: bool,
    /// Logical moves in the commit: old path to new path.
    pub moves: BTreeMap<String, String>,
}

/// The version metadata, the operation and the optional recorder of one commit.
pub(super) struct Prepared {
    meta: VersionMeta,
    operation: CommitOperation,
    own: Vec<JournalEntry>,
    recorder: Option<CommitRecorder>,
}

impl<B, R, S> FileService<B, R, S> {
    /// Record provenance as `context`: every file and metadata write then commits as the
    /// installation and writes its PROV-O record in the same version, naming journal entry
    /// IDs that `ids` allocates before the commit (API-S-028). Returns false, keeping the
    /// first, when already installed.
    pub fn install_provenance(
        &self,
        context: ProvenanceContext,
        ids: Arc<dyn JournalStore>,
    ) -> bool {
        self.provenance.set(FileProvenance { context, ids }).is_ok()
    }

    /// The version metadata, journal entries and recorder of `write`. Allocates journal IDs
    /// when provenance is installed.
    pub(super) fn prepare(&self, write: &Write<'_>) -> Result<Prepared, FileError> {
        let created = self.created();
        let operation = commit_operation(write, &created)?;
        let own = own_entries(write);
        let Some(provenance) = self.provenance.get() else {
            let user = User {
                name: write.caller.actor.clone(),
                address: None,
            };
            let meta = VersionMeta::new(created)
                .with_user(user)
                .with_message(operation.message());
            return Ok(Prepared {
                meta,
                operation,
                own,
                recorder: None,
            });
        };
        let recorder = recorder_of(provenance, &operation, &own)?;
        Ok(Prepared {
            meta: commit_meta(&provenance.context, &operation, created),
            operation,
            own,
            recorder: Some(recorder),
        })
    }
}

impl Prepared {
    /// The operation ID the commit carries.
    pub(super) fn operation_id(&self) -> &OperationId {
        &self.operation.operation_id
    }

    /// The version metadata to commit with.
    pub(super) fn meta(&self) -> VersionMeta {
        self.meta.clone()
    }

    /// The activity span of this commit, so engine warnings raised while it commits are
    /// journaled with its operation ID (API-S-031).
    pub(super) fn span(&self) -> tracing::Span {
        crate::canonical::message::commit_span(self.meta.message.as_deref())
    }

    /// The recorder as the change model's version-bound file function, if any.
    pub(super) fn bound(&self) -> Option<&dyn VersionBoundFiles> {
        self.recorder.as_ref().map(|r| r as &dyn VersionBoundFiles)
    }

    /// The metadata store's commit options of this commit (MD-S-014).
    pub(super) fn options(&self) -> CommitOptions<'_> {
        CommitOptions {
            user: self.meta.user.clone(),
            message: self.meta.message.clone(),
            bound: self.bound(),
        }
    }

    /// The answer `view` of the published `receipt`, with the commit's journal entries.
    pub(super) fn finish<V>(self, view: V, receipt: &CommitReceipt) -> Committed<V> {
        let entries = match &self.recorder {
            Some(recorder) => recorder.journal_entries(receipt),
            None => self.unrecorded_entries(receipt),
        };
        Committed { view, entries }
    }

    /// The answer of the published `receipt` as a [`CommitView`] of `library_id`.
    pub(super) fn finish_view(self, library_id: &str, receipt: CommitReceipt) -> Committed {
        let view = commit_view(library_id, receipt.clone());
        self.finish(view, &receipt)
    }

    /// The journal entries of a commit without provenance: `ocfl.commit` (with the takeover
    /// warning, if any) and the operation's own, each with the operation ID, the object
    /// reference and the actor, redacted. No ID: nothing was allocated.
    fn unrecorded_entries(&self, receipt: &CommitReceipt) -> Vec<JournalEntry> {
        let operation_id = &self.operation.operation_id;
        let Ok(reference) = VersionRef::new(receipt.object_id.clone(), receipt.version.to_string())
        else {
            tracing::warn!(target: super::LOG_TARGET, "a commit receipt names an unusable object reference");
            return Vec::new();
        };
        let facts = CommitFacts {
            operation_id,
            version: &reference,
            took_over: receipt.took_over,
        };
        let object_ref = reference.object_ref();
        let commit = ocfl_commit_entries(&facts)
            .into_iter()
            .map(|entry| entry.actor(&self.operation.actor));
        let own = self.own.iter().map(|entry| {
            entry
                .clone()
                .operation_id(operation_id.as_str())
                .object_ref(object_ref.as_str())
        });
        commit.chain(own).map(JournalEntry::redacted).collect()
    }
}

/// The caller's version message as the version label: `None` without one, else the checked
/// label (1 to `MAX_LABEL_BYTES` bytes, not blank, no control characters). Pure.
pub(super) fn label_of(message: Option<&str>) -> Result<Option<String>, FileError> {
    match message {
        None => Ok(None),
        Some(text) => check_label(text).map(|()| Some(text.to_string())),
    }
}

/// The operation of `write` that started at `created`: the caller's operation ID or a new
/// one, with the label, moves and actor.
fn commit_operation(write: &Write<'_>, created: &str) -> Result<CommitOperation, FileError> {
    let operation_id = match &write.caller.operation_id {
        Some(id) => id.clone(),
        None => new_operation_id()?,
    };
    let operation = CommitOperation::new(operation_id, write.kind, created)
        .with_actor(write.caller.actor.as_str())
        .with_moves(write.moves.clone());
    match &write.label {
        None => Ok(operation),
        Some(label) => operation
            .with_label(label.clone())
            .map_err(|_| FileError::invalid("A label is too long.")),
    }
}

/// A new UUID v4 operation ID, for a write without a command envelope.
fn new_operation_id() -> Result<OperationId, FileError> {
    mint_uuid_v4()
        .ok()
        .and_then(|id| OperationId::new(id).ok())
        .ok_or_else(|| FileError::unavailable("no operation id could be minted"))
}

/// The operation's own journal entries: the route's, and `metadata.edit` for a write of a
/// metadata record, each naming the library and the object. Pure.
fn own_entries(write: &Write<'_>) -> Vec<JournalEntry> {
    let entry = |operation: &str| {
        JournalEntry::new(operation, Outcome::Succeeded)
            .actor(write.caller.actor.as_str())
            .target(write.library_id)
            .target(write.object_id)
    };
    let mut entries = vec![entry(write.operation)];
    if write.metadata {
        entries.push(entry(METADATA_EDIT_OPERATION));
    }
    entries
}

/// Journal IDs a commit allocates besides its operation's own entries: the `ocfl.commit`
/// entry's and the `ocfl.lease.takeover` warning's, used only when the commit takes over a
/// held reservation.
const COMMIT_IDS: usize = CommitRecorder::JOURNAL_IDS + 1;

/// The recorder of `operation`, with the IDs of its `ocfl.commit` entry, its possible
/// takeover warning and the `own` entries, allocated from the journal store before the
/// commit so all of them can ride the version.
fn recorder_of(
    provenance: &FileProvenance,
    operation: &CommitOperation,
    own: &[JournalEntry],
) -> Result<CommitRecorder, FileError> {
    let count = COMMIT_IDS + own.len();
    let ids = provenance
        .ids
        .allocate_ids(count)
        .ok()
        .filter(|ids| ids.len() == count)
        .ok_or_else(|| FileError::unavailable("no journal ids for the commit record"))?;
    let entries = own
        .iter()
        .zip(&ids[COMMIT_IDS..])
        .map(|(entry, id)| {
            let mut entry = entry.clone();
            entry.id = Some(*id);
            entry
        })
        .collect();
    let recorder = CommitRecorder::new(provenance.context.clone(), operation.clone(), ids[0])
        .with_takeover_id(ids[1]);
    Ok(recorder.with_entries(entries))
}

/// Commit what `model` has staged with `prepared`'s metadata and recorder. A publication is
/// a success even when the engine kept its staging area; on a refusal the staging area is
/// reset, so a later write starts clean. Storage I/O.
pub(super) async fn commit_staged<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    prepared: &Prepared,
) -> Result<CommitReceipt, FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let commit = model.commit_with(prepared.meta(), prepared.bound());
    let err = match commit.instrument(prepared.span()).await {
        Ok(receipt) => return Ok(receipt),
        Err(ChangeError::StagingNotCleared { receipt, reason }) => {
            tracing::warn!(target: super::LOG_TARGET, %reason, "a version kept its staging area");
            return Ok(*receipt);
        }
        Err(err) => err,
    };
    discard_staging(model).await;
    Err(err.into())
}

/// Discard what `model` has staged after a refused write, so the next write starts clean;
/// a failure to do so is logged. Storage I/O.
///
/// Only this write's changes are there to discard: [`require_clean`] found the staging area
/// empty and cleared it, and the caller holds the library's write lock from that check to
/// this cleanup, so no other writer of this host staged in between.
pub(super) async fn discard_staging<B, R, S>(model: &ChangeModel<'_, B, R, S>)
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    if let Err(reset) = model.reset().await {
        tracing::warn!(target: super::LOG_TARGET, error = %reset, "a refused write left its staging area");
    }
}

/// Refuse, as a `conflict`, a write to an object whose staging area holds another change;
/// otherwise clear the staging area, so an index left without changes (for example by a
/// published commit that could not clear it) never gives this write an older base than the
/// head. The caller holds the library's write lock. Storage I/O.
pub(super) async fn require_clean<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
) -> Result<(), FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    if !model.status().await?.changes.is_empty() {
        return Err(FileError::conflict(
            "Another change to this object is in progress; try again when it has finished.",
        ));
    }
    model.reset().await?;
    Ok(())
}
