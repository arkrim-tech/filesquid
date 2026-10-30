//! Home collections (Task API-F2.14; API-S-011 **Destination** and **Same logical path**,
//! API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 3, 4, 5 and 12): every new file has exactly one home collection and a logical
//! path inside it, which the collection snapshot records with its object under the operation
//! that made the file.
//!
//! [`HomeCollections`] is the host port the upload and copy routes register their files
//! through: the first free path for a new item (numbered beside a taken one), the file ID an
//! operation reserved (so a retry after a crash completes the same file once), and the
//! registration itself. [`MemoryHomeCollections`] is its reference implementation, for tests
//! and hosts without a catalog; `filesquid-native` implements it over the library's SQLite
//! catalog. [`destination`] holds the one destination check of the import, upload and copy
//! routes: a named collection of the library that the caller may read.

pub mod destination;
mod memory;

pub use destination::{Destination, DestinationRefusal};
pub use memory::MemoryHomeCollections;

use crate::canonical::collection_path::{ItemKind, PathError};
use crate::journal::{JournalEntry, Outcome};
use crate::library::CatalogError;
use filesquid_core::{CollectionId, FileId, LibraryId};

/// Journal operation of the snapshot version that registers a file in its home collection;
/// the import job's registrations use the same name.
pub const REGISTER_OPERATION: &str = "library.files.register";

/// One file to register in its home collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeRequest {
    /// The file, whose object `urn:uuid:<file-id>` is committed.
    pub file_id: FileId,
    /// Its home collection, a collection of the library.
    pub collection_id: CollectionId,
    /// Its collection logical path, asked for; a taken path is numbered beside it.
    pub path: String,
}

/// What became of a registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placed {
    /// Registered at `path`: the one asked for, or the first free numbered path beside it.
    Registered {
        /// The collection logical path it got.
        path: String,
    },
    /// The file was registered before (a resumed operation); nothing changed.
    AlreadyRegistered {
        /// Its collection logical path.
        path: String,
    },
    /// Refused with the error code, changing nothing: `collection_not_found`,
    /// `path_suffix_exhausted`, `name_too_long` or `invalid_path`.
    Refused(&'static str),
}

/// The host port of home collections. Every method is one store read or one atomic write and
/// may block; the routes call it on a blocking thread.
pub trait HomeCollections: Send + Sync {
    /// The library that holds the collection `collection_id`, if any library this host has
    /// open holds it. Reads only.
    ///
    /// # Errors
    /// A store failure.
    fn collection_library(
        &self,
        collection_id: &CollectionId,
    ) -> Result<Option<LibraryId>, CatalogError>;

    /// The first free collection logical path for a new item of `kind` at `path` in
    /// `collection_id` of `library_id`: `path` itself, or the first numbered path beside it
    /// (API-S-011 **Same logical path**); a path held by a file of any state, a recoverably
    /// deleted one included, is taken. Reads only.
    ///
    /// # Errors
    /// A store failure; a path the rule refuses is an `Ok(Err(..))` answer.
    fn free_path(
        &self,
        library_id: &LibraryId,
        collection_id: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<Result<String, PathError>, CatalogError>;

    /// The file ID of the new file the operation `operation_id` makes in `library_id`: the
    /// one an earlier, unfinished run of the same operation reserved, else `minted`, which is
    /// reserved until the file is registered. A retry after a crash between the file's object
    /// and its registration so completes the same file once.
    ///
    /// # Errors
    /// A store failure.
    fn reserve(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        minted: &FileId,
    ) -> Result<FileId, CatalogError>;

    /// Register `request`'s file in `library_id` and release the reservation of
    /// `operation_id`, publishing the file's entity in the collection snapshot under that
    /// operation ID with `journal` (the file and collection as targets, never a path or a
    /// name). A file registered before is answered with its path and changes nothing.
    ///
    /// # Errors
    /// A store failure; a refused registration is an `Ok(Placed::Refused(..))` answer.
    fn register(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError>;
}

/// The journal entry a registration is published with: [`REGISTER_OPERATION`] under the
/// operation ID; the port adds the file and collection as targets. Pure.
pub fn register_entry(operation_id: &str) -> JournalEntry {
    JournalEntry::new(REGISTER_OPERATION, Outcome::Succeeded).operation_id(operation_id)
}
