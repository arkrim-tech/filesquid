//! Errors returned by every storage backend.

use thiserror::Error;

/// A storage backend failure. Every backend maps its native errors onto these
/// variants so the engine handles them the same way on every host.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    /// The key or listing prefix breaks a key rule (see
    /// [`StorageKey`](super::StorageKey)).
    #[error("invalid storage key {key:?}: {reason}")]
    InvalidKey {
        /// The rejected key or prefix.
        key: String,
        /// The rule it breaks.
        reason: &'static str,
    },
    /// No object is stored at the key.
    #[error("no object is stored at {key}")]
    NotFound {
        /// The missing key.
        key: String,
    },
    /// A compare-and-swap precondition did not hold; nothing was written.
    #[error("precondition failed for {key}: the object changed or already exists")]
    PreconditionFailed {
        /// The key whose precondition failed.
        key: String,
    },
    /// A size or count is above one of the bounds such as
    /// [`MAX_CHUNK_BYTES`](super::MAX_CHUNK_BYTES).
    #[error("{what} exceeds the limit of {limit}")]
    LimitExceeded {
        /// What was too large.
        what: &'static str,
        /// The bound that applies.
        limit: u64,
    },
    /// An argument is outside its allowed range or order.
    #[error("invalid argument: {reason}")]
    InvalidArgument {
        /// What is wrong with the argument.
        reason: String,
    },
    /// The multipart upload is unknown, completed or aborted.
    #[error("unknown multipart upload {upload_id}")]
    UnknownUpload {
        /// The upload identifier.
        upload_id: String,
    },
    /// A completed part is missing or its entity tag does not match the
    /// stored bytes.
    #[error("multipart part {part_number} is missing or does not match its entity tag")]
    PartMismatch {
        /// The 1-based part number.
        part_number: u32,
    },
    /// The backend was configured in a way the contract forbids.
    #[error("invalid backend configuration: {reason}")]
    InvalidConfig {
        /// What is wrong with the configuration.
        reason: String,
    },
    /// An operating-system I/O error.
    #[error("I/O error on {key}: {message}")]
    Io {
        /// The key or path the operation touched.
        key: String,
        /// The I/O error kind.
        kind: std::io::ErrorKind,
        /// The operating-system message.
        message: String,
    },
    /// Any other backend failure.
    #[error("storage backend error: {reason}")]
    Backend {
        /// The backend's description of the failure.
        reason: String,
    },
}
