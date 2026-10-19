//! [`MemoryFindingsStore`]: the in-memory reference findings store, for tests and for hosts
//! without an operational database. It keeps nothing across restarts.

use super::store::{ErrorSummary, Finding, FindingChange, FindingsStore, RunRecord, Severity, StoreError};
use filesquid_core::LibraryId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// Everything the store holds.
#[derive(Default)]
struct State {
    /// Open findings per (library, object), in stored order.
    findings: BTreeMap<(String, String), Vec<Finding>>,
    cursors: BTreeMap<String, String>,
    runs: BTreeMap<String, RunRecord>,
}

/// The in-memory findings store.
#[derive(Default)]
pub struct MemoryFindingsStore {
    state: Mutex<State>,
}

impl std::fmt::Debug for MemoryFindingsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryFindingsStore").finish_non_exhaustive()
    }
}

impl MemoryFindingsStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `f` on the locked state.
    fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> Result<T, StoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StoreError("poisoned".into()))?;
        Ok(f(&mut state))
    }
}

/// What `next` opens and closes against `previous`.
fn diff(previous: &[Finding], next: &[Finding]) -> FindingChange {
    let opened = next
        .iter()
        .filter(|f| !previous.iter().any(|p| p.same_as(f)))
        .cloned()
        .collect();
    let closed = previous
        .iter()
        .filter(|p| !next.iter().any(|f| f.same_as(p)))
        .cloned()
        .collect();
    FindingChange { opened, closed }
}

impl FindingsStore for MemoryFindingsStore {
    fn replace_findings(
        &self,
        library_id: &LibraryId,
        object_id: &str,
        findings: Vec<Finding>,
    ) -> Result<FindingChange, StoreError> {
        self.with(|state| {
            let key = (library_id.as_str().to_string(), object_id.to_string());
            let previous = state.findings.remove(&key).unwrap_or_default();
            let change = diff(&previous, &findings);
            if !findings.is_empty() {
                state.findings.insert(key, findings);
            }
            change
        })
    }

    fn objects_with_findings(&self, library_id: &LibraryId) -> Result<Vec<String>, StoreError> {
        self.with(|state| {
            state
                .findings
                .keys()
                .filter(|(library, _)| library == library_id.as_str())
                .map(|(_, object)| object.clone())
                .collect()
        })
    }

    fn open_findings(&self, limit: usize) -> Result<Vec<Finding>, StoreError> {
        self.with(|state| {
            let mut all: Vec<Finding> = state.findings.values().flatten().cloned().collect();
            all.sort_by(|a, b| {
                (a.library_id.as_str(), &a.object_id, &a.code, &a.location)
                    .cmp(&(b.library_id.as_str(), &b.object_id, &b.code, &b.location))
            });
            all.truncate(limit);
            all
        })
    }

    fn error_summary(&self) -> Result<ErrorSummary, StoreError> {
        self.with(|state| {
            let errors = state.findings.values().flatten().filter(|f| f.severity == Severity::Error);
            let codes: BTreeSet<String> = errors.clone().map(|f| f.code.clone()).collect();
            let objects: BTreeSet<(&str, &String)> =
                errors.map(|f| (f.library_id.as_str(), &f.object_id)).collect();
            ErrorSummary {
                codes: codes.into_iter().collect(),
                objects: objects.len(),
            }
        })
    }

    fn cursor(&self, library_id: &LibraryId) -> Result<Option<String>, StoreError> {
        self.with(|state| state.cursors.get(library_id.as_str()).cloned())
    }

    fn set_cursor(&self, library_id: &LibraryId, cursor: Option<&str>) -> Result<(), StoreError> {
        self.with(|state| match cursor {
            Some(id) => {
                state.cursors.insert(library_id.as_str().to_string(), id.to_string());
            }
            None => {
                state.cursors.remove(library_id.as_str());
            }
        })
    }

    fn record_run(&self, run: &RunRecord) -> Result<(), StoreError> {
        self.with(|state| {
            state.runs.insert(run.library_id.as_str().to_string(), run.clone());
        })
    }

    fn last_run(&self, library_id: &LibraryId) -> Result<Option<RunRecord>, StoreError> {
        self.with(|state| state.runs.get(library_id.as_str()).cloned())
    }
}
