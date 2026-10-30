//! The home collections of a library's catalog (Task API-F2.14; API-S-011, API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 4, 5 and 12): [`SqliteCatalog`] answers the upload and copy routes' port through
//! the same registration the import job uses ([`SqliteCatalog::register_file`]), the same
//! path-taken rule, and a reservation table that keeps each unfinished operation's file ID.

use super::rows::sql;
use super::{placement, RegisterFile, Registration, SqliteCatalog};
use filesquid_api::canonical::collection_path::{free_path, ItemKind, PathError};
use filesquid_api::home::{HomeCollections, HomeRequest, Placed, Reserved};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{CatalogError, LibraryCatalog};
use filesquid_core::{CollectionId, FileId, LibraryId};
use rusqlite::{params, Connection, OptionalExtension};

impl SqliteCatalog {
    /// The file ID `operation_id` reserved in `library_id` for the input `fingerprint`,
    /// reserving `minted` when it has none; [`Reserved::Reused`] when the reservation belongs
    /// to another input. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    fn reserve_file(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        (fingerprint, minted): (&str, &FileId),
    ) -> Result<Reserved, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        insert_reservation(&library.conn, operation_id, fingerprint, minted)?;
        let (held_fingerprint, held) = read_reservation(&library.conn, operation_id)?;
        if held_fingerprint != fingerprint {
            return Ok(Reserved::Reused);
        }
        let file = FileId::new(held).map_err(|_| super::failed("a reserved file ID is not one"))?;
        Ok(Reserved::File(file))
    }

    /// The first free path for `path` in `collection_id` of `library_id`, read from that
    /// library's own catalog. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    fn free_in(
        &self,
        library_id: &LibraryId,
        (collection_id, path, kind): (&CollectionId, &str, ItemKind),
    ) -> Result<Result<String, PathError>, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        let mut taken = |candidate: &str| placement::taken(&library.conn, collection_id, candidate);
        free_path(path, kind, &mut taken)
    }

    /// The home collection of the registered file `file_id` of `library_id`. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library, an unknown file or a database failure.
    fn home_collection(
        &self,
        library_id: &LibraryId,
        file_id: &FileId,
    ) -> Result<CollectionId, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        let home: String = library
            .conn
            .query_row(
                "SELECT home_collection_id FROM files WHERE id = ?1",
                params![file_id.as_str()],
                |r| r.get(0),
            )
            .map_err(sql)?;
        CollectionId::new(home).map_err(|_| super::failed("a stored collection ID"))
    }

    /// Remove the reservation of `operation_id` in `library_id`. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    fn release_file(&self, library_id: &LibraryId, operation_id: &str) -> Result<(), CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        library
            .conn
            .execute(
                "DELETE FROM reserved_files WHERE operation_id = ?1",
                params![operation_id],
            )
            .map_err(sql)?;
        Ok(())
    }
}

/// Store `file_id` for the input `fingerprint` as the reservation of `operation_id` unless
/// it has one. Database I/O.
fn insert_reservation(
    conn: &Connection,
    operation_id: &str,
    fingerprint: &str,
    file_id: &FileId,
) -> Result<(), CatalogError> {
    conn.execute(
        "INSERT OR IGNORE INTO reserved_files (operation_id, fingerprint, file_id)
         VALUES (?1, ?2, ?3)",
        params![operation_id, fingerprint, file_id.as_str()],
    )
    .map_err(sql)?;
    Ok(())
}

/// The input fingerprint and file ID reserved by `operation_id`. Database I/O.
fn read_reservation(
    conn: &Connection,
    operation_id: &str,
) -> Result<(String, String), CatalogError> {
    conn.query_row(
        "SELECT fingerprint, file_id FROM reserved_files WHERE operation_id = ?1",
        params![operation_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(sql)?
    .ok_or_else(|| super::failed("a reservation was not stored"))
}

impl HomeCollections for SqliteCatalog {
    fn collection_library(
        &self,
        collection_id: &CollectionId,
    ) -> Result<Option<LibraryId>, CatalogError> {
        let found = LibraryCatalog::collection(self, collection_id)?;
        Ok(found.map(|collection| collection.library_id().clone()))
    }

    fn free_path(
        &self,
        library_id: &LibraryId,
        collection_id: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<Result<String, PathError>, CatalogError> {
        self.free_in(library_id, (collection_id, path, kind))
    }

    fn reserve(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        fingerprint: &str,
        minted: &FileId,
    ) -> Result<Reserved, CatalogError> {
        self.reserve_file(library_id, operation_id, (fingerprint, minted))
    }

    fn release(&self, library_id: &LibraryId, operation_id: &str) -> Result<(), CatalogError> {
        self.release_file(library_id, operation_id)
    }

    fn register(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError> {
        let register = RegisterFile {
            file_id: request.file_id.clone(),
            collection_id: request.collection_id.clone(),
            path: request.path.clone(),
        };
        let answer = match self.register_file(library_id, &register, journal)? {
            Registration::Registered { path } => Placed::Registered { path },
            Registration::AlreadyRegistered { path } => Placed::AlreadyRegistered {
                collection_id: self.home_collection(library_id, &request.file_id)?,
                path,
            },
            Registration::Refused(code) => return Ok(Placed::Refused(code)),
        };
        self.release_file(library_id, operation_id)?;
        Ok(answer)
    }
}
