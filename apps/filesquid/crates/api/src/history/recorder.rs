//! The provenance files of one commit (API-S-028, API-S-032; ADR 040 Decisions 3, 4 and 6).
//!
//! [`CommitRecorder`] is handed to the change model as its version-bound file function. For
//! the version the change model reserved it returns two logical files, written in that same
//! version:
//!
//! - `provenance/<version>.jsonld`: the PROV-O record built by `filesquid-prov`, naming the
//!   commit's journal entry ID;
//! - `provenance/<version>.journal.jsonl`: that journal entry (`ocfl.commit`, redacted) as
//!   JSON Lines, carrying its pre-allocated ID, the operation ID and the object reference
//!   `<object-id>@<version>`.
//!
//! A rebase reserves another version and calls the recorder again, so both files always name
//! the published version. The caller appends [`CommitRecorder::journal_entries`] to the
//! journal after the commit; their IDs were allocated before it, so a replay stores nothing
//! twice.

use std::collections::BTreeMap;
use std::sync::Mutex;

use filesquid_core::{InstallationId, OperationId, OperationKind, OperationMessage, VersionRef};
use filesquid_ocfl::change::VersionMeta;
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::write::{BoundContext, CommitReceipt, VersionBoundFiles};
use filesquid_prov::ocfl::{bound_record, OperationFacts};
use filesquid_prov::{HostFacts, OcflUser, RECORD_PREFIX};

use crate::journal::commit::{ocfl_commit_entries, CommitFacts};
use crate::journal::{now_ms, parse_rfc3339_ms, JournalEntry};

/// The OCFL version `user.name` of every FileSquid commit; the address names the
/// installation. Never a person's name (owner decision, ADR 040 Decision 4).
pub const INSTALLATION_USER_NAME: &str = "FileSquid installation";

/// File-name suffix of a commit's journal entries.
pub const JOURNAL_SUFFIX: &str = ".journal.jsonl";

/// The logical path of the journal entries of `version`: `provenance/<version>.journal.jsonl`.
pub fn journal_path(version: &str) -> String {
    format!("{RECORD_PREFIX}{version}{JOURNAL_SUFFIX}")
}

/// Who commits: the installation and the FileSquid host it runs. The host configures it
/// once at startup (DE-S-012 installation UUID).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceContext {
    /// The installation UUID: the owner agent of every record and the OCFL `user` address.
    pub installation: InstallationId,
    /// The host software agent: product, distribution and version, never a host name.
    pub host: HostFacts,
}

/// The OCFL version `user` of `installation`: name [`INSTALLATION_USER_NAME`], address
/// `urn:uuid:<uuid>`.
pub fn version_user(installation: &InstallationId) -> User {
    User {
        name: INSTALLATION_USER_NAME.to_string(),
        address: Some(installation.urn()),
    }
}

/// What the caller knows about the operation one commit belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitOperation {
    /// The operation ID, shared by every version and journal entry of the operation.
    pub operation_id: OperationId,
    /// The operation kind.
    pub kind: OperationKind,
    /// When the operation started: RFC 3339 UTC, no later than the commit's `created`.
    pub started: String,
    /// The version label (PRD F-065), when set.
    pub label: Option<String>,
    /// Logical moves in the commit: old path to new path.
    pub moves: BTreeMap<String, String>,
    /// The journal actor: `system` or an opaque principal name.
    pub actor: String,
}

impl CommitOperation {
    /// An operation `operation_id` of `kind` that started at `started` (RFC 3339 UTC), with
    /// no label or moves, journaled as `system`.
    pub fn new(operation_id: OperationId, kind: OperationKind, started: impl Into<String>) -> Self {
        Self {
            operation_id,
            kind,
            started: started.into(),
            label: None,
            moves: BTreeMap::new(),
            actor: "system".to_string(),
        }
    }

    /// The same operation with a version label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The same operation with the logical moves of its commit.
    #[must_use]
    pub fn with_moves(mut self, moves: BTreeMap<String, String>) -> Self {
        self.moves = moves;
        self
    }

    /// The same operation journaled as `actor`.
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = actor.into();
        self
    }

    /// The `filesquid.operation.v1` version message of this operation.
    pub fn message(&self) -> String {
        let message = OperationMessage::new(self.operation_id.clone(), self.kind);
        match &self.label {
            Some(label) => message.with_label(label.clone()).format(),
            None => message.format(),
        }
    }
}

/// The version metadata of a commit of `operation` by `context`'s installation, created at
/// `created` (RFC 3339 UTC): the installation `user` and the operation message. Usable for
/// every commit path, including the metadata store's commit options.
pub fn commit_meta(
    context: &ProvenanceContext,
    operation: &CommitOperation,
    created: impl Into<String>,
) -> VersionMeta {
    VersionMeta::new(created)
        .with_user(version_user(&context.installation))
        .with_message(operation.message())
}

/// The change model's version-bound file function that writes a commit's PROV-O record and
/// journal entries (OCFL-S-024), and keeps the entries of the last version it described.
#[derive(Debug)]
pub struct CommitRecorder {
    context: ProvenanceContext,
    operation: CommitOperation,
    journal_id: i64,
    last: Mutex<Option<JournalEntry>>,
}

impl CommitRecorder {
    /// How many journal IDs one commit needs, allocated before it
    /// ([`crate::journal::JournalStore::allocate_ids`]).
    pub const JOURNAL_IDS: usize = 1;

    /// A recorder of the commit of `operation` by `context`, whose `ocfl.commit` entry is
    /// stored under `journal_id`.
    pub fn new(context: ProvenanceContext, operation: CommitOperation, journal_id: i64) -> Self {
        Self {
            context,
            operation,
            journal_id,
            last: Mutex::new(None),
        }
    }

    /// The journal entries to append after the commit `receipt` published: the `ocfl.commit`
    /// entry the version's files carry (same ID), and an `ocfl.lease.takeover` warning when
    /// the reservation took over a lease. When the recorder described another version (it
    /// never should), the commit entry is rebuilt from the receipt without an ID, so the
    /// commit is still journaled.
    pub fn journal_entries(&self, receipt: &CommitReceipt) -> Vec<JournalEntry> {
        let version = receipt.version.to_string();
        let Ok(reference) = VersionRef::new(receipt.object_id.clone(), version) else {
            tracing::warn!("a commit receipt names an unusable object reference");
            return Vec::new();
        };
        let facts = CommitFacts {
            operation_id: &self.operation.operation_id,
            version: &reference,
            took_over: receipt.took_over,
        };
        let mut entries = self.stamped(ocfl_commit_entries(&facts), now_ms());
        let kept = self
            .last_entry()
            .filter(|entry| entry.object_ref.as_deref() == Some(reference.object_ref().as_str()));
        match kept {
            Some(entry) => entries[0] = entry,
            None => tracing::warn!("the recorded commit entry names another version"),
        }
        entries
    }

    /// The commit entry generated last, if any.
    fn last_entry(&self) -> Option<JournalEntry> {
        let guard = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    }

    /// `entries` with this operation's actor and time `at_ms`, redacted.
    fn stamped(&self, entries: Vec<JournalEntry>, at_ms: i64) -> Vec<JournalEntry> {
        entries
            .into_iter()
            .map(|entry| entry.actor(&self.operation.actor).at_ms(at_ms).redacted())
            .collect()
    }

    /// The `ocfl.commit` entry of the version `context` describes, with the pre-allocated ID.
    fn commit_entry(&self, context: &BoundContext<'_>) -> Result<JournalEntry, String> {
        let reference = VersionRef::new(context.object_id, context.version.to_string())
            .map_err(|err| err.to_string())?;
        let facts = CommitFacts {
            operation_id: &self.operation.operation_id,
            version: &reference,
            took_over: false,
        };
        let at_ms = parse_rfc3339_ms(context.created).unwrap_or_else(now_ms);
        let mut entries = self.stamped(ocfl_commit_entries(&facts), at_ms);
        let mut entry = entries.remove(0);
        entry.id = Some(self.journal_id);
        Ok(entry)
    }

    /// The `filesquid-prov` facts of this operation, naming the commit's journal entry.
    fn operation_facts(&self) -> OperationFacts {
        let user = version_user(&self.context.installation);
        OperationFacts {
            operation_id: self.operation.operation_id.as_str().to_string(),
            kind: self.operation.kind.as_str().to_string(),
            started: self.operation.started.clone(),
            installation: Some(self.context.installation.as_str().to_string()),
            host: Some(self.context.host.clone()),
            journal_entries: vec![self.journal_id.to_string()],
            moves: self.operation.moves.clone(),
            user: Some(OcflUser {
                name: user.name,
                address: user.address,
            }),
            message: Some(self.operation.message()),
        }
    }
}

/// `entries` as JSON Lines, one entry per line.
fn journal_lines(entries: &[JournalEntry]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for entry in entries {
        let line = serde_json::to_vec(entry).map_err(|err| err.to_string())?;
        out.extend_from_slice(&line);
        out.push(b'\n');
    }
    Ok(out)
}

impl VersionBoundFiles for CommitRecorder {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        let entry = self.commit_entry(context)?;
        let (record_path, record) =
            bound_record(context, &self.operation_facts()).map_err(|err| err.to_string())?;
        let lines = journal_lines(std::slice::from_ref(&entry))?;
        let mut last = self
            .last
            .lock()
            .map_err(|_| "the recorder lock is poisoned")?;
        *last = Some(entry);
        let version = context.version.to_string();
        Ok(BTreeMap::from([
            (record_path, record),
            (journal_path(&version), lines),
        ]))
    }
}
