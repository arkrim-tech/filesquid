//! Errors of the write path: reservations, commits and the version-inventory
//! repair.

use thiserror::Error;

use crate::error::OcflError;
use crate::inventory::FormatError;
use crate::read::ReadError;
use crate::storage::StorageError;
use crate::validate::Finding;

/// Why a version could not be reserved, or why a reservation no longer
/// authorises writes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReservationError {
    /// Another writer holds the object's reservation and it has not
    /// expired. Nothing was written.
    #[error("object {object_id} is reserved by {holder}")]
    Held {
        /// The object identifier.
        object_id: String,
        /// The writer that holds the reservation.
        holder: String,
    },
    /// The reservation expired, was released, or passed to another writer.
    /// Its holder MUST NOT write content or publish the head (ADR 007).
    #[error("the reservation of {version} of {object_id} expired or passed to another writer")]
    Lost {
        /// The object identifier.
        object_id: String,
        /// The version the reservation was for.
        version: String,
    },
    /// A stored reservation record could not be read as one.
    #[error("{key} is not a reservation record: {reason}")]
    Corrupt {
        /// The record's storage key.
        key: String,
        /// What is wrong with it.
        reason: String,
    },
    /// The reservation store was configured in a way the contract forbids.
    #[error("invalid reservation configuration: {reason}")]
    InvalidConfig {
        /// What is wrong with the configuration.
        reason: String,
    },
    /// The clock could not tell the time, so no lease can be taken,
    /// confirmed or released safely.
    #[error("the clock cannot tell the time")]
    ClockUnavailable,
    /// The store's backend failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Why a commit published no version.
///
/// A commit that fails publishes nothing: the root inventory still names
/// the previous head, and any `vN` bytes it left are unpublished. The next
/// commit of that version, under a new reservation, reconciles them.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommitError {
    /// The version could not be reserved, or the reservation was lost
    /// before publication (expired, released or reassigned).
    #[error(transparent)]
    Reservation(#[from] ReservationError),
    /// The committed head is not the one the caller expected. `None` means
    /// no committed version. Nothing was written; the caller decides
    /// whether to rebase (the change model, IO-0.10) or report a conflict.
    #[error("the object head is {found:?}, not the expected {expected:?}")]
    HeadMoved {
        /// The head the caller expected.
        expected: Option<String>,
        /// The committed head found.
        found: Option<String>,
    },
    /// The draft cannot become a valid OCFL 1.1 version: a `created` that
    /// is not RFC 3339 UTC, a digest the manifest does not hold, another
    /// object's identifier, or a logical path OCFL forbids.
    #[error("invalid version draft: {reason}")]
    InvalidDraft {
        /// What is wrong with the draft.
        reason: String,
    },
    /// The object's content-addressing algorithm is not `sha256`, the only
    /// one the engine writes (OCFL-S-002).
    #[error("the engine writes sha256 objects only; this object uses {algorithm}")]
    UnsupportedAlgorithm {
        /// The object's `digestAlgorithm`.
        algorithm: String,
    },
    /// Staged bytes changed after the version was assembled. Nothing was
    /// published for them.
    #[error("the staged file {key} changed after the version was assembled")]
    StagingChanged {
        /// The staging key.
        key: String,
    },
    /// Another writer changed a key of this version, or the root inventory,
    /// while this commit held its reservation.
    #[error("{key} was changed by another writer during the commit")]
    ForeignWrite {
        /// The storage key.
        key: String,
    },
    /// The committed head has no version inventory (OCFL allows that, W010),
    /// or its version inventory has no sidecar (an interrupted repair).
    /// The writer refuses such a base: a crash between its root inventory
    /// and root sidecar could not be read back, because readers find the
    /// committed version through that version inventory. The consented
    /// [`ObjectWriter::repair_version_inventory`](super::ObjectWriter::repair_version_inventory)
    /// lifts the refusal (OCFL-S-020).
    #[error("the committed head has no version inventory at {key}")]
    MissingVersionInventory {
        /// The missing key.
        key: String,
    },
    /// The Git guard refused the object root (OCFL-S-001).
    #[error(transparent)]
    Guard(OcflError),
    /// The committed object could not be read.
    #[error(transparent)]
    Read(ReadError),
    /// A storage backend failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl From<ReadError> for CommitError {
    /// Keep a storage failure met while reading as [`CommitError::Storage`].
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::Storage(storage) => Self::Storage(storage),
            other => Self::Read(other),
        }
    }
}

impl From<OcflError> for CommitError {
    /// Keep a Git refusal as [`CommitError::Guard`]; a storage failure met
    /// by the guard is a [`CommitError::Storage`].
    fn from(err: OcflError) -> Self {
        match err {
            OcflError::Storage(storage) => Self::Storage(storage),
            git => Self::Guard(git),
        }
    }
}

impl CommitError {
    /// An invalid draft, for `reason`.
    pub(crate) fn draft(reason: impl Into<String>) -> Self {
        Self::InvalidDraft {
            reason: reason.into(),
        }
    }
}

impl From<FormatError> for CommitError {
    /// A version that would break an OCFL rule is an invalid draft; an
    /// inventory above the engine's parsing bound (no OCFL code) is not the
    /// draft's fault and stays a format error.
    fn from(err: FormatError) -> Self {
        match err {
            FormatError::Invalid { .. } => Self::draft(err.to_string()),
            other => Self::Read(ReadError::Format(other)),
        }
    }
}

/// Why a version-inventory repair (OCFL-S-020) wrote nothing more.
///
/// Every refusal is decided before the repair's first write. Only
/// [`RepairError::Reservation`], [`RepairError::ForeignWrite`] and
/// [`RepairError::Storage`] can stop a repair between its two writes; the
/// object then holds the head version inventory without its sidecar, which
/// leaves the logical state unchanged and is completed by running the repair
/// again.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepairError {
    /// The object could not be reserved, or the reservation was lost
    /// before a write (expired, released or reassigned).
    #[error(transparent)]
    Reservation(#[from] ReservationError),
    /// The root sidecar is missing, malformed or does not match the root
    /// inventory, so the root inventory is not a committed head to copy.
    #[error("the root sidecar {key} does not match the root inventory")]
    RootSidecarMismatch {
        /// The root sidecar's key.
        key: String,
    },
    /// The root inventory or its sidecar changed between the first read
    /// and the read under the reservation.
    #[error("{key} changed while the repair was being reserved")]
    RootChanged {
        /// The key that changed.
        key: String,
    },
    /// The validator reports errors other than the missing version sidecar
    /// of an interrupted repair; content digest errors included.
    #[error("the object has validation errors: {}", distinct_codes(errors))]
    Invalid {
        /// The errors that refuse the repair.
        errors: Vec<Finding>,
    },
    /// The head version directory is not a head without a version
    /// inventory, nor an interrupted repair: its version inventory differs
    /// from the root inventory, its version sidecar differs from the root
    /// sidecar or has no inventory, or it holds a file the head's inventory
    /// does not list.
    #[error("{key} cannot be repaired: {reason}")]
    NotRepairable {
        /// The key that refuses the repair.
        key: String,
        /// Why.
        reason: String,
    },
    /// The head already holds its version inventory and sidecar, each
    /// byte-identical to the root files: the object is not W010, or a
    /// repair already completed.
    #[error("the head already holds its version inventory at {key}")]
    NothingToRepair {
        /// The head version inventory's key.
        key: String,
    },
    /// A file the repair writes appeared during the repair.
    #[error("{key} was written by another writer during the repair")]
    ForeignWrite {
        /// The storage key.
        key: String,
    },
    /// The Git guard refused the object root (OCFL-S-001).
    #[error(transparent)]
    Guard(OcflError),
    /// The object could not be read or validated.
    #[error(transparent)]
    Read(ReadError),
    /// A storage backend failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// The distinct codes of `errors`, sorted, for the message.
fn distinct_codes(errors: &[Finding]) -> String {
    let codes: std::collections::BTreeSet<&str> = errors.iter().map(|f| f.code).collect();
    codes.into_iter().collect::<Vec<_>>().join(", ")
}

impl From<ReadError> for RepairError {
    /// Keep a storage failure met while reading as [`RepairError::Storage`].
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::Storage(storage) => Self::Storage(storage),
            other => Self::Read(other),
        }
    }
}

impl From<OcflError> for RepairError {
    /// Keep a Git refusal as [`RepairError::Guard`]; a storage failure met
    /// by the guard is a [`RepairError::Storage`].
    fn from(err: OcflError) -> Self {
        match err {
            OcflError::Storage(storage) => Self::Storage(storage),
            git => Self::Guard(git),
        }
    }
}
