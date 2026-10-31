//! The expiry of deleted files (Task API-F2.15; API-S-019, ADR 041 Decisions 8 and 11).
//!
//! When a deleted file's `purge_after` has passed, one run per library expires it as a
//! recorded `purge` operation: [`LibraryService::expire_library`] commits one catalog version
//! under one operation ID that removes every due file from the catalog and the collection
//! snapshot ([`CatalogChange::ExpireFile`]) together with its journal entry (opaque IDs
//! only), so the purge is journaled with, and before, any deletion of bytes. In the same
//! transaction the host records each file's **object work**, which the injected
//! [`ObjectPurger`] then does: under `delete_all_version_history` it deletes the file's OCFL
//! object; under `keep_version_history` (the default) it rewrites the object as a tombstone
//! that keeps only the version history, the PROV-O records and a purge record. Object work
//! that fails is journaled as `failed` and stays recorded, so the next run (or the recovery
//! at launch) completes it; it is never lost and never repeated once done.
//!
//! A library that is not open cannot be reached: a file whose `purge_after` passed while its
//! library was closed is expired by the first run after the library is opened. Each run is
//! bounded: it reads at most [`MAX_EXPIRY_SCAN`] deleted files and expires at most
//! [`MAX_EXPIRY_FILES`] of them, finishing at most [`MAX_PENDING_PURGES`] recorded object
//! works first; what remains waits for the next run.

use super::catalog::{CatalogChange, CatalogCommit, CatalogError, FileListing, FileScope, PageKey};
use super::policy::{ExpiryMode, VisibilityFilter};
use super::service::LibraryService;
use super::{mint_uuid_v4, LibraryError};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::operation::{ExpectedRevision, OperationId};
use filesquid_core::{FileId, LibraryId};

/// Journal operation of an expiry run's commit: the recorded `purge` of the due files.
pub const EXPIRY_OPERATION: &str = "library.file.expire";

/// Journal operation of one file's object work (deletion or tombstone) that failed.
pub const EXPIRY_OBJECT_OPERATION: &str = "library.file.expire.object";

/// Durable job kind of the expiry runs (API-S-033).
pub const EXPIRY_JOB_KIND: &str = "library.expiry";

/// Most files one run expires in one library.
pub const MAX_EXPIRY_FILES: usize = 1_000;

/// Most deleted files one run reads in one library to find the due ones.
pub const MAX_EXPIRY_SCAN: usize = 10_000;

/// Most recorded object works one run finishes in one library.
pub const MAX_PENDING_PURGES: usize = 1_000;

/// Deleted files read per catalog page.
const SCAN_PAGE: usize = 1_000;

/// One expired file's object work that a host recorded and has not finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPurge {
    /// The expired file.
    pub file_id: FileId,
    /// The `purge` operation that expired it.
    pub operation_id: String,
    /// What to do with its object.
    pub mode: ExpiryMode,
    /// When the purge expired it (Unix milliseconds, UTC): the time the tombstone records for
    /// the original's deletion, however late the object work runs.
    pub purged_at_ms: i64,
}

/// The host's object work of expired files.
pub trait ObjectPurger: Send + Sync {
    /// The recorded, unfinished object works of `library_id`, at most `limit`, oldest first.
    ///
    /// # Errors
    /// A store failure.
    fn pending_purges(
        &self,
        library_id: &LibraryId,
        limit: usize,
    ) -> Result<Vec<PendingPurge>, CatalogError>;

    /// Do `purge`'s object work for `actor` and forget it once done: delete the object, or
    /// rewrite it as a tombstone. Doing it again after a crash completes it; a purge already
    /// done changes nothing.
    ///
    /// # Errors
    /// A store failure; the work stays recorded for the next run.
    fn purge_object(
        &self,
        library_id: &LibraryId,
        purge: &PendingPurge,
        actor: &str,
    ) -> Result<(), CatalogError>;
}

/// What one run did in one library.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpiryReport {
    /// Files the run expired (removed from the catalog and the snapshot).
    pub expired: usize,
    /// Object works finished (this run's and earlier ones').
    pub objects_done: usize,
    /// Object works that failed and wait for the next run.
    pub objects_failed: usize,
    /// The run's `purge` operation ID, when it expired a file.
    pub operation_id: Option<String>,
}

impl LibraryService {
    /// Expire the due deleted files of `library_id` for `actor` (`system` for the scheduled
    /// job, the admin when started by hand) under the installation's expiry setting, then do
    /// their object work through `purger`. Recorded object works of earlier runs are finished
    /// first. A run with nothing due commits and journals nothing.
    ///
    /// # Errors
    /// `not_found` for a library that is not open; `conflict` when the library changed during
    /// the run (the next run tries again); `unavailable` or `internal` when a store fails.
    pub fn expire_library(
        &self,
        actor: &str,
        library_id: &LibraryId,
        purger: &dyn ObjectPurger,
    ) -> Result<ExpiryReport, LibraryError> {
        let mut report = ExpiryReport::default();
        self.finish_pending(actor, library_id, purger, None, &mut report)?;
        let mode = self.installation_settings()?.expiry;
        let due = self.due_files(library_id)?;
        if due.is_empty() {
            return Ok(report);
        }
        let operation_id = self.commit_expiry(actor, library_id, &due, mode)?;
        report.expired = due.len();
        self.finish_pending(actor, library_id, purger, Some(&operation_id), &mut report)?;
        report.operation_id = Some(operation_id);
        Ok(report)
    }

    /// Do the recorded object works of `library_id` (only those of `operation`, when named,
    /// so work an earlier pass of this run tried is not tried twice), within one budget of
    /// [`MAX_PENDING_PURGES`] per run, counting what was done and what failed; each failure
    /// is journaled once as `failed`.
    fn finish_pending(
        &self,
        actor: &str,
        library_id: &LibraryId,
        purger: &dyn ObjectPurger,
        operation: Option<&str>,
        report: &mut ExpiryReport,
    ) -> Result<(), LibraryError> {
        let spent = report.objects_done + report.objects_failed;
        let budget = MAX_PENDING_PURGES.saturating_sub(spent);
        let pending = purger.pending_purges(library_id, MAX_PENDING_PURGES)?;
        let mine = pending
            .into_iter()
            .filter(|purge| operation.is_none_or(|op| purge.operation_id == op))
            .take(budget);
        for purge in mine {
            match purger.purge_object(library_id, &purge, actor) {
                Ok(()) => report.objects_done += 1,
                Err(err) => {
                    report.objects_failed += 1;
                    self.journal_object_failure(actor, library_id, &purge, &err);
                }
            }
        }
        Ok(())
    }

    /// Journal one failed object work: opaque IDs and the error code, never a path.
    fn journal_object_failure(
        &self,
        actor: &str,
        library_id: &LibraryId,
        purge: &PendingPurge,
        err: &CatalogError,
    ) {
        let error: LibraryError = err.clone().into();
        let entry = JournalEntry::new(EXPIRY_OBJECT_OPERATION, Outcome::Failed)
            .actor(actor)
            .level(Level::Warn)
            .error_code(error.error_code())
            .operation_id(purge.operation_id.as_str())
            .target(library_id.as_str())
            .target(purge.file_id.as_str())
            .at_ms(self.now_ms());
        self.send_failure(entry);
    }

    /// The deleted files of `library_id` whose `purge_after` has passed, at most
    /// [`MAX_EXPIRY_FILES`], from at most [`MAX_EXPIRY_SCAN`] deleted files read.
    fn due_files(&self, library_id: &LibraryId) -> Result<Vec<FileId>, LibraryError> {
        let now = self.now_ms();
        let listing = FileListing {
            library_id: library_id.clone(),
            scope: FileScope::Library,
            visibility: VisibilityFilter::Deleted,
        };
        let mut due = Vec::new();
        let mut after: Option<PageKey> = None;
        for _ in 0..MAX_EXPIRY_SCAN / SCAN_PAGE {
            let page = self.catalog().files(&listing, after.as_ref(), SCAN_PAGE)?;
            let full = page.len() == SCAN_PAGE;
            for keyed in page {
                after = Some(keyed.key);
                let passed = keyed
                    .item
                    .state
                    .deletion
                    .is_some_and(|d| d.purge_after_ms <= now);
                if passed {
                    due.push(keyed.item.file.id().clone());
                }
                if due.len() == MAX_EXPIRY_FILES {
                    return Ok(due);
                }
            }
            if !full {
                break;
            }
        }
        Ok(due)
    }

    /// Commit the expiry of `due` under a new operation ID with its journal entry (the library
    /// and the files as targets); answer the operation ID.
    fn commit_expiry(
        &self,
        actor: &str,
        library_id: &LibraryId,
        due: &[FileId],
        mode: ExpiryMode,
    ) -> Result<String, LibraryError> {
        let state = self
            .catalog()
            .library(library_id)?
            .ok_or(LibraryError::NotFound("library"))?;
        let operation_id = mint_uuid_v4()?;
        let id = OperationId::new(operation_id.clone()).map_err(|_| LibraryError::Internal)?;
        let expected = ExpectedRevision::new(state.revision).map_err(|_| LibraryError::Internal)?;
        let journal = due.iter().fold(
            JournalEntry::new(EXPIRY_OPERATION, Outcome::Succeeded)
                .actor(actor)
                .target(library_id.as_str()),
            |entry, file| entry.target(file.as_str()),
        );
        let changes = due
            .iter()
            .map(|file_id| CatalogChange::ExpireFile {
                file_id: file_id.clone(),
                mode,
            })
            .collect();
        self.catalog().commit(CatalogCommit {
            operation_id: id,
            library_id: library_id.clone(),
            expected_revision: expected,
            changes,
            journal: journal
                .operation_id(operation_id.as_str())
                .at_ms(self.now_ms()),
            created_id: None,
            deletion: None,
            restored_into: Vec::new(),
        })?;
        self.announce_everything();
        Ok(operation_id)
    }
}
