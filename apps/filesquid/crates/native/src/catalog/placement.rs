//! Where each file sits in its home collection (Task DE-1.7; DE-S-009, DE-S-018, API-S-026,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)):
//! the catalog rows of a file's collection logical path and its own object, and the indexed
//! reads the navigation and listing routes answer from.
//!
//! A file row keeps its path, its object, `name` (the path's last segment) and `dir_key`, the
//! path of the folder that holds it with every `/` written as U+0001 (`""` at the collection's
//! root). A collection logical path holds no control character, so U+0001 sorts below every
//! character a name can hold: the rows of one folder and of every folder below it form one
//! contiguous range of `dir_key`, and the next subfolder is one indexed seek past
//! `<folder><name>` + U+0002. Folder listings count only ordinary files (neither hidden nor
//! deleted, API-S-021); whether a path is taken counts every file, a deleted one included
//! (ADR 041 Decision 12). Database reads and writes only; nothing here names a filesystem path.

use super::failed;
use super::rows::sql;
use filesquid_api::canonical::file_object_id;
use filesquid_api::files::{ScopeAfter, ScopeEntry};
use filesquid_api::library::CatalogError;
use filesquid_core::{CollectionId, FileId};
use rusqlite::{params, Connection, OptionalExtension};

/// The separator `dir_key` writes for `/`.
const SEP: char = '\u{1}';

/// The bound just past a folder's range of `dir_key` values.
const PAST: char = '\u{2}';

/// The condition of a row that folder listings count: a placed, ordinary file.
const LISTED: &str = "path IS NOT NULL AND hidden = 0 AND deleted_at_ms IS NULL";

/// Most subfolder seeks one listing makes: the largest page, plus the one more a listing asks
/// for to learn whether a next page exists (AGENTS.md §22).
const MAX_SEEKS: usize = 1_001;

/// The key of the folder `folder` (a collection logical path, `""` for the root). Pure.
pub(crate) fn folder_key(folder: &str) -> String {
    folder.replace('/', &SEP.to_string())
}

/// The `dir_key` and `name` of a file at `path`. Pure.
pub(crate) fn split(path: &str) -> (String, &str) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (folder_key(dir), name),
        None => (String::new(), path),
    }
}

/// Record the collection logical path `path` and the object of `file_id` on its stored row.
/// Database I/O.
pub(crate) fn place(conn: &Connection, file_id: &FileId, path: &str) -> Result<(), CatalogError> {
    let (dir_key, name) = split(path);
    conn.execute(
        "UPDATE files SET path = ?2, object_id = ?3, dir_key = ?4, name = ?5 WHERE id = ?1",
        params![
            file_id.as_str(),
            path,
            file_object_id(file_id),
            dir_key,
            name
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// The path and object stored for `file_id`, each `None` for a file of the earlier form.
/// Database I/O.
pub(crate) fn placement_of(
    conn: &Connection,
    file_id: &FileId,
) -> Result<(Option<String>, Option<String>), CatalogError> {
    let row = conn
        .query_row(
            "SELECT path, object_id FROM files WHERE id = ?1",
            params![file_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.unwrap_or((None, None)))
}

/// Whether `path` is taken in `collection_id`: a file of any state sits there, below it (the
/// path names a folder) or at one of its ancestors (a file cannot also be a folder).
/// Database I/O.
pub(crate) fn taken(
    conn: &Connection,
    collection_id: &CollectionId,
    path: &str,
) -> Result<bool, CatalogError> {
    if at_or_below(conn, collection_id, path)? {
        return Ok(true);
    }
    for ancestor in ancestors(path) {
        if file_at(conn, collection_id, ancestor)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The proper ancestors of `path`, nearest first: at most one per `/`, so the walk is
/// bounded by the path's 1,024 bytes. Pure.
fn ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').rev().map(|(at, _)| &path[..at])
}

/// Whether a file of any state sits exactly at `path` in `collection_id`. Database I/O.
fn file_at(
    conn: &Connection,
    collection_id: &CollectionId,
    path: &str,
) -> Result<bool, CatalogError> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM files WHERE home_collection_id = ?1 AND path = ?2)",
        params![collection_id.as_str(), path],
        |r| r.get::<_, bool>(0),
    )
    .map_err(sql)
}

/// Whether a file of any state sits at `path` in `collection_id` or below it. Database I/O.
fn at_or_below(
    conn: &Connection,
    collection_id: &CollectionId,
    path: &str,
) -> Result<bool, CatalogError> {
    let below = format!("{path}/");
    let past = format!("{path}0");
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM files WHERE home_collection_id = ?1
             AND (path = ?2 OR (path >= ?3 AND path < ?4)))",
        params![collection_id.as_str(), path, below, past],
        |r| r.get::<_, bool>(0),
    )
    .map_err(sql)
}

/// The names of the folders directly below `parent` in `collection_id` that ordinary files
/// imply, in code-point order, those after `after` when given, at most `limit`.
/// Database I/O.
pub(crate) fn subfolders(
    conn: &Connection,
    collection_id: &CollectionId,
    parent: &str,
    after: Option<&str>,
    limit: usize,
) -> Result<Vec<String>, CatalogError> {
    let (base, upper) = match parent {
        "" => (String::new(), None),
        parent => {
            let key = folder_key(parent);
            (format!("{key}{SEP}"), Some(format!("{key}{PAST}")))
        }
    };
    let mut lower = match after {
        Some(name) => format!("{base}{name}{PAST}"),
        None if base.is_empty() => SEP.to_string(),
        None => base.clone(),
    };
    let mut names = Vec::new();
    for _ in 0..limit.min(MAX_SEEKS) {
        let Some(key) = next_dir(conn, collection_id, &lower, upper.as_deref())? else {
            break;
        };
        let rest = key.get(base.len()..).unwrap_or_default();
        let name = rest.split(SEP).next().unwrap_or_default().to_string();
        lower = format!("{base}{name}{PAST}");
        names.push(name);
    }
    Ok(names)
}

/// The smallest `dir_key` of an ordinary file of `collection_id` at or after `lower` and
/// before `upper` (no bound when `None`). Database I/O.
fn next_dir(
    conn: &Connection,
    collection_id: &CollectionId,
    lower: &str,
    upper: Option<&str>,
) -> Result<Option<String>, CatalogError> {
    let statement = format!(
        "SELECT dir_key FROM files WHERE home_collection_id = ?1 AND {LISTED}
         AND dir_key >= ?2 AND (?3 IS NULL OR dir_key < ?3) ORDER BY dir_key LIMIT 1"
    );
    conn.query_row(
        &statement,
        params![collection_id.as_str(), lower, upper],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql)
}

/// Whether ordinary files of `collection_id` imply the folder `path`. Database I/O.
pub(crate) fn folder_exists(
    conn: &Connection,
    collection_id: &CollectionId,
    path: &str,
) -> Result<bool, CatalogError> {
    let key = folder_key(path);
    let statement = format!(
        "SELECT EXISTS (SELECT 1 FROM files WHERE home_collection_id = ?1 AND {LISTED}
             AND dir_key >= ?2 AND dir_key < ?3)"
    );
    conn.query_row(
        &statement,
        params![collection_id.as_str(), key, format!("{key}{PAST}")],
        |r| r.get::<_, bool>(0),
    )
    .map_err(sql)
}

/// One page of the folder `folder` of `collection_id`: its subfolders (unless `after` names a
/// file), then the ordinary files directly in it by name and ID, at most `limit` entries after
/// `after`. Database I/O.
pub(crate) fn entries(
    conn: &Connection,
    collection_id: &CollectionId,
    folder: &str,
    after: Option<&ScopeAfter>,
    limit: usize,
) -> Result<Vec<ScopeEntry>, CatalogError> {
    let mut out = Vec::new();
    if !matches!(after, Some(ScopeAfter::File { .. })) {
        let after_folder = match after {
            Some(ScopeAfter::Folder { name }) => Some(name.as_str()),
            _ => None,
        };
        let folders = subfolders(conn, collection_id, folder, after_folder, limit)?;
        out.extend(folders.into_iter().map(|name| ScopeEntry::Folder { name }));
    }
    let room = limit.saturating_sub(out.len());
    let (after_name, after_id) = match after {
        Some(ScopeAfter::File { name, file_id }) => (name.as_str(), file_id.as_str()),
        _ => ("", ""),
    };
    let files = files_in(conn, collection_id, folder, (after_name, after_id), room)?;
    out.extend(files);
    Ok(out)
}

/// The ordinary files directly in `folder` of `collection_id` after (name, ID) `after`, by
/// name then ID, at most `limit`. Database I/O.
fn files_in(
    conn: &Connection,
    collection_id: &CollectionId,
    folder: &str,
    after: (&str, &str),
    limit: usize,
) -> Result<Vec<ScopeEntry>, CatalogError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let statement = format!(
        "SELECT name, id FROM files WHERE home_collection_id = ?1 AND {LISTED} AND dir_key = ?2
         AND (name > ?3 OR (name = ?3 AND id > ?4)) ORDER BY name, id LIMIT ?5"
    );
    let mut stmt = conn.prepare(&statement).map_err(sql)?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let bound = params![
        collection_id.as_str(),
        folder_key(folder),
        after.0,
        after.1,
        limit
    ];
    let rows = stmt
        .query_map(bound, |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(sql)?;
    rows.map(|row| {
        let (name, file_id) = row.map_err(sql)?;
        Ok(ScopeEntry::File { name, file_id })
    })
    .collect()
}

/// The next listing order of a new file: one past the largest stored. Database I/O.
pub(crate) fn next_order(conn: &Connection) -> Result<i64, CatalogError> {
    let largest: Option<i64> = conn
        .query_row("SELECT MAX(list_order) FROM files", [], |r| r.get(0))
        .map_err(sql)?;
    largest
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| failed("file listing order exhausted"))
}
