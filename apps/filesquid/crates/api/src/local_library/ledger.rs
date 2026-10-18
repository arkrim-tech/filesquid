//! The repair ledger port (API-S-023): where a host records the owner's consent to a
//! version-inventory repair and, per object, the outcome journal entry together with the
//! repair receipt in one transaction.
//!
//! The desktop host stores both in its SQLite operational store next to the activity
//! journal; a server stores them in PostgreSQL. [`MemoryRepairLedger`] is the in-memory
//! reference ledger for tests and for hosts without a database. Entries are already redacted
//! (opaque IDs and codes only) when they reach the ledger.

use crate::journal::JournalEntry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// What the engine wrote for one repaired object: the receipt the ledger keeps with the
/// outcome entry. `written` holds paths relative to the object root (`v2/inventory.json`);
/// the ledger keeps them, and no answer to a UI carries them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRepair {
    /// The object identifier.
    pub object_id: String,
    /// The head version whose directory gained the files.
    pub version: String,
    /// The object's digest algorithm (`sha256` or `sha512`).
    pub digest_algorithm: String,
    /// The digest of the copied inventory, as the root sidecar states it.
    pub inventory_digest: String,
    /// The files written, relative to the object root, in write order.
    pub written: Vec<String>,
}

/// One object's outcome: its journal entry and, when the object was repaired, its receipt.
/// A host stores both in the same transaction or neither.
#[derive(Clone, Debug, PartialEq)]
pub struct RepairRecord {
    /// The `ocfl.repair.version_inventory` entry (outcome, opaque IDs, version, error code).
    pub journal: JournalEntry,
    /// The receipt of a repaired object; `None` for a refused one.
    pub receipt: Option<RecordedRepair>,
}

/// A failure of the injected ledger. Details go to stdout only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    /// The store is unreachable now; a retry may succeed.
    Unavailable(String),
    /// The store failed; a retry will not help.
    Failed(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(detail) => write!(f, "repair ledger unavailable: {detail}"),
            Self::Failed(detail) => write!(f, "repair ledger failed: {detail}"),
        }
    }
}

/// The ledger a host injects for the version-inventory repair.
pub trait RepairLedger: Send + Sync {
    /// Durably record the owner's consent entry before any object is repaired.
    ///
    /// # Errors
    ///
    /// A [`LedgerError`] when it was not recorded; the service then repairs nothing.
    fn record_consent(&self, entry: JournalEntry) -> Result<(), LedgerError>;

    /// Record one object's outcome entry and its receipt in one transaction.
    ///
    /// # Errors
    ///
    /// A [`LedgerError`] when neither was recorded; the service then sends the entry through
    /// the bounded journal queue so the outcome is still journaled.
    fn record_outcome(&self, record: RepairRecord) -> Result<(), LedgerError>;
}

/// The in-memory reference ledger. It can be told to fail, to test the service's refusals.
#[derive(Debug, Default)]
pub struct MemoryRepairLedger {
    consents: Mutex<Vec<JournalEntry>>,
    outcomes: Mutex<Vec<RepairRecord>>,
    fail_consents: AtomicBool,
    fail_outcomes: AtomicBool,
}

impl MemoryRepairLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every consent entry recorded, oldest first.
    pub fn consents(&self) -> Vec<JournalEntry> {
        self.consents.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Every outcome record, oldest first.
    pub fn outcomes(&self) -> Vec<RepairRecord> {
        self.outcomes.lock().map(|o| o.clone()).unwrap_or_default()
    }

    /// Make [`RepairLedger::record_consent`] fail (`true`) or work (`false`).
    pub fn fail_consents(&self, fail: bool) {
        self.fail_consents.store(fail, Ordering::SeqCst);
    }

    /// Make [`RepairLedger::record_outcome`] fail (`true`) or work (`false`).
    pub fn fail_outcomes(&self, fail: bool) {
        self.fail_outcomes.store(fail, Ordering::SeqCst);
    }
}

impl RepairLedger for MemoryRepairLedger {
    fn record_consent(&self, entry: JournalEntry) -> Result<(), LedgerError> {
        if self.fail_consents.load(Ordering::SeqCst) {
            return Err(LedgerError::Unavailable("injected failure".into()));
        }
        let mut consents = self
            .consents
            .lock()
            .map_err(|_| LedgerError::Failed("poisoned".into()))?;
        consents.push(entry);
        Ok(())
    }

    fn record_outcome(&self, record: RepairRecord) -> Result<(), LedgerError> {
        if self.fail_outcomes.load(Ordering::SeqCst) {
            return Err(LedgerError::Unavailable("injected failure".into()));
        }
        let mut outcomes = self
            .outcomes
            .lock()
            .map_err(|_| LedgerError::Failed("poisoned".into()))?;
        outcomes.push(record);
        Ok(())
    }
}
