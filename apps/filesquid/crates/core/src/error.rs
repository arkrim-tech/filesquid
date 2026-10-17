//! Domain errors for portable contract construction.

use serde::{Deserialize, Serialize};

/// Failure when constructing an opaque identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidOpaqueId {
    /// The identifier string was empty.
    Empty,
}

impl std::fmt::Display for InvalidOpaqueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "opaque identifier must be a non-empty string"),
        }
    }
}

impl std::error::Error for InvalidOpaqueId {}

/// Failure when constructing or updating a portable domain value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainError {
    /// A display name or title was empty.
    EmptyName,
    /// A file already has a different home collection.
    SecondHomeCollection,
    /// Original file resource bytes cannot be replaced.
    OriginalImmutable,
    /// A SHA-256 digest was not 64 hexadecimal characters.
    InvalidDigest,
    /// Page size was zero or above the portable maximum.
    InvalidPageSize,
    /// A command scope or expected revision was empty.
    EmptyScope,
    /// An HKDF purpose label was registered twice.
    DuplicateHkdfPurpose,
    /// An RFC 3339 UTC timestamp was malformed.
    InvalidTimestamp,
    /// A signed C2PA manifest cannot be mutated in place.
    SignedManifestImmutable,
    /// A capability register had no rows.
    EmptyCapabilitySet,
    /// A mandatory capability declared no supported formats.
    EmptySupportedSet,
    /// Active transfers exceeded the default cap of four.
    TransferCapExceeded,
    /// A replacement lease token was not strictly greater than the current token.
    NonIncreasingLease,
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "name must be a non-empty string"),
            Self::SecondHomeCollection => {
                write!(f, "a file's bytes live in exactly one collection")
            }
            Self::OriginalImmutable => write!(f, "original file resources are immutable"),
            Self::InvalidDigest => {
                write!(f, "SHA-256 digest must be 64 hexadecimal characters")
            }
            Self::InvalidPageSize => {
                write!(f, "page size must be between 1 and 1000 inclusive")
            }
            Self::EmptyScope => write!(f, "scope and expected revision must be non-empty"),
            Self::DuplicateHkdfPurpose => {
                write!(f, "HKDF info labels must not be reused across purposes")
            }
            Self::InvalidTimestamp => write!(f, "timestamp must be RFC 3339 UTC ending in Z"),
            Self::SignedManifestImmutable => {
                write!(f, "signed C2PA manifests are immutable")
            }
            Self::EmptyCapabilitySet => write!(f, "capability register rows must be nonempty"),
            Self::EmptySupportedSet => {
                write!(f, "a capability register cannot be all unsupported")
            }
            Self::TransferCapExceeded => write!(f, "default active transfers must be at most 4"),
            Self::NonIncreasingLease => {
                write!(f, "lease fencing token must strictly increase")
            }
        }
    }
}

impl std::error::Error for DomainError {}
