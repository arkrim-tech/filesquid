//! The version-inventory repair (OCFL-S-020): give a committed head that
//! has no version inventory of its own (W010) a copy of the root inventory
//! and root sidecar, so the object can become the base of a commit
//! (OCFL-S-018).
//!
//! OCFL 1.1 allows such a head (§3.7: every version directory SHOULD hold
//! an inventory), but the writer refuses to build on it
//! ([`CommitError::MissingVersionInventory`](super::CommitError::MissingVersionInventory)):
//! readers find the committed version through that file while a new root
//! inventory is not yet committed by its sidecar. The repair adds exactly
//! that file and its sidecar (§3.6: every inventory MUST have a sidecar),
//! byte-identical to the root files (§3.7: the head's inventory MUST equal
//! the root inventory), and changes nothing else.
//!
//! [`ObjectWriter::repair_version_inventory`]:
//!
//! 1. Read the root inventory and require its sidecar to match it.
//! 2. Reserve the object as a commit would ([`VersionReservations`]).
//! 3. Under the reservation, run every refusal check before any write:
//!    read the root files again and refuse a change, run the Git and
//!    hierarchy guards, classify the head version directory, and run the
//!    full validator.
//! 4. Write what is missing of `v<head>/inventory.json` and then its
//!    sidecar, each with `IfAbsent` and a fence check before it.
//! 5. Release the reservation and return a [`RepairReceipt`].
//!
//! One shape counts as a repair in progress rather than as an error: the
//! head holds a version inventory byte-identical to the root inventory, no
//! version sidecar, and no other file its inventory does not list. The
//! only finding that shape causes, the missing version sidecar (E058), is
//! allowed; a run on it writes the sidecar only. A head that already holds
//! both files is refused with [`RepairError::NothingToRepair`].
//!
//! The engine never starts a repair: opening, reading, validating,
//! inspecting and copying an object never call it. The host runs it after
//! the owner consents and journals it (API-S-023).

use std::collections::BTreeSet;

use super::commit::ObjectWriter;
use super::error::{RepairError, ReservationError};
use super::listing::list_version_keys;
use super::reconcile::relative;
use super::reservation::{Reservation, VersionReservations};
use crate::digest::DigestAlgorithm;
use crate::guard::reject_git_in_root;
use crate::inventory::{
    sidecar_name, verify_sidecar, Inventory, VersionNum, INVENTORY_FILE, MAX_INVENTORY_BYTES,
    MAX_SIDECAR_BYTES,
};
use crate::read::committed::at_inventory;
use crate::read::small::{child, key_in, missing_as, read_small};
use crate::read::ReadError;
use crate::storage::{Precondition, StorageBackend, StorageError, StorageKey};
use crate::validate::{validate_object, Finding};

/// What a version-inventory repair wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReceipt {
    /// The object identifier.
    pub object_id: String,
    /// The head version whose directory was repaired. No version was
    /// published.
    pub version: VersionNum,
    /// The object's content-addressing algorithm, which also names the
    /// sidecar.
    pub digest_algorithm: DigestAlgorithm,
    /// The digest of the inventory copied, in `digest_algorithm` (the
    /// value the root sidecar states).
    pub inventory_digest: String,
    /// The paths written, relative to the object root, in write order: the
    /// version inventory (unless an interrupted repair left it) and then
    /// its sidecar.
    pub written: Vec<String>,
    /// Whether the repair's reservation took over one that was still held,
    /// not released, when it expired or passed to this writer
    /// ([`Reservation::took_over`]); the host SHOULD validate the object
    /// now (API-S-022).
    pub took_over: bool,
}

/// The root inventory and its sidecar, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RootFiles {
    inventory: Inventory,
    bytes: Vec<u8>,
    sidecar: Vec<u8>,
    sidecar_key: StorageKey,
}

impl RootFiles {
    /// `<head>/inventory.json`, relative to the object root.
    fn inventory_rel(&self) -> String {
        format!("{}/{INVENTORY_FILE}", self.inventory.head)
    }

    /// `<head>/inventory.json.<algorithm>`, relative to the object root.
    fn sidecar_rel(&self) -> String {
        let name = sidecar_name(self.inventory.digest_algorithm);
        format!("{}/{name}", self.inventory.head)
    }
}

/// What the head version directory holds of the two repair files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeadShape {
    /// Neither: the W010 head.
    Missing,
    /// The version inventory without its sidecar: an interrupted repair.
    InventoryOnly,
}

/// Where the head version directory's files were found.
#[derive(Debug, Default)]
struct HeadFiles {
    inventory: Option<StorageKey>,
    sidecar: Option<StorageKey>,
}

impl<B: StorageBackend, R: VersionReservations> ObjectWriter<'_, B, R> {
    /// Repair the object at `root` whose committed head has no version
    /// inventory (W010), under this writer's reservation of the object
    /// (OCFL-S-020).
    ///
    /// Writes only `v<head>/inventory.json` and then its sidecar, each
    /// byte-identical to the root inventory and root sidecar and never over
    /// an existing file. No content, logical state, other version or root
    /// file changes, and no version is published. An object repaired this
    /// way validates without W010 and is no longer refused by
    /// [`commit`](Self::commit) for its missing version inventory; a sha512
    /// object is still refused there because the writer extends sha256
    /// objects only (OCFL-S-002).
    ///
    /// The engine never calls this itself: the caller runs it once the
    /// owner consented, and records the receipt.
    ///
    /// # Errors
    ///
    /// Before any write: [`RepairError::RootSidecarMismatch`],
    /// [`RepairError::Reservation`] (held by another writer),
    /// [`RepairError::RootChanged`], [`RepairError::Guard`],
    /// [`RepairError::NotRepairable`], [`RepairError::NothingToRepair`] and
    /// [`RepairError::Invalid`]; [`RepairError::Read`] for an object that
    /// cannot be read. Between or at the writes:
    /// [`RepairError::Reservation`] (lost), [`RepairError::ForeignWrite`]
    /// or [`RepairError::Storage`]; running the repair again completes it.
    pub async fn repair_version_inventory(&self, root: &str) -> Result<RepairReceipt, RepairError> {
        let root = crate::read::small::root_dir(root)?;
        let first = read_root_files(self.storage, &root).await?;
        let reservation = self
            .reservations
            .reserve(&first.inventory.id, &first.inventory.head, &self.holder)
            .await?;
        let outcome = self.repair_reserved(&root, &first, &reservation).await;
        self.finish_repair(&reservation, outcome).await
    }

    /// Steps 3 and 4, under the reservation.
    async fn repair_reserved(
        &self,
        root: &str,
        first: &RootFiles,
        reservation: &Reservation,
    ) -> Result<RepairReceipt, RepairError> {
        let files = read_root_files(self.storage, root).await?;
        require_unchanged(root, first, &files)?;
        reject_git_in_root(self.storage, root).await?;
        self.storage.check_hierarchy(root).await?;
        let shape = head_shape(self.storage, root, &files).await?;
        check_findings(self.storage, root, &files, shape).await?;
        let written = self.write_missing(root, &files, shape, reservation).await?;
        Ok(RepairReceipt {
            object_id: files.inventory.id.clone(),
            version: files.inventory.head.clone(),
            digest_algorithm: files.inventory.digest_algorithm,
            inventory_digest: files.inventory.digest_algorithm.digest_hex(&files.bytes),
            written,
            took_over: reservation.took_over(),
        })
    }

    /// Write the version inventory (when `shape` lacks it) and then the
    /// version sidecar, confirming the reservation before each write.
    async fn write_missing(
        &self,
        root: &str,
        files: &RootFiles,
        shape: HeadShape,
        reservation: &Reservation,
    ) -> Result<Vec<String>, RepairError> {
        let mut writes = Vec::with_capacity(2);
        if shape == HeadShape::Missing {
            writes.push((files.inventory_rel(), files.bytes.as_slice()));
        }
        writes.push((files.sidecar_rel(), files.sidecar.as_slice()));
        let mut written = Vec::with_capacity(writes.len());
        for (rel, bytes) in writes {
            let key = key_in(root, &rel)?;
            self.reservations.confirm(reservation).await?;
            self.storage
                .put_if(&key, bytes, &Precondition::IfAbsent)
                .await
                .map_err(|err| foreign(err, &key))?;
            written.push(rel);
        }
        Ok(written)
    }

    /// Release the reservation unless it was lost, and log the outcome.
    async fn finish_repair(
        &self,
        reservation: &Reservation,
        outcome: Result<RepairReceipt, RepairError>,
    ) -> Result<RepairReceipt, RepairError> {
        let lost = matches!(
            outcome,
            Err(RepairError::Reservation(ReservationError::Lost { .. }))
        );
        if !lost {
            if let Err(err) = self.reservations.release(reservation).await {
                tracing::warn!(error = %err, "could not release the repair reservation");
            }
        }
        log_repair(reservation, &outcome);
        outcome
    }
}

/// Read the root inventory and its sidecar, and require the sidecar to
/// match the inventory.
///
/// # Errors
///
/// [`RepairError::RootSidecarMismatch`] for a missing, malformed or
/// mismatching sidecar; [`RepairError::Read`] for a missing or invalid
/// inventory or an engine bound; a backend error.
async fn read_root_files<B: StorageBackend>(
    storage: &B,
    root: &str,
) -> Result<RootFiles, RepairError> {
    let key = key_in(root, INVENTORY_FILE)?;
    let bytes = read_small(storage, &key, MAX_INVENTORY_BYTES, "inventory")
        .await
        .map_err(|err| missing_as(err, "E063", key.as_str(), "the object has no inventory"))?;
    let inventory = Inventory::parse(&bytes).map_err(|err| at_inventory(err, &key))?;
    let algorithm = inventory.digest_algorithm;
    let sidecar_key = key_in(root, &sidecar_name(algorithm))?;
    let mismatch = || RepairError::RootSidecarMismatch {
        key: sidecar_key.to_string(),
    };
    let sidecar = match read_small(storage, &sidecar_key, MAX_SIDECAR_BYTES, "sidecar").await {
        Ok(sidecar) => sidecar,
        Err(ReadError::Storage(StorageError::NotFound { .. })) => return Err(mismatch()),
        Err(err) => return Err(err.into()),
    };
    verify_sidecar(algorithm, &bytes, &sidecar).map_err(|_| mismatch())?;
    Ok(RootFiles {
        inventory,
        bytes,
        sidecar,
        sidecar_key,
    })
}

/// Refuse root files that changed between the read before the reservation
/// and the read under it.
fn require_unchanged(root: &str, first: &RootFiles, now: &RootFiles) -> Result<(), RepairError> {
    if first.bytes != now.bytes {
        return Err(RepairError::RootChanged {
            key: child(root, INVENTORY_FILE),
        });
    }
    if first.sidecar != now.sidecar {
        return Err(RepairError::RootChanged {
            key: now.sidecar_key.to_string(),
        });
    }
    Ok(())
}

/// Classify the head version directory: a W010 head, an interrupted
/// repair, or a refusal.
///
/// # Errors
///
/// [`RepairError::NotRepairable`] for an unlisted file, a version
/// inventory or sidecar that is not byte-identical to its root file, or a
/// sidecar without an inventory; [`RepairError::NothingToRepair`] when both
/// files are already there; a backend error.
async fn head_shape<B: StorageBackend>(
    storage: &B,
    root: &str,
    files: &RootFiles,
) -> Result<HeadShape, RepairError> {
    let found = find_head_files(storage, root, files).await?;
    match (found.inventory, found.sidecar) {
        (None, None) => Ok(HeadShape::Missing),
        (None, Some(sidecar)) => Err(not_repairable(
            &sidecar,
            "a version sidecar without a version inventory",
        )),
        (Some(inventory), sidecar) => {
            require_same(storage, &inventory, &files.bytes, "the root inventory").await?;
            let Some(sidecar) = sidecar else {
                return Ok(HeadShape::InventoryOnly);
            };
            require_same(storage, &sidecar, &files.sidecar, "the root sidecar").await?;
            Err(RepairError::NothingToRepair {
                key: inventory.to_string(),
            })
        }
    }
}

/// List the head version directory and find its version inventory and
/// sidecar. Every other file must be a content path the root inventory's
/// manifest lists.
///
/// # Errors
///
/// [`RepairError::NotRepairable`] naming the first unlisted file, or a
/// backend error.
async fn find_head_files<B: StorageBackend>(
    storage: &B,
    root: &str,
    files: &RootFiles,
) -> Result<HeadFiles, RepairError> {
    let keys = list_version_keys(storage, root, &files.inventory.head).await?;
    let listed: BTreeSet<&str> = files
        .inventory
        .manifest
        .values()
        .flatten()
        .map(String::as_str)
        .collect();
    let (inventory_rel, sidecar_rel) = (files.inventory_rel(), files.sidecar_rel());
    let mut found = HeadFiles::default();
    for key in keys {
        let rel = relative(root, &key);
        if rel == inventory_rel {
            found.inventory = Some(key);
        } else if rel == sidecar_rel {
            found.sidecar = Some(key);
        } else if !listed.contains(rel) {
            return Err(not_repairable(
                &key,
                "a file the head's inventory does not list",
            ));
        }
    }
    Ok(found)
}

/// Require the file at `key` to hold exactly `expected`.
///
/// # Errors
///
/// [`RepairError::NotRepairable`] when it differs from `what`, or a
/// backend error.
async fn require_same<B: StorageBackend>(
    storage: &B,
    key: &StorageKey,
    expected: &[u8],
    what: &str,
) -> Result<(), RepairError> {
    let found = match read_small(storage, key, expected.len(), "repair file").await {
        Ok(found) => found,
        Err(ReadError::Format(_)) => return Err(not_repairable(key, &differs(what))),
        Err(err) => return Err(err.into()),
    };
    if found != expected {
        return Err(not_repairable(key, &differs(what)));
    }
    Ok(())
}

/// Run the full validator and refuse any error but the missing version
/// sidecar of an interrupted repair.
///
/// # Errors
///
/// [`RepairError::Invalid`] with the refusing errors, or an error that
/// stopped validation.
async fn check_findings<B: StorageBackend>(
    storage: &B,
    root: &str,
    files: &RootFiles,
    shape: HeadShape,
) -> Result<(), RepairError> {
    let report = validate_object(storage, root).await?;
    let allowed = match shape {
        HeadShape::InventoryOnly => Some(key_in(root, &files.sidecar_rel())?.to_string()),
        HeadShape::Missing => None,
    };
    let errors: Vec<Finding> = report
        .errors()
        .iter()
        .filter(|f| !(f.code == "E058" && Some(&f.at) == allowed.as_ref()))
        .cloned()
        .collect();
    if errors.is_empty() {
        return Ok(());
    }
    Err(RepairError::Invalid { errors })
}

/// A head file that refuses the repair.
fn not_repairable(key: &StorageKey, reason: &str) -> RepairError {
    RepairError::NotRepairable {
        key: key.to_string(),
        reason: reason.to_string(),
    }
}

/// The reason for a file that is not a copy of `what`.
fn differs(what: &str) -> String {
    format!("it is not byte-identical to {what}")
}

/// A failed `IfAbsent` means another writer put the file there.
fn foreign(err: StorageError, key: &StorageKey) -> RepairError {
    match err {
        StorageError::PreconditionFailed { .. } => RepairError::ForeignWrite {
            key: key.to_string(),
        },
        other => other.into(),
    }
}

/// Log a repair, or why it stopped.
fn log_repair(reservation: &Reservation, outcome: &Result<RepairReceipt, RepairError>) {
    match outcome {
        Ok(receipt) => tracing::info!(
            object = %receipt.object_id,
            version = %receipt.version,
            written = receipt.written.len(),
            "repaired the head version inventory"
        ),
        Err(err) => tracing::warn!(
            object = %reservation.object_id(),
            version = %reservation.version(),
            error = %err,
            "the version-inventory repair stopped"
        ),
    }
}
