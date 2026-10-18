//! The replica cache port and its in-memory reference (API-S-024, T-022).
//!
//! Browser storage is a cache, never canonical, and it can be evicted. [`answer_from_cache`]
//! therefore answers from a [`ReplicaCache`] only for the exact identity that stored an
//! entry, always with its [`Coverage`], and turns a missing or evicted entry into a visible
//! recovery state instead of an empty page.

use super::{CacheIdentity, CacheKey, Coverage, Recovery, ReplicaError, LOG_TARGET};
use crate::library::{QueryAnswer, QueryRequest};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, MutexGuard};

/// Entries [`MemoryReplicaCache::with_default_capacity`] holds.
pub const DEFAULT_CACHE_CAPACITY: usize = 1_024;

/// One cached answer with the identity it was made for and the coverage behind it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedPage {
    /// Who the answer was made for.
    pub identity: CacheIdentity,
    /// How much of the replica backed it.
    pub coverage: Coverage,
    /// The answer.
    pub answer: QueryAnswer,
}

/// What a cache knows about a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheLookup {
    /// The entry.
    Hit(Box<CachedPage>),
    /// The cache evicted the entry and remembers that it did.
    Evicted,
    /// The cache holds nothing for the key.
    Miss,
}

/// The cache backend a host injects (an in-memory map, OPFS or IndexedDB). It stores
/// ciphertext-free replica answers on the device only; the API does no storage I/O itself.
pub trait ReplicaCache: Send + Sync {
    /// Look `key` up.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] when the backend fails.
    fn lookup(&self, key: &CacheKey) -> Result<CacheLookup, ReplicaError>;

    /// Store `page` under `key`, replacing an earlier entry and clearing any eviction note.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] when the backend fails.
    fn store(&self, key: CacheKey, page: CachedPage) -> Result<(), ReplicaError>;
}

/// A replica query asked under an identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaRequest {
    /// Who asks.
    pub identity: CacheIdentity,
    /// The versioned library query.
    pub request: QueryRequest,
}

/// An answer read from the replica, with its coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaAnswer {
    /// The key the answer was found under.
    pub cache_key: CacheKey,
    /// How much of the replica backs the answer; show it when incomplete.
    pub coverage: Coverage,
    /// The answer.
    pub answer: QueryAnswer,
}

/// Answer `request` from `cache`.
///
/// # Errors
///
/// Returns [`ReplicaError::Recovery`] (`not_cached` or `evicted`, both retryable) when the
/// cache cannot answer, [`ReplicaError::IdentityMismatch`] when the entry under the key was
/// made for another identity, and [`ReplicaError::Internal`] when the backend fails.
pub fn answer_from_cache(
    cache: &dyn ReplicaCache,
    request: &ReplicaRequest,
) -> Result<ReplicaAnswer, ReplicaError> {
    let key = request.identity.query_key(&request.request)?;
    let page = match cache.lookup(&key)? {
        CacheLookup::Hit(page) => page,
        CacheLookup::Evicted => return Err(ReplicaError::Recovery(Recovery::Evicted)),
        CacheLookup::Miss => return Err(ReplicaError::Recovery(Recovery::NotCached)),
    };
    if page.identity != request.identity {
        return Err(ReplicaError::IdentityMismatch);
    }
    Ok(ReplicaAnswer {
        cache_key: key,
        coverage: page.coverage,
        answer: page.answer,
    })
}

/// Entries, their age order and the bounded memory of evictions.
#[derive(Default)]
struct State {
    entries: HashMap<CacheKey, CachedPage>,
    order: VecDeque<CacheKey>,
    evicted: HashSet<CacheKey>,
    evicted_order: VecDeque<CacheKey>,
}

impl State {
    /// Remove `key` and remember that it was evicted, forgetting the oldest memory beyond
    /// `capacity`. Returns whether an entry was removed.
    fn evict(&mut self, key: &CacheKey, capacity: usize) -> bool {
        if self.entries.remove(key).is_none() {
            return false;
        }
        self.order.retain(|k| k != key);
        if self.evicted.insert(key.clone()) {
            self.evicted_order.push_back(key.clone());
        }
        while self.evicted_order.len() > capacity {
            if let Some(old) = self.evicted_order.pop_front() {
                self.evicted.remove(&old);
            }
        }
        true
    }
}

/// An in-memory [`ReplicaCache`]: at most `capacity` entries, oldest evicted first, with a
/// memory of evictions bounded to the same size.
pub struct MemoryReplicaCache {
    capacity: usize,
    state: Mutex<State>,
}

impl std::fmt::Debug for MemoryReplicaCache {
    /// Show the capacity only; cached answers are never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryReplicaCache")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl MemoryReplicaCache {
    /// A cache holding at most `capacity` entries.
    ///
    /// # Errors
    ///
    /// Returns `invalid_capacity` when `capacity` is 0.
    pub fn new(capacity: usize) -> Result<Self, ReplicaError> {
        if capacity == 0 {
            return Err(ReplicaError::invalid(
                "invalid_capacity",
                "The local cache must hold at least one entry.",
            ));
        }
        Ok(Self {
            capacity,
            state: Mutex::new(State::default()),
        })
    }

    /// A cache of [`DEFAULT_CACHE_CAPACITY`] entries.
    pub fn with_default_capacity() -> Self {
        Self {
            capacity: DEFAULT_CACHE_CAPACITY,
            state: Mutex::new(State::default()),
        }
    }

    /// Evict the entry under `key`, as a browser does under storage pressure. Returns whether
    /// there was one; its key then reads as evicted.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] if the cache state is poisoned.
    pub fn evict(&self, key: &CacheKey) -> Result<bool, ReplicaError> {
        Ok(self.lock()?.evict(key, self.capacity))
    }

    /// Entries held now.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] if the cache state is poisoned.
    pub fn len(&self) -> Result<usize, ReplicaError> {
        Ok(self.lock()?.entries.len())
    }

    /// Whether the cache holds no entry.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] if the cache state is poisoned.
    pub fn is_empty(&self) -> Result<bool, ReplicaError> {
        Ok(self.len()? == 0)
    }

    /// Lock the state, logging a poisoned lock to stdout only.
    fn lock(&self) -> Result<MutexGuard<'_, State>, ReplicaError> {
        self.state.lock().map_err(|_| {
            tracing::info!(target: LOG_TARGET, error_code = "internal", "replica cache state poisoned");
            ReplicaError::Internal
        })
    }
}

impl ReplicaCache for MemoryReplicaCache {
    fn lookup(&self, key: &CacheKey) -> Result<CacheLookup, ReplicaError> {
        let state = self.lock()?;
        if let Some(page) = state.entries.get(key) {
            return Ok(CacheLookup::Hit(Box::new(page.clone())));
        }
        if state.evicted.contains(key) {
            return Ok(CacheLookup::Evicted);
        }
        Ok(CacheLookup::Miss)
    }

    fn store(&self, key: CacheKey, page: CachedPage) -> Result<(), ReplicaError> {
        let mut state = self.lock()?;
        state.evicted.remove(&key);
        state.evicted_order.retain(|k| k != &key);
        if state.entries.insert(key.clone(), page).is_some() {
            return Ok(());
        }
        state.order.push_back(key);
        if state.entries.len() > self.capacity {
            if let Some(oldest) = state.order.front().cloned() {
                state.evict(&oldest, self.capacity);
            }
        }
        Ok(())
    }
}
