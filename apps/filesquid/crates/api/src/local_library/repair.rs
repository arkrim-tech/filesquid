//! The version-inventory repair command and its candidate query (API-S-023, OCFL-S-020;
//! owner decision 2026-10-18).
//!
//! An object whose committed head has no version inventory (W010) is valid OCFL, but the
//! writer refuses to build on it (OCFL-S-018). The IO-1.4 engine repair adds one copy of the
//! root inventory and its sidecar to the head version directory and changes nothing else.
//! This module decides who may ask for it and when: a [`RepairRequest`] names the library and
//! the objects, carries the owner's [`RepairConsent`] from the UI-1.1 dialog, and is checked
//! before anything is written. The candidate query that finds the objects to offer lives in
//! [`super::candidates`].

use super::candidates::{head_files, open_or_skip};
use super::error::{is_outage, LocalLibraryError};
use super::ledger::RecordedRepair;
use filesquid_core::LibraryId;
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageError};
use filesquid_ocfl::write::{ObjectWriter, RepairError, ReservationError, VersionReservations};
use filesquid_ocfl::{ReadError, RepairReceipt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The journal operation of the repair: the consent, every refusal and each object's outcome.
pub const REPAIR_OPERATION: &str = "ocfl.repair.version_inventory";

/// Most objects one repair command may name: the portable page bound (CORE-S-005).
pub const MAX_REPAIR_OBJECTS: usize = filesquid_core::MAX_PAGE_SIZE as usize;

/// Most candidates one query answers; more set [`RepairCandidates::truncated`].
pub const MAX_CANDIDATES: usize = filesquid_core::MAX_PAGE_SIZE as usize;

/// Longest object identifier a request may name, in bytes.
const MAX_OBJECT_ID_BYTES: usize = 1024;

/// Identifier of the consent statement below. A consent that names another statement was
/// given to different words and is refused.
pub const CONSENT_STATEMENT_ID: &str = "version_inventory_repair.v1";

/// What the owner consents to, in the internal vocabulary; the UI shows its own label-table
/// wording of the same statement, naming the objects.
pub const CONSENT_STATEMENT: &str = "For each object named, one version inventory and its \
     sidecar, copies of the root inventory, will be added to the object's latest version \
     directory; nothing else changes.";

/// The owner's consent from the UI-1.1 dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairConsent {
    /// The library the dialog named.
    pub library_id: LibraryId,
    /// The objects the dialog named, exactly those of the request.
    pub object_ids: Vec<String>,
    /// The statement shown: [`CONSENT_STATEMENT_ID`].
    pub statement_id: String,
    /// True only when the owner confirmed; cancelling sends no request at all.
    pub confirmed: bool,
}

/// The explicit repair command: the library, the objects and the owner's consent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairRequest {
    /// The library whose storage root the host injected.
    pub library_id: LibraryId,
    /// The objects to repair (1 to [`MAX_REPAIR_OBJECTS`], distinct).
    pub object_ids: Vec<String>,
    /// The consent; a request without it is refused.
    #[serde(default)]
    pub consent: Option<RepairConsent>,
}

/// What happened to one object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairResult {
    /// The version inventory and sidecar were added.
    Repaired,
    /// Nothing was written for this object; `error_code` says why.
    Refused,
}

/// One object's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRepair {
    /// The object identifier, as requested.
    pub object_id: String,
    /// The head version, when the object was found.
    pub version: Option<String>,
    /// Repaired or refused.
    pub result: RepairResult,
    /// Why it was refused; `None` when repaired.
    pub error_code: Option<String>,
    /// Whether the same request may succeed later (a held reservation, a storage outage).
    pub retryable: bool,
}

/// The answer to a repair command, one entry per requested object in request order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairAnswer {
    /// The library.
    pub library_id: LibraryId,
    /// Per-object outcomes.
    pub objects: Vec<ObjectRepair>,
}

/// An object the UI may offer to repair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairCandidate {
    /// The object identifier.
    pub object_id: String,
    /// The committed head version that lacks its version inventory.
    pub head: String,
    /// The object's digest algorithm (`sha256` or `sha512`).
    pub digest_algorithm: String,
    /// True when the head already holds its version inventory but not its sidecar: an
    /// interrupted repair the command completes.
    pub interrupted: bool,
    /// Whether the repair also opens the object to commits: true for sha256 objects; a
    /// sha512 object stays readable-only because the writer extends sha256 objects only
    /// (OCFL-S-002).
    pub committable_after_repair: bool,
}

/// The candidate query's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairCandidates {
    /// Candidates in object-root order, at most [`MAX_CANDIDATES`].
    pub candidates: Vec<RepairCandidate>,
    /// True when more candidates exist than were answered.
    pub truncated: bool,
}

impl RepairRequest {
    /// Check the object list and the consent. Nothing is read or written.
    ///
    /// # Errors
    ///
    /// `invalid_objects` for an empty, oversized or repeating list or a blank or oversized
    /// ID; `consent_required` without a confirmed consent; `consent_mismatch` when the
    /// consent names another library, other objects or another statement.
    pub fn validate(&self) -> Result<(), LocalLibraryError> {
        let distinct: BTreeSet<&String> = self.object_ids.iter().collect();
        let sizes_ok = self
            .object_ids
            .iter()
            .all(|id| !id.trim().is_empty() && id.len() <= MAX_OBJECT_ID_BYTES);
        let count_ok = (1..=MAX_REPAIR_OBJECTS).contains(&self.object_ids.len());
        if !(count_ok && sizes_ok && distinct.len() == self.object_ids.len()) {
            return Err(LocalLibraryError::invalid(
                "invalid_objects",
                "Name between one and 1,000 different items to repair.",
            ));
        }
        self.check_consent(&distinct)
    }

    /// The consent must be confirmed and name exactly this library, these objects and the
    /// current statement.
    fn check_consent(&self, objects: &BTreeSet<&String>) -> Result<(), LocalLibraryError> {
        let consent =
            self.consent
                .as_ref()
                .filter(|c| c.confirmed)
                .ok_or(LocalLibraryError::refused(
                    "consent_required",
                    "The owner must confirm the repair before it runs.",
                ))?;
        let consented: BTreeSet<&String> = consent.object_ids.iter().collect();
        let same = consent.library_id == self.library_id
            && consent.statement_id == CONSENT_STATEMENT_ID
            && consented == *objects
            && consent.object_ids.len() == objects.len();
        if !same {
            return Err(LocalLibraryError::refused(
                "consent_mismatch",
                "The confirmation does not match this repair. Ask the owner again.",
            ));
        }
        Ok(())
    }
}

/// A located object: its root in the storage root.
pub(super) struct Located {
    pub(super) root: String,
}

/// Find the roots of the requested objects by opening every object of the storage root.
/// Objects that cannot be opened are skipped (they are reported as not found).
///
/// # Errors
///
/// The open errors of the storage root; `unavailable` for a storage outage.
pub(super) async fn locate<B: StorageBackend>(
    backend: &B,
    wanted: &[String],
) -> Result<BTreeMap<String, Located>, LocalLibraryError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|err| LocalLibraryError::from_open(&err))?;
    let wanted: BTreeSet<&str> = wanted.iter().map(String::as_str).collect();
    let mut found = BTreeMap::new();
    for root in storage.objects() {
        let Some(object) = open_or_skip(&storage, root).await? else {
            continue;
        };
        if wanted.contains(object.id()) {
            let located = Located { root: root.clone() };
            found.insert(object.id().to_string(), located);
        }
    }
    Ok(found)
}

/// Why one object was not repaired: a stable code, whether a retry may succeed, and the
/// head version of the object as reopened for the repair, when it could be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObjectRefusal {
    pub(super) code: &'static str,
    pub(super) retryable: bool,
    pub(super) version: Option<String>,
}

impl ObjectRefusal {
    /// A refusal a retry will not clear.
    pub(super) const fn fixed(code: &'static str) -> Self {
        Self {
            code,
            retryable: false,
            version: None,
        }
    }

    /// A refusal a retry may clear.
    const fn transient(code: &'static str) -> Self {
        Self {
            code,
            retryable: true,
            version: None,
        }
    }

    /// This refusal, for the head `version` of the reopened object.
    fn at(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }
}

/// Repair the object `object_id` located at `located`: reopen it and refuse when another
/// object now sits at that root or its head is already complete, without calling the
/// engine; then run the engine repair, which re-reads the object under its reservation.
pub(super) async fn repair_object<B: StorageBackend, R: VersionReservations>(
    backend: &B,
    reservations: &R,
    holder: &str,
    object_id: &str,
    located: &Located,
) -> Result<RepairReceipt, ObjectRefusal> {
    let object = Object::open(backend, &located.root)
        .await
        .map_err(|err| read_refusal(&err))?;
    let version = object.head().to_string();
    if object.id() != object_id {
        return Err(ObjectRefusal::fixed("object_not_found"));
    }
    let files = head_files(backend, &object)
        .await
        .map_err(|err| storage_refusal(&err).at(&version))?;
    if files.complete() {
        return Err(ObjectRefusal::fixed("nothing_to_repair").at(&version));
    }
    let writer = ObjectWriter::new(backend, reservations, holder);
    writer
        .repair_version_inventory(&located.root)
        .await
        .map_err(|err| engine_refusal(&err).at(&version))
}

/// The refusal for a storage failure: an outage is retryable, anything else is not.
fn storage_refusal(err: &StorageError) -> ObjectRefusal {
    if is_outage(err) {
        ObjectRefusal::transient("storage_unavailable")
    } else {
        ObjectRefusal::fixed("storage_failed")
    }
}

/// The refusal for an object that cannot be read before the repair.
fn read_refusal(err: &ReadError) -> ObjectRefusal {
    tracing::info!(target: super::LOG_TARGET, error = %err, "repair could not read the object");
    match err {
        ReadError::Storage(storage) => storage_refusal(storage),
        _ => ObjectRefusal::fixed("read_failed"),
    }
}

/// The refusal for a reservation failure: a held or lost reservation, or a storage outage,
/// may clear on retry; a corrupt record, a bad configuration or no clock will not.
fn reservation_refusal(err: &ReservationError) -> ObjectRefusal {
    match err {
        ReservationError::Held { .. } => ObjectRefusal::transient("reservation_held"),
        ReservationError::Lost { .. } => ObjectRefusal::transient("reservation_lost"),
        ReservationError::Storage(storage) if is_outage(storage) => {
            ObjectRefusal::transient("storage_unavailable")
        }
        _ => ObjectRefusal::fixed("reservation_failed"),
    }
}

/// Map an engine refusal to its stable code (details to stdout only).
fn engine_refusal(err: &RepairError) -> ObjectRefusal {
    tracing::info!(target: super::LOG_TARGET, error = %err, "repair refused by the engine");
    match err {
        RepairError::Reservation(reservation) => reservation_refusal(reservation),
        RepairError::RootChanged { .. } => ObjectRefusal::transient("root_changed"),
        RepairError::Storage(storage) => storage_refusal(storage),
        RepairError::RootSidecarMismatch { .. } => ObjectRefusal::fixed("root_sidecar_mismatch"),
        RepairError::Invalid { .. } => ObjectRefusal::fixed("object_invalid"),
        RepairError::NotRepairable { .. } => ObjectRefusal::fixed("not_repairable"),
        RepairError::NothingToRepair { .. } => ObjectRefusal::fixed("nothing_to_repair"),
        RepairError::ForeignWrite { .. } => ObjectRefusal::fixed("foreign_write"),
        RepairError::Guard(_) => ObjectRefusal::fixed("git_in_root"),
        RepairError::Read(_) => ObjectRefusal::fixed("read_failed"),
    }
}

/// The ledger's copy of an engine receipt.
pub(super) fn recorded(receipt: &RepairReceipt) -> RecordedRepair {
    RecordedRepair {
        object_id: receipt.object_id.clone(),
        version: receipt.version.to_string(),
        digest_algorithm: receipt.digest_algorithm.name().to_string(),
        inventory_digest: receipt.inventory_digest.clone(),
        written: receipt.written.clone(),
    }
}
