//! Listing every key under one version directory, bounded and checked.
//!
//! Pages may be short, so the walk is bounded by keys
//! ([`MAX_WALK_ENTRIES`]), not by pages. Each continuation must be the
//! page's last key and move past the previous one
//! ([`next_start`](crate::guard::next_start)), so a faulty backend cannot
//! make the writer skip a key it must reconcile or verify.

use super::error::CommitError;
use crate::guard::next_start;
use crate::inventory::VersionNum;
use crate::read::small::child;
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE, MAX_WALK_ENTRIES};

/// Most listing requests for one version directory: every page that
/// carries a continuation holds at least one new key, so the key bound also
/// bounds the requests.
const MAX_REQUESTS: usize = MAX_WALK_ENTRIES + 1;

/// Every key under `<root>/<version>/`, in lexical order.
///
/// # Errors
///
/// [`CommitError::Storage`] for a backend error, a faulty continuation, or
/// more than [`MAX_WALK_ENTRIES`] keys.
pub(crate) async fn list_version_keys<B: StorageBackend>(
    storage: &B,
    root: &str,
    version: &VersionNum,
) -> Result<Vec<StorageKey>, CommitError> {
    let prefix = format!("{}/", child(root, &version.to_string()));
    let mut keys = Vec::new();
    let mut after: Option<StorageKey> = None;
    for _ in 0..MAX_REQUESTS {
        let page = storage.list(&prefix, after.as_ref(), MAX_LIST_PAGE).await?;
        if keys.len().saturating_add(page.keys.len()) > MAX_WALK_ENTRIES {
            return Err(over_bound("keys listed for one version", MAX_WALK_ENTRIES));
        }
        keys.extend(page.keys.iter().cloned());
        match next_start(&page, after.as_ref())? {
            None => return Ok(keys),
            Some(next) => after = Some(next),
        }
    }
    Err(over_bound("listing requests for one version", MAX_REQUESTS))
}

/// The error for a listing above `limit`.
fn over_bound(what: &'static str, limit: usize) -> CommitError {
    StorageError::LimitExceeded {
        what,
        limit: limit as u64,
    }
    .into()
}
