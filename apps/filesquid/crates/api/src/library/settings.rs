//! The installation's Recently deleted settings (Tasks API-F2.15 and API-F2.30; API-S-019,
//! ADR 041 Decisions 10 and 11): the default retention period, the expiry setting and the
//! expiry interval.
//!
//! They belong to the installation, not to a library, so a host keeps them in its
//! operational store (on the desktop `<data dir>/filesquid.sqlite`) behind the
//! [`RetentionSettings`] port; [`MemorySettings`] is the in-memory reference. A store writes a
//! change and its journal entry together, so no change is ever left unjournaled. A library's
//! own period is a library command instead ([`super::LibraryCommand::SetLibraryRetention`]),
//! kept in its collection snapshot.

use super::catalog::CatalogError;
use super::policy::{
    is_expiry_interval, is_retention, ExpiryMode, DEFAULT_EXPIRY_INTERVAL_MINUTES, RECOVERABLE_DAYS,
};
use crate::journal::JournalEntry;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Journal operation of a change of the installation settings.
pub const SETTINGS_OPERATION: &str = "settings.recently_deleted.set";

/// The installation's Recently deleted settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationSettings {
    /// Days a deleted file stays recoverable in a library that set no period of its own
    /// (1 to 3,650; 30 by default).
    pub default_retention_days: u32,
    /// What expiry does to a deleted file's object.
    pub expiry: ExpiryMode,
    /// Minutes between two expiry runs of the host (5 to 1,440; 60 by default). A changed
    /// interval takes effect for the host's next run ([`super::ExpirySchedule`]).
    pub expiry_interval_minutes: u32,
}

impl Default for InstallationSettings {
    /// 30 days, `keep_version_history` (ADR 041 Decisions 10 and 11) and an expiry run every
    /// 60 minutes (API-S-019).
    fn default() -> Self {
        Self {
            default_retention_days: RECOVERABLE_DAYS,
            expiry: ExpiryMode::KeepVersionHistory,
            expiry_interval_minutes: DEFAULT_EXPIRY_INTERVAL_MINUTES,
        }
    }
}

impl InstallationSettings {
    /// Whether the default retention period and the expiry interval are within their bounds.
    /// Pure.
    pub fn is_valid(&self) -> bool {
        is_retention(self.default_retention_days)
            && is_expiry_interval(self.expiry_interval_minutes)
    }
}

/// Where a host keeps the installation settings.
pub trait RetentionSettings: Send + Sync {
    /// The stored settings, or [`InstallationSettings::default`] when none were stored.
    ///
    /// # Errors
    /// A store failure.
    fn read(&self) -> Result<InstallationSettings, CatalogError>;

    /// Store `settings` with `journal` (opaque IDs only) in one step.
    ///
    /// # Errors
    /// A store failure; nothing is stored then.
    fn store(
        &self,
        settings: &InstallationSettings,
        journal: JournalEntry,
    ) -> Result<(), CatalogError>;
}

/// In-memory [`RetentionSettings`] for tests and hosts without an operational store.
#[derive(Debug, Default)]
pub struct MemorySettings {
    /// The settings and the journal entries stored with them.
    state: Mutex<(InstallationSettings, Vec<JournalEntry>)>,
}

impl MemorySettings {
    /// Settings at their defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// The journal entries stored with the changes, oldest first.
    pub fn journal_entries(&self) -> Vec<JournalEntry> {
        self.state
            .lock()
            .map(|state| state.1.clone())
            .unwrap_or_default()
    }
}

impl RetentionSettings for MemorySettings {
    fn read(&self) -> Result<InstallationSettings, CatalogError> {
        self.state
            .lock()
            .map(|state| state.0)
            .map_err(|_| CatalogError::Failed("memory settings lock poisoned".into()))
    }

    fn store(
        &self,
        settings: &InstallationSettings,
        journal: JournalEntry,
    ) -> Result<(), CatalogError> {
        if !settings.is_valid() {
            return Err(CatalogError::Failed("settings out of range".into()));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| CatalogError::Failed("memory settings lock poisoned".into()))?;
        state.0 = *settings;
        state.1.push(journal.redacted());
        Ok(())
    }
}
