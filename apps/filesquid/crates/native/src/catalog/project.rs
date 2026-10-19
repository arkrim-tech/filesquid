//! Projecting catalog rows onto snapshot entities (Task DE-0.3): after a commit's changes are
//! applied inside its open SQLite transaction, the entities they touched are read back and
//! encoded by `filesquid-api` into the delta of one snapshot version. Database reads only.

use super::failed;
use super::rows::{self, sql};
use filesquid_api::canonical::{
    touched, CanonicalError, LibraryHead, SnapshotDelta, SnapshotEntity, SnapshotFile, SnapshotKey,
    VirtualEntry,
};
use filesquid_api::library::{CatalogChange, CatalogError};
use filesquid_core::{
    CollectionId, FileId, FolderId, LibraryId, VirtualCollectionId, VirtualMembership,
};
use rusqlite::{params, Connection, OptionalExtension};

/// Most entities one full projection reads (AGENTS.md §22).
const MAX_PROJECTED: i64 = 1_000_000;

/// A canonical-layer failure as a catalog error.
fn canonical(err: CanonicalError) -> CatalogError {
    err.into()
}

/// The virtual collections `changes` purge files from; read before the purge removes the
/// memberships. Database I/O.
pub(crate) fn purged_memberships(
    conn: &Connection,
    changes: &[CatalogChange],
) -> Result<Vec<SnapshotKey>, CatalogError> {
    let mut keys = Vec::new();
    let mut stmt = conn
        .prepare("SELECT virtual_collection_id FROM virtual_members WHERE file_id = ?1")
        .map_err(sql)?;
    for change in changes {
        let CatalogChange::PurgeFile(file_id) = change else {
            continue;
        };
        let rows = stmt
            .query_map(params![file_id.as_str()], |r| r.get::<_, String>(0))
            .map_err(sql)?;
        for vc in rows {
            let id = VirtualCollectionId::new(vc.map_err(sql)?)
                .map_err(|_| failed("empty identifier in the catalog"))?;
            keys.push(SnapshotKey::VirtualCollection(id));
        }
    }
    Ok(keys)
}

/// The delta of the entities `changes` (and `extra`) touched, read from `conn` after the
/// changes were applied: present entities are written, missing ones removed. Database I/O.
pub(crate) fn changed_delta(
    conn: &Connection,
    library_id: &LibraryId,
    changes: &[CatalogChange],
    extra: Vec<SnapshotKey>,
) -> Result<SnapshotDelta, CatalogError> {
    let mut keys: Vec<SnapshotKey> = changes.iter().flat_map(touched).chain(extra).collect();
    keys.dedup();
    delta_of(conn, library_id, keys)
}

/// The delta of exactly `keys`, read from `conn`. Database I/O.
pub(crate) fn delta_of(
    conn: &Connection,
    library_id: &LibraryId,
    keys: Vec<SnapshotKey>,
) -> Result<SnapshotDelta, CatalogError> {
    let mut delta = SnapshotDelta::default();
    let mut seen = std::collections::HashSet::new();
    for key in keys.into_iter().filter(|k| seen.insert(k.clone())) {
        match entity(conn, library_id, &key)? {
            Some(found) => delta.put(&found).map_err(canonical)?,
            None => delta.remove(&key).map_err(canonical)?,
        }
    }
    Ok(delta)
}

/// The delta that writes every entity of the library: the first snapshot of a catalog that
/// has none yet. Database I/O.
pub(crate) fn full_delta(
    conn: &Connection,
    library_id: &LibraryId,
) -> Result<SnapshotDelta, CatalogError> {
    let mut keys = vec![SnapshotKey::Library];
    keys.extend(keys_of(conn, "collections", |id| {
        CollectionId::new(id).ok().map(SnapshotKey::Collection)
    })?);
    keys.extend(keys_of(conn, "virtual_collections", |id| {
        VirtualCollectionId::new(id)
            .ok()
            .map(SnapshotKey::VirtualCollection)
    })?);
    keys.extend(keys_of(conn, "folders", |id| {
        FolderId::new(id).ok().map(SnapshotKey::Folder)
    })?);
    keys.extend(keys_of(conn, "files", |id| {
        FileId::new(id).ok().map(SnapshotKey::File)
    })?);
    delta_of(conn, library_id, keys)
}

/// The key of every row of `table` (a fixed table name), made by `make`, at most
/// [`MAX_PROJECTED`]. Database I/O.
fn keys_of(
    conn: &Connection,
    table: &str,
    make: impl Fn(String) -> Option<SnapshotKey>,
) -> Result<Vec<SnapshotKey>, CatalogError> {
    let mut stmt = conn
        .prepare(&format!("SELECT id FROM {table} ORDER BY id LIMIT ?1"))
        .map_err(sql)?;
    let rows = stmt
        .query_map(params![MAX_PROJECTED], |r| r.get::<_, String>(0))
        .map_err(sql)?;
    rows.map(|r| make(r.map_err(sql)?).ok_or_else(|| failed("empty identifier in the catalog")))
        .collect()
}

/// The entity `key` names, or `None` when the catalog no longer holds it. Database I/O.
fn entity(
    conn: &Connection,
    library_id: &LibraryId,
    key: &SnapshotKey,
) -> Result<Option<SnapshotEntity>, CatalogError> {
    Ok(match key {
        SnapshotKey::Library => library(conn)?.map(SnapshotEntity::Library),
        SnapshotKey::Collection(id) => {
            rows::collection(conn, library_id, id)?.map(SnapshotEntity::Collection)
        }
        SnapshotKey::VirtualCollection(id) => {
            virtual_entry(conn, library_id, id)?.map(SnapshotEntity::VirtualCollection)
        }
        SnapshotKey::Folder(id) => rows::folder(conn, library_id, id)?.map(SnapshotEntity::Folder),
        SnapshotKey::File(id) => snapshot_file(conn, library_id, id)?.map(SnapshotEntity::File),
    })
}

/// The library entity: record, revision and creation order. Database I/O.
fn library(conn: &Connection) -> Result<Option<LibraryHead>, CatalogError> {
    Ok(
        rows::library(conn)?.map(|(record, revision, created_order)| LibraryHead {
            record,
            revision: u64::try_from(revision).unwrap_or(0),
            created_order,
        }),
    )
}

/// A virtual collection with its members in membership order. Database I/O.
fn virtual_entry(
    conn: &Connection,
    library_id: &LibraryId,
    id: &VirtualCollectionId,
) -> Result<Option<VirtualEntry>, CatalogError> {
    let Some(collection) = rows::virtual_collection(conn, library_id, id)? else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT file_id, order_key FROM virtual_members WHERE virtual_collection_id = ?1 ORDER BY order_key, file_id LIMIT ?2")
        .map_err(sql)?;
    let rows = stmt
        .query_map(params![id.as_str(), MAX_PROJECTED], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(sql)?;
    let members = rows
        .map(|row| {
            let (file, order) = row.map_err(sql)?;
            let file = FileId::new(file).map_err(|_| failed("empty identifier in the catalog"))?;
            Ok(VirtualMembership::new(id.clone(), file, order))
        })
        .collect::<Result<Vec<_>, CatalogError>>()?;
    Ok(Some(VirtualEntry {
        collection,
        members,
    }))
}

/// A file's place and state. Database I/O.
fn snapshot_file(
    conn: &Connection,
    library_id: &LibraryId,
    id: &FileId,
) -> Result<Option<SnapshotFile>, CatalogError> {
    let Some(entry) = rows::file(conn, library_id, id)? else {
        return Ok(None);
    };
    let order: Option<i64> = conn
        .query_row(
            "SELECT list_order FROM files WHERE id = ?1",
            params![id.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(order.map(|order| SnapshotFile {
        file: entry.file,
        order,
        state: entry.state,
    }))
}
