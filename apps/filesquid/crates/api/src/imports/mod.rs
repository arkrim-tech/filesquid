//! The import routes (Tasks DE-1.2 and API-F2.16; F-002, API-S-036): the owner picks files
//! or a folder in the host's own native dialog, reviews the plan the host made of them, and
//! starts one durable job that imports them into a library. No path crosses the API
//! (API-S-005): the host reads the source and keeps where it is; the answers name opaque IDs,
//! counts and base names only.
//!
//! **Review** (owner answer 5, 2026-10-22, "Keep the review"): `POST /api/v1/imports`
//! (`files.write`) answers `{outcome: pending, import: planned, plan_id, plan}` before any byte
//! is staged; `GET /api/v1/imports/{plan_id}/items` pages the planned files;
//! `POST /api/v1/imports/{plan_id}/start` starts the job once (a repeated start answers the
//! same job); `DELETE /api/v1/imports/{plan_id}` discards the plan, as the host does on its
//! own after [`PLAN_TTL_MS`]. A source that changed since the plan is refused as
//! `source_changed`.
//!
//! An import commits, so its body carries a [`CommandEnvelope`] (API-S-002): its operation ID
//! is the idempotency key, and the same operation ID answers what it already produced (its
//! open plan, its started job, or `cancelled`) instead of showing a second dialog. An import
//! only adds new files and changes no existing one, so the envelope's scope MUST be
//! `library:<library_id>` and its expected revision is not compared.
//!
//! Every import names its destination (owner decision 2026-10-22, "Must choose", ADR 041
//! Decision 4): `collection_id`, a collection of the library, and optionally `folder`, a
//! collection logical folder path inside it. A request without `collection_id` is refused
//! before any dialog ([`DESTINATION_REQUIRED`]), so nothing is imported to a library's top
//! level and no default collection is chosen; an unknown collection, or one of another
//! library, is `not_found` ([`COLLECTION_NOT_FOUND`]); a folder outside the collection-path
//! rule or inside a reserved folder is `invalid_request` (`invalid_path`). The collection is
//! looked up through the navigation service's library catalog, the one meaning of API-S-019.
//!
//! What the dialog shows, how the plan is made and how the job runs is the host's, behind
//! [`ImportStarter`], installed with [`crate::ApiState::install_import_starter`]; a host
//! without one (`filesquid-server`) answers `503 unavailable`. [`PlanBook`] is the in-memory
//! book of plans a host or a test keeps its plans in: expiry, the replay rule and the paging
//! of the planned files, once. A host whose starter has no review (the desktop until Task
//! DE-1.8) starts the job at once and answers `started`, as before this task.
//!
//! Each plan request is journaled once as [`IMPORT_PLAN_OPERATION`] (`succeeded` with the
//! plan ID, `cancelled`, or `failed` with its error code), each start as
//! [`IMPORT_START_OPERATION`] (`succeeded` with the job ID, or `failed`), each discard as
//! [`IMPORT_DISCARD_OPERATION`]; never a path or a file name.

mod plan;
pub(crate) mod routes;

pub use plan::{
    ImportDestination, ImportPlanView, PlanBook, PlanDiscarded, PlanFacts, PlanItem, PlanItemPage,
    StartDecision, MAX_PLAN_ITEMS, MAX_REMEMBERED_PLANS, PLAN_TTL_MS,
};

use filesquid_core::{CommandEnvelope, CommandOutcome, CommandOutcomeKind, OperationId};
use serde::{Deserialize, Serialize};

/// Journal operation of a plan request (`POST /api/v1/imports`).
pub const IMPORT_PLAN_OPERATION: &str = "import.plan";

/// Journal operation of a start (`POST /api/v1/imports/{plan_id}/start`, or the one-step
/// start of a host without the review).
pub const IMPORT_START_OPERATION: &str = "import.start";

/// Journal operation of a discarded plan (`DELETE /api/v1/imports/{plan_id}`).
pub const IMPORT_DISCARD_OPERATION: &str = "import.discard";

/// Journal operation of a refused read of a plan's items.
pub const IMPORT_PLAN_ITEMS_OPERATION: &str = "import.plan_items";

/// Journal error code of a request that names no destination collection.
pub const DESTINATION_REQUIRED: &str = "destination_required";

/// Journal error code of a destination collection that is unknown or of another library.
pub const COLLECTION_NOT_FOUND: &str = "collection_not_found";

/// Journal and API error code of an unknown, expired or discarded plan.
pub const PLAN_NOT_FOUND: &str = "plan_not_found";

/// Journal and API error code of a source that changed since its plan.
pub const SOURCE_CHANGED: &str = "source_changed";

/// What the owner picks in the host's dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    /// One or more files.
    Files,
    /// One folder, with every file below it.
    Folder,
}

/// Body of `POST /api/v1/imports`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    /// The command envelope: the operation ID is the idempotency key; the scope is
    /// `library:<library_id>`.
    pub envelope: CommandEnvelope,
    /// The open library to import into.
    pub library_id: String,
    /// The destination collection, a collection of the library. Required: a request
    /// without it is refused ([`DESTINATION_REQUIRED`]); it is optional here only so that
    /// refusal can be journaled with its own code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    /// The destination folder inside the collection, a collection logical path; the
    /// collection's root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Files or a folder.
    pub source: ImportSource,
}

impl ImportRequest {
    /// Whether `other` asks for the same import: the same library, destination and kind of
    /// source, so a repeated operation ID may be answered with what the first request
    /// produced. Pure.
    pub fn same_import(&self, other: &ImportRequest) -> bool {
        self.library_id == other.library_id
            && self.collection_id == other.collection_id
            && self.folder == other.folder
            && self.source == other.source
    }
}

/// What became of an import request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// The host planned the import and waits for the owner's confirmation; `plan_id` and
    /// `plan` describe it (API-S-036 **Review**).
    Planned,
    /// The import job runs (or ran, for a repeated operation ID); `job_id` names it.
    Started,
    /// The owner closed the dialog without picking anything; no plan and no job were made.
    Cancelled,
}

/// The answer of an import request or start: the command outcome API-S-002 requires, and
/// what became of the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportAnswer {
    /// `pending` for a planned or started import (the work is durable and not yet complete; a
    /// repeated operation ID answers it again), `committed` for a cancelled one (nothing
    /// changed, as a library command that changes nothing answers).
    pub outcome: CommandOutcome,
    /// Planned, started or cancelled.
    pub import: ImportOutcome,
    /// The plan awaiting confirmation; present when planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    /// What the plan shows for the owner's review; present when planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ImportPlanView>,
    /// The import job; present when started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

impl ImportAnswer {
    /// The answer of `operation_id` planned as `plan`. Pure.
    pub fn planned(operation_id: &OperationId, plan: ImportPlanView) -> Self {
        Self {
            outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Pending),
            import: ImportOutcome::Planned,
            plan_id: Some(plan.plan_id.clone()),
            plan: Some(plan),
            job_id: None,
        }
    }

    /// The answer naming the import job `job_id` of `operation_id`. Pure.
    pub fn started(operation_id: &OperationId, job_id: String) -> Self {
        Self {
            outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Pending),
            import: ImportOutcome::Started,
            plan_id: None,
            plan: None,
            job_id: Some(job_id),
        }
    }

    /// The answer of `operation_id` whose dialog the owner closed. Pure.
    pub fn cancelled(operation_id: &OperationId) -> Self {
        Self {
            outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Committed),
            import: ImportOutcome::Cancelled,
            plan_id: None,
            plan: None,
            job_id: None,
        }
    }
}

/// Why a host did not plan, start or discard an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportStartError {
    /// The library is not open on this host.
    LibraryNotFound,
    /// A dialog is already open.
    Busy,
    /// The host cannot show a dialog, plan or run a job now.
    Unavailable,
    /// The operation ID already planned or started an import of another library, destination
    /// or source; it is never answered with that plan or job.
    OperationReused,
    /// No open plan has this ID: it is unknown, expired, discarded or already started.
    PlanNotFound,
    /// The picked source changed since the plan was made.
    SourceChanged,
}

impl ImportStartError {
    /// The journal error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            ImportStartError::LibraryNotFound => "library_not_found",
            ImportStartError::Busy => "dialog_open",
            ImportStartError::Unavailable => "unavailable",
            ImportStartError::OperationReused => "operation_reused",
            ImportStartError::PlanNotFound => PLAN_NOT_FOUND,
            ImportStartError::SourceChanged => SOURCE_CHANGED,
        }
    }
}

/// The host's import dialog, plans and job runner. Every method has a default so that a
/// host adopts the review step by step: a host that implements only [`Self::start`] (the
/// desktop until Task DE-1.8) starts the job as soon as the owner picked the source.
pub trait ImportStarter: Send + Sync {
    /// The answer of what `request`'s operation ID already produced, if anything, without a
    /// dialog: its open plan (`planned`, with the same plan ID), its started job
    /// (`started`), or `cancelled` when the owner closed the dialog. The route answers a
    /// repeated operation with it before checking the destination again, so a retry is
    /// never refused because its collection changed since. The default knows of nothing.
    ///
    /// # Errors
    /// [`ImportStartError::OperationReused`] when the operation ID produced an import of
    /// another library, collection, folder or kind of source;
    /// [`ImportStartError::PlanNotFound`] when its plan expired or was discarded;
    /// [`ImportStartError::Unavailable`] when the host cannot tell.
    fn replay(&self, request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        let _ = request;
        Ok(None)
    }

    /// Let the owner pick the source of `request` in the host's dialog, as `actor`, and plan
    /// the import without staging a byte: `planned` with the plan, or `cancelled` when the
    /// owner closed the dialog. Blocks until the dialog closes. The default starts the job
    /// at once through [`Self::start`]: the host has no review yet.
    ///
    /// # Errors
    /// An [`ImportStartError`]; no plan is made.
    fn plan(&self, actor: &str, request: &ImportRequest) -> Result<ImportAnswer, ImportStartError> {
        self.start(actor, request)
    }

    /// Let the owner pick the source of `request` and start its import job at once, as
    /// `actor`, without a review (the one step of a host before Task DE-1.8). The default
    /// has no dialog.
    ///
    /// # Errors
    /// An [`ImportStartError`]; no job is made.
    fn start(
        &self,
        actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        let _ = (actor, request);
        Err(ImportStartError::Unavailable)
    }

    /// Submit one durable import job for the open plan `plan_id` and start it in the
    /// background, as `actor`: `started` with the job; a repeated start of the same plan
    /// answers the same job. The default knows no plan.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`], [`ImportStartError::SourceChanged`] or
    /// [`ImportStartError::Unavailable`]; a refused start changes nothing.
    fn start_plan(&self, actor: &str, plan_id: &str) -> Result<ImportAnswer, ImportStartError> {
        let _ = (actor, plan_id);
        Err(ImportStartError::PlanNotFound)
    }

    /// Discard the open plan `plan_id`, as `actor`, changing nothing else. The default knows
    /// no plan.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] or [`ImportStartError::Unavailable`].
    fn discard(&self, actor: &str, plan_id: &str) -> Result<(), ImportStartError> {
        let _ = (actor, plan_id);
        Err(ImportStartError::PlanNotFound)
    }

    /// At most `limit` planned files of the open plan `plan_id` after `cursor`, base names
    /// only. The default knows no plan.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] or [`ImportStartError::Unavailable`].
    fn plan_items(
        &self,
        plan_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<PlanItemPage, ImportStartError> {
        let _ = (plan_id, cursor, limit);
        Err(ImportStartError::PlanNotFound)
    }
}
