//! The home collections of a library's catalog (Task API-F2.14; API-S-011, API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decisions 4, 5 and 12): [`SqliteCatalog`] answers the upload and copy routes' port through
//! the same registration the import job uses ([`SqliteCatalog::register_file`]), the same
//! path-taken rule, and a reservation table that keeps each unfinished operation's file ID.

use super::rows::{self, sql};
use super::{placement, RegisterFile, Registration, SqliteCatalog};
use filesquid_api::canonical::collection_path::{free_path, ItemKind, PathError};
use filesquid_api::canonical::SnapshotKey;
use filesquid_api::home::{HomeCollections, HomeRequest, Placed, Reserved};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{CatalogError, LibraryCatalog};
use filesquid_core::{CollectionId, FileId, LibraryId};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

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

impl SqliteCatalog {
    /// Relocate `request`'s registered file in `library_id` as
    /// [`HomeCollections::relocate`] says, publishing its entity under the operation ID
    /// `journal` names; a refusal is journaled `failed` with its code. Database, filesystem
    /// and journal I/O.
    ///
    /// # Errors
    /// As [`SqliteCatalog::register_file`].
    fn relocate_file(
        &self,
        library_id: &LibraryId,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError> {
        let journal = journal
            .target(request.file_id.as_str())
            .target(request.collection_id.as_str());
        let keys = vec![SnapshotKey::File(request.file_id.clone())];
        let mut outcome = Placed::Refused("internal");
        self.write_journaled(library_id, journal.clone(), keys, |tx| {
            outcome = relocate_in(tx, library_id, request)?;
            Ok(())
        })?;
        if let Placed::Refused(code) = outcome {
            self.journal_refused(library_id, super::register::refused_entry(journal, code))?;
        }
        Ok(outcome)
    }

    /// The home collection and logical path of `file_id` in `library_id`, if registered.
    /// Database I/O.
    ///
    /// # Errors
    /// [`CatalogError::Failed`] for an unknown library or a database failure.
    fn placement_in(
        &self,
        library_id: &LibraryId,
        file_id: &FileId,
    ) -> Result<Option<(CollectionId, String)>, CatalogError> {
        let libraries = self.lock()?;
        let library = libraries
            .get(library_id.as_str())
            .ok_or_else(|| super::failed("unknown library"))?;
        home_of(&library.conn, file_id)
    }
}

/// The home collection and logical path of the registered file `file_id`. Database I/O.
fn home_of(
    conn: &Connection,
    file_id: &FileId,
) -> Result<Option<(CollectionId, String)>, CatalogError> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT home_collection_id, path FROM files WHERE id = ?1",
            params![file_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((collection, Some(path))) = row else {
        return Ok(None);
    };
    let collection =
        CollectionId::new(collection).map_err(|_| super::failed("a stored collection ID"))?;
    Ok(Some((collection, path)))
}

/// Relocate `request`'s file inside `tx`: refuse an unregistered file and a collection the
/// library does not hold; answer a file already there as it is; else give it the first free
/// path, its own path never counting as taken, and store its new home. Database I/O.
fn relocate_in(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    request: &HomeRequest,
) -> Result<Placed, CatalogError> {
    let Some((home, current)) = home_of(tx, &request.file_id)? else {
        return Ok(Placed::Refused("not_found"));
    };
    if home == request.collection_id && current == request.path {
        return Ok(Placed::AlreadyRegistered {
            collection_id: home,
            path: current,
        });
    }
    if rows::collection(tx, library_id, &request.collection_id)?.is_none() {
        return Ok(Placed::Refused("collection_not_found"));
    }
    let own = |candidate: &str| home == request.collection_id && candidate == current;
    let mut taken = |candidate: &str| match own(candidate) {
        true => Ok(false),
        false => placement::taken(tx, &request.collection_id, candidate),
    };
    let path = match free_path(&request.path, ItemKind::File, &mut taken)? {
        Ok(path) => path,
        Err(refused) => return Ok(Placed::Refused(refused.error_code())),
    };
    tx.execute(
        "UPDATE files SET home_collection_id = ?2 WHERE id = ?1",
        params![request.file_id.as_str(), request.collection_id.as_str()],
    )
    .map_err(sql)?;
    placement::place(tx, &request.file_id, &path)?;
    Ok(Placed::Registered { path })
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

    fn placement(
        &self,
        library_id: &LibraryId,
        file_id: &FileId,
    ) -> Result<Option<(CollectionId, String)>, CatalogError> {
        self.placement_in(library_id, file_id)
    }

    fn relocate(
        &self,
        library_id: &LibraryId,
        request: &HomeRequest,
        journal: JournalEntry,
    ) -> Result<Placed, CatalogError> {
        self.relocate_file(library_id, request, journal)
    }
}
