//! The PROV-O history of a catalog's libraries for the API (Task FS-2.8 host wiring; API-S-029,
//! ADR 040 Decision 7).
//!
//! - [`CatalogRoots`]: the OCFL storage roots of the libraries the catalog serves, listed
//!   again for every read, which the host hands to `filesquid-api`'s
//!   [`OcflRootsHistory`](filesquid_api::history::OcflRootsHistory) for the history routes.
//! - [`SqliteCatalog::backfill_history`]: the API's history [`Backfill`] job over every
//!   library, run under the catalog's lock and the library's own version reservations, so it
//!   never races a catalog commit. After a run that committed records the catalog tries to
//!   reconcile with the new snapshot head straight away (the provenance-only version changes
//!   no entity); if that fails it is logged as a warning and the next open reconciles. The job is resumable by rescanning: a version that has
//!   a record is skipped, and a second call commits nothing.

use super::LOG_TARGET;
use super::{error_code, reconcile, reconcile_library, OpenLibrary, SqliteCatalog};
use crate::locations::OpenedRoots;
use filesquid_api::history::backfill::{Backfill, BackfillError, BackfillReport};
use filesquid_api::history::{HistoryError, HistoryRoots, RootResult};
use filesquid_api::library::CatalogError;
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::FsBackend;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Most backfill runs one library gets per call. Each run reconstructs at most the job's
/// record cap; a run that reached it is followed by another until the library is done, this
/// cap is reached or the call is cancelled. What is left is filled by the next call.
pub const MAX_BACKFILL_RUNS: usize = 100;

/// The OCFL storage roots of the libraries a catalog serves, for the history routes.
#[derive(Clone, Debug)]
pub struct CatalogRoots(pub Arc<SqliteCatalog>);

impl HistoryRoots for CatalogRoots {
    type Backend = FsBackend;

    /// A reader of each served library's storage root (filesystem I/O). A library whose
    /// root cannot be opened is answered as unavailable on its own, so the others are still
    /// read; every failure is logged as a warning with its code only.
    fn roots(&self) -> Result<Vec<RootResult<FsBackend>>, HistoryError> {
        let roots = self.0.history_roots().map_err(roots_unavailable)?;
        Ok(roots.into_iter().map(Ok).collect())
    }
}

/// Every OCFL storage root the desktop serves, for the history routes: the catalog's
/// libraries ([`CatalogRoots`]) and the libraries opened in folders the owner chose
/// ([`OpenedRoots`], Task DE-1.1), listed again for every read.
#[derive(Clone, Debug)]
pub struct HostRoots {
    /// The catalog's libraries.
    pub catalog: CatalogRoots,
    /// The libraries opened in owner-chosen folders.
    pub opened: Arc<OpenedRoots>,
}

impl HistoryRoots for HostRoots {
    type Backend = FsBackend;

    /// The catalog's roots (filesystem I/O), then the opened folders' roots, leaving out an
    /// opened library the catalog already serves, so no library is read twice.
    fn roots(&self) -> Result<Vec<RootResult<FsBackend>>, HistoryError> {
        let mut roots = self.catalog.roots()?;
        let served = self.catalog.0.library_ids().map_err(roots_unavailable)?;
        let opened = self
            .opened
            .list()
            .map_err(|_| roots_unavailable(CatalogError::Unavailable("opened roots".into())))?;
        let extra = opened
            .into_iter()
            .filter(|(id, _)| !served.contains(id))
            .map(|(_, root)| Ok(root.backend));
        roots.extend(extra);
        Ok(roots)
    }
}

/// A failure to list or open the libraries' storage roots, logged as a warning with its code
/// only (no path), as the history's unavailable answer.
fn roots_unavailable(err: CatalogError) -> HistoryError {
    tracing::warn!(
        target: LOG_TARGET,
        operation = "library.history.roots",
        error_code = error_code(&err),
        "a library's storage root could not be opened for the history"
    );
    HistoryError::Unavailable
}

/// What the backfill did for one library.
#[derive(Debug)]
pub struct LibraryBackfill {
    /// The library.
    pub library_id: LibraryId,
    /// The report of each run, in order (each run registers and journals itself).
    pub reports: Vec<BackfillReport>,
    /// Why a run could not start, if one could not; the runs before it are kept.
    pub error: Option<BackfillError>,
}

impl SqliteCatalog {
    /// The OCFL storage root of every library this catalog serves (at most
    /// [`super::MAX_LIBRARIES`]): the owner's folders, through the backends the file routes
    /// share. No path leaves this crate.
    ///
    /// # Errors
    /// [`CatalogError`] when the lock is poisoned.
    pub fn history_roots(&self) -> Result<Vec<Arc<FsBackend>>, CatalogError> {
        let libraries = self.lock()?;
        Ok(libraries
            .values()
            .map(|library| library.canonical.backend())
            .collect())
    }

    /// Run `backfill` over every library this catalog serves, one library at a time under
    /// the catalog's lock, until each is done, [`MAX_BACKFILL_RUNS`] runs were made for it,
    /// or `cancel` is set (checked between runs and inside each run between objects).
    /// Filesystem, database and journal I/O.
    ///
    /// # Errors
    /// [`CatalogError`] when the lock is poisoned before any library was listed.
    pub fn backfill_history(
        &self,
        backfill: &Backfill,
        cancel: &AtomicBool,
    ) -> Result<Vec<LibraryBackfill>, CatalogError> {
        let done = self
            .library_ids()?
            .into_iter()
            .take_while(|_| !cancel.load(Ordering::SeqCst))
            .map(|id| self.backfill_library(id, backfill, cancel))
            .collect();
        Ok(done)
    }

    /// The libraries this catalog serves, in ID order (at most [`super::MAX_LIBRARIES`]).
    ///
    /// # Errors
    /// [`CatalogError`] when the lock is poisoned.
    pub fn library_ids(&self) -> Result<Vec<LibraryId>, CatalogError> {
        Ok(self.lock()?.values().map(|l| l.id.clone()).collect())
    }

    /// Run `backfill` over `library_id` until it is done, capped by [`MAX_BACKFILL_RUNS`],
    /// cancelled (the last report says so) or a run cannot start (a library no longer
    /// served included). Filesystem, database and journal I/O.
    pub fn backfill_library(
        &self,
        library_id: LibraryId,
        backfill: &Backfill,
        cancel: &AtomicBool,
    ) -> LibraryBackfill {
        let mut done = LibraryBackfill {
            library_id,
            reports: Vec::new(),
            error: None,
        };
        for _ in 0..MAX_BACKFILL_RUNS {
            match self.backfill_once(&done.library_id, backfill, cancel) {
                Ok(report) => {
                    let again = report.reached_cap && !report.cancelled;
                    done.reports.push(report);
                    if !again {
                        break;
                    }
                }
                Err(err) => {
                    done.error = Some(err);
                    break;
                }
            }
        }
        done
    }

    /// One run over `library_id` under the catalog's lock; when it committed records, bring
    /// the catalog up to the new head. Filesystem, database and journal I/O.
    ///
    /// # Errors
    /// [`BackfillError::Unavailable`] when the lock is poisoned, the library is no longer
    /// served, or the run cannot start.
    fn backfill_once(
        &self,
        library_id: &LibraryId,
        backfill: &Backfill,
        cancel: &AtomicBool,
    ) -> Result<BackfillReport, BackfillError> {
        let mut libraries = self.lock().map_err(|_| BackfillError::Unavailable)?;
        let library = libraries
            .get_mut(library_id.as_str())
            .ok_or(BackfillError::Unavailable)?;
        let report = library.canonical.backfill(backfill, cancel)?;
        if !report.objects.is_empty() {
            self.catch_up(library);
        }
        Ok(report)
    }

    /// Reconcile `library`'s catalog with its snapshot head after a backfill commit, so the
    /// catalog reflects the head it published, and forward the replayed journal entries
    /// (their pre-allocated IDs keep each stored once). A failure is logged as a warning (the
    /// journal layer journals it); the next open reconciles instead. Filesystem, database and
    /// journal I/O.
    fn catch_up(&self, library: &mut OpenLibrary) {
        if let Err(err) = reconcile_library(&mut library.conn, &library.canonical, &library.id) {
            tracing::warn!(
                target: LOG_TARGET,
                operation = reconcile::RECONCILE_OPERATION,
                error_code = error_code(&err),
                "a library was not reconciled after the history backfill; its next open will"
            );
        }
        self.forward_logged(&library.conn);
    }
}
