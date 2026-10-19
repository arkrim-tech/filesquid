//! The findings store port (API-S-022): where a host keeps the latest validation findings per
//! object, the per-library cursor and the last run, in its operational store (SQLite on the
//! desktop through `filesquid-native`, the interim store on the server until SV-F1.2).
//!
//! The store holds the finding's location (a storage key and, for inventory values, a JSON
//! pointer) because the Monitor shows it to a caller who may read the object. The location
//! never reaches the journal, a health reason or stdout.

use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};

/// Whether a finding is an OCFL error (`E###`) or warning (`W###`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// An OCFL error: the object breaks a MUST of the specification.
    Error,
    /// An OCFL warning: the object breaks a SHOULD; health is unchanged.
    Warning,
}

impl Severity {
    /// Lowercase wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One validation finding of one object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// The library the object belongs to.
    pub library_id: LibraryId,
    /// The object identifier (opaque), or `root:<digest>` for an object whose inventory
    /// cannot be read.
    pub object_id: String,
    /// The object's committed head when it could be read.
    pub version: Option<String>,
    /// The OCFL validation code, such as `E023` or `W010`.
    pub code: String,
    /// Error or warning.
    pub severity: Severity,
    /// Where the fault is: a storage key from the storage root, and for inventory values a
    /// JSON pointer. Shown only to a caller who may read the object.
    pub location: String,
}

impl Finding {
    /// `findings` as a store keeps them: each under `library_id` and `object_id`, whatever
    /// it names itself, and once per code and location (a later one replaces an earlier
    /// one). Every store normalizes with this before it writes and before it answers the
    /// change, so both stores answer the same.
    pub fn normalize(
        library_id: &LibraryId,
        object_id: &str,
        findings: Vec<Finding>,
    ) -> Vec<Finding> {
        let mut kept: Vec<Finding> = Vec::with_capacity(findings.len());
        for found in findings {
            let found = Finding {
                library_id: library_id.clone(),
                object_id: object_id.to_string(),
                ..found
            };
            match kept.iter_mut().find(|k| k.same_as(&found)) {
                Some(existing) => *existing = found,
                None => kept.push(found),
            }
        }
        kept
    }

    /// The identity of a finding within its object: the same code at the same location is
    /// the same finding, whatever the version.
    pub fn same_as(&self, other: &Finding) -> bool {
        self.code == other.code && self.location == other.location
    }
}

/// What replacing an object's findings changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindingChange {
    /// Findings that were not open before.
    pub opened: Vec<Finding>,
    /// Findings that were open and are no longer reported.
    pub closed: Vec<Finding>,
}

impl FindingChange {
    /// What `next` opens and closes against `previous`: a finding is the same when
    /// [`Finding::same_as`] says so. Every store answers replacements with this.
    pub fn between(previous: &[Finding], next: &[Finding]) -> Self {
        Self {
            opened: next
                .iter()
                .filter(|f| !previous.iter().any(|p| p.same_as(f)))
                .cloned()
                .collect(),
            closed: previous
                .iter()
                .filter(|p| !next.iter().any(|f| f.same_as(p)))
                .cloned()
                .collect(),
        }
    }

    /// True when nothing opened or closed.
    pub fn is_empty(&self) -> bool {
        self.opened.is_empty() && self.closed.is_empty()
    }
}

/// The open error findings in short: what the health rule needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ErrorSummary {
    /// Distinct error codes, sorted.
    pub codes: Vec<String>,
    /// Objects with at least one open error finding.
    pub objects: usize,
}

/// What started a validation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    /// The schedule.
    Scheduled,
    /// A request through the API.
    Requested,
}

impl RunTrigger {
    /// Wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            RunTrigger::Scheduled => "scheduled",
            RunTrigger::Requested => "requested",
        }
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// Every object of the library was checked.
    Completed,
    /// The run stopped at its object cap.
    Capped,
    /// The run was cancelled.
    Cancelled,
    /// The run stopped on a storage or store failure.
    Failed,
}

impl RunOutcome {
    /// Wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            RunOutcome::Completed => "completed",
            RunOutcome::Capped => "capped",
            RunOutcome::Cancelled => "cancelled",
            RunOutcome::Failed => "failed",
        }
    }
}

/// One finished validation run of one library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    /// The library.
    pub library_id: LibraryId,
    /// What started it.
    pub trigger: RunTrigger,
    /// Start time, Unix milliseconds (UTC).
    pub started_ms: i64,
    /// End time, Unix milliseconds (UTC).
    pub ended_ms: i64,
    /// How it ended.
    pub outcome: RunOutcome,
    /// Objects checked.
    pub checked: u64,
    /// Objects of the library the run did not check.
    pub not_checked: u64,
    /// The failure code of a failed run.
    pub error_code: Option<String>,
}

/// A failure of the injected store. Details go to stdout only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError {
    /// What failed, for stdout.
    pub detail: String,
    /// Whether a retry may succeed (the store was busy or unreachable). A failure a retry
    /// will not fix (a corrupt row, a poisoned lock) is not transient.
    pub transient: bool,
}

impl StoreError {
    /// A failure a retry may clear.
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            transient: true,
        }
    }

    /// A failure a retry will not clear.
    pub fn failed(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            transient: false,
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "findings store failed: {}", self.detail)
    }
}

/// The findings store a host injects.
pub trait FindingsStore: Send + Sync {
    /// Replace the findings of `object_id` in `library_id` with `findings` (all of one
    /// validation of that object), in one transaction, and answer what opened and closed
    /// ([`FindingChange::between`]). The findings are first normalized with
    /// [`Finding::normalize`]: each under `library_id` and `object_id`, once per code and
    /// location. An empty
    /// `findings` closes every open finding of the object; replacing findings with the same
    /// ones answers an empty change.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store failed; nothing is changed then.
    fn replace_findings(
        &self,
        library_id: &LibraryId,
        object_id: &str,
        findings: Vec<Finding>,
    ) -> Result<FindingChange, StoreError>;

    /// The objects of `library_id` with open findings, sorted.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn objects_with_findings(&self, library_id: &LibraryId) -> Result<Vec<String>, StoreError>;

    /// At most `limit` open findings of every library, ordered by library, object, code and
    /// location.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn open_findings(&self, limit: usize) -> Result<Vec<Finding>, StoreError>;

    /// The open error findings in short, for the health rule.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn error_summary(&self) -> Result<ErrorSummary, StoreError>;

    /// The cursor of `library_id`: the last object ID a run checked, or `None` at the start
    /// of a pass.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn cursor(&self, library_id: &LibraryId) -> Result<Option<String>, StoreError>;

    /// Move the cursor of `library_id` (`None` starts the next pass at the beginning).
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the cursor did not move.
    fn set_cursor(&self, library_id: &LibraryId, cursor: Option<&str>) -> Result<(), StoreError>;

    /// Keep `run` as the last run of its library.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn record_run(&self, run: &RunRecord) -> Result<(), StoreError>;

    /// The last run of `library_id`.
    ///
    /// # Errors
    ///
    /// A [`StoreError`].
    fn last_run(&self, library_id: &LibraryId) -> Result<Option<RunRecord>, StoreError>;
}
