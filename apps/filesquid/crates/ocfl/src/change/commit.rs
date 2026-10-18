//! Publishing a change set: commit, revert and reinstate, each through one
//! [`ObjectWriter::commit`](crate::write::ObjectWriter::commit) under the
//! fenced `vN` reservation and head compare-and-swap (OCFL-S-016).
//!
//! When the object's head is no longer the change set's base, the change
//! set is rebased onto the new head if no intervening version changed a
//! staged logical path, and refused as a [`ChangeError::Conflict`]
//! otherwise, without publishing. The same check runs again when the head
//! moves between this read and the writer's reservation, at most
//! [`MAX_REBASES`] times.

use std::collections::BTreeSet;
use std::ops::Bound;

use super::delta::{state_digests, WorkingState};
use super::error::ChangeError;
use super::index::{Staged, StagingIndex};
use super::model::{base_state, known, ChangeModel, VersionMeta};
use crate::inventory::{Inventory, VersionNum};
use crate::storage::StorageBackend;
use crate::write::{CommitError, CommitReceipt, VersionDraft, VersionReservations};

/// Most times one change set is rebased before the commit gives up.
pub const MAX_REBASES: usize = 16;

impl<B, R, S> ChangeModel<'_, B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Publish everything staged as exactly one new version, then clear
    /// the staging area. Only bytes whose digest the object's `manifest`
    /// lacks are written (OCFL-S-014).
    ///
    /// # Errors
    ///
    /// [`ChangeError::NothingToCommit`] when the staged changes leave the
    /// state as it is; [`ChangeError::Conflict`] when the head moved and an
    /// intervening version changed a staged path;
    /// [`ChangeError::TooManyRebases`]; [`ChangeError::Commit`] for a
    /// refusal of the write path, such as
    /// [`CommitError::MissingVersionInventory`] for a head without its
    /// version inventory (W010), which is never repaired here
    /// (OCFL-S-020); or a read or backend error. On every error nothing is
    /// published and the staging area is kept.
    pub async fn commit(&self, meta: VersionMeta) -> Result<CommitReceipt, ChangeError> {
        let (_, index) = self.load().await?;
        let receipt = self.publish(index, &meta).await?;
        if let Err(err) = self.staging.clear().await {
            tracing::warn!(error = %err, "published, but could not clear the staging area");
        }
        Ok(receipt)
    }

    /// Publish a new version whose logical state is that of the earlier
    /// version `version`, pointing every path at existing `manifest`
    /// entries. Earlier versions are not modified or removed, and the
    /// staging area is not used.
    ///
    /// # Errors
    ///
    /// As [`commit`](Self::commit), and [`ChangeError::Read`] for a
    /// version the object lacks.
    pub async fn revert(
        &self,
        version: &VersionNum,
        meta: VersionMeta,
    ) -> Result<CommitReceipt, ChangeError> {
        let (inventory, head, head_state) = self.head_state().await?;
        let target = state_digests(Some(&inventory), version)?;
        let mut index = StagingIndex::new(&self.object_id, Some(head));
        for path in head_state.keys().filter(|p| !target.contains_key(*p)) {
            index.set(&head_state, path, Staged::Removed);
        }
        for (path, digest) in target {
            index.set(&head_state, &path, Staged::File(digest));
        }
        self.publish(index, &meta).await
    }

    /// Publish a new version that restores each of `paths` to its content
    /// in the earlier version `version`; every other path keeps its
    /// content from the head. No bytes are written, earlier versions are
    /// not modified or removed, and the staging area is not used.
    ///
    /// # Errors
    ///
    /// [`ChangeError::PathNotFound`] for a path `version` does not hold,
    /// or as [`revert`](Self::revert).
    pub async fn reinstate(
        &self,
        version: &VersionNum,
        paths: &[&str],
        meta: VersionMeta,
    ) -> Result<CommitReceipt, ChangeError> {
        let (inventory, head, head_state) = self.head_state().await?;
        let source = state_digests(Some(&inventory), version)?;
        let mut index = StagingIndex::new(&self.object_id, Some(head));
        for path in paths {
            let digest = source.get(*path).ok_or_else(|| ChangeError::PathNotFound {
                path: (*path).to_string(),
                version: Some(version.to_string()),
            })?;
            index.set(&head_state, path, Staged::File(digest.clone()));
        }
        self.publish(index, &meta).await
    }

    /// The committed inventory, its head and the head's logical state.
    ///
    /// # Errors
    ///
    /// [`ChangeError::Read`] when no version is committed, or as
    /// [`committed`](Self::committed).
    async fn head_state(&self) -> Result<(Inventory, VersionNum, WorkingState), ChangeError> {
        let inventory = self.committed().await?;
        let inventory = inventory.ok_or_else(|| crate::read::ReadError::VersionNotFound {
            version: VersionNum::first().to_string(),
        })?;
        let head = inventory.head.clone();
        let state = state_digests(Some(&inventory), &head)?;
        Ok((inventory, head, state))
    }

    /// Publish `index` as one version, rebasing it while the head moves.
    async fn publish(
        &self,
        mut index: StagingIndex,
        meta: &VersionMeta,
    ) -> Result<CommitReceipt, ChangeError> {
        for _ in 0..MAX_REBASES {
            let inventory = self.committed().await?;
            rebase(&mut index, inventory.as_ref())?;
            let draft = self.draft(&index, inventory.as_ref(), meta)?;
            let staging = self.staging.backend();
            let result = self
                .writer
                .commit(&self.root, index.base.as_ref(), staging, &draft)
                .await;
            match result {
                Err(CommitError::HeadMoved { .. }) => continue,
                other => return other.map_err(ChangeError::from),
            }
        }
        Err(ChangeError::TooManyRebases { limit: MAX_REBASES })
    }

    /// The draft of the next version: the base state with every staged
    /// path applied. Content the manifest holds is kept; other content is
    /// read from the staging area's blobs.
    ///
    /// # Errors
    ///
    /// [`ChangeError::NothingToCommit`] when the state would not change.
    fn draft(
        &self,
        index: &StagingIndex,
        inventory: Option<&Inventory>,
        meta: &VersionMeta,
    ) -> Result<VersionDraft, ChangeError> {
        let base = base_state(inventory, index)?;
        let working = index.working(&base);
        if working == base {
            return Err(ChangeError::NothingToCommit);
        }
        let known = known(inventory);
        let mut draft = VersionDraft::new(self.object_id.clone(), meta.created.clone());
        for (path, digest) in working {
            if known.contains(&digest) {
                draft.keep(path, digest);
            } else {
                draft.stage(path, self.staging.blob_key(&digest)?);
            }
        }
        if let Some(message) = &meta.message {
            draft.set_message(message.clone());
        }
        if let Some(user) = &meta.user {
            draft.set_user(user.clone());
        }
        Ok(draft)
    }
}

/// Move `index` onto the committed head of `inventory` when no version
/// after its base changed a staged path.
///
/// # Errors
///
/// [`ChangeError::Conflict`] naming the staged paths that changed.
fn rebase(index: &mut StagingIndex, inventory: Option<&Inventory>) -> Result<(), ChangeError> {
    let head = inventory.map(|inv| inv.head.clone());
    if head == index.base {
        return Ok(());
    }
    let changed = changed_since_base(index, inventory)?;
    if !changed.is_empty() {
        tracing::warn!(
            paths = changed.len(),
            "the change set conflicts with a newer head"
        );
        return Err(ChangeError::Conflict {
            base: index.base.as_ref().map(ToString::to_string),
            head: head.as_ref().map(ToString::to_string),
            paths: changed.into_iter().collect(),
        });
    }
    tracing::info!(
        from = ?index.base.as_ref().map(ToString::to_string),
        to = ?head.as_ref().map(ToString::to_string),
        "rebased the change set onto the new head"
    );
    index.base = head;
    Ok(())
}

/// The staged paths whose content any version after the index's base
/// differs on from the base. Every staged path counts when the object has
/// no committed version any more.
///
/// # Errors
///
/// [`ChangeError::Read`] when the base is not a version of `inventory`.
fn changed_since_base(
    index: &StagingIndex,
    inventory: Option<&Inventory>,
) -> Result<BTreeSet<String>, ChangeError> {
    let Some(inventory) = inventory else {
        return Ok(index.paths.keys().cloned().collect());
    };
    let base = base_state(Some(inventory), index)?;
    let after = match &index.base {
        Some(version) => Bound::Excluded(version),
        None => Bound::Unbounded,
    };
    let mut changed = BTreeSet::new();
    for version in inventory
        .versions
        .range((after, Bound::Unbounded))
        .map(|(v, _)| v)
    {
        let state = state_digests(Some(inventory), version)?;
        let differs = |path: &&String| state.get(*path) != base.get(*path);
        changed.extend(index.paths.keys().filter(differs).cloned());
    }
    Ok(changed)
}
