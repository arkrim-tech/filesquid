//! Helpers for change-model tests (IO-0.10) on top of the write rig: a
//! [`ChangeModel`] per writer, each with its own staging prefix on the rig's
//! staging backend, outside the storage root.

#![allow(dead_code)]

use std::collections::BTreeMap;

use filesquid_ocfl::change::{
    ChangeError, ChangeModel, PathChange, StagingArea, Status, VersionMeta,
};
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::CommitReceipt;

use crate::fault::FaultBackend;
use crate::rig::{ready, v, Leases, Rig, OBJECT_ID, ROOT};

/// The change model type the rig hands out.
pub type Model<'r, B> = ChangeModel<'r, FaultBackend<B>, Leases, MemoryBackend>;

/// A change model for `holder`, staging under `changes/<holder>`.
pub fn model<'r, B: StorageBackend>(rig: &'r Rig<B>, holder: &str) -> Model<'r, B> {
    let staging = StagingArea::new(&*rig.staging, &format!("changes/{holder}")).expect("prefix");
    ChangeModel::new(&rig.storage, &*rig.leases, holder, ROOT, OBJECT_ID, staging)
}

/// Version metadata created now, with `message`.
pub fn meta<B: StorageBackend>(rig: &Rig<B>, message: &str) -> VersionMeta {
    VersionMeta::new(rig.now()).with_message(message)
}

/// Stage every `(path, bytes)` and commit with `message`.
pub fn commit_files<B: StorageBackend>(
    model: &Model<'_, B>,
    rig: &Rig<B>,
    files: &[(&str, &[u8])],
    message: &str,
) -> Result<CommitReceipt, ChangeError> {
    for (path, bytes) in files {
        ready(model.stage_bytes(path, bytes))?;
    }
    ready(model.commit(meta(rig, message)))
}

/// The status of `model`.
pub fn status<B: StorageBackend>(model: &Model<'_, B>) -> Status {
    ready(model.status()).expect("status")
}

/// The logical state of `version` as path → digest.
pub fn state_of<B: StorageBackend>(rig: &Rig<B>, version: &str) -> BTreeMap<String, String> {
    let object = rig.open().expect("open");
    let state = object.state(&v(version)).expect("state");
    state
        .files()
        .map(|f| (f.path.clone(), f.digest.clone()))
        .collect()
}

/// Every key the rig's staging backend holds under `prefix`.
pub fn staged_keys(rig: &Rig<impl StorageBackend>, prefix: &str) -> Vec<String> {
    let page = ready(rig.staging.list(prefix, None, 1_000)).expect("list staging");
    page.keys.iter().map(|k| k.as_str().to_string()).collect()
}

/// The paths of `changes`, each with a short kind label, in order.
pub fn labels(changes: &[PathChange]) -> Vec<String> {
    changes
        .iter()
        .map(|change| match change {
            PathChange::Added { path, .. } => format!("added {path}"),
            PathChange::Changed { path, .. } => format!("changed {path}"),
            PathChange::Renamed { from, to, .. } => format!("renamed {from} -> {to}"),
            PathChange::Deleted { path, .. } => format!("deleted {path}"),
            PathChange::Reinstated { path, .. } => format!("reinstated {path}"),
        })
        .collect()
}
