//! OCFL validation findings in health and the Monitor (Task API-F2.7; API-S-022, F-061,
//! F-042).
//!
//! Every host that runs `filesquid-api` runs the `filesquid-ocfl` validator over every object
//! of every library it has open, through the storage backends it injects:
//!
//! - **On a schedule**, every 7 days by default ([`ValidationConfig`]).
//! - **After a commit that took over an expired or lost reservation**
//!   ([`OcflValidation::on_commit`]). This catches the stray `vN/` file of the
//!   writer-fencing gap (OCFL-S-018), which the validator reports as E023.
//! - **On request**, through `POST /api/v1/admin/ocfl/validation-runs`.
//!
//! A run registers as a process, runs at most once per library at a time, walks the library
//! in object-ID order from a persistent cursor, and stops at its cap or when cancelled. The
//! latest findings per object live in the host's [`FindingsStore`]. While any error finding
//! is open, health is at least `degraded` with reason `ocfl_validation_errors`, carrying the
//! codes and the number of affected objects. Warnings (W010 and others) are listed and do not
//! change health. `GET /api/v1/admin/ocfl/findings` lists the open findings and the runs, and
//! shows a finding's location only to a caller who may read the object.
//!
//! The journal and health carry opaque object IDs, versions and codes only, never a path,
//! title or content. This module does no storage I/O of its own: the host injects the
//! backends and the store ([API-S-006](../../resources/srs.md)).

/// `tracing` target of this module's stdout-only diagnostics.
const LOG_TARGET: &str = "filesquid::ocfl_validation";

/// Journal operation of each newly opened error finding.
pub const FINDING_OPERATION: &str = "ocfl.validation.finding";

/// Journal operation of each closed finding (fixed, or its object deleted).
pub const FINDING_CLOSED_OPERATION: &str = "ocfl.validation.finding_closed";

/// Journal operation of a run's outcome or refusal.
pub const RUN_OPERATION: &str = "ocfl.validation.run";

/// Journal operation of a schedule or cap change.
pub const CONFIGURE_OPERATION: &str = "ocfl.validation.configure";

/// Most findings one findings view answers.
pub const MAX_FINDINGS_VIEW: usize = filesquid_core::MAX_PAGE_SIZE as usize;

mod error;
mod handle;
mod memory;
mod routes;
mod run;
mod service;
mod store;
mod view;

pub use error::ValidationError;
pub use handle::OcflValidation;
pub use memory::MemoryFindingsStore;
pub(crate) use routes::routes;
pub use service::{ValidationConfig, ValidationService, DEFAULT_INTERVAL_MS, DEFAULT_OBJECT_CAP};
pub use store::{
    ErrorSummary, Finding, FindingChange, FindingsStore, RunOutcome, RunRecord, RunTrigger,
    Severity, StoreError,
};
pub use view::{FindingView, FindingsView, LibraryRuns, RunAccepted, RunRefusal, RunRequest, RunView};
