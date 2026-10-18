//! Reconciling an unpublished `vN` under a new reservation (OCFL-S-018;
//! ADR 016 "Published head (readers)": recovery MUST reconcile or replace
//! an incomplete `vN` under the same reservation and MUST NOT invent a
//! second published `vN`).
//!
//! A crashed or stale writer can leave, for the version this commit
//! reserved: content files, a version inventory and sidecar, and a root
//! inventory whose sidecar was never written. Under its reservation the
//! commit first restores the committed root inventory, then keeps each
//! content file whose bytes are exactly the bytes it would upload to that
//! path, and deletes every other key under `vN/`. Earlier versions are
//! never touched.

use std::collections::BTreeMap;

use super::base::Base;
use super::error::CommitError;
use super::fence::Fence;
use super::plan::VersionPlan;
use super::reservation::VersionReservations;
use crate::digest::DigestAlgorithm;
use crate::inventory::INVENTORY_FILE;
use crate::read::small::{child, key_in};
use crate::storage::{
    ChunkReader, ETag, Precondition, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES,
    MAX_LIST_PAGE, MAX_OBJECT_BYTES, MAX_WALK_ENTRIES,
};

/// Most listing pages one reconciliation reads.
pub(crate) const MAX_PAGES: usize = MAX_WALK_ENTRIES / MAX_LIST_PAGE + 1;

/// Most chunks one kept file is hashed in.
const MAX_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Restore the committed root inventory when the root is ahead, then
/// reconcile the keys under `vN/`. Returns the content paths kept, with
/// their entity tags.
///
/// # Errors
///
/// [`CommitError::Reservation`] when the fence fails,
/// [`CommitError::ForeignWrite`] when the root inventory changed, or a
/// backend error.
pub(crate) async fn reconcile<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    plan: &VersionPlan,
    base: &mut Base,
    fence: &Fence<'_, R>,
) -> Result<BTreeMap<String, ETag>, CommitError> {
    roll_back_root(storage, root, base, fence).await?;
    let prefix = format!("{}/", child(root, &plan.version.to_string()));
    let mut kept = BTreeMap::new();
    let mut after: Option<StorageKey> = None;
    for _ in 0..MAX_PAGES {
        let page = storage.list(&prefix, after.as_ref(), MAX_LIST_PAGE).await?;
        for key in &page.keys {
            reconcile_key(storage, root, plan, key, fence, &mut kept).await?;
        }
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(kept),
        }
    }
    Err(StorageError::LimitExceeded {
        what: "listing pages of one unpublished version",
        limit: MAX_PAGES as u64,
    }
    .into())
}

/// Keep `key` when it holds exactly the bytes the plan uploads there;
/// otherwise delete it.
async fn reconcile_key<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    plan: &VersionPlan,
    key: &StorageKey,
    fence: &Fence<'_, R>,
    kept: &mut BTreeMap<String, ETag>,
) -> Result<(), CommitError> {
    let rel = relative(root, key);
    if let Some(upload) = plan.upload(rel) {
        if let Some((digest, tag)) = stored_digest(storage, key).await? {
            if digest == upload.digest {
                kept.insert(rel.to_string(), tag);
                return Ok(());
            }
        }
    }
    fence.check().await?;
    storage.delete(key).await?;
    tracing::info!(key = %key, "removed a leftover of an unpublished version");
    Ok(())
}

/// Put the committed root inventory back when a writer replaced it but
/// never wrote its sidecar.
async fn roll_back_root<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    base: &mut Base,
    fence: &Fence<'_, R>,
) -> Result<(), CommitError> {
    let Some(committed) = base.committed.as_ref().filter(|_| base.root_is_ahead()) else {
        return Ok(());
    };
    fence.check().await?;
    let key = key_in(root, INVENTORY_FILE)?;
    let precondition = Precondition::IfMatch(committed.root_tag.clone());
    let meta = storage
        .put_if(&key, &committed.bytes, &precondition)
        .await
        .map_err(|err| foreign(err, &key))?;
    tracing::warn!(
        object = %committed.inventory.id,
        head = %committed.inventory.head,
        "restored the committed root inventory over an unpublished head"
    );
    if let Some(c) = base.committed.as_mut() {
        c.unpublished = None;
        c.root_tag = meta.etag.clone();
    }
    base.root_tag = Some(meta.etag);
    Ok(())
}

/// The sha256 digest of the bytes at `key` and the tag they were read
/// under, or `None` when the object changed during the read.
async fn stored_digest<B: StorageBackend>(
    storage: &B,
    key: &StorageKey,
) -> Result<Option<(String, ETag)>, CommitError> {
    let before = storage.head(key).await?;
    let mut reader = ChunkReader::new(storage, key.clone(), MAX_CHUNK_BYTES)?;
    let mut hasher = DigestAlgorithm::ENGINE.hasher();
    for _ in 0..MAX_CHUNKS {
        let Some(bytes) = reader.next_chunk().await? else {
            let after = storage.head(key).await?;
            let digest = hasher.finalize_hex();
            return Ok((after.etag == before.etag).then_some((digest, before.etag)));
        };
        hasher.update(&bytes);
    }
    Ok(None)
}

/// `key` relative to the object root.
pub(crate) fn relative<'k>(root: &str, key: &'k StorageKey) -> &'k str {
    if root.is_empty() {
        return key.as_str();
    }
    key.as_str()
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(key.as_str())
}

/// A failed compare-and-swap means another writer changed `key`.
pub(crate) fn foreign(err: StorageError, key: &StorageKey) -> CommitError {
    match err {
        StorageError::PreconditionFailed { .. } => CommitError::ForeignWrite {
            key: key.as_str().to_string(),
        },
        other => other.into(),
    }
}
