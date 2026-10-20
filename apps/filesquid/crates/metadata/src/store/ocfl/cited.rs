//! The commit-time check of an import's cited content (MD-S-013).
//!
//! An import entry cites the logical path and SHA-256 of the stored
//! original. The store checks them before it commits, but a metadata-only
//! change set (a re-import) stages only the record, so the change model
//! rebases it past any version that changed the original without changing
//! the record (OCFL-S-016). [`CitedContent`] repeats the check inside the
//! commit, as a version-bound-files function (OCFL-S-024): the writer calls
//! it after it has reserved the version, with the exact state it will
//! publish, and again after every rebase. A mismatch stops the commit before
//! any version byte is written.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use filesquid_ocfl::write::{BoundContext, VersionBoundFiles};

use crate::file_record::FileRecord;
use crate::store::StoreError;

/// A [`VersionBoundFiles`] function that refuses a version whose state no
/// longer holds the cited content, then returns the caller's own bound files.
pub(super) struct CitedContent<'g> {
    /// The cited logical paths and their SHA-256 digests.
    cited: Vec<(String, String)>,
    /// The caller's own bound files (MD-S-014), if any.
    inner: Option<&'g dyn VersionBoundFiles>,
    /// Why the check refused the version, once it has.
    refused: OnceLock<String>,
}

impl<'g> CitedContent<'g> {
    /// The check of the managed entries of `record` from `first_new` on,
    /// wrapping the caller's `inner` bound files.
    pub(super) fn new(
        record: &FileRecord,
        first_new: usize,
        inner: Option<&'g dyn VersionBoundFiles>,
    ) -> Self {
        Self {
            cited: cited_paths(record, first_new),
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
}

impl VersionBoundFiles for CitedContent<'_> {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        let moved = self
            .cited
            .iter()
            .find(|(path, digest)| context.state.get(path) != Some(digest));
        if let Some((path, _)) = moved {
            let reason = format!(
                "{path} no longer holds the content the import entry cites in {}",
                context.version
            );
            tracing::warn!(%path, version = %context.version, "refused an import commit: the cited content changed");
            self.refused.get_or_init(|| reason.clone());
            return Err(reason);
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
