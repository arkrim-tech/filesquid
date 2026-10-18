//! Visible replica coverage (ADR 008, T-022): whether every index shard behind an answer is
//! present. An incomplete replica says so; it never presents partial data as the whole.

use super::ReplicaError;
use serde::{Deserialize, Serialize};

/// Most shards one replica view may be split into.
pub const MAX_SHARDS: u32 = 4096;

/// How much of the replica backs an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "coverage", rename_all = "snake_case")]
pub enum Coverage {
    /// Every shard is loaded.
    Complete,
    /// Some shards are still to load; the answer covers only the loaded ones.
    Incomplete {
        /// Shards loaded so far.
        loaded: u32,
        /// Shards in the whole view.
        total: u32,
    },
}

impl Coverage {
    /// The coverage of `loaded` out of `total` shards: [`Coverage::Complete`] only when all
    /// are loaded.
    ///
    /// # Errors
    ///
    /// Returns `invalid_coverage` when `total` is 0 or above [`MAX_SHARDS`], or `loaded` is
    /// above `total`.
    pub fn from_shards(loaded: u32, total: u32) -> Result<Self, ReplicaError> {
        if total == 0 || total > MAX_SHARDS || loaded > total {
            return Err(ReplicaError::invalid(
                "invalid_coverage",
                "The stored view reports an impossible number of loaded parts.",
            ));
        }
        if loaded == total {
            return Ok(Self::Complete);
        }
        Ok(Self::Incomplete { loaded, total })
    }
}
