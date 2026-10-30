//! Receipts of the file write commands (Task API-F2.13; API-S-002, API-S-019): what each
//! operation ID first answered, so a repeated operation ID answers its first outcome with no
//! second change.
//!
//! A command claims its operation ID before it runs. The claim answers one of four things:
//! the stored answer of an earlier run of the same command ([`Claim::Replay`]); a refusal
//! when the ID belongs to another command, another library or another input
//! ([`Claim::Reused`]); a refusal when a run of that ID is still in flight
//! ([`Claim::Busy`]); or a [`ClaimGuard`] for a fresh run. The guard stores the answer of a
//! command that changed something ([`ClaimGuard::store`]); dropping it without storing, on
//! any path (a refusal, a command that changed nothing, a failure, a cancelled request),
//! releases the ID, so its retry is evaluated again (owner decision 2026-10-19: a command that
//! changes nothing stores no receipt).
//!
//! Receipts live in memory for the life of the host process, at most [`MAX_RECEIPTS`] of
//! them; the oldest is forgotten first. Pure apart from the lock; no I/O.

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

/// Most receipts kept; storing one more forgets the oldest.
pub const MAX_RECEIPTS: usize = 10_000;

/// What makes two runs of one operation ID the same command: its route, its library and
/// its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    /// The route's journal operation, for example `file.move`.
    pub route: &'static str,
    /// The library the command names.
    pub library_id: String,
    /// The command's input as JSON, the envelope left out.
    pub input: Value,
}

/// The stored answer of one command.
#[derive(Clone, Debug)]
struct Stored {
    fingerprint: Fingerprint,
    answer: Value,
}

/// The receipts and the operation IDs in flight.
#[derive(Debug, Default)]
struct Inner {
    stored: HashMap<String, Stored>,
    order: VecDeque<String>,
    claimed: HashSet<String>,
}

/// The receipts of the file write commands of one host process.
#[derive(Debug, Default)]
pub struct CommandReceipts {
    inner: Mutex<Inner>,
}

/// What a claim of an operation ID answered.
#[derive(Debug)]
pub enum Claim {
    /// No receipt and no run in flight: run the command, then store or drop the guard.
    Fresh(ClaimGuard),
    /// The same command already ran and changed something: answer this again.
    Replay(Value),
    /// The ID already names another command, library or input.
    Reused,
    /// A run of this ID has not finished yet.
    Busy,
}

/// A claimed operation ID. Dropping it without [`ClaimGuard::store`] releases the ID.
#[derive(Debug)]
pub struct ClaimGuard {
    receipts: Arc<CommandReceipts>,
    operation_id: String,
    fingerprint: Fingerprint,
    released: bool,
}

impl CommandReceipts {
    /// An empty set of receipts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Claim `operation_id` for the command `fingerprint` names. A stored receipt is
    /// answered before anything else is checked (API-S-019).
    pub fn claim(self: &Arc<Self>, operation_id: &str, fingerprint: Fingerprint) -> Claim {
        let mut inner = self.lock();
        if let Some(stored) = inner.stored.get(operation_id) {
            return match stored.fingerprint == fingerprint {
                true => Claim::Replay(stored.answer.clone()),
                false => Claim::Reused,
            };
        }
        if !inner.claimed.insert(operation_id.to_string()) {
            return Claim::Busy;
        }
        Claim::Fresh(ClaimGuard {
            receipts: Arc::clone(self),
            operation_id: operation_id.to_string(),
            fingerprint,
            released: false,
        })
    }

    /// How many receipts are stored.
    pub fn len(&self) -> usize {
        self.lock().stored.len()
    }

    /// Whether no receipt is stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The lock, recovering the receipts when a holder panicked: every update below leaves
    /// them consistent, so a poisoned lock holds usable state.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ClaimGuard {
    /// Store `answer` as the receipt of the claimed ID and release the claim, forgetting the
    /// oldest receipt when [`MAX_RECEIPTS`] are kept.
    pub fn store(mut self, answer: Value) {
        let mut inner = self.receipts.lock();
        inner.claimed.remove(&self.operation_id);
        if inner.order.len() >= MAX_RECEIPTS {
            if let Some(oldest) = inner.order.pop_front() {
                inner.stored.remove(&oldest);
            }
        }
        let stored = Stored {
            fingerprint: self.fingerprint.clone(),
            answer,
        };
        inner.order.push_back(self.operation_id.clone());
        inner.stored.insert(self.operation_id.clone(), stored);
        self.released = true;
    }
}

impl Drop for ClaimGuard {
    /// Release the claim of a run that stored no receipt.
    fn drop(&mut self) {
        if !self.released {
            self.receipts.lock().claimed.remove(&self.operation_id);
        }
    }
}
