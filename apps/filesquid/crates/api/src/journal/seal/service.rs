//! The sealing service (API-S-032; ADR 033 Decision 7). The host calls [`SealService::seal_all`]
//! every hour and at app close, and [`SealService::close_library`] at library close.
//!
//! Each run, for every scope (each open library, then the installation):
//!
//! 1. reads how far the scope's journal object is sealed (from its segments);
//! 2. collects the entries after that, in journal ID order: a library's entries name its
//!    library ID among their targets; the installation takes every other entry (an entry of
//!    a library that is not open stays with the installation); at most
//!    [`SealConfig::max_entries_per_scope`] entries and [`MAX_SEGMENT_BYTES`] per segment;
//! 3. commits them as one segment, a new version of the object.
//!
//! A run in which no scope has anything but seal bookkeeping to seal registers nothing,
//! commits nothing and journals nothing. Otherwise the run is a registered `journal.seal`
//! process, and each scope's result is journaled as [`SEAL_OPERATION`] (`succeeded` with its
//! counts, or `failed` with its error code), with the segment's `ocfl.commit`. A failed scope
//! keeps its entries for the next run; its object, and so its cursor, is unchanged.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use filesquid_core::{OperationId, VersionRef};

use super::object::{JournalObject, SealError, Segment, MAX_SEGMENT_BYTES};
use super::{
    is_seal_bookkeeping, library_scope, INSTALLATION_SCOPE, SEAL_OPERATION, SEAL_PROCESS_KIND,
};
use crate::journal::commit::{ocfl_commit_entries, CommitFacts};
use crate::journal::{JournalEntry, JournalQuery, JournalStore, Level, Outcome, MAX_PAGE};
use crate::library::mint_uuid_v4;
use crate::monitor::Monitor;
use crate::registry::{ProcessHandle, ProcessSpec};

/// Most journal pages one scope reads per run (AGENTS.md §22): an installation scope skips
/// library entries, so it may read more pages than it seals.
const MAX_SCAN_PAGES: usize = 1_000;

/// Longest silence before a seal run counts as stalled.
const SEAL_HEARTBEAT_MS: i64 = 300_000;

/// Seal settings.
#[derive(Clone, Copy, Debug)]
pub struct SealConfig {
    /// Most entries one scope seals per run; the rest waits for the next run.
    pub max_entries_per_scope: usize,
}

impl Default for SealConfig {
    /// 50,000 entries per scope per run.
    fn default() -> Self {
        Self {
            max_entries_per_scope: 50_000,
        }
    }
}

/// What one scope's seal did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SealOutcome {
    /// A segment was committed.
    Sealed {
        /// Entries sealed.
        entries: usize,
        /// The new version of the journal object.
        version: String,
    },
    /// Nothing but seal bookkeeping was unsealed: no commit.
    Nothing,
    /// The seal failed; its entries wait for the next run.
    Failed {
        /// The journal error code.
        error_code: String,
    },
}

/// One scope of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeReport {
    /// `installation` or `library:<id>`.
    pub scope: String,
    /// What happened.
    pub outcome: SealOutcome,
}

/// What one run did, scope by scope.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SealReport {
    /// The scopes, libraries first.
    pub scopes: Vec<ScopeReport>,
}

impl SealReport {
    /// True when no scope failed.
    pub fn all_sealed(&self) -> bool {
        !self
            .scopes
            .iter()
            .any(|s| matches!(s.outcome, SealOutcome::Failed { .. }))
    }

    /// True when no scope had anything to seal.
    pub fn nothing_to_seal(&self) -> bool {
        self.scopes
            .iter()
            .all(|s| s.outcome == SealOutcome::Nothing)
    }

    /// The outcome of `scope`, if the run covered it.
    pub fn outcome(&self, scope: &str) -> Option<&SealOutcome> {
        self.scopes
            .iter()
            .find(|s| s.scope == scope)
            .map(|s| &s.outcome)
    }
}

/// The input of the `journal_unsealed` health rule.
pub trait SealStatus: Send + Sync {
    /// When the oldest entry that a seal left unsealed was recorded (Unix ms, UTC), or
    /// `None` when every entry the last runs read was sealed.
    fn oldest_unsealed_ms(&self) -> Option<i64>;
}

/// One scope to seal in a run.
struct Target {
    scope: String,
    library_id: Option<String>,
    object: Arc<dyn JournalObject>,
}

/// One scope's plan: the segment to commit, or nothing, or why it could not be planned.
struct Plan {
    target: Target,
    planned: Result<Option<Collected>, SealError>,
}

/// The entries one scope collected.
struct Collected {
    segment: Segment,
    /// When the first entry left for a later run was recorded, if the cap stopped it.
    left_ms: Option<i64>,
}

/// The sealing service of one host.
pub struct SealService {
    monitor: Monitor,
    installation: Arc<dyn JournalObject>,
    libraries: Mutex<BTreeMap<String, Arc<dyn JournalObject>>>,
    config: SealConfig,
    running: tokio::sync::Mutex<()>,
    pending: Mutex<BTreeMap<String, i64>>,
}

impl SealService {
    /// A service sealing host events into `installation`, journaling through `monitor`.
    pub fn new(monitor: Monitor, installation: Arc<dyn JournalObject>, config: SealConfig) -> Self {
        Self {
            monitor,
            installation,
            libraries: Mutex::new(BTreeMap::new()),
            config,
            running: tokio::sync::Mutex::new(()),
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    /// Seal `library_id`'s events into `object` from now on. False, keeping the first, when
    /// the library is already open.
    pub fn open_library(&self, library_id: &str, object: Arc<dyn JournalObject>) -> bool {
        let mut libraries = lock(&self.libraries);
        if libraries.contains_key(library_id) {
            return false;
        }
        libraries.insert(library_id.to_string(), object);
        true
    }

    /// Whether `library_id` is open for sealing.
    pub fn is_open(&self, library_id: &str) -> bool {
        lock(&self.libraries).contains_key(library_id)
    }

    /// Seal every open library and the installation (hourly and at app close). Journal and
    /// storage I/O.
    pub async fn seal_all(&self) -> SealReport {
        let _run = self.running.lock().await;
        let mut targets: Vec<Target> = lock(&self.libraries)
            .iter()
            .map(|(id, object)| library_target(id, object.clone()))
            .collect();
        targets.push(Target {
            scope: INSTALLATION_SCOPE.to_string(),
            library_id: None,
            object: self.installation.clone(),
        });
        self.run(targets).await
    }

    /// Seal `library_id` (at library close), then stop sealing into it. An unknown library
    /// seals nothing. Journal and storage I/O.
    pub async fn close_library(&self, library_id: &str) -> SealReport {
        let _run = self.running.lock().await;
        let Some(object) = lock(&self.libraries).get(library_id).cloned() else {
            return SealReport::default();
        };
        let report = self.run(vec![library_target(library_id, object)]).await;
        lock(&self.libraries).remove(library_id);
        report
    }

    /// Plan every target, then commit the planned segments in one registered process.
    async fn run(&self, targets: Vec<Target>) -> SealReport {
        let open: Vec<String> = lock(&self.libraries).keys().cloned().collect();
        let mut plans = Vec::with_capacity(targets.len());
        for target in targets {
            let planned = self.plan(&target, &open).await;
            plans.push(Plan { target, planned });
        }
        let idle = plans.iter().all(|p| matches!(p.planned, Ok(None)));
        if idle {
            plans
                .iter()
                .for_each(|p| self.set_pending(&p.target.scope, None));
            return report_of(
                plans
                    .iter()
                    .map(|p| (p.target.scope.clone(), SealOutcome::Nothing)),
            );
        }
        let process = self
            .monitor
            .registry()
            .register(ProcessSpec::new(SEAL_PROCESS_KIND, SEAL_HEARTBEAT_MS));
        let total = plans.len() as u64;
        let mut outcomes = Vec::with_capacity(plans.len());
        for (done, plan) in plans.into_iter().enumerate() {
            let outcome = self.finish(plan, &process).await;
            process.progress(done as u64 + 1, total);
            outcomes.push(outcome);
        }
        let report = report_of(outcomes.into_iter());
        match report.all_sealed() {
            true => process.complete(),
            false => process.fail("seal_failed"),
        }
        report
    }

    /// Commit one scope's plan, journal the result and record what it left unsealed.
    async fn finish(&self, plan: Plan, process: &ProcessHandle) -> (String, SealOutcome) {
        let scope = plan.target.scope.clone();
        let collected = match plan.planned {
            Ok(Some(collected)) => collected,
            Ok(None) => {
                self.set_pending(&scope, None);
                return (scope, SealOutcome::Nothing);
            }
            Err(err) => {
                self.keep_pending(&scope);
                self.journal_failure(&scope, &err, process);
                return (scope, failed(&err));
            }
        };
        let segment = &collected.segment;
        match plan.target.object.commit_segment(segment).await {
            Ok(version) => {
                self.journal_success(&plan.target, segment, &version, process);
                self.set_pending(&scope, collected.left_ms);
                let entries = segment.entries.len();
                (scope, SealOutcome::Sealed { entries, version })
            }
            Err(err) => {
                let oldest = segment
                    .entries
                    .iter()
                    .filter(|e| !is_seal_bookkeeping(e))
                    .map(|e| e.time_ms)
                    .min();
                self.set_pending(&scope, oldest);
                self.journal_failure(&scope, &err, process);
                (scope, failed(&err))
            }
        }
    }

    /// The segment `target` would seal now, or `None` when only bookkeeping is unsealed.
    /// Storage and journal-store I/O.
    async fn plan(&self, target: &Target, open: &[String]) -> Result<Option<Collected>, SealError> {
        let cursor = target.object.last_sealed().await?;
        let store = self.monitor.store();
        let library = target.library_id.clone();
        let open = open.to_vec();
        let cap = self.config.max_entries_per_scope.max(1);
        let collect = move || collect(store.as_ref(), cursor, library.as_deref(), &open, cap);
        let collected = tokio::task::spawn_blocking(collect)
            .await
            .map_err(|e| SealError::Unavailable(e.to_string()))??;
        let Some((entries, through, left_ms)) = collected else {
            return Ok(None);
        };
        let run_id = mint_uuid_v4().map_err(|e| SealError::Unavailable(e.to_string()))?;
        let segment = Segment {
            first: entries[0].id.unwrap_or(cursor + 1),
            through,
            entries,
            run_id,
            sealed_ms: self.monitor.now_ms(),
        };
        Ok(Some(Collected { segment, left_ms }))
    }

    /// Journal a committed segment: its `ocfl.commit` and the scope's `journal.seal`.
    fn journal_success(
        &self,
        target: &Target,
        segment: &Segment,
        version: &str,
        process: &ProcessHandle,
    ) {
        let journal = self.monitor.journal();
        let Ok(reference) = VersionRef::new(target.object.object_id(), version) else {
            tracing::warn!(target: "filesquid::journal", "a journal segment names an unusable version");
            return;
        };
        if let Ok(operation_id) = OperationId::new(segment.run_id.as_str()) {
            let facts = CommitFacts {
                operation_id: &operation_id,
                version: &reference,
                took_over: false,
            };
            for entry in ocfl_commit_entries(&facts) {
                journal.send(entry.process(process.id()));
            }
        }
        let entry = JournalEntry::new(SEAL_OPERATION, Outcome::Succeeded)
            .process(process.id())
            .operation_id(segment.run_id.as_str())
            .object_ref(reference.object_ref())
            .target(target.scope.as_str())
            .target(format!("entries:{}", segment.entries.len()))
            .target(format!("first:{}", segment.first))
            .target(format!("through:{}", segment.through));
        journal.send(entry);
    }

    /// Journal a failed scope as a `journal.seal` warning with its error code.
    fn journal_failure(&self, scope: &str, err: &SealError, process: &ProcessHandle) {
        tracing::warn!(target: "filesquid::journal", scope, error = %err, "a journal seal failed");
        let entry = JournalEntry::new(SEAL_OPERATION, Outcome::Failed)
            .level(Level::Warn)
            .process(process.id())
            .target(scope)
            .error_code(err.code());
        self.monitor.journal().send(entry);
    }

    /// Record when the oldest entry `scope` left unsealed was recorded (`None`: none).
    fn set_pending(&self, scope: &str, oldest: Option<i64>) {
        let mut pending = lock(&self.pending);
        match oldest {
            Some(ms) => pending.insert(scope.to_string(), ms),
            None => pending.remove(scope),
        };
    }

    /// Keep `scope`'s pending time after a run that could not read its object; when none is
    /// known yet, count from now, so a scope that never seals still turns health degraded.
    fn keep_pending(&self, scope: &str) {
        let now = self.monitor.now_ms();
        lock(&self.pending).entry(scope.to_string()).or_insert(now);
    }
}

impl SealStatus for SealService {
    fn oldest_unsealed_ms(&self) -> Option<i64> {
        lock(&self.pending).values().copied().min()
    }
}

/// The target of library `id`.
fn library_target(id: &str, object: Arc<dyn JournalObject>) -> Target {
    Target {
        scope: library_scope(id),
        library_id: Some(id.to_string()),
        object,
    }
}

/// The failed outcome of `err`.
fn failed(err: &SealError) -> SealOutcome {
    SealOutcome::Failed {
        error_code: err.code().to_string(),
    }
}

/// A report of `outcomes`, in order.
fn report_of(outcomes: impl Iterator<Item = (String, SealOutcome)>) -> SealReport {
    SealReport {
        scopes: outcomes
            .map(|(scope, outcome)| ScopeReport { scope, outcome })
            .collect(),
    }
}

/// Lock `mutex`, recovering a poisoned one (the data is plain bookkeeping).
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

/// What one scope collected: its entries, the ID it read through, and when the first entry
/// left for a later run was recorded.
type Collection = Option<(Vec<JournalEntry>, i64, Option<i64>)>;

/// The entries after `cursor` that belong to the scope (`library` names it; `None` is the
/// installation, which skips every entry naming an `open` library), at most `cap` entries
/// and [`MAX_SEGMENT_BYTES`]; `None` when they are only seal bookkeeping. Journal-store I/O.
fn collect(
    store: &dyn JournalStore,
    cursor: i64,
    library: Option<&str>,
    open: &[String],
    cap: usize,
) -> Result<Collection, SealError> {
    let mut query = JournalQuery {
        after_id: Some(cursor),
        library_id: library.map(str::to_string),
        limit: MAX_PAGE,
        ..JournalQuery::default()
    };
    let mut scan = Scan::new(cursor, cap);
    for _ in 0..MAX_SCAN_PAGES {
        let page = store
            .query(&query)
            .map_err(|e| SealError::Unavailable(e.to_string()))?;
        let belongs = |e: &JournalEntry| library.is_some() || !names_open_library(e, open);
        if scan.take(page.entries, belongs) {
            break;
        }
        let Some(next) = page.next_cursor else {
            break;
        };
        query.after_id = Some(next);
    }
    Ok(scan.finish())
}

/// True when `entry` names one of the `open` libraries among its targets. Pure.
fn names_open_library(entry: &JournalEntry, open: &[String]) -> bool {
    entry.target_ids.iter().any(|t| open.contains(t))
}

/// The state of one scope's collection.
struct Scan {
    entries: Vec<JournalEntry>,
    through: i64,
    bytes: usize,
    cap: usize,
    left_ms: Option<i64>,
}

impl Scan {
    fn new(cursor: i64, cap: usize) -> Self {
        Self {
            entries: Vec::new(),
            through: cursor,
            bytes: 0,
            cap,
            left_ms: None,
        }
    }

    /// Take the entries of one page that `belongs` accepts; true when the scan is full.
    fn take(&mut self, page: Vec<JournalEntry>, belongs: impl Fn(&JournalEntry) -> bool) -> bool {
        for entry in page {
            let id = entry.id.unwrap_or(self.through);
            if !belongs(&entry) {
                self.through = self.through.max(id);
                continue;
            }
            let size = serde_json::to_vec(&entry).map(|v| v.len() + 1).unwrap_or(0);
            if self.entries.len() >= self.cap || self.bytes + size > MAX_SEGMENT_BYTES {
                self.left_ms = Some(entry.time_ms);
                return true;
            }
            self.bytes += size;
            self.through = self.through.max(id);
            self.entries.push(entry);
        }
        false
    }

    /// The collection, or `None` when it holds only seal bookkeeping and nothing was left
    /// for a later run.
    fn finish(self) -> Collection {
        if self.left_ms.is_none() && self.entries.iter().all(is_seal_bookkeeping) {
            return None;
        }
        Some((self.entries, self.through, self.left_ms))
    }
}
