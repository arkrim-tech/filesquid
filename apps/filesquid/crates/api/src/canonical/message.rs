//! The version message that names the operation a snapshot version publishes. It carries
//! the operation's receipt, so a replay of the operation ID answers its first outcome even
//! when the catalog was lost and rebuilt from the snapshot. Every function is pure.

use crate::library::Deletion;
use serde::{Deserialize, Serialize};

/// Prefix of every operation message.
pub const OPERATION_PREFIX: &str = "filesquid.operation.v1 ";

/// What one published operation answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRecord {
    /// The operation's idempotency key.
    pub operation_id: String,
    /// The library revision after the operation.
    pub revision: String,
    /// Identifier the operation created, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_id: Option<String>,
    /// Deletion times of files the operation newly deleted, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Deletion>,
}

/// The version message of `record`.
pub fn operation_message(record: &OperationRecord) -> String {
    let json = serde_json::to_string(record).unwrap_or_else(|_| "{}".into());
    format!("{OPERATION_PREFIX}{json}")
}

/// The operation a version message names: `Ok(None)` for any message without
/// [`OPERATION_PREFIX`] (a version made by another tool).
///
/// # Errors
/// A fixed reason for a prefixed message that cannot be read, so its receipt is never
/// silently dropped.
pub fn parse_operation_message(message: &str) -> Result<Option<OperationRecord>, String> {
    let Some(json) = message.strip_prefix(OPERATION_PREFIX) else {
        return Ok(None);
    };
    serde_json::from_str(json)
        .map(Some)
        .map_err(|_| "unreadable operation message".to_string())
}
