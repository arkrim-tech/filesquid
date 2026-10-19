//! Version-bound logical files (OCFL-S-024): files whose bytes depend on
//! the version they are committed in, such as a record that names its own
//! version and `created` time.
//!
//! The caller hands a commit a [`VersionBoundFiles`] function. The writer
//! calls it after it has reserved `vN` and before it writes any `vN` byte,
//! with a [`BoundContext`] naming the reserved version, the `created` time
//! the inventory will carry and the logical states before and after the
//! change. It digests the returned bytes and writes them like any other
//! new content; when the function fails, nothing is published. A rebased
//! commit reserves a new version, so the function is called again with the
//! new name. The engine gives the files no meaning of its own.

use std::collections::BTreeMap;

use super::error::CommitError;
use super::plan::{manifest_key, Upload, UploadSource, VersionPlan};
use crate::digest::DigestAlgorithm;
use crate::inventory::{DigestMap, Inventory, VersionNum};
use crate::storage::{MaybeSync, StorageKey, MAX_PUT_BYTES};

/// Most version-bound files one commit may add.
pub const MAX_BOUND_FILES: usize = 64;

/// What a [`VersionBoundFiles`] function is told about the version it
/// writes into.
#[derive(Debug, Clone, Copy)]
pub struct BoundContext<'a> {
    /// The object identifier.
    pub object_id: &'a str,
    /// The reserved version: the name the inventory will give it.
    pub version: &'a VersionNum,
    /// The version's `created` value, as the inventory will carry it.
    pub created: &'a str,
    /// The version it follows (`None` for a new object's first version).
    pub base: Option<&'a VersionNum>,
    /// The logical state of `base` (logical path → digest; empty for a new
    /// object).
    pub base_state: &'a BTreeMap<String, String>,
    /// The new version's logical state before the bound files are added
    /// (logical path → digest).
    pub state: &'a BTreeMap<String, String>,
}

/// A caller-supplied function that returns the logical files bound to the
/// version being committed (logical path → bytes).
///
/// Paths MUST be new to the version's state (a bound file never replaces a
/// staged one), at most [`MAX_BOUND_FILES`] per commit and each at most
/// [`MAX_PUT_BYTES`] bytes. An `Err` stops the commit before anything is
/// written; its text is carried in [`CommitError::BoundFiles`].
pub trait VersionBoundFiles: MaybeSync {
    /// The bound files of the version `context` describes.
    ///
    /// # Errors
    ///
    /// A reason, when the files cannot be produced.
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String>;
}

impl<F> VersionBoundFiles for F
where
    F: Fn(&BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> + MaybeSync,
{
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        self(context)
    }
}

/// Call `bound` for `plan`'s reserved version and return the plan with its
/// files added to the version's state, the manifest and the uploads.
///
/// # Errors
///
/// [`CommitError::BoundFiles`] when the function fails, returns too many
/// files, a file over the size bound, or a path the state already holds;
/// [`CommitError::InvalidDraft`] when a path cannot be a logical path or
/// the extended inventory is invalid.
pub(crate) fn bind_files(
    plan: &VersionPlan,
    bound: &dyn VersionBoundFiles,
) -> Result<VersionPlan, CommitError> {
    let base = previous_version(&plan.inventory, &plan.version);
    let base_state = flat_state(&plan.inventory, base.as_ref());
    let state = flat_state(&plan.inventory, Some(&plan.version));
    let created = head_created(plan);
    let context = BoundContext {
        object_id: plan.object_id(),
        version: &plan.version,
        created: &created,
        base: base.as_ref(),
        base_state: &base_state,
        state: &state,
    };
    let files = bound.files(&context).map_err(bound_error)?;
    check_files(&files, &state)?;
    extend_plan(plan, files)
}

/// The version before `version` in `inventory`, if any.
fn previous_version(inventory: &Inventory, version: &VersionNum) -> Option<VersionNum> {
    inventory
        .versions
        .range(..version.clone())
        .next_back()
        .map(|(name, _)| name.clone())
}

/// The logical state of `version` as logical path → digest.
fn flat_state(inventory: &Inventory, version: Option<&VersionNum>) -> BTreeMap<String, String> {
    let Some(block) = version.and_then(|v| inventory.versions.get(v)) else {
        return BTreeMap::new();
    };
    flatten(&block.state)
}

/// A digest → paths map as path → digest.
fn flatten(state: &DigestMap) -> BTreeMap<String, String> {
    state
        .iter()
        .flat_map(|(digest, paths)| paths.iter().map(move |path| (path.clone(), digest.clone())))
        .collect()
}

/// The `created` value of the plan's new version.
fn head_created(plan: &VersionPlan) -> String {
    plan.inventory
        .versions
        .get(&plan.version)
        .map(|block| block.created.clone())
        .unwrap_or_default()
}

/// Refuse more than [`MAX_BOUND_FILES`] files, a file over
/// [`MAX_PUT_BYTES`] or a path the state already holds.
fn check_files(
    files: &BTreeMap<String, Vec<u8>>,
    state: &BTreeMap<String, String>,
) -> Result<(), CommitError> {
    if files.len() > MAX_BOUND_FILES {
        return Err(bound_error(format!(
            "{} files exceed the bound of {MAX_BOUND_FILES}",
            files.len()
        )));
    }
    for (path, bytes) in files {
        if bytes.len() as u64 > MAX_PUT_BYTES {
            return Err(bound_error(format!("{path} exceeds {MAX_PUT_BYTES} bytes")));
        }
        if state.contains_key(path) {
            return Err(bound_error(format!("{path} is already in the version")));
        }
    }
    Ok(())
}

/// The plan with `files` added: each path joins the state under its digest,
/// and bytes the manifest lacks become inline uploads under `vN/`.
fn extend_plan(
    plan: &VersionPlan,
    files: BTreeMap<String, Vec<u8>>,
) -> Result<VersionPlan, CommitError> {
    let mut plan = plan.clone();
    let content_dir = plan.inventory.content_directory().to_string();
    for (path, bytes) in files {
        let computed = DigestAlgorithm::ENGINE.digest_hex(&bytes);
        let manifest = &plan.inventory.manifest;
        let digest = manifest_key(manifest, &computed)
            .cloned()
            .unwrap_or(computed);
        if !plan.inventory.manifest.contains_key(&digest) {
            let content_path = format!("{}/{content_dir}/{path}", plan.version);
            StorageKey::new(&content_path)
                .map_err(|err| CommitError::draft(format!("{path}: {err}")))?;
            plan.inventory
                .manifest
                .insert(digest.clone(), vec![content_path.clone()]);
            plan.uploads.push(Upload {
                content_path,
                source: UploadSource::Inline(bytes),
                digest: digest.clone(),
            });
        }
        let block = plan
            .inventory
            .versions
            .get_mut(&plan.version)
            .ok_or_else(|| CommitError::draft("the plan has no new version"))?;
        block.state.entry(digest).or_default().push(path);
    }
    plan.uploads
        .sort_by(|a, b| a.content_path.cmp(&b.content_path));
    plan.bytes = plan.inventory.to_json();
    Inventory::parse(&plan.bytes)?;
    Ok(plan)
}

/// A [`CommitError::BoundFiles`] carrying `reason`.
fn bound_error(reason: impl Into<String>) -> CommitError {
    CommitError::BoundFiles {
        reason: reason.into(),
    }
}
