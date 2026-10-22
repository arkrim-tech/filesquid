//! The commit-time check of an import's citation (MD-S-008, MD-S-013).
//!
//! An import entry cites the OCFL version, logical path and SHA-256 of the
//! stored original. The store checks the content before it commits, but a
//! metadata-only change set (a re-import) stages only the record, so the
//! change model rebases it past any version that changed the original
//! without changing the record (OCFL-S-016). [`CitedContent`] repeats the
//! content check inside the commit, as a version-bound-files function
//! (OCFL-S-024): the writer calls it after it has reserved the version, with
//! the exact state it will publish, and again after every rebase. A mismatch
//! stops the commit before any version byte is written.
//!
//! The same function checks that every new entry cites the version being
//! committed (owner decision 2026-10-20, [ADR 038](../../../../../../../resources/architecture/038-import-file-record.md)
//! Decision 10). A rebase changes the reserved version, so an import commit
//! that would rebase fails as a retryable conflict that publishes nothing:
//! the approved exception to the rebase rule of MD-S-008, for import commits
//! only. Statement edits do not use this function and still rebase.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use filesquid_ocfl::write::{BoundContext, VersionBoundFiles};

use crate::file_record::FileRecord;
use crate::store::StoreError;

/// A [`VersionBoundFiles`] function that refuses a version other than the
/// one the new entries cite, or whose state no longer holds the cited
/// content, then returns the caller's own bound files.
pub(super) struct CitedContent<'g> {
    /// The cited logical paths and their SHA-256 digests.
    cited: Vec<(String, String)>,
    /// The OCFL version each new entry cites, in entry order.
    versions: Vec<String>,
    /// The caller's own bound files (MD-S-014), if any.
    inner: Option<&'g dyn VersionBoundFiles>,
    /// Why the check refused the version, once it has.
    refused: OnceLock<String>,
}

impl<'g> CitedContent<'g> {
    /// The check of the entries of `record` from `first_new` on, wrapping
    /// the caller's `inner` bound files.
    pub(super) fn new(
        record: &FileRecord,
        first_new: usize,
        inner: Option<&'g dyn VersionBoundFiles>,
    ) -> Self {
        Self {
            cited: cited_paths(record, first_new),
            versions: cited_versions(record, first_new),
            inner,
            refused: OnceLock::new(),
        }
    }

    /// The conflict to report when the check refused the version, else
    /// `err` unchanged.
    pub(super) fn explain(&self, err: StoreError) -> StoreError {
        match self.refused.get() {
            Some(reason) => StoreError::Conflict {
                reason: reason.clone(),
            },
            None => err,
        }
    }

    /// Keep `reason` as the refusal and return it as the function's error.
    fn refuse(&self, reason: String) -> Result<BTreeMap<String, Vec<u8>>, String> {
        self.refused.get_or_init(|| reason.clone());
        Err(reason)
    }

    /// Why the version `context` describes no longer holds the cited
    /// content, if it does not.
    fn moved_content(&self, context: &BoundContext<'_>) -> Option<String> {
        let (path, _) = self
            .cited
            .iter()
            .find(|(path, digest)| context.state.get(path) != Some(digest))?;
        tracing::warn!(%path, version = %context.version, "refused an import commit: the cited content changed");
        Some(format!(
            "{path} no longer holds the content the import entry cites in {}",
            context.version
        ))
    }

    /// Why the version `context` describes is not the one a new entry
    /// cites, if it is not.
    fn other_version(&self, context: &BoundContext<'_>) -> Option<String> {
        let committing = context.version.to_string();
        let cited = self.versions.iter().find(|cited| **cited != committing)?;
        tracing::warn!(%cited, version = %committing, "refused an import commit: the cited version is not the version being committed");
        Some(format!(
            "the import entry cites {cited}, but the commit would publish {committing}; retry citing the new version"
        ))
    }
}

impl VersionBoundFiles for CitedContent<'_> {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        if let Some(reason) = self.moved_content(context) {
            return self.refuse(reason);
        }
        if let Some(reason) = self.other_version(context) {
            return self.refuse(reason);
        }
        self.inner
            .map_or_else(|| Ok(BTreeMap::new()), |inner| inner.files(context))
    }
}

/// The logical path and SHA-256 each managed entry from `first_new` on cites.
fn cited_paths(record: &FileRecord, first_new: usize) -> Vec<(String, String)> {
    record
        .imports()
        .iter()
        .skip(first_new)
        .filter_map(|entry| {
            let cited = &entry.acquisition;
            let path = cited.logical_path.clone()?;
            Some((path, cited.sha256.clone()))
        })
        .collect()
}

/// The OCFL version each entry from `first_new` on cites.
fn cited_versions(record: &FileRecord, first_new: usize) -> Vec<String> {
    record
        .imports()
        .iter()
        .skip(first_new)
        .map(|entry| entry.acquisition.object_version.clone())
        .collect()
}
