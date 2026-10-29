//! Where an import job registers its files (Task DE-1.7; DE-S-019, API-S-036,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)):
//! the library's catalog index, behind a port so the job can be tested against any index.
//!
//! The job checks that its destination collection exists, numbers each picked source whose
//! path is taken when it makes its manifest, records the import's pending intent before its
//! first file object is committed, registers each committed file in its home collection at its
//! logical path, and clears the intent when the job ends (DE-S-010). [`SqliteCatalog`] is the
//! host's index.

use crate::catalog::{PendingIntent, RegisterFile, Registration, SqliteCatalog};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{CatalogError, LibraryCatalog};
use filesquid_api::operation::OperationId;
use filesquid_core::{CollectionId, LibraryId};

/// The catalog index an import job writes through. Every call may block on the index.
pub trait ImportIndex: Send + Sync {
    /// Whether `collection_id` is a collection of `library_id`.
    ///
    /// # Errors
    /// A message with no path when the index cannot be read.
    fn has_collection(
        &self,
        library_id: &LibraryId,
        collection_id: &CollectionId,
    ) -> Result<bool, String>;

    /// Whether `path` is taken in `collection_id` (by a file of any state, or as a folder).
    ///
    /// # Errors
    /// A message with no path when the index cannot be read.
    fn path_taken(&self, collection_id: &CollectionId, path: &str) -> Result<bool, String>;

    /// Record the import's pending intent; recording it again changes nothing.
    ///
    /// # Errors
    /// A message with no path when the intent cannot be kept.
    fn begin(&self, intent: &PendingIntent) -> Result<(), String>;

    /// Register one committed file, journaling `journal` with its version.
    ///
    /// # Errors
    /// A message with no path when the index cannot be written; a refused registration is an
    /// `Ok` answer.
    fn register(
        &self,
        library_id: &LibraryId,
        file: &RegisterFile,
        journal: JournalEntry,
    ) -> Result<Registration, String>;

    /// Clear the import's pending intent once its job has ended.
    ///
    /// # Errors
    /// A message with no path when the intent cannot be cleared.
    fn finish(&self, library_id: &LibraryId, operation_id: &OperationId) -> Result<(), String>;
}

/// A catalog failure as a message with no path.
fn message(err: CatalogError) -> String {
    format!("the library index failed ({err:?})")
}

impl ImportIndex for SqliteCatalog {
    fn has_collection(
        &self,
        library_id: &LibraryId,
        collection_id: &CollectionId,
    ) -> Result<bool, String> {
        let found = self.collection(collection_id).map_err(message)?;
        Ok(found.is_some_and(|collection| collection.library_id() == library_id))
    }

    fn path_taken(&self, collection_id: &CollectionId, path: &str) -> Result<bool, String> {
        SqliteCatalog::path_taken(self, collection_id, path).map_err(message)
    }

    fn begin(&self, intent: &PendingIntent) -> Result<(), String> {
        self.record_intent(intent).map_err(message)
    }

    fn register(
        &self,
        library_id: &LibraryId,
        file: &RegisterFile,
        journal: JournalEntry,
    ) -> Result<Registration, String> {
        self.register_file(library_id, file, journal)
            .map_err(message)
    }

    fn finish(&self, library_id: &LibraryId, operation_id: &OperationId) -> Result<(), String> {
        self.clear_intent(library_id, operation_id).map_err(message)
    }
}
