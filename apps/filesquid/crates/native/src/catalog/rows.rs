//! Reading catalog rows back into `filesquid-core` and `filesquid-api` values. Each reader
//! runs one indexed query on one library's connection; the library identifier comes from
//! the file, not from a column.

use super::failed;
use filesquid_api::library::{
    CatalogError, CommitReceipt, Deletion, DeletionCause, FileEntry, FileListing, FileScope,
    FileState, Keyed, LibraryRecord, PageKey, VisibilityFilter, MAX_RETENTION_DAYS,
};
use filesquid_core::{
    Collection, CollectionId, CollectionSort, File, FileId, Folder, FolderId, Library, LibraryId,
    RepositoryId, VirtualCollection, VirtualCollectionId,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

/// Columns of a file listing row: ID, home collection, listing order, hidden flag, deletion
/// and purge-after times.
type FileRow = (
    String,
    String,
    i64,
    bool,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
);

/// Map a database error to a store failure. The message never names a path.
pub(crate) fn sql(err: rusqlite::Error) -> CatalogError {
    failed(format!("catalog database: {err}"))
}

/// An opaque identifier read back from a column, or a store failure for an empty one.
fn id<T>(
    make: impl FnOnce(String) -> Result<T, filesquid_core::InvalidOpaqueId>,
    raw: String,
) -> Result<T, CatalogError> {
    make(raw).map_err(|_| failed("empty identifier in the catalog"))
}

/// A collection sort stored as its wire name (`manual`, `title_ascending`, ...). Pure.
pub(crate) fn sort_name(sort: CollectionSort) -> Result<String, CatalogError> {
    match serde_json::to_value(sort) {
        Ok(serde_json::Value::String(name)) => Ok(name),
        _ => Err(failed("collection sort has no wire name")),
    }
}

/// The collection sort named `name`. Pure.
fn sort_of(name: String) -> Result<CollectionSort, CatalogError> {
    serde_json::from_value(serde_json::Value::String(name))
        .map_err(|_| failed("unknown collection sort in the catalog"))
}

/// The library record and its revision counter, or `None` when the file has no library row
/// (a create that never committed). Database I/O.
pub(crate) fn library(
    conn: &Connection,
) -> Result<Option<(LibraryRecord, i64, i64)>, CatalogError> {
    let row = conn
        .query_row(
            "SELECT id, repository_id, name, revision, created_order FROM library",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(sql)?;
    let Some((lib, repo, name, revision, order)) = row else {
        return Ok(None);
    };
    let library = Library::new(id(LibraryId::new, lib)?, id(RepositoryId::new, repo)?);
    Ok(Some((LibraryRecord { library, name }, revision, order)))
}

/// The library's own retention period in days (1 to [`MAX_RETENTION_DAYS`]), when its
/// snapshot sets one (ADR 041 Decision 10). Database I/O.
pub(crate) fn retention_days(conn: &Connection) -> Result<Option<u32>, CatalogError> {
    let row: Option<Option<i64>> = conn
        .query_row("SELECT retention_days FROM library", [], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    row.flatten()
        .map(|days| {
            u32::try_from(days)
                .ok()
                .filter(|days| (1..=MAX_RETENTION_DAYS).contains(days))
                .ok_or_else(|| failed("retention out of range in the catalog"))
        })
        .transpose()
}

/// The identity of the folder the index was built from, if recorded. Database I/O.
pub(crate) fn folder_identity(conn: &Connection) -> Result<Option<String>, CatalogError> {
    let row: Option<Option<String>> = conn
        .query_row("SELECT folder_identity FROM library", [], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok(row.flatten())
}

/// The receipt stored for `operation_id`, if any. Database I/O.
pub(crate) fn receipt(
    conn: &Connection,
    library_id: &LibraryId,
    operation_id: &str,
) -> Result<Option<CommitReceipt>, CatalogError> {
    conn.query_row(
        "SELECT revision, created_id, deleted_at_ms, purge_after_ms, deletion_cause
         FROM receipts WHERE operation_id = ?1",
        params![operation_id],
        |r| {
            Ok(CommitReceipt {
                library_id: library_id.clone(),
                revision: r.get(0)?,
                created_id: r.get(1)?,
                deletion: deletion(r.get(2)?, r.get(3)?, r.get(4)?),
            })
        },
    )
    .optional()
    .map_err(sql)
}

/// A deletion from its two stored times (both present or both absent) and its stored cause
/// (`NULL` for one by one). Pure.
fn deletion(
    deleted_at_ms: Option<i64>,
    purge_after_ms: Option<i64>,
    cause: Option<String>,
) -> Option<Deletion> {
    let cause = match cause.as_deref() {
        Some("with_collection") => DeletionCause::WithCollection,
        _ => DeletionCause::OneByOne,
    };
    Some(Deletion {
        deleted_at_ms: deleted_at_ms?,
        purge_after_ms: purge_after_ms?,
        cause,
    })
}

/// The stored form of a deletion cause: `NULL` for one by one. Pure.
pub(crate) fn cause_column(deletion: Option<Deletion>) -> Option<&'static str> {
    deletion
        .map(|d| d.cause)
        .filter(|cause| *cause == DeletionCause::WithCollection)
        .map(DeletionCause::as_str)
}

/// Columns read by [`collection_row`].
pub(crate) const COLLECTION_COLUMNS: &str = "id, folder_id, title, sort, cover_file_id, order_key";

/// A collection of `library_id` from a row of [`COLLECTION_COLUMNS`].
pub(crate) fn collection_row(
    library_id: &LibraryId,
    row: &Row<'_>,
) -> rusqlite::Result<Result<Collection, CatalogError>> {
    let (raw_id, folder, title, sort, cover, order): (
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        i64,
    ) = (
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    );
    Ok(build_collection(
        library_id, raw_id, folder, title, sort, cover, order,
    ))
}

/// Assemble a collection from its stored values. Pure.
fn build_collection(
    library_id: &LibraryId,
    raw_id: String,
    folder: Option<String>,
    title: String,
    sort: String,
    cover: Option<String>,
    order: i64,
) -> Result<Collection, CatalogError> {
    let mut collection = Collection::new(id(CollectionId::new, raw_id)?, library_id.clone(), title)
        .map_err(|_| failed("collection without a title in the catalog"))?
        .with_sort(sort_of(sort)?)
        .with_order_key(order);
    if let Some(folder) = folder {
        collection = collection.with_folder(id(FolderId::new, folder)?);
    }
    if let Some(cover) = cover {
        collection = collection.with_cover(id(FileId::new, cover)?);
    }
    Ok(collection)
}

/// Columns read by [`virtual_row`].
pub(crate) const VIRTUAL_COLUMNS: &str = "id, title, expression, cover_file_id, order_key";

/// A virtual collection of `library_id` from a row of [`VIRTUAL_COLUMNS`].
pub(crate) fn virtual_row(
    library_id: &LibraryId,
    row: &Row<'_>,
) -> rusqlite::Result<Result<VirtualCollection, CatalogError>> {
    let (raw_id, title, expression, cover, order): (
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
    ) = (
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    );
    Ok(build_virtual(
        library_id, raw_id, title, expression, cover, order,
    ))
}

/// Assemble a virtual collection from its stored values. The core type has no cover or order
/// setter, so it is rebuilt through its validated wire form. Pure.
fn build_virtual(
    library_id: &LibraryId,
    raw_id: String,
    title: String,
    expression: Option<String>,
    cover: Option<String>,
    order: i64,
) -> Result<VirtualCollection, CatalogError> {
    let wire = serde_json::json!({
        "id": raw_id,
        "library_id": library_id.as_str(),
        "title": title,
        "expression": expression,
        "cover_file_id": cover,
        "order_key": order,
    });
    serde_json::from_value(wire).map_err(|_| failed("invalid virtual collection in the catalog"))
}

/// One folder of `library_id`, if stored. Database I/O.
pub(crate) fn folder(
    conn: &Connection,
    library_id: &LibraryId,
    folder_id: &FolderId,
) -> Result<Option<Folder>, CatalogError> {
    let row: Option<(Option<String>, String, i64)> = conn
        .query_row(
            "SELECT parent_id, name, order_key FROM folders WHERE id = ?1",
            params![folder_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((parent, name, order)) = row else {
        return Ok(None);
    };
    let wire = serde_json::json!({
        "id": folder_id.as_str(),
        "library_id": library_id.as_str(),
        "parent_id": parent,
        "name": name,
        "order_key": order,
    });
    serde_json::from_value(wire)
        .map(Some)
        .map_err(|_| failed("invalid folder in the catalog"))
}

/// Columns read by [`file_row`], with `{ORD}` standing for the listing's order expression.
const FILE_COLUMNS: &str =
    "f.id, f.home_collection_id, {ORD}, f.hidden, f.deleted_at_ms, f.purge_after_ms, \
     f.deletion_cause, f.removed_home_title";

/// A raw file listing row.
fn file_row(row: &Row<'_>) -> rusqlite::Result<FileRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

/// A keyed file entry of `library_id` from a raw row. Pure.
fn file_entry(library_id: &LibraryId, row: FileRow) -> Result<Keyed<FileEntry>, CatalogError> {
    let (raw_id, home, order, hidden, deleted, purge, cause, removed_home_title) = row;
    let file = File::new(
        id(FileId::new, raw_id.clone())?,
        library_id.clone(),
        id(CollectionId::new, home)?,
    );
    let state = FileState {
        hidden,
        deletion: deletion(deleted, purge, cause),
    };
    Ok(Keyed {
        key: PageKey { order, id: raw_id },
        item: FileEntry {
            file,
            state,
            removed_home_title,
        },
    })
}

/// One file of `library_id` with its state, if stored. Database I/O.
pub(crate) fn file(
    conn: &Connection,
    library_id: &LibraryId,
    file_id: &FileId,
) -> Result<Option<FileEntry>, CatalogError> {
    Ok(keyed_file(conn, library_id, file_id)?.map(|k| k.item))
}

/// One file of `library_id` with its state, keyed by its listing order, if stored.
/// Database I/O.
pub(crate) fn keyed_file(
    conn: &Connection,
    library_id: &LibraryId,
    file_id: &FileId,
) -> Result<Option<Keyed<FileEntry>>, CatalogError> {
    let sql_text = format!(
        "SELECT {} FROM files f WHERE f.id = ?1",
        FILE_COLUMNS.replace("{ORD}", "f.list_order")
    );
    let row = conn
        .query_row(&sql_text, params![file_id.as_str()], file_row)
        .optional()
        .map_err(sql)?;
    row.map(|r| file_entry(library_id, r)).transpose()
}

/// The visibility condition of a listing. Pure.
fn visibility_sql(visibility: VisibilityFilter) -> &'static str {
    match visibility {
        VisibilityFilter::Ordinary => "f.hidden = 0 AND f.deleted_at_ms IS NULL",
        VisibilityFilter::Hidden => "f.hidden = 1 AND f.deleted_at_ms IS NULL",
        VisibilityFilter::Deleted => "f.deleted_at_ms IS NOT NULL",
    }
}

/// The source, order expression and scope condition of a listing; `?4` is the scope's
/// identifier, or the empty string for the library scope (which check it is
/// empty, so every listing binds the same four parameters). Pure.
fn scope_sql(scope: &FileScope) -> (&'static str, &'static str, &'static str) {
    match scope {
        FileScope::Library => ("files f", "f.list_order", "?4 = ''"),
        FileScope::Collection { .. } => ("files f", "f.list_order", "f.home_collection_id = ?4"),
        FileScope::VirtualCollection { .. } => (
            "virtual_members m JOIN files f ON f.id = m.file_id",
            "m.order_key",
            "m.virtual_collection_id = ?4",
        ),
    }
}

/// The scope's identifier bound to `?4`, or an empty string for scopes without one. Pure.
fn scope_id(scope: &FileScope) -> &str {
    match scope {
        FileScope::Collection { collection_id } => collection_id.as_str(),
        FileScope::VirtualCollection {
            virtual_collection_id,
        } => virtual_collection_id.as_str(),
        FileScope::Library => "",
    }
}

/// The SQL text of a file listing page, built only from the fixed fragments above. Pure.
fn files_sql(listing: &FileListing) -> String {
    let (source, order, condition) = scope_sql(&listing.scope);
    format!(
        "SELECT {columns} FROM {source} WHERE {condition} AND {visible}
         AND ({order} > ?1 OR ({order} = ?1 AND f.id > ?2))
         ORDER BY {order}, f.id LIMIT ?3",
        columns = FILE_COLUMNS.replace("{ORD}", order),
        visible = visibility_sql(listing.visibility),
    )
}

/// One page of `listing` after `after`, at most `limit` items, by (order, ID). Database I/O.
pub(crate) fn files(
    conn: &Connection,
    listing: &FileListing,
    after: Option<&PageKey>,
    limit: usize,
) -> Result<Vec<Keyed<FileEntry>>, CatalogError> {
    let (after_order, after_id) = after.map_or((i64::MIN, ""), |k| (k.order, k.id.as_str()));
    let mut stmt = conn.prepare(&files_sql(listing)).map_err(sql)?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let bound = params![after_order, after_id, limit, scope_id(&listing.scope)];
    let rows = stmt.query_map(bound, file_row).map_err(sql)?;
    rows.map(|row| file_entry(&listing.library_id, row.map_err(sql)?))
        .collect()
}

/// Answer the first row of `statement` bound to `id`, decoded by `decode`. Database I/O.
fn one<T>(
    conn: &Connection,
    statement: &str,
    id: &str,
    decode: impl FnOnce(&Row<'_>) -> rusqlite::Result<Result<T, CatalogError>>,
) -> Result<Option<T>, CatalogError> {
    conn.query_row(statement, params![id], decode)
        .optional()
        .map_err(sql)?
        .transpose()
}

/// One collection of `library_id`, if stored. Database I/O.
pub(crate) fn collection(
    conn: &Connection,
    library_id: &LibraryId,
    collection_id: &CollectionId,
) -> Result<Option<Collection>, CatalogError> {
    let statement = format!("SELECT {COLLECTION_COLUMNS} FROM collections WHERE id = ?1");
    one(conn, &statement, collection_id.as_str(), |r| {
        collection_row(library_id, r)
    })
}

/// One virtual collection of `library_id`, if stored. Database I/O.
pub(crate) fn virtual_collection(
    conn: &Connection,
    library_id: &LibraryId,
    virtual_collection_id: &VirtualCollectionId,
) -> Result<Option<VirtualCollection>, CatalogError> {
    let statement = format!("SELECT {VIRTUAL_COLUMNS} FROM virtual_collections WHERE id = ?1");
    one(conn, &statement, virtual_collection_id.as_str(), |r| {
        virtual_row(library_id, r)
    })
}

/// The SQL text of one page of `table` by (order key, ID). Pure.
fn page_sql(columns: &str, table: &str) -> String {
    format!(
        "SELECT {columns} FROM {table}
         WHERE order_key > ?1 OR (order_key = ?1 AND id > ?2)
         ORDER BY order_key, id LIMIT ?3"
    )
}

/// One page of rows of `statement` after `after`, at most `limit`, keyed by `key`.
/// Database I/O.
fn page<T>(
    conn: &Connection,
    statement: &str,
    after: Option<&PageKey>,
    limit: usize,
    decode: impl Fn(&Row<'_>) -> rusqlite::Result<Result<T, CatalogError>>,
    key: impl Fn(&T) -> PageKey,
) -> Result<Vec<Keyed<T>>, CatalogError> {
    let (after_order, after_id) = after.map_or((i64::MIN, ""), |k| (k.order, k.id.as_str()));
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = conn.prepare(statement).map_err(sql)?;
    let rows = stmt
        .query_map(params![after_order, after_id, limit], decode)
        .map_err(sql)?;
    rows.map(|row| {
        let item = row.map_err(sql)??;
        Ok(Keyed {
            key: key(&item),
            item,
        })
    })
    .collect()
}

/// Collections of `library_id` after `after`, at most `limit`, by (order key, ID).
/// Database I/O.
pub(crate) fn collections(
    conn: &Connection,
    library_id: &LibraryId,
    after: Option<&PageKey>,
    limit: usize,
) -> Result<Vec<Keyed<Collection>>, CatalogError> {
    let statement = page_sql(COLLECTION_COLUMNS, "collections");
    page(
        conn,
        &statement,
        after,
        limit,
        |r| collection_row(library_id, r),
        |c| PageKey {
            order: c.order_key(),
            id: c.id().as_str().to_string(),
        },
    )
}

/// Virtual collections of `library_id` after `after`, at most `limit`, by (order key, ID).
/// Database I/O.
pub(crate) fn virtual_collections(
    conn: &Connection,
    library_id: &LibraryId,
    after: Option<&PageKey>,
    limit: usize,
) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError> {
    let statement = page_sql(VIRTUAL_COLUMNS, "virtual_collections");
    page(
        conn,
        &statement,
        after,
        limit,
        |r| virtual_row(library_id, r),
        |v| PageKey {
            order: v.order_key(),
            id: v.id().as_str().to_string(),
        },
    )
}

/// `Some(())` when `file_id` is a member of `virtual_collection_id`. Database I/O.
pub(crate) fn is_member(
    conn: &Connection,
    virtual_collection_id: &VirtualCollectionId,
    file_id: &FileId,
) -> Result<Option<()>, CatalogError> {
    conn.query_row(
        "SELECT 1 FROM virtual_members WHERE virtual_collection_id = ?1 AND file_id = ?2",
        params![virtual_collection_id.as_str(), file_id.as_str()],
        |_| Ok(()),
    )
    .optional()
    .map_err(sql)
}

/// The highest membership order key of `virtual_collection_id`, or `None` when it has no
/// member (in this library's file). New members are ordered after it. Database I/O.
pub(crate) fn last_member_order(
    conn: &Connection,
    virtual_collection_id: &VirtualCollectionId,
) -> Result<Option<i64>, CatalogError> {
    conn.query_row(
        "SELECT MAX(order_key) FROM virtual_members WHERE virtual_collection_id = ?1",
        params![virtual_collection_id.as_str()],
        |r| r.get(0),
    )
    .map_err(sql)
}
