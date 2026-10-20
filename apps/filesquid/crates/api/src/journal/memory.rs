//! In-memory journal store for tests and for hosts without a database.

use super::{
    retention_cutoff, retention_entry, JournalEntry, JournalError, JournalPage, JournalQuery,
    JournalStore, RetentionPolicy,
};
use std::collections::HashMap;
use std::sync::Mutex;

/// The entries in id order, the highest id handed out, and the id of the entry holding each
/// client event ID.
#[derive(Default)]
struct State {
    entries: Vec<JournalEntry>,
    last_id: i64,
    client_events: HashMap<String, i64>,
}

/// Journal store that keeps entries in memory (lost when the process ends).
pub struct MemoryJournal {
    host: String,
    state: Mutex<State>,
}

impl MemoryJournal {
    /// Create an empty store for `host`.
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, JournalError> {
        self.state
            .lock()
            .map_err(|_| JournalError("memory journal lock poisoned".into()))
    }
}

/// Store `entry` in id order and return its id and whether it was stored now: an entry whose
/// client event ID is already held is not stored (the holder's id is answered); a preset id
/// is kept (and an entry already stored under it is not stored again); otherwise the next
/// id is assigned.
fn store_one(state: &mut State, mut entry: JournalEntry) -> (i64, bool) {
    if let Some(held) = entry
        .client_event_id
        .as_ref()
        .and_then(|c| state.client_events.get(c))
    {
        return (*held, false);
    }
    let id = match entry.id {
        Some(id) => id,
        None => state.last_id + 1,
    };
    entry.id = Some(id);
    state.last_id = state.last_id.max(id);
    let at = match state.entries.binary_search_by_key(&Some(id), |e| e.id) {
        Ok(_) => return (id, false),
        Err(at) => at,
    };
    if let Some(client) = &entry.client_event_id {
        state.client_events.insert(client.clone(), id);
    }
    state.entries.insert(at, entry);
    (id, true)
}

impl JournalStore for MemoryJournal {
    fn host(&self) -> &str {
        &self.host
    }

    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        let stored = self.append_new(entries)?;
        Ok(stored.into_iter().map(|(id, _)| id).collect())
    }

    fn append_new(&self, entries: &[JournalEntry]) -> Result<Vec<(i64, bool)>, JournalError> {
        let mut guard = self.lock()?;
        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let mut stored = entry.clone().redacted();
            if stored.host.is_empty() {
                stored.host = self.host.clone();
            }
            ids.push(store_one(&mut guard, stored));
        }
        Ok(ids)
    }

    fn allocate_ids(&self, count: usize) -> Result<Vec<i64>, JournalError> {
        let count = super::check_allocation(count)?;
        let mut guard = self.lock()?;
        let first = guard.last_id + 1;
        guard.last_id += count;
        Ok((first..=guard.last_id).collect())
    }

    fn query(&self, query: &JournalQuery) -> Result<JournalPage, JournalError> {
        let guard = self.lock()?;
        let limit = query.effective_limit();
        let matching = |e: &&JournalEntry| query.past_cursor(e) && query.matches(e);
        let hits: Vec<JournalEntry> = if query.descending {
            guard
                .entries
                .iter()
                .rev()
                .filter(matching)
                .take(limit + 1)
                .cloned()
                .collect()
        } else {
            guard
                .entries
                .iter()
                .filter(matching)
                .take(limit + 1)
                .cloned()
                .collect()
        };
        Ok(JournalPage::from_overfetch(hits, limit))
    }

    fn count(&self, query: &JournalQuery) -> Result<u64, JournalError> {
        Ok(self
            .lock()?
            .entries
            .iter()
            .filter(|e| query.matches(e))
            .count() as u64)
    }

    fn apply_retention(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, JournalError> {
        let Some(cutoff) = retention_cutoff(policy, now_ms) else {
            return Ok(0);
        };
        let deleted = {
            let mut guard = self.lock()?;
            let before = guard.entries.len();
            guard.entries.retain(|e| e.time_ms >= cutoff);
            let kept: HashMap<String, i64> = guard
                .entries
                .iter()
                .filter_map(|e| Some((e.client_event_id.clone()?, e.id?)))
                .collect();
            guard.client_events = kept;
            (before - guard.entries.len()) as u64
        };
        self.append(&[retention_entry(deleted, now_ms)])?;
        Ok(deleted)
    }

    fn ping(&self) -> Result<(), JournalError> {
        self.lock().map(|_| ())
    }
}
