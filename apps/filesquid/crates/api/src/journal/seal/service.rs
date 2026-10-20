//! The sealing service (API-S-032; ADR 033 Decision 7). The host calls [`SealService::seal_all`]
//! every hour and at app close, and [`SealService::close_library`] at library close.
//!
//! Each run, for every scope (each open library, then the installation):
//!
//! 1. reads how far the scope's journal object is sealed (from its segments);
//! 2. collects the entries after that, in journal ID order: a library's entries name its
//!    library ID among their targets; the installation takes every other entry (an entry of
//!    a library never opened by this service stays with the installation; one of a library
//!    opened and later closed waits for that library's next open); at most
//!    [`SealConfig::max_entries_per_scope`] entries and [`MAX_SEGMENT_BYTES`] per segment;
//! 3. commits them as one segment, a new version of the object.
//!
//! A run in which no scope has anything but seal bookkeeping to seal registers nothing,
//! commits nothing and journals nothing. A scope that read [`SealConfig::max_scan_pages`]
//! pages of other scopes' entries commits a segment of its bookkeeping anyway, so its cursor moves on. Otherwise the run is a registered `journal.seal`
//! process, and each scope's result is journaled as [`SEAL_OPERATION`] (`succeeded` with its
//! counts, or `failed` with its error code), with the segment's `ocfl.commit`. A failed scope
//! keeps its entries for the next run; its object, and so its cursor, is unchanged.

use std::collections::{BTreeMap, BTreeSet};
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

/// Longest silence before a seal run counts as stalled.
const SEAL_HEARTBEAT_MS: i64 = 300_000;

/// Seal settings.
#[derive(Clone, Copy, Debug)]
pub struct SealConfig {
    /// Most entries one scope seals per run; the rest waits for the next run.
    pub max_entries_per_scope: usize,
    /// Most journal pages one scope reads per run (AGENTS.md §22): an installation scope
    /// skips library entries, so it may read more pages than it seals.
    pub max_scan_pages: usize,
}

impl Default for SealConfig {
    /// 50,000 entries and 1,000 journal pages per scope per run.
    fn default() -> Self {
        Self {
            max_entries_per_scope: 50_000,
            max_scan_pages: 1_000,
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
    /// Every library opened since the service started, open or closed: the installation
    /// scope never seals their entries.
    known: Mutex<BTreeSet<String>>,
    /// The last cursor read from each scope's object.
    cursors: Mutex<BTreeMap<String, i64>>,
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
            known: Mutex::new(BTreeSet::new()),
            cursors: Mutex::new(BTreeMap::new()),
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
        lock(&self.known).insert(library_id.to_string());
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
        let known: Vec<String> = lock(&self.known).iter().cloned().collect();
        let mut plans = Vec::with_capacity(targets.len());
        for target in targets {
            let planned = self.plan(&target, &known).await;
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
                self.keep_pending(&plan.target).await;
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
    async fn plan(
        &self,
        target: &Target,
        known: &[String],
    ) -> Result<Option<Collected>, SealError> {
        let cursor = target.object.last_sealed().await?;
        lock(&self.cursors).insert(target.scope.clone(), cursor);
        let store = self.monitor.store();
        let library = target.library_id.clone();
        let known = known.to_vec();
        let limits = Limits {
            entries: self.config.max_entries_per_scope.max(1),
            pages: self.config.max_scan_pages.max(1),
        };
        let collect = move || collect(store.as_ref(), cursor, library.as_deref(), &known, limits);
        let collected = tokio::task::spawn_blocking(collect)
            .await
            .map_err(|e| SealError::Unavailable(e.to_string()))??;
        let Some((entries, through, left_ms)) = collected else {
            return Ok(None);
        };
        let run_id = mint_uuid_v4().map_err(|e| SealError::Unavailable(e.to_string()))?;
        let segment = Segment {
            first: entries.first().and_then(|e| e.id).unwrap_or(cursor + 1),
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

    /// Record `target`'s pending time after a run that could not plan it: the oldest entry
    /// after its last known cursor (0 when none was read yet), or now when the store cannot
    /// say, so a scope that never seals still turns health degraded. Journal-store I/O.
    async fn keep_pending(&self, target: &Target) {
        let cursor = lock(&self.cursors).get(&target.scope).copied().unwrap_or(0);
        let store = self.monitor.store();
        let library = target.library_id.clone();
        let oldest = move || oldest_after(store.as_ref(), cursor, library.as_deref());
        let oldest = tokio::task::spawn_blocking(oldest).await.ok().flatten();
        let since = oldest.unwrap_or_else(|| self.monitor.now_ms());
        let mut pending = lock(&self.pending);
        let kept = pending
            .get(&target.scope)
            .map_or(since, |&ms| ms.min(since));
        pending.insert(target.scope.clone(), kept);
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

/// When the first entry after `cursor` (of `library`, if named) was recorded, or `None` when
/// there is none or the store fails. Journal-store I/O.
fn oldest_after(store: &dyn JournalStore, cursor: i64, library: Option<&str>) -> Option<i64> {
    let query = JournalQuery {
        after_id: Some(cursor),
        library_id: library.map(str::to_string),
        limit: 1,
        ..JournalQuery::default()
    };
    let page = store.query(&query).ok()?;
    page.entries.first().map(|e| e.time_ms)
}

/// The entries after `cursor` that belong to the scope (`library` names it; `None` is the
/// installation, which skips every entry naming a `known` library), at most `cap` entries
/// and [`MAX_SEGMENT_BYTES`]; `None` when they are only seal bookkeeping. An entry too large
/// for any segment is an error. Journal-store I/O.
fn collect(
    store: &dyn JournalStore,
    cursor: i64,
    library: Option<&str>,
    known: &[String],
    limits: Limits,
) -> Result<Collection, SealError> {
    let mut query = JournalQuery {
        after_id: Some(cursor),
        library_id: library.map(str::to_string),
        limit: MAX_PAGE,
        ..JournalQuery::default()
    };
    let mut scan = Scan::new(cursor, limits.entries);
    let mut pages = 0;
    while pages < limits.pages {
        pages += 1;
        let page = store
            .query(&query)
            .map_err(|e| SealError::Unavailable(e.to_string()))?;
        let belongs = |e: &JournalEntry| library.is_some() || !names_library(e, known);
        if scan.take(page.entries, belongs)? {
            return Ok(scan.finish());
        }
        let Some(next) = page.next_cursor else {
            return Ok(scan.finish());
        };
        query.after_id = Some(next);
    }
    scan.exhausted = true;
    Ok(scan.finish())
}

/// True when `entry` names one of the `known` libraries among its targets. Pure.
fn names_library(entry: &JournalEntry, known: &[String]) -> bool {
    entry.target_ids.iter().any(|t| known.contains(t))
}

/// How much one scope's collection may take.
#[derive(Clone, Copy)]
struct Limits {
    entries: usize,
    pages: usize,
}

/// The state of one scope's collection.
struct Scan {
    entries: Vec<JournalEntry>,
    through: i64,
    bytes: usize,
    cap: usize,
    left_ms: Option<i64>,
    /// True when the scan stopped at its page limit with entries still to read.
    exhausted: bool,
}

impl Scan {
    fn new(cursor: i64, cap: usize) -> Self {
        Self {
            entries: Vec::new(),
            through: cursor,
            bytes: 0,
            cap,
            left_ms: None,
            exhausted: false,
        }
    }

    /// Take the entries of one page that `belongs` accepts; true when the scan is full. An
    /// entry larger than [`MAX_SEGMENT_BYTES`] on its own is an error: no segment can hold it.
    fn take(
        &mut self,
        page: Vec<JournalEntry>,
        belongs: impl Fn(&JournalEntry) -> bool,
    ) -> Result<bool, SealError> {
        for entry in page {
            let id = entry.id.unwrap_or(self.through);
            if !belongs(&entry) {
                self.through = self.through.max(id);
                continue;
            }
            let size = serde_json::to_vec(&entry).map(|v| v.len() + 1).unwrap_or(0);
            if size > MAX_SEGMENT_BYTES {
                return Err(SealError::Invalid(format!(
                    "journal entry {id} exceeds the segment limit"
                )));
            }
            if self.entries.len() >= self.cap || self.bytes + size > MAX_SEGMENT_BYTES {
                self.left_ms = Some(entry.time_ms);
                return Ok(true);
            }
            self.bytes += size;
            self.through = self.through.max(id);
            self.entries.push(entry);
        }
        Ok(false)
    }

    /// The collection, or `None` when it holds only seal bookkeeping, nothing was left for a
    /// later run and the page cap did not stop the scan.
    fn finish(self) -> Collection {
        let idle = self.left_ms.is_none() && !self.exhausted;
        if idle && self.entries.iter().all(is_seal_bookkeeping) {
            return None;
        }
        Some((self.entries, self.through, self.left_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, target: &str) -> JournalEntry {
        let mut entry = JournalEntry::new("host.started", Outcome::Info).target(target);
        entry.id = Some(id);
        entry
    }

    #[test]
    fn an_entry_larger_than_any_segment_is_an_error_not_a_panic() {
        let mut scan = Scan::new(0, 10);
        let huge = "x".repeat(MAX_SEGMENT_BYTES + 1);
        let taken = scan.take(vec![entry(1, &huge)], |_| true);
        assert!(matches!(taken, Err(SealError::Invalid(_))), "{taken:?}");
    }

    #[test]
    fn a_scan_stopped_by_its_page_limit_still_moves_the_cursor() {
        let mut scan = Scan::new(0, 10);
        let taken = scan.take(vec![entry(1, "library-a"), entry(2, "library-a")], |_| {
            false
        });
        assert!(!taken.expect("take"));
        scan.exhausted = true;
        let (entries, through, left) = scan.finish().expect("a cursor-only segment");
        assert!(entries.is_empty());
        assert_eq!((through, left), (2, None));
    }

    #[test]
    fn a_scan_of_other_scopes_entries_only_is_idle_within_its_page_limit() {
        let mut scan = Scan::new(0, 10);
        let taken = scan.take(vec![entry(1, "library-a")], |_| false);
        assert!(!taken.expect("take"));
        assert!(scan.finish().is_none());
    }
}
