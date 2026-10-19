//! Error types with fixed codes (PROV-S-004).
//!
//! Every error carries a stable code such as `PROV_TOO_LARGE`, and its message
//! starts with that code. Messages name only fixed text and field names, never
//! a value taken from a record or from caller facts, so an error can be
//! journaled or shown without leaking stored content.

/// Why a stored record was refused by [`crate::parse_record`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProvError {
    /// The record is larger than [`crate::MAX_RECORD_BYTES`].
    #[error("PROV_TOO_LARGE: the record is larger than 1 MiB")]
    TooLarge,
    /// The record nests JSON arrays or objects deeper than [`crate::MAX_DEPTH`].
    #[error("PROV_TOO_DEEP: the record nests JSON deeper than 16 levels")]
    TooDeep,
    /// One JSON object of the record repeats a member name.
    #[error("PROV_DUPLICATE_KEY: an object of the record repeats a member name")]
    DuplicateKey,
    /// The record names no profile this reader knows, or no profile at all.
    #[error("PROV_UNKNOWN_PROFILE: the record names no profile this reader knows")]
    UnknownProfile,
    /// A node or member of the record breaks its profile.
    #[error("PROV_INVALID_NODE: the record breaks its profile ({reason})")]
    InvalidNode {
        /// Fixed description of the broken rule; never a stored value.
        reason: &'static str,
    },
    /// The record is not well-formed UTF-8 JSON.
    #[error("PROV_INVALID_JSON: the record is not well-formed UTF-8 JSON")]
    InvalidJson,
}

impl ProvError {
    /// Return the fixed error code, for example `PROV_TOO_DEEP`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TooLarge => "PROV_TOO_LARGE",
            Self::TooDeep => "PROV_TOO_DEEP",
            Self::DuplicateKey => "PROV_DUPLICATE_KEY",
            Self::UnknownProfile => "PROV_UNKNOWN_PROFILE",
            Self::InvalidNode { .. } => "PROV_INVALID_NODE",
            Self::InvalidJson => "PROV_INVALID_JSON",
        }
    }
}

/// Why [`crate::build_record`] refused a set of version facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// A fact is missing or malformed; `field` names the [`crate::VersionFacts`] field.
    #[error("PROV_INVALID_FACT: the fact `{field}` is missing or malformed")]
    InvalidFact {
        /// Name of the offending field of [`crate::VersionFacts`].
        field: &'static str,
    },
    /// A move names a source that is not in the base state only, or a target
    /// that is not in the new state only, or two moves share a target.
    #[error("PROV_INVALID_MOVE: a move does not match the base and new states")]
    InvalidMove,
    /// The started time is after the `created` (end) time.
    #[error("PROV_TIME_ORDER: the activity starts after the version was created")]
    TimeOrder,
    /// The record would be larger than [`crate::MAX_RECORD_BYTES`], so no reader could read it.
    #[error("PROV_TOO_LARGE: the record would be larger than 1 MiB")]
    TooLarge,
    /// The facts produce a graph that breaks the profile.
    #[error("PROV_INCONSISTENT_FACTS: the facts break the profile ({reason})")]
    Inconsistent {
        /// Fixed description of the broken rule; never a fact value.
        reason: &'static str,
    },
}

impl BuildError {
    /// Return the fixed error code, for example `PROV_INVALID_MOVE`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidFact { .. } => "PROV_INVALID_FACT",
            Self::InvalidMove => "PROV_INVALID_MOVE",
            Self::TimeOrder => "PROV_TIME_ORDER",
            Self::TooLarge => "PROV_TOO_LARGE",
            Self::Inconsistent { .. } => "PROV_INCONSISTENT_FACTS",
        }
    }
}

/// Why the [`crate::HistoryIndex`] refused a request or a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    /// A page size outside 1 to [`crate::MAX_PAGE_SIZE`] was asked for.
    #[error("PROV_PAGE_SIZE: a page holds 1 to 1000 activities")]
    PageSize,
    /// A cursor token is not one the index issued.
    #[error("PROV_INVALID_CURSOR: the cursor is not a history index cursor")]
    InvalidCursor,
    /// A different record with the same activity identifier is already indexed.
    #[error("PROV_CONFLICTING_RECORD: another record holds the same activity identifier")]
    ConflictingRecord,
}

impl IndexError {
    /// Return the fixed error code, for example `PROV_PAGE_SIZE`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::PageSize => "PROV_PAGE_SIZE",
            Self::InvalidCursor => "PROV_INVALID_CURSOR",
            Self::ConflictingRecord => "PROV_CONFLICTING_RECORD",
        }
    }
}

/// A scalar value (timestamp, digest, UUID) is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("PROV_INVALID_VALUE: the value is not a valid {what}")]
pub struct InvalidValue {
    /// What the value should have been, for example `RFC 3339 UTC timestamp`.
    pub what: &'static str,
}
