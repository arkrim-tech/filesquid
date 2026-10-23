//! [`LocalLibraryService`]: runs the local library operations for every host and journals
//! them (ADR 033, `logging-enforce.md`).
//!
//! Create, copy and rebuild are domain operations: their outcome, succeeded or failed, is
//! journaled once through the host's bounded journal queue (producers wait; nothing is
//! dropped). Open and the candidate query are reads: only their failures are journaled. The
//! version-inventory repair journals through the injected [`RepairLedger`]: the owner's
//! consent before anything is written, then each object's outcome together with its receipt
//! in one transaction. A refused repair command, and any outcome the ledger cannot take, goes
//! through the bounded queue instead, so no refusal or outcome is left unjournaled.

use super::candidates::candidates;
use super::error::LocalLibraryError;
use super::journal_rebuild::{rebuild_journal, JournalRestore};
use super::ledger::{RepairLedger, RepairRecord};
use super::provenance::{commit_repair_record, Provenance, Repaired, PROVENANCE_NOT_CONFIGURED};
use super::rebuild::{rebuild, rebuild_from, RebuildEnd, RebuildProgress, RebuildReport};
use super::repair::{
    locate, recorded, repair_object, Located, ObjectRefusal, ObjectRepair, RepairAnswer,
    RepairCandidates, RepairRequest, RepairResult, REPAIR_OPERATION,
};
use super::root::{create_root, open_root, RootSummary};
use super::snapshot::{copy_snapshot, CopyReport};
use super::LOG_TARGET;
use crate::auth::Principal;
use crate::history::{ProvenanceContext, PROV_RECORD_OPERATION};
use crate::journal::{now_ms, JournalEntry, JournalStore, Level, Outcome};
use crate::journal_writer::JournalSender;
use crate::library::mint_uuid_v4;
use crate::registry::Clock;
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;
use filesquid_ocfl::RepairReceipt;
use std::sync::Arc;

/// Journal operation of creating a library's storage root.
pub const CREATE_OPERATION: &str = "library.root.create";
/// Journal operation of opening a storage root (failures only).
pub const OPEN_OPERATION: &str = "library.root.open";
/// Journal operation of copying a completed snapshot.
pub const COPY_OPERATION: &str = "library.snapshot.copy";
/// Journal operation of rebuilding from files.
pub const REBUILD_OPERATION: &str = "library.rebuild";
/// Journal operation of the repair candidate query (failures only).
pub const CANDIDATES_OPERATION: &str = "library.repair_candidates";

/// Runs the local library operations of [`crate::local_library`] for every host.
#[derive(Clone)]
pub struct LocalLibraryService {
    journal: JournalSender,
    ledger: Arc<dyn RepairLedger>,
    clock: Clock,
    provenance: Option<Provenance>,
    journal_restore: Option<JournalRestore>,
}

impl std::fmt::Debug for LocalLibraryService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalLibraryService")
            .finish_non_exhaustive()
    }
}

impl LocalLibraryService {
    /// A service that journals through `journal`, the host's bounded journal queue, and
    /// records repairs in `ledger`.
    pub fn new(journal: JournalSender, ledger: Arc<dyn RepairLedger>) -> Self {
        Self {
            journal,
            ledger,
            clock: Arc::new(now_ms),
            provenance: None,
            journal_restore: None,
        }
    }

    /// Record provenance as `context`: after each repaired object, commit a provenance-only
    /// version whose PROV-O record (kind `repair`) shares the repair run's ID, naming a
    /// journal entry ID that `ids` allocates before the commit (API-S-028). Without it a
    /// repair writes no record and journals that it did not.
    pub fn with_provenance(
        mut self,
        context: ProvenanceContext,
        ids: Arc<dyn JournalStore>,
    ) -> Self {
        self.provenance = Some(Provenance { context, ids });
        self
    }

    /// Also restore the journal on every rebuild from files: the library's per-commit entries
    /// and sealed segments, into `restore`'s store, de-duplicated by journal ID (Task CL-1.9,
    /// API-S-032). Without it a rebuild reports no journal.
    pub fn with_journal_restore(mut self, restore: JournalRestore) -> Self {
        self.journal_restore = Some(restore);
        self
    }

    /// Use `clock` (Unix milliseconds, UTC) for journal times.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Make `root`, an empty folder the owner chose, the OCFL 1.1 storage root of
    /// `library_id`, for `actor`. Needs no account or connection.
    ///
    /// # Errors
    ///
    /// `root_not_empty` (nothing written), the open errors, or `unavailable`.
    pub async fn create_root<B: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        root: &B,
    ) -> Result<RootSummary, LocalLibraryError> {
        let result = create_root(root).await;
        self.journal_outcome(actor, CREATE_OPERATION, library_id, &result);
        result
    }

    /// Open the storage root of `library_id` for `actor`. Writes and repairs nothing; only a
    /// failure is journaled.
    ///
    /// # Errors
    ///
    /// `not_a_storage_root`, `git_in_root` or `unavailable`.
    pub async fn open_root<B: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        root: &B,
    ) -> Result<RootSummary, LocalLibraryError> {
        let result = open_root(root).await;
        self.journal_failure(actor, OPEN_OPERATION, library_id, &result);
        result
    }

    /// Copy the completed snapshot of `source` into the empty `dest` for `actor`, and
    /// validate the copy. The source is only read.
    ///
    /// # Errors
    ///
    /// `destination_not_empty`, `snapshot_incomplete` (retryable), `snapshot_unreadable`,
    /// the open errors of the source, or `unavailable`.
    pub async fn copy_snapshot<S: StorageBackend, D: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        source: &S,
        dest: &D,
    ) -> Result<CopyReport, LocalLibraryError> {
        let result = copy_snapshot(source, dest).await;
        self.journal_outcome(actor, COPY_OPERATION, library_id, &result);
        result
    }

    /// Rebuild what `root` holds from its OCFL files alone, for `actor`, and with a journal
    /// restore installed, its journal. Findings are reported, never repaired; nothing is
    /// written to `root`. The journal restore needs a Tokio runtime.
    ///
    /// # Errors
    ///
    /// The open errors of the storage root, `journal_too_large`, or `unavailable`.
    pub async fn rebuild<B: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        root: &B,
    ) -> Result<RebuildReport, LocalLibraryError> {
        let result = self.rebuild_all(root).await;
        self.journal_outcome(actor, REBUILD_OPERATION, library_id, &result);
        result
    }

    /// Rebuild what `root` holds after the cursor `after` (from the start without one), for
    /// `actor`, telling `progress` about each object: the step of a rebuild job
    /// (API-S-025 **Rebuild as a job**). Once every object has been gone through, the journal
    /// is restored when a journal restore is installed, and the rebuild's outcome is journaled
    /// once as `library.rebuild`; a rebuild `progress` stops is not journaled here, and a
    /// failure is journaled as failed. Nothing is written to `root`.
    ///
    /// # Errors
    ///
    /// The open errors of the storage root, `journal_too_large`, or `unavailable`.
    pub async fn rebuild_from<B: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        root: &B,
        after: Option<String>,
        progress: &mut (dyn RebuildProgress + Send),
    ) -> Result<RebuildEnd, LocalLibraryError> {
        let result = self.rebuild_objects(root, after.as_deref(), progress).await;
        if !matches!(result, Ok(RebuildEnd::Stopped)) {
            self.journal_outcome(actor, REBUILD_OPERATION, library_id, &result);
        }
        result
    }

    /// The objects of `root` after `after`, then, once they are all gone through and a
    /// journal restore is installed, its journal. Storage and journal-store I/O.
    async fn rebuild_objects<B: StorageBackend>(
        &self,
        root: &B,
        after: Option<&str>,
        progress: &mut (dyn RebuildProgress + Send),
    ) -> Result<RebuildEnd, LocalLibraryError> {
        let Some(total) = rebuild_from(root, after, progress).await? else {
            return Ok(RebuildEnd::Stopped);
        };
        let journal = match &self.journal_restore {
            Some(restore) => Some(rebuild_journal(root, restore).await?),
            None => None,
        };
        Ok(RebuildEnd::Finished { total, journal })
    }

    /// The objects of `root` whose committed head lacks its version inventory: what the
    /// UI-1.1 dialog may offer to repair, for `actor`. Writes and repairs nothing; only a
    /// failure is journaled.
    ///
    /// # Errors
    ///
    /// The open errors of the storage root, or `unavailable`.
    pub async fn repair_candidates<B: StorageBackend>(
        &self,
        actor: &str,
        library_id: &LibraryId,
        root: &B,
    ) -> Result<RepairCandidates, LocalLibraryError> {
        let result = candidates(root).await;
        self.journal_failure(actor, CANDIDATES_OPERATION, library_id, &result);
        result
    }

    /// Repair the version inventory of the objects `request` names in `root`, for `caller`,
    /// after the owner's consent (API-S-023). Each object gets its own outcome; a refused
    /// object does not stop the others.
    ///
    /// # Errors
    ///
    /// `forbidden` for anyone but the desktop owner; `invalid_objects`, `consent_required`
    /// or `consent_mismatch`; the open errors of the storage root; `unavailable` when the
    /// consent cannot be recorded. Nothing is written on any of them, and each is journaled.
    pub async fn repair_version_inventory<B: StorageBackend, R: VersionReservations>(
        &self,
        caller: &Principal,
        request: RepairRequest,
        root: &B,
        reservations: &R,
    ) -> Result<RepairAnswer, LocalLibraryError> {
        let library_id = request.library_id.clone();
        let result = self.run_repair(caller, request, root, reservations).await;
        self.journal_failure(caller.actor(), REPAIR_OPERATION, &library_id, &result);
        result
    }

    /// Authorise, check, locate, record the consent, then repair each object in order.
    async fn run_repair<B: StorageBackend, R: VersionReservations>(
        &self,
        caller: &Principal,
        request: RepairRequest,
        root: &B,
        reservations: &R,
    ) -> Result<RepairAnswer, LocalLibraryError> {
        authorise_repair(caller)?;
        request.validate()?;
        let located = locate(root, &request.object_ids).await?;
        let actor = caller.actor();
        let run_id = holder_id()?;
        let holder = format!("filesquid-repair-{run_id}");
        self.record_consent(actor, &request, &run_id)?;
        let mut objects = Vec::with_capacity(request.object_ids.len());
        for object_id in &request.object_ids {
            let found = located.get(object_id);
            let outcome = match found {
                Some(found) => repair_object(root, reservations, &holder, object_id, found).await,
                None => Err(ObjectRefusal::fixed("object_not_found")),
            };
            let target = Target {
                actor,
                library_id: &request.library_id,
                object_id,
                run_id: &run_id,
            };
            let mut answer = self.record_outcome(&target, outcome);
            if let (RepairResult::Repaired, Some(found)) = (answer.result, found) {
                let follow_up = (root, reservations, holder.as_str(), found);
                self.record_provenance(&target, follow_up, &mut answer)
                    .await;
            }
            objects.push(answer);
        }
        Ok(RepairAnswer {
            library_id: request.library_id,
            run_id,
            objects,
        })
    }

    /// Commit the provenance record of a repaired object and journal its entries; without a
    /// provenance context, or when the commit fails, journal why no record was written and
    /// say so in `answer`. Never silent (API-S-028).
    async fn record_provenance<B: StorageBackend, R: VersionReservations>(
        &self,
        target: &Target<'_>,
        (root, reservations, holder, found): (&B, &R, &str, &Located),
        answer: &mut ObjectRepair,
    ) {
        let Some(provenance) = &self.provenance else {
            self.journal_unrecorded(target, PROVENANCE_NOT_CONFIGURED, Level::Warn);
            answer.provenance_error_code = Some(PROVENANCE_NOT_CONFIGURED.to_string());
            return;
        };
        let repaired = Repaired {
            object_root: &found.root,
            object_id: target.object_id,
            run_id: target.run_id,
            actor: target.actor,
            holder,
            now_ms: (self.clock)(),
        };
        match commit_repair_record(provenance, root, reservations, &repaired).await {
            Ok((version, entries)) => {
                entries
                    .into_iter()
                    .for_each(|entry| self.journal.send(entry));
                answer.provenance_version = Some(version);
            }
            Err(code) => {
                self.journal_unrecorded(target, code, Level::Error);
                answer.provenance_error_code = Some(code.to_string());
            }
        }
    }

    /// Journal that no provenance record was written for `target`, and why.
    fn journal_unrecorded(&self, target: &Target<'_>, code: &'static str, level: Level) {
        let outcome = match level {
            Level::Error => Outcome::Failed,
            _ => Outcome::Info,
        };
        self.journal.send(
            JournalEntry::new(PROV_RECORD_OPERATION, outcome)
                .level(level)
                .error_code(code)
                .actor(target.actor)
                .operation_id(target.run_id)
                .target(target.object_id)
                .at_ms((self.clock)()),
        );
    }

    /// Durably record the consent entry before any repair, under the run's ID.
    fn record_consent(
        &self,
        actor: &str,
        request: &RepairRequest,
        run_id: &str,
    ) -> Result<(), LocalLibraryError> {
        let entry = request
            .object_ids
            .iter()
            .fold(
                JournalEntry::new(REPAIR_OPERATION, Outcome::Started)
                    .actor(actor)
                    .operation_id(run_id)
                    .target(request.library_id.as_str()),
                |entry, id| entry.target(id.as_str()),
            )
            .at_ms((self.clock)())
            .redacted();
        self.ledger.record_consent(entry).map_err(|err| {
            tracing::info!(target: LOG_TARGET, error = %err, "repair consent not recorded");
            LocalLibraryError::Unavailable
        })
    }

    /// Record one object's outcome entry with its receipt in the ledger, falling back to the
    /// bounded journal queue when the ledger cannot take it, and answer the outcome.
    fn record_outcome(
        &self,
        target: &Target<'_>,
        outcome: Result<RepairReceipt, ObjectRefusal>,
    ) -> ObjectRepair {
        let version = match &outcome {
            Ok(receipt) => Some(receipt.version.to_string()),
            Err(refusal) => refusal.version.clone(),
        };
        let entry = outcome_entry(target, version.as_deref(), &outcome).at_ms((self.clock)());
        let record = RepairRecord {
            journal: entry.clone().redacted(),
            receipt: outcome.as_ref().ok().map(recorded),
        };
        if let Err(err) = self.ledger.record_outcome(record) {
            tracing::warn!(target: LOG_TARGET, error = %err, "repair outcome sent to the journal queue");
            self.journal.send(entry);
        }
        object_answer(target.object_id, version, outcome)
    }

    /// The catalog rebuild of `root`, then, with a journal restore installed, its journal.
    /// Storage and journal-store I/O.
    async fn rebuild_all<B: StorageBackend>(
        &self,
        root: &B,
    ) -> Result<RebuildReport, LocalLibraryError> {
        let mut report = rebuild(root).await?;
        if let Some(restore) = &self.journal_restore {
            report.journal = Some(rebuild_journal(root, restore).await?);
        }
        Ok(report)
    }

    /// Journal `result` of a domain operation, succeeded or failed.
    fn journal_outcome<T>(
        &self,
        actor: &str,
        operation: &'static str,
        library_id: &LibraryId,
        result: &Result<T, LocalLibraryError>,
    ) {
        let entry = match result {
            Ok(_) => JournalEntry::new(operation, Outcome::Succeeded),
            Err(err) => failure_entry(operation, err),
        };
        self.journal.send(
            entry
                .actor(actor)
                .target(library_id.as_str())
                .at_ms((self.clock)()),
        );
    }

    /// Journal a refusal of `operation` that the API made before reaching the operation
    /// itself (an unknown location or library, a bound), naming `target` when there is one.
    pub(super) fn journal_refusal(
        &self,
        actor: &str,
        operation: &'static str,
        target: Option<&str>,
        err: &LocalLibraryError,
    ) {
        let entry = failure_entry(operation, err).actor(actor);
        let entry = match target {
            Some(target) => entry.target(target),
            None => entry,
        };
        self.journal.send(entry.at_ms((self.clock)()));
    }

    /// Journal `result` only when it failed.
    fn journal_failure<T>(
        &self,
        actor: &str,
        operation: &'static str,
        library_id: &LibraryId,
        result: &Result<T, LocalLibraryError>,
    ) {
        if result.is_err() {
            self.journal_outcome(actor, operation, library_id, result);
        }
    }
}

/// Who and what one outcome entry is about.
struct Target<'a> {
    actor: &'a str,
    library_id: &'a LibraryId,
    object_id: &'a str,
    /// The repair run: consent, outcomes and the follow-up provenance
    /// commit share it as their operation ID (ADR 040 Decision 6).
    run_id: &'a str,
}

/// Only the desktop owner may repair. A server library owner becomes a [`Principal`] with
/// server accounts (API-F1.9); adding it makes this match fail to compile until its
/// permission is decided. The server administrator does not own libraries and is refused.
fn authorise_repair(caller: &Principal) -> Result<(), LocalLibraryError> {
    match caller {
        Principal::Owner => Ok(()),
        Principal::Administrator => Err(LocalLibraryError::Forbidden),
    }
}

/// A fresh opaque reservation-holder identifier for one repair command.
fn holder_id() -> Result<String, LocalLibraryError> {
    mint_uuid_v4().map_err(|_| LocalLibraryError::Unavailable)
}

/// The failed entry of `operation`, at warn level for a refusal and error level for a
/// fault.
fn failure_entry(operation: &'static str, err: &LocalLibraryError) -> JournalEntry {
    let level = match err {
        LocalLibraryError::Unavailable | LocalLibraryError::Internal => Level::Error,
        _ => Level::Warn,
    };
    tracing::info!(target: LOG_TARGET, operation, error_code = err.error_code(), "refused");
    JournalEntry::new(operation, Outcome::Failed)
        .error_code(err.error_code())
        .level(level)
}

/// The outcome entry of one object: the run ID as its operation ID, library, object and
/// version as targets, the object reference `<object-id>@vN` only when the object was
/// repaired (a refused entry has no object reference, so no PROV-O record links it, though
/// it keeps the version it found as a target), and the refusal code when refused.
fn outcome_entry(
    target: &Target<'_>,
    version: Option<&str>,
    outcome: &Result<RepairReceipt, ObjectRefusal>,
) -> JournalEntry {
    let entry = match outcome {
        Ok(_) => JournalEntry::new(REPAIR_OPERATION, Outcome::Succeeded),
        Err(refusal) => JournalEntry::new(REPAIR_OPERATION, Outcome::Failed)
            .error_code(refusal.code)
            .level(Level::Warn),
    };
    let entry = entry
        .actor(target.actor)
        .operation_id(target.run_id)
        .target(target.library_id.as_str())
        .target(target.object_id);
    let Some(version) = version else {
        return entry;
    };
    let entry = entry.target(version);
    match outcome {
        Ok(_) => entry.object_ref(format!("{}@{version}", target.object_id)),
        Err(_) => entry,
    }
}

/// The answer for one object.
fn object_answer(
    object_id: &str,
    version: Option<String>,
    outcome: Result<RepairReceipt, ObjectRefusal>,
) -> ObjectRepair {
    let (result, error_code, retryable) = match outcome {
        Ok(_) => (RepairResult::Repaired, None, false),
        Err(refusal) => (
            RepairResult::Refused,
            Some(refusal.code.to_string()),
            refusal.retryable,
        ),
    };
    ObjectRepair {
        object_id: object_id.to_string(),
        version,
        result,
        error_code,
        retryable,
        provenance_version: None,
        provenance_error_code: None,
    }
}
