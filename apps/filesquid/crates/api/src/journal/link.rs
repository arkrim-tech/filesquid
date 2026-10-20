//! The link from a journal entry to the PROV-O activity of its operation (API-S-031; ADR 033
//! Decision 7; ADR 040).
//!
//! An entry of a committed operation names the operation ID and the object version
//! (`<object-id>@vN`) its PROV-O activity carries, so it links to that version's record
//! (`GET /api/v1/files/{id}/versions/{version}/prov` for a FileSquid object
//! `urn:uuid:<id>`) and to every activity of the operation (`GET /api/v1/history`). Failed
//! and refused entries, entries without a succeeded outcome or without both identifiers, and
//! the `file.download` entries of single-file downloads have no activity and are journal-only.

use filesquid_core::VersionRef;
use serde::{Deserialize, Serialize};

use super::{is_object_ref, is_opaque, JournalEntry, Outcome};
use crate::API_PREFIX;

/// Operations journaled without a PROV-O record even when they succeed with an object
/// reference: a single-file download (owner decision 2026-10-19).
pub const JOURNAL_ONLY_OPERATIONS: [&str; 1] = ["file.download"];

/// Prefix of FileSquid object IDs, whose records the file history routes serve.
const URN_UUID: &str = "urn:uuid:";

/// Where an entry's PROV-O activity is, or that it has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceLink {
    /// True when the entry has no PROV-O activity: it stays in the journal only.
    pub journal_only: bool,
    /// The operation ID the activity carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// The object version the activity is about, `<object-id>@vN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_ref: Option<String>,
    /// API path of that version's PROV-O record, for a FileSquid object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// API path of every activity of the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activities: Option<String>,
}

impl ProvenanceLink {
    /// The link of an entry without an activity.
    pub fn journal_only() -> Self {
        Self {
            journal_only: true,
            operation_id: None,
            object_ref: None,
            record: None,
            activities: None,
        }
    }
}

/// The PROV-O link of `entry`. Pure.
pub fn provenance_of(entry: &JournalEntry) -> ProvenanceLink {
    let (Some(operation_id), Some(object_ref)) = (&entry.operation_id, &entry.object_ref) else {
        return ProvenanceLink::journal_only();
    };
    let committed = entry.outcome == Outcome::Succeeded
        && !JOURNAL_ONLY_OPERATIONS.contains(&entry.operation.as_str())
        && is_opaque(operation_id)
        && is_object_ref(object_ref);
    if !committed {
        return ProvenanceLink::journal_only();
    }
    ProvenanceLink {
        journal_only: false,
        operation_id: Some(operation_id.clone()),
        object_ref: Some(object_ref.clone()),
        record: record_path(object_ref),
        activities: Some(format!("{API_PREFIX}/history?operation={operation_id}")),
    }
}

/// The record path of `object_ref` when its object is `urn:uuid:<id>`.
fn record_path(object_ref: &str) -> Option<String> {
    let reference = VersionRef::parse_object_ref(object_ref).ok()?;
    let id = reference.object_id().strip_prefix(URN_UUID)?;
    Some(format!(
        "{API_PREFIX}/files/{id}/versions/{}/prov",
        reference.version()
    ))
}
