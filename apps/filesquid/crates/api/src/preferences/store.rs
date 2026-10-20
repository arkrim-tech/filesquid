//! [`PreferenceStore`]: where a host keeps each user's preferences, and
//! [`MemoryPreferenceStore`], the in-memory store of tests and of hosts that keep none.
//!
//! The design is that the desktop host keeps them in its SQLite database and
//! `filesquid-server` in PostgreSQL, so a user's choices follow them to every host they sign in
//! to (SRS-F-062). Neither host installs a store yet, so the routes answer `unavailable` until
//! one does. Calls may block on the database; the routes call them on a blocking thread.

use super::model::Preferences;
use std::collections::HashMap;
use std::sync::Mutex;

/// Most users one [`MemoryPreferenceStore`] keeps (AGENTS.md section 22).
pub const MAX_MEMORY_USERS: usize = 10_000;

/// Why a store could not load or save.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreferenceStoreError {
    /// The store cannot be reached now; a retry may succeed. The detail goes to stdout only.
    Unavailable(String),
    /// The store keeps no more users; retrying does not help.
    Full,
}

/// Each user's saved preferences, keyed by the user's opaque actor name.
pub trait PreferenceStore: Send + Sync {
    /// The preferences `user` saved, or `None` when they saved none. May block.
    ///
    /// # Errors
    ///
    /// [`PreferenceStoreError::Unavailable`] when the store cannot be read.
    fn load(&self, user: &str) -> Result<Option<Preferences>, PreferenceStoreError>;

    /// Replace the preferences of `user` with `preferences`, already checked. May block.
    ///
    /// # Errors
    ///
    /// [`PreferenceStoreError::Unavailable`] when the store cannot be written, or
    /// [`PreferenceStoreError::Full`] when it keeps no more users; either way nothing was
    /// saved.
    fn save(&self, user: &str, preferences: &Preferences) -> Result<(), PreferenceStoreError>;
}

/// Preferences kept in memory for at most [`MAX_MEMORY_USERS`] users; they end with the
/// process.
#[derive(Debug, Default)]
pub struct MemoryPreferenceStore {
    users: Mutex<HashMap<String, Preferences>>,
}

impl MemoryPreferenceStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// The map, recovering from a poisoned lock: every update leaves it consistent.
    fn users(&self) -> std::sync::MutexGuard<'_, HashMap<String, Preferences>> {
        self.users
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl PreferenceStore for MemoryPreferenceStore {
    fn load(&self, user: &str) -> Result<Option<Preferences>, PreferenceStoreError> {
        Ok(self.users().get(user).cloned())
    }

    fn save(&self, user: &str, preferences: &Preferences) -> Result<(), PreferenceStoreError> {
        let mut users = self.users();
        if users.len() >= MAX_MEMORY_USERS && !users.contains_key(user) {
            return Err(PreferenceStoreError::Full);
        }
        users.insert(user.to_string(), preferences.clone());
        Ok(())
    }
}
