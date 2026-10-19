//! Errors of the change model.

use thiserror::Error;

use crate::read::ReadError;
use crate::storage::StorageError;
use crate::write::{CommitError, CommitReceipt};

/// Why a change-model operation did not complete.
///
/// A commit, revert or reinstate that fails publishes nothing: the object's
/// head is unchanged and the staging area keeps what was staged. The one
/// exception is [`ChangeError::StagingNotCleared`], returned after a
/// successful publication with its receipt when the staging area was kept.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChangeError {
    /// The object's head moved past the change set's base and an
    /// intervening version changed a logical path the change set also
    /// changes (OCFL-S-016). Nothing was published.
    #[error("conflict: {} changed between {base:?} and {head:?}", paths.join(", "))]
    Conflict {
        /// The head the change set was staged against (`None`: no version).
        base: Option<String>,
        /// The head the object has now.
        head: Option<String>,
        /// The staged logical paths the intervening versions changed.
        paths: Vec<String>,
    },
    /// The change set leaves the logical state as it is, so there is no new
    /// version to publish.
    #[error("nothing to commit: the change set leaves the logical state unchanged")]
    NothingToCommit,
    /// A logical path the operation names is not in the version it reads.
    #[error("{path} is not in {version:?}")]
    PathNotFound {
        /// The logical path.
        path: String,
        /// The version read (`None`: the staged working state).
        version: Option<String>,
    },
    /// A logical path breaks the OCFL 1.1 path rules (E052, E053).
    #[error("{path:?} is not a valid logical path: {reason}")]
    InvalidPath {
        /// The path as given.
        path: String,
        /// The rule it breaks.
        reason: String,
    },
    /// The staging index cannot be read as one.
    #[error("the staging index {key} is damaged: {reason}")]
    StagingCorrupt {
        /// The index key.
        key: String,
        /// What is wrong with it.
        reason: String,
    },
    /// The staging area holds changes for another object.
    #[error("the staging area holds changes for {found:?}, not {expected:?}")]
    StagingMismatch {
        /// The object this change model edits.
        expected: String,
        /// The object the staging index names.
        found: String,
    },
    /// The object root holds another object than the one this change model
    /// edits.
    #[error("the object root holds {found:?}, not {expected:?}")]
    WrongObject {
        /// The object this change model edits.
        expected: String,
        /// The object found.
        found: String,
    },
    /// A working file changed while it was being staged.
    #[error("the working file {key} changed while it was staged")]
    SourceChanged {
        /// The working file's key.
        key: String,
    },
    /// The head kept moving: the change set was rebased this many times
    /// without publishing.
    #[error("the head kept moving; gave up after {limit} rebases")]
    TooManyRebases {
        /// The rebase limit.
        limit: usize,
    },
    /// The version was published, but the staging area was not cleared:
    /// its index changed during the commit (nothing is deleted then), or
    /// clearing it failed. The receipt is the published version's; the
    /// caller SHOULD inspect the staging area and [`reset`] it.
    ///
    /// [`reset`]: super::ChangeModel::reset
    #[error("published {}, but the staging area was not cleared: {reason}", receipt.version)]
    StagingNotCleared {
        /// The published version's receipt.
        receipt: Box<CommitReceipt>,
        /// Why the staging area was kept.
        reason: String,
    },
    /// The write path refused or failed the commit (for example a head
    /// without its version inventory, W010, which the change model never
    /// repairs).
    #[error(transparent)]
    Commit(CommitError),
    /// The object, or a version of it, could not be read; or a staging or
    /// checkout location is inside an OCFL root.
    #[error(transparent)]
    Read(ReadError),
    /// A storage backend failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl From<ReadError> for ChangeError {
    /// Keep a storage failure met while reading as [`ChangeError::Storage`].
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::Storage(storage) => Self::Storage(storage),
            other => Self::Read(other),
        }
    }
}

impl From<CommitError> for ChangeError {
    /// Keep a storage failure met by the writer as [`ChangeError::Storage`].
    fn from(err: CommitError) -> Self {
        match err {
            CommitError::Storage(storage) => Self::Storage(storage),
            other => Self::Commit(other),
        }
    }
}
