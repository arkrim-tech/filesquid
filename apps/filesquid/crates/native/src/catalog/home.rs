//! The home collections of a library's catalog (Task API-F2.14; API-S-011, API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 4, 5 and 12): [`SqliteCatalog`] answers the upload and copy routes' port through
//! the same registration the import job uses ([`SqliteCatalog::register_file`]), the same
//! path-taken rule, and a reservation table that keeps each unfinished operation's file ID.

use super::rows::sql;
use super::{RegisterFile, Registration, SqliteCatalog};
use filesquid_api::canonical::collection_path::{free_path, ItemKind, PathError};
use filesquid_api::home::{HomeCollections, HomeRequest, Placed};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{CatalogError, LibraryCatalog};
use filesquid_core::{CollectionId, FileId, LibraryId};
use rusqlite::{params, Connection, OptionalExtension};

impl SqliteCatalog {
    /// The file ID `operation_id` reserved in `library_id`, reserving `minted` when it has
    /// none. Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    fn reserve_file(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        minted: &FileId,
    ) -> Result<FileId, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        insert_reservation(&library.conn, operation_id, minted)?;
        let held = read_reservation(&library.conn, operation_id)?;
        FileId::new(held).map_err(|_| super::failed("a reserved file ID is not one"))
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

/// Store `file_id` as the reservation of `operation_id` unless it has one. Database I/O.
fn insert_reservation(
    conn: &Connection,
    operation_id: &str,
    file_id: &FileId,
) -> Result<(), CatalogError> {
    conn.execute(
        "INSERT OR IGNORE INTO reserved_files (operation_id, file_id) VALUES (?1, ?2)",
        params![operation_id, file_id.as_str()],
    )
    .map_err(sql)?;
    Ok(())
}

/// The file ID reserved by `operation_id`. Database I/O.
fn read_reservation(conn: &Connection, operation_id: &str) -> Result<String, CatalogError> {
    conn.query_row(
        "SELECT file_id FROM reserved_files WHERE operation_id = ?1",
        params![operation_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql)?
    .ok_or_else(|| super::failed("a reservation was not stored"))
}

/// The port's answer for a registration. Pure.
fn placed(registration: Registration) -> Placed {
    match registration {
        Registration::Registered { path } => Placed::Registered { path },
        Registration::AlreadyRegistered { path } => Placed::AlreadyRegistered { path },
        Registration::Refused(code) => Placed::Refused(code),
    }
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
        _library_id: &LibraryId,
        collection_id: &CollectionId,
        path: &str,
        kind: ItemKind,
    ) -> Result<Result<String, PathError>, CatalogError> {
        let mut taken = |candidate: &str| self.path_taken(collection_id, candidate);
        free_path(path, kind, &mut taken)
    }

    fn reserve(
        &self,
        library_id: &LibraryId,
        operation_id: &str,
        minted: &FileId,
    ) -> Result<FileId, CatalogError> {
        self.reserve_file(library_id, operation_id, minted)
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
        let answer = placed(self.register_file(library_id, &register, journal)?);
        if !matches!(answer, Placed::Refused(_)) {
            self.release_file(library_id, operation_id)?;
        }
        Ok(answer)
    }
}
