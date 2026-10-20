//! A fact that is either known or listed as unavailable with a reason.
//!
//! Every field of the import record is a [`Fact`] where a source or host can
//! fail to provide it, so a missing value is never silent and never guessed
//! ([ADR 038](../../../../resources/architecture/038-import-file-record.md)
//! Decision 3, MD-S-013).

use serde::{Deserialize, Serialize};

/// Why a fact is not in the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The source system or its filesystem has no such fact.
    NotSupportedBySource,
    /// The operating system refused access to it.
    PermissionDenied,
    /// An upload carries only a name, size, time and media type.
    NotProvidedByUpload,
    /// The outgoing disclosure policy removed it (an identity field, F-046).
    WithheldByPolicy,
}

/// The listed form of a missing fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unavailable {
    /// Why the fact is missing.
    pub unavailable: Reason,
    /// Optional fixed detail, for example an operating-system error name.
    /// Never a stored value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A known value or the reason it is unavailable.
///
/// Serialized as the value itself, or as `{"unavailable": <reason>}` with an
/// optional `detail`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fact<T> {
    /// The fact is listed as unavailable.
    Unavailable(Unavailable),
    /// The fact as the source or host provided it.
    Value(T),
}

impl<T> Fact<T> {
    /// A known fact.
    pub fn value(value: T) -> Self {
        Self::Value(value)
    }

    /// An unavailable fact with its `reason`.
    pub fn unavailable(reason: Reason) -> Self {
        Self::Unavailable(Unavailable {
            unavailable: reason,
            detail: None,
        })
    }

    /// An unavailable fact with its `reason` and a fixed `detail`.
    pub fn unavailable_because(reason: Reason, detail: &str) -> Self {
        Self::Unavailable(Unavailable {
            unavailable: reason,
            detail: Some(detail.to_string()),
        })
    }

    /// The value, when known.
    pub fn as_value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }
}
