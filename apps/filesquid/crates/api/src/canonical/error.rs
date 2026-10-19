//! Failures of the FileSquid application profile's canonical writes and reads.

use crate::library::CatalogError;
use filesquid_ocfl::change::ChangeError;
use filesquid_ocfl::write::{CommitError, ReservationError};
use filesquid_ocfl::ReadError;

/// The detail of every storage failure: a backend message can name a filesystem path, which
/// MUST NOT reach the API (DE-S-006), so only this fixed text is passed on.
const STORAGE: &str = "storage unavailable";

/// A failure to encode, commit or read a library's canonical OCFL state. Details name
/// logical paths and engine codes, never a filesystem path or a title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalError {
    /// An identifier cannot name a logical path (it is empty or holds a character outside
    /// `[A-Za-z0-9._-]`).
    InvalidIdentifier(String),
    /// An entity could not be encoded.
    Encode(String),
    /// A logical file of the snapshot is not a valid entity.
    Decode {
        /// The logical path.
        path: String,
        /// Why it was refused.
        reason: String,
    },
    /// The object's head moved under the commit, or an intervening version touched the same
    /// entity; nothing was written.
    Conflict,
    /// The head has no version inventory of its own (W010). Commits on it are refused until
    /// the owner consents to the repair (OCFL-S-018, OCFL-S-020); it is never repaired here.
    HeadWithoutInventory,
    /// The storage or the reservation store is unreachable now; a retry may succeed.
    Unavailable(String),
    /// The engine refused the commit or the read; a retry will not help.
    Failed(String),
}

impl CanonicalError {
    /// The error's stable code, for journals and logs.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "invalid_identifier",
            Self::Encode(_) => "encode_failed",
            Self::Decode { .. } => "snapshot_invalid",
            Self::Conflict => "conflict",
            Self::HeadWithoutInventory => "head_without_inventory",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "internal",
        }
    }
}

impl From<ChangeError> for CanonicalError {
    /// Map a change-model failure: a moved head or a conflicting intervening version is a
    /// conflict, a W010 head its own refusal, storage and reservation trouble unavailable.
    fn from(err: ChangeError) -> Self {
        match err {
            ChangeError::Conflict { .. } | ChangeError::TooManyRebases { .. } => Self::Conflict,
            ChangeError::Commit(commit) => commit.into(),
            ChangeError::Storage(_) => Self::Unavailable(STORAGE.into()),
            ChangeError::Read(e) => e.into(),
            other => Self::Failed(other.to_string()),
        }
    }
}

impl From<CommitError> for CanonicalError {
    /// Map a writer failure (see [`From<ChangeError>`]).
    fn from(err: CommitError) -> Self {
        match err {
            CommitError::HeadMoved { .. } => Self::Conflict,
            CommitError::MissingVersionInventory { .. } => Self::HeadWithoutInventory,
            CommitError::Reservation(e) => reservation(e),
            CommitError::Storage(_) => Self::Unavailable(STORAGE.into()),
            other => Self::Failed(other.to_string()),
        }
    }
}

/// A reservation failure: a held or lost lease, an unreadable clock or a storage failure may
/// clear on retry; a corrupt record or an invalid configuration will not.
fn reservation(err: ReservationError) -> CanonicalError {
    match err {
        ReservationError::Held { .. }
        | ReservationError::Lost { .. }
        | ReservationError::ClockUnavailable
        | ReservationError::Storage(_) => CanonicalError::Unavailable("version reservation".into()),
        other => CanonicalError::Failed(format!("version reservation: {other}")),
    }
}

impl From<ReadError> for CanonicalError {
    /// Map a read failure: storage trouble is unavailable, anything else a failure.
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::Storage(_) => Self::Unavailable(STORAGE.into()),
            other => Self::Failed(other.to_string()),
        }
    }
}

impl From<CanonicalError> for CatalogError {
    /// The catalog port's view: conflicts stay conflicts, unavailability stays retryable, and
    /// everything else (a W010 head included) is a failure with its code in the detail.
    fn from(err: CanonicalError) -> Self {
        match err {
            CanonicalError::Conflict => CatalogError::Conflict,
            CanonicalError::Unavailable(detail) => CatalogError::Unavailable(detail),
            other => CatalogError::Failed(format!("canonical {}: {other:?}", other.code())),
        }
    }
}
