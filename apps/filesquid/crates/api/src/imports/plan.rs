//! The import plan the owner reviews (API-S-036 **Review**) and [`PlanBook`], the in-memory
//! book of a host's plans: what a repeated operation ID answers, when a plan expires, which
//! job a started plan runs as, and the paged planned files. The book keeps the rule once for
//! every host and test; a host that keeps its plans in a database (Task DE-1.8) wraps it or
//! mirrors it.

use super::{ImportAnswer, ImportRequest, ImportSource, ImportStartError};
use crate::journal::format_rfc3339_ms;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// How long an open plan lasts before the host discards it on its own: 1 hour.
pub const PLAN_TTL_MS: i64 = 3_600_000;

/// Most planned files one items page answers.
pub const MAX_PLAN_ITEMS: usize = 1_000;

/// Most plans a [`PlanBook`] remembers (started, discarded and cancelled ones included, for
/// the replay rule); the oldest is forgotten first.
pub const MAX_REMEMBERED_PLANS: usize = 256;

/// Where an import goes: a collection of the library and a folder inside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDestination {
    /// The destination collection.
    pub collection_id: String,
    /// The destination folder inside it, a collection logical path; the collection's root
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl ImportDestination {
    /// The destination `request` names; `collection_id` empty when the request named none
    /// (such a request never reaches a plan). Pure.
    pub fn of(request: &ImportRequest) -> Self {
        Self {
            collection_id: request.collection_id.clone().unwrap_or_default(),
            folder: request.folder.clone(),
        }
    }
}

/// What the host found when it planned an import, before any byte is staged (mockup
/// `18-import-review.svg`). Counts and sizes only; never a path or a name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFacts {
    /// The label of the volume the source is on, as the dialog showed it (never a path);
    /// absent when the host does not know it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_volume: Option<String>,
    /// Files found in the picked source.
    pub files_found: u64,
    /// Files not yet in this library: the ones the job copies.
    pub files_new: u64,
    /// Files already in this library (by digest), which the job skips.
    pub files_present: u64,
    /// Bytes the job copies.
    pub bytes_to_copy: u64,
    /// Files and folders whose names will be numbered because their path is taken
    /// (API-S-011).
    pub names_numbered: u64,
    /// Free bytes on the library's volume after the upload; `None` when the host cannot read
    /// the volume.
    pub free_bytes_after: Option<u64>,
    /// Most bytes the host stages at once.
    pub staging_limit_bytes: u64,
}

/// The plan as the import route answers it and the owner reviews it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPlanView {
    /// The plan.
    pub plan_id: String,
    /// What the owner picked.
    pub source: ImportSource,
    /// Where the files go.
    pub destination: ImportDestination,
    /// What the host found.
    #[serde(flatten)]
    pub facts: PlanFacts,
    /// When the host discards the plan unless it is started or discarded first (RFC 3339,
    /// UTC).
    pub expires: String,
}

/// One planned file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    /// The file ID the host minted for it; the item ID of the job once started.
    pub item_id: String,
    /// Its display name: the base name only, never a folder path.
    pub name: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// Whether the library already holds it (the job skips it).
    pub present: bool,
    /// Whether its name will be numbered because its path is taken.
    pub numbered: bool,
}

/// One page of a plan's files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItemPage {
    /// The files of this page, in the plan's order.
    pub items: Vec<PlanItem>,
    /// The cursor of the next page; `null` after the last.
    pub next_cursor: Option<String>,
}

/// The answer of `DELETE /api/v1/imports/{plan_id}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDiscarded {
    /// The plan that was discarded.
    pub plan_id: String,
    /// Always true: the plan is gone and nothing else changed.
    pub discarded: bool,
}

/// What became of a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PlanStatus {
    /// Awaiting the owner's confirmation until it expires.
    Open,
    /// Started as the job named.
    Started(String),
    /// Discarded by the owner.
    Discarded,
}

/// One remembered plan.
#[derive(Clone, Debug)]
struct Entry {
    operation_id: String,
    request: ImportRequest,
    view: ImportPlanView,
    items: Vec<PlanItem>,
    fingerprint: String,
    planned_ms: i64,
    status: PlanStatus,
}

impl Entry {
    /// Whether the plan is still open at `now_ms`.
    fn is_open(&self, now_ms: i64) -> bool {
        self.status == PlanStatus::Open && now_ms < self.planned_ms + PLAN_TTL_MS
    }
}

/// What a start found: the job a started plan already runs as, or the open plan to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartDecision {
    /// The plan was started before; the answer names its job again.
    Started(ImportAnswer),
    /// The plan is open and its source unchanged: the host submits its job, then records it
    /// with [`PlanBook::started`]. Carries the operation ID and the request of the plan.
    Open(String, ImportRequest),
}

/// The bounded, in-memory book of a host's import plans, the one meaning of the replay rule,
/// the expiry and the items paging (API-S-036 **Review**). Every method is answered from
/// memory; the caller passes the time.
#[derive(Debug, Default)]
pub struct PlanBook {
    plans: Mutex<HashMap<String, Entry>>,
    cancelled: Mutex<HashMap<String, ImportRequest>>,
}

impl PlanBook {
    /// An empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember the plan `plan_id` of `request`, planned at `now_ms` with `facts` and the
    /// files `items` (in the plan's order), whose source is identified by the opaque
    /// `fingerprint`; answer `planned`. The oldest plan is forgotten once
    /// [`MAX_REMEMBERED_PLANS`] are kept.
    pub fn open(
        &self,
        plan_id: &str,
        request: &ImportRequest,
        facts: PlanFacts,
        items: Vec<PlanItem>,
        fingerprint: &str,
        now_ms: i64,
    ) -> ImportAnswer {
        let view = ImportPlanView {
            plan_id: plan_id.to_string(),
            source: request.source,
            destination: ImportDestination::of(request),
            facts,
            expires: format_rfc3339_ms(now_ms + PLAN_TTL_MS),
        };
        let entry = Entry {
            operation_id: request.envelope.operation_id().as_str().to_string(),
            request: request.clone(),
            view: view.clone(),
            items,
            fingerprint: fingerprint.to_string(),
            planned_ms: now_ms,
            status: PlanStatus::Open,
        };
        let mut plans = self.lock();
        if plans.len() >= MAX_REMEMBERED_PLANS {
            forget_oldest(&mut plans);
        }
        plans.insert(plan_id.to_string(), entry);
        ImportAnswer::planned(request.envelope.operation_id(), view)
    }

    /// Remember that the owner closed the dialog of `request`, so a repeated operation ID
    /// answers `cancelled` without a dialog.
    pub fn cancelled(&self, request: &ImportRequest) -> ImportAnswer {
        let operation = request.envelope.operation_id();
        let mut cancelled = self.lock_cancelled();
        if cancelled.len() >= MAX_REMEMBERED_PLANS {
            let oldest = cancelled.keys().next().cloned();
            if let Some(key) = oldest {
                cancelled.remove(&key);
            }
        }
        cancelled.insert(operation.as_str().to_string(), request.clone());
        ImportAnswer::cancelled(operation)
    }

    /// What `request`'s operation ID already produced, at `now_ms`: its open plan, its
    /// started job, `cancelled`, or nothing.
    ///
    /// # Errors
    /// [`ImportStartError::OperationReused`] for another import under the same operation
    /// ID; [`ImportStartError::PlanNotFound`] for a plan that expired or was discarded.
    pub fn replay(
        &self,
        request: &ImportRequest,
        now_ms: i64,
    ) -> Result<Option<ImportAnswer>, ImportStartError> {
        let operation = request.envelope.operation_id();
        if let Some(earlier) = self.lock_cancelled().get(operation.as_str()) {
            return match earlier.same_import(request) {
                true => Ok(Some(ImportAnswer::cancelled(operation))),
                false => Err(ImportStartError::OperationReused),
            };
        }
        let plans = self.lock();
        let Some(entry) = plans
            .values()
            .find(|entry| entry.operation_id == operation.as_str())
        else {
            return Ok(None);
        };
        if !entry.request.same_import(request) {
            return Err(ImportStartError::OperationReused);
        }
        match &entry.status {
            PlanStatus::Started(job) => Ok(Some(ImportAnswer::started(operation, job.clone()))),
            PlanStatus::Open if entry.is_open(now_ms) => {
                Ok(Some(ImportAnswer::planned(operation, entry.view.clone())))
            }
            PlanStatus::Open | PlanStatus::Discarded => Err(ImportStartError::PlanNotFound),
        }
    }

    /// The open plan `plan_id` at `now_ms`, with the fingerprint of its source.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] when no open plan has the ID.
    pub fn plan(
        &self,
        plan_id: &str,
        now_ms: i64,
    ) -> Result<(ImportPlanView, String), ImportStartError> {
        let plans = self.lock();
        let entry = plans.get(plan_id).ok_or(ImportStartError::PlanNotFound)?;
        match entry.is_open(now_ms) {
            true => Ok((entry.view.clone(), entry.fingerprint.clone())),
            false => Err(ImportStartError::PlanNotFound),
        }
    }

    /// At most `limit` (capped at [`MAX_PLAN_ITEMS`]) files of the open plan `plan_id` after
    /// `cursor`, at `now_ms`.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] when no open plan has the ID.
    pub fn items(
        &self,
        plan_id: &str,
        cursor: Option<&str>,
        limit: usize,
        now_ms: i64,
    ) -> Result<PlanItemPage, ImportStartError> {
        let plans = self.lock();
        let entry = plans.get(plan_id).ok_or(ImportStartError::PlanNotFound)?;
        if !entry.is_open(now_ms) {
            return Err(ImportStartError::PlanNotFound);
        }
        Ok(page_of(&entry.items, cursor, limit))
    }

    /// What starting the plan `plan_id` at `now_ms` means, given the `fingerprint` of its
    /// source now.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] for an unknown, expired or discarded plan;
    /// [`ImportStartError::SourceChanged`] when the source's fingerprint differs from the
    /// plan's (the plan stays open).
    pub fn starting(
        &self,
        plan_id: &str,
        fingerprint: &str,
        now_ms: i64,
    ) -> Result<StartDecision, ImportStartError> {
        let plans = self.lock();
        let entry = plans.get(plan_id).ok_or(ImportStartError::PlanNotFound)?;
        let operation = filesquid_core::OperationId::new(entry.operation_id.clone())
            .map_err(|_| ImportStartError::Unavailable)?;
        match &entry.status {
            PlanStatus::Started(job) => Ok(StartDecision::Started(ImportAnswer::started(
                &operation,
                job.clone(),
            ))),
            PlanStatus::Open if !entry.is_open(now_ms) => Err(ImportStartError::PlanNotFound),
            PlanStatus::Open if entry.fingerprint != fingerprint => {
                Err(ImportStartError::SourceChanged)
            }
            PlanStatus::Open => Ok(StartDecision::Open(
                entry.operation_id.clone(),
                entry.request.clone(),
            )),
            PlanStatus::Discarded => Err(ImportStartError::PlanNotFound),
        }
    }

    /// Record that the plan `plan_id` was started as `job_id`; answer `started`.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] when the plan is unknown.
    pub fn started(&self, plan_id: &str, job_id: &str) -> Result<ImportAnswer, ImportStartError> {
        let mut plans = self.lock();
        let entry = plans
            .get_mut(plan_id)
            .ok_or(ImportStartError::PlanNotFound)?;
        entry.status = PlanStatus::Started(job_id.to_string());
        let operation = filesquid_core::OperationId::new(entry.operation_id.clone())
            .map_err(|_| ImportStartError::Unavailable)?;
        Ok(ImportAnswer::started(&operation, job_id.to_string()))
    }

    /// Discard the open plan `plan_id` at `now_ms`; nothing else changes.
    ///
    /// # Errors
    /// [`ImportStartError::PlanNotFound`] when no open plan has the ID.
    pub fn discard(&self, plan_id: &str, now_ms: i64) -> Result<(), ImportStartError> {
        let mut plans = self.lock();
        let entry = plans
            .get_mut(plan_id)
            .ok_or(ImportStartError::PlanNotFound)?;
        if !entry.is_open(now_ms) {
            return Err(ImportStartError::PlanNotFound);
        }
        entry.status = PlanStatus::Discarded;
        Ok(())
    }

    /// Whether the plan `plan_id` is open at `now_ms`.
    pub fn is_open(&self, plan_id: &str, now_ms: i64) -> bool {
        self.lock()
            .get(plan_id)
            .is_some_and(|entry| entry.is_open(now_ms))
    }

    /// The plans, locked; a poisoned lock is recovered (the book holds plain data).
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The cancelled operations, locked.
    fn lock_cancelled(&self) -> MutexGuard<'_, HashMap<String, ImportRequest>> {
        self.cancelled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Forget the plan planned first. Pure over the map.
fn forget_oldest(plans: &mut HashMap<String, Entry>) {
    let oldest = plans
        .iter()
        .min_by_key(|(id, entry)| (entry.planned_ms, (*id).clone()))
        .map(|(id, _)| id.clone());
    if let Some(id) = oldest {
        plans.remove(&id);
    }
}

/// At most `limit` (1 to [`MAX_PLAN_ITEMS`]) of `items` after `cursor`, the position of the
/// first item of the page; a cursor that is not a position starts at the first item. Pure.
pub(crate) fn page_of(items: &[PlanItem], cursor: Option<&str>, limit: usize) -> PlanItemPage {
    let start = cursor
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0)
        .min(items.len());
    let limit = limit.clamp(1, MAX_PLAN_ITEMS);
    let end = start.saturating_add(limit).min(items.len());
    let next_cursor = (end < items.len()).then(|| end.to_string());
    PlanItemPage {
        items: items[start..end].to_vec(),
        next_cursor,
    }
}
