//! What the findings and validation-run routes answer and accept (API-S-022).

use super::store::{Finding, RunOutcome, RunRecord, RunTrigger, Severity};
use crate::journal::format_rfc3339_ms;
use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};

/// One open finding as the Monitor shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingView {
    /// The library.
    pub library_id: LibraryId,
    /// The object identifier.
    pub object_id: String,
    /// The object's head version when it could be read.
    pub version: Option<String>,
    /// The OCFL code.
    pub code: String,
    /// Error or warning.
    pub severity: Severity,
    /// Where the fault is; `null` for a caller who may not read the object.
    pub location: Option<String>,
}

impl FindingView {
    /// The view of `finding`, with its location only when `show_location`.
    pub fn of(finding: Finding, show_location: bool) -> Self {
        Self {
            library_id: finding.library_id,
            object_id: finding.object_id,
            version: finding.version,
            code: finding.code,
            severity: finding.severity,
            location: show_location.then_some(finding.location),
        }
    }
}

/// A finished run as the Monitor shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunView {
    /// What started it.
    pub trigger: RunTrigger,
    /// Start time, RFC 3339 UTC.
    pub started: String,
    /// End time, RFC 3339 UTC.
    pub ended: String,
    /// How it ended.
    pub outcome: RunOutcome,
    /// Objects checked.
    pub checked: u64,
    /// Objects not checked.
    pub not_checked: u64,
    /// The failure code of a failed run.
    pub error_code: Option<String>,
}

impl From<RunRecord> for RunView {
    fn from(run: RunRecord) -> Self {
        Self {
            trigger: run.trigger,
            started: format_rfc3339_ms(run.started_ms),
            ended: format_rfc3339_ms(run.ended_ms),
            outcome: run.outcome,
            checked: run.checked,
            not_checked: run.not_checked,
            error_code: run.error_code,
        }
    }
}

/// One open library's runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryRuns {
    /// The library.
    pub library_id: LibraryId,
    /// Whether a run is going now.
    pub running: bool,
    /// The last finished run.
    pub last_run: Option<RunView>,
    /// The next scheduled run time, RFC 3339 UTC.
    pub next_run: Option<String>,
}

/// The answer of `GET /api/v1/admin/ocfl/findings`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingsView {
    /// Open findings, ordered by library, object, code and location.
    pub findings: Vec<FindingView>,
    /// True when more findings are open than were answered.
    pub truncated: bool,
    /// Every open library, by identifier.
    pub libraries: Vec<LibraryRuns>,
}

/// The body of `POST /api/v1/admin/ocfl/validation-runs`; an empty body runs every open
/// library from its cursor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    /// The one library to run; every open library when absent.
    #[serde(default)]
    pub library_id: Option<LibraryId>,
    /// Start at the beginning of the object order instead of the stored cursor.
    #[serde(default)]
    pub from_start: bool,
}

/// A library a run request could not start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRefusal {
    /// The library.
    pub library_id: LibraryId,
    /// Why (`already_running`).
    pub error_code: String,
}

/// The answer of `POST /api/v1/admin/ocfl/validation-runs`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAccepted {
    /// Libraries whose run started.
    pub started: Vec<LibraryId>,
    /// Libraries whose run did not start.
    pub refused: Vec<RunRefusal>,
}
