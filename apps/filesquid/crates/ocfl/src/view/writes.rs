//! Version-producing writes of the view API (OCFL-S-012, OCFL-S-016): add,
//! rename or move, remove and restore. Each one stages its change in the
//! change model's staging area and publishes exactly one version through
//! [`ChangeModel::commit`] or [`ChangeModel::reinstate`]; there is no
//! second write path. A head without its version inventory (W010) is
//! refused by the change model and never repaired here (OCFL-S-020).
//!
//! A view write commits only its own change: it is refused while the
//! staging area holds other changes, and a write that fails leaves the
//! staging area empty again. OCFL stores no empty folder, so creating a
//! folder is not a write: the UI keeps it pending until a file is added
//! beneath its prefix.

use std::future::Future;

use super::error::ViewError;
use super::{check_path, check_prefix};
use crate::change::{state_digests, ChangeError, ChangeModel, VersionMeta, WorkingState};
use crate::inventory::VersionNum;
use crate::storage::StorageBackend;
use crate::write::{CommitReceipt, VersionReservations};

/// Add `bytes` as the logical file `path` (or replace its content) in one
/// new version.
///
/// # Errors
///
/// [`ViewError::StagingNotEmpty`], [`ViewError::InvalidRequest`] for an
/// invalid path, or [`ViewError::Change`] for a refusal of the change model
/// (a conflict, a W010 head, nothing to commit).
pub async fn add_file<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    path: &str,
    bytes: &[u8],
    meta: VersionMeta,
) -> Result<CommitReceipt, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    check_path(path)?;
    require_clean(model).await?;
    staged_commit(model, meta, async {
        Ok(model.stage_bytes(path, bytes).await?)
    })
    .await
}

/// Rename or move the logical file or folder `from` to `to` in one new
/// version. A folder moves every file below it; no bytes are written.
///
/// # Errors
///
/// [`ViewError::PathNotFound`] when the head has no file or folder `from`,
/// or as [`add_file`].
pub async fn move_path<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    from: &str,
    to: &str,
    meta: VersionMeta,
) -> Result<CommitReceipt, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let from = check_prefix(from)?;
    let to = check_prefix(to)?;
    require_clean(model).await?;
    let moves = targets(&head_state(model).await?, &from)?;
    staged_commit(model, meta, async {
        for path in &moves {
            let rest = &path[from.len()..];
            model.stage_rename(path, &format!("{to}{rest}")).await?;
        }
        Ok(())
    })
    .await
}

/// Remove the logical file or folder `path` in one new version. Its bytes
/// stay in the object for history and [`restore`].
///
/// # Errors
///
/// As [`move_path`].
pub async fn remove_path<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    path: &str,
    meta: VersionMeta,
) -> Result<CommitReceipt, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let path = check_prefix(path)?;
    require_clean(model).await?;
    let removals = targets(&head_state(model).await?, &path)?;
    staged_commit(model, meta, async {
        for file in &removals {
            model.stage_remove(file).await?;
        }
        Ok(())
    })
    .await
}

/// Restore each of `paths` (a file, or a folder with every file below it)
/// to its content in the earlier version `version`, in one new version
/// that points at existing manifest entries ([`ChangeModel::reinstate`]).
///
/// # Errors
///
/// [`ViewError::PathNotFound`] when `version` has no such file or folder,
/// or as [`add_file`].
pub async fn restore<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    version: &VersionNum,
    paths: &[&str],
    meta: VersionMeta,
) -> Result<CommitReceipt, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    require_clean(model).await?;
    let inventory = model.committed().await?;
    let source = state_digests(inventory.as_ref(), version)?;
    let mut files = Vec::new();
    for path in paths {
        files.extend(targets(&source, &check_prefix(path)?)?);
    }
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    Ok(model.reinstate(version, &refs, meta).await?)
}

/// Refuse a staging area that holds changes; clear one that holds none, so
/// the write starts from the current head.
async fn require_clean<B, R, S>(model: &ChangeModel<'_, B, R, S>) -> Result<(), ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    if !model.status().await?.changes.is_empty() {
        return Err(ViewError::StagingNotEmpty);
    }
    Ok(model.reset().await?)
}

/// The head's logical state (empty for an object with no version).
async fn head_state<B, R, S>(model: &ChangeModel<'_, B, R, S>) -> Result<WorkingState, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let Some(inventory) = model.committed().await? else {
        return Ok(WorkingState::new());
    };
    Ok(state_digests(Some(&inventory), &inventory.head)?)
}

/// The file `path` itself, or every file below the folder `path`.
fn targets(state: &WorkingState, path: &str) -> Result<Vec<String>, ViewError> {
    if state.contains_key(path) {
        return Ok(vec![path.to_string()]);
    }
    let below = format!("{path}/");
    let files: Vec<String> = state
        .keys()
        .filter(|p| p.starts_with(&below))
        .cloned()
        .collect();
    if files.is_empty() {
        return Err(ViewError::PathNotFound {
            path: path.to_string(),
        });
    }
    Ok(files)
}

/// Run `stage`, then commit; on any failure before publication, reset the
/// staging area so it is empty again. A publication whose staging area was
/// kept is still a success: the area is reset and the receipt returned.
async fn staged_commit<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    meta: VersionMeta,
    stage: impl Future<Output = Result<(), ViewError>>,
) -> Result<CommitReceipt, ViewError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let outcome = match stage.await {
        Ok(()) => model.commit(meta).await.map_err(ViewError::from),
        Err(err) => Err(err),
    };
    let receipt = match outcome {
        Ok(receipt) => return Ok(receipt),
        Err(ViewError::Change(ChangeError::StagingNotCleared { receipt, .. })) => Ok(*receipt),
        Err(err) => Err(err),
    };
    if let Err(reset) = model.reset().await {
        tracing::warn!(error = %reset, "could not clear the staging area after a view write");
    }
    receipt
}
