//! Applying a [`CatalogCommit`] to one library's file in one SQLite transaction, in the order
//! `LibraryCatalog::commit` states: the create or revision rule, every change, the new
//! revision, the receipt, the pending intent's removal and the journal entry (into the
//! outbox). A failure at any step rolls the whole transaction back.

use super::rows::{sort_name, sql};
use super::{failed, intent, outbox};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileState, LibraryRecord,
};
use filesquid_core::{Collection, File, Folder, LibraryId, VirtualCollection};
use rusqlite::{params, Connection, Transaction};

/// Whether `commit` creates its library. A create that is not the first change, is repeated,
/// or names another library makes the commit malformed (as in the API's reference catalog).
/// Pure.
pub(crate) fn creates_library(commit: &CatalogCommit) -> Result<bool, CatalogError> {
    let creates: Vec<(usize, &LibraryRecord)> = commit
        .changes
        .iter()
        .enumerate()
        .filter_map(|(index, change)| match change {
            CatalogChange::CreateLibrary(record) => Some((index, record)),
            _ => None,
        })
        .collect();
    match creates.as_slice() {
        [] => Ok(false),
        [(0, record)] if record.library.id() == &commit.library_id => Ok(true),
        _ => Err(failed("malformed commit: misplaced create-library change")),
    }
}

/// The library's revision once `commit` applies, from its `current` revision (`None` when the
/// file holds no library yet). A created library starts at 1; otherwise the expected
/// revision must match, and advances by one when the commit changes something. Pure.
fn next_revision(
    creates: bool,
    current: Option<i64>,
    commit: &CatalogCommit,
) -> Result<i64, CatalogError> {
    let changed = i64::from(!commit.changes.is_empty());
    match (creates, current) {
        (true, None) => Ok(1),
        (false, Some(rev)) if rev.to_string() == commit.expected_revision.as_str() => {
            Ok(rev + changed)
        }
        _ => Err(CatalogError::Conflict),
    }
}

/// Apply `commit` to `conn` in one transaction and answer its receipt. Database I/O.
///
/// # Errors
/// [`CatalogError::Conflict`] for a failed create or revision rule; [`CatalogError::Failed`]
/// for a change the schema refuses (for example a membership of a missing file). Nothing is
/// written in either case.
pub(crate) fn apply_commit(
    conn: &mut Connection,
    commit: &CatalogCommit,
    creates: bool,
) -> Result<CommitReceipt, CatalogError> {
    let tx = conn.transaction().map_err(sql)?;
    let current = current_revision(&tx)?;
    let revision = next_revision(creates, current, commit)?;
    for change in &commit.changes {
        apply_change(&tx, &commit.library_id, change, commit.journal.time_ms)?;
    }
    tx.execute("UPDATE library SET revision = ?1", params![revision])
        .map_err(sql)?;
    let receipt = CommitReceipt {
        library_id: commit.library_id.clone(),
        revision: revision.to_string(),
        created_id: commit.created_id.clone(),
        deletion: commit.deletion,
    };
    store_receipt(&tx, commit.operation_id.as_str(), &receipt)?;
    intent::clear(&tx, commit.operation_id.as_str())?;
    outbox::push(&tx, &commit.journal)?;
    tx.commit().map_err(sql)?;
    Ok(receipt)
}

/// The stored revision counter, or `None` when the file holds no library. Database I/O.
fn current_revision(tx: &Transaction<'_>) -> Result<Option<i64>, CatalogError> {
    use rusqlite::OptionalExtension;
    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))
        .optional()
        .map_err(sql)
}

/// Store the receipt of `operation_id`. Database I/O.
fn store_receipt(
    tx: &Transaction<'_>,
    operation_id: &str,
    receipt: &CommitReceipt,
) -> Result<(), CatalogError> {
    tx.execute(
        "INSERT INTO receipts (operation_id, revision, created_id, deleted_at_ms, purge_after_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            operation_id,
            receipt.revision,
            receipt.created_id,
            receipt.deletion.map(|d| d.deleted_at_ms),
            receipt.deletion.map(|d| d.purge_after_ms),
        ],
    )
    .map_err(sql)?;
    Ok(())
}

/// Refuse a record that names a library other than this file's. Pure.
fn same_library(library_id: &LibraryId, named: &LibraryId) -> Result<(), CatalogError> {
    (library_id == named)
        .then_some(())
        .ok_or_else(|| failed("change names another library"))
}

/// Apply one change inside the commit's transaction; `now_ms` orders a created library.
/// Database I/O.
fn apply_change(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    change: &CatalogChange,
    now_ms: i64,
) -> Result<(), CatalogError> {
    match change {
        CatalogChange::CreateLibrary(record) => insert_library(tx, record, now_ms),
        CatalogChange::PutCollection(c) => {
            same_library(library_id, c.library_id())?;
            put_collection(tx, c)
        }
        CatalogChange::RemoveCollection(id) => {
            exec(tx, "DELETE FROM collections WHERE id = ?1", id.as_str())
        }
        CatalogChange::PutVirtualCollection(v) => {
            same_library(library_id, v.library_id())?;
            put_virtual(tx, v)
        }
        CatalogChange::RemoveVirtualCollection(id) => exec(
            tx,
            "DELETE FROM virtual_collections WHERE id = ?1",
            id.as_str(),
        ),
        CatalogChange::AddVirtualMember(m) => tx
            .execute(
                "INSERT INTO virtual_members (virtual_collection_id, file_id, order_key)
                 VALUES (?1, ?2, ?3)",
                params![
                    m.virtual_collection_id().as_str(),
                    m.file_id().as_str(),
                    m.order_key()
                ],
            )
            .map(drop)
            .map_err(sql),
        CatalogChange::RemoveVirtualMember {
            virtual_collection_id,
            file_id,
        } => tx
            .execute(
                "DELETE FROM virtual_members WHERE virtual_collection_id = ?1 AND file_id = ?2",
                params![virtual_collection_id.as_str(), file_id.as_str()],
            )
            .map(drop)
            .map_err(sql),
        CatalogChange::SetFileState { file_id, state } => {
            set_file_state(tx, file_id.as_str(), state)
        }
        CatalogChange::PurgeFile(id) => exec(tx, "DELETE FROM files WHERE id = ?1", id.as_str()),
    }
}

/// Run a one-parameter statement. Database I/O.
fn exec(tx: &Transaction<'_>, statement: &str, id: &str) -> Result<(), CatalogError> {
    tx.execute(statement, params![id]).map(drop).map_err(sql)
}

/// Store the library row at revision 0 (the commit then sets its first revision).
/// Database I/O.
fn insert_library(
    tx: &Transaction<'_>,
    record: &LibraryRecord,
    created_order: i64,
) -> Result<(), CatalogError> {
    tx.execute(
        "INSERT INTO library (singleton, id, repository_id, name, revision, created_order)
         VALUES (1, ?1, ?2, ?3, 0, ?4)",
        params![
            record.library.id().as_str(),
            record.library.repository_id().as_str(),
            record.name,
            created_order,
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Store a collection, replacing the one with the same identifier. Database I/O.
fn put_collection(tx: &Transaction<'_>, c: &Collection) -> Result<(), CatalogError> {
    tx.execute(
        "INSERT INTO collections (id, folder_id, title, sort, cover_file_id, order_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET folder_id = excluded.folder_id, title = excluded.title,
             sort = excluded.sort, cover_file_id = excluded.cover_file_id,
             order_key = excluded.order_key",
        params![
            c.id().as_str(),
            c.folder_id().map(|f| f.as_str()),
            c.title(),
            sort_name(c.sort())?,
            c.cover_file_id().map(|f| f.as_str()),
            c.order_key(),
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Store a virtual collection, replacing the one with the same identifier and keeping its
/// memberships (an upsert, never a delete that would cascade). Database I/O.
fn put_virtual(tx: &Transaction<'_>, v: &VirtualCollection) -> Result<(), CatalogError> {
    tx.execute(
        "INSERT INTO virtual_collections (id, title, expression, cover_file_id, order_key)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title,
             expression = excluded.expression, cover_file_id = excluded.cover_file_id,
             order_key = excluded.order_key",
        params![
            v.id().as_str(),
            v.title(),
            v.expression(),
            v.cover_file_id().map(|f| f.as_str()),
            v.order_key(),
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Replace a file's hide and delete state. Database I/O.
fn set_file_state(
    tx: &Transaction<'_>,
    file_id: &str,
    state: &FileState,
) -> Result<(), CatalogError> {
    tx.execute(
        "UPDATE files SET hidden = ?2, deleted_at_ms = ?3, purge_after_ms = ?4 WHERE id = ?1",
        params![
            file_id,
            state.hidden,
            state.deletion.map(|d| d.deleted_at_ms),
            state.deletion.map(|d| d.purge_after_ms),
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Store files of `library_id` with their listing order, as an import or a rebuild from OCFL
/// does, keeping the hide and delete state of files already stored. Database I/O.
pub(crate) fn index_files(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    files: &[(File, i64)],
) -> Result<(), CatalogError> {
    let mut stmt = tx
        .prepare(
            "INSERT INTO files (id, home_collection_id, list_order) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET home_collection_id = excluded.home_collection_id,
                 list_order = excluded.list_order",
        )
        .map_err(sql)?;
    for (file, order) in files {
        same_library(library_id, file.library_id())?;
        stmt.execute(params![
            file.id().as_str(),
            file.home_collection_id().as_str(),
            order
        ])
        .map_err(sql)?;
    }
    Ok(())
}

/// Store folders of `library_id`, replacing those with the same identifiers. Database I/O.
pub(crate) fn index_folders(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    folders: &[Folder],
) -> Result<(), CatalogError> {
    let mut stmt = tx
        .prepare(
            "INSERT INTO folders (id, parent_id, name, order_key) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET parent_id = excluded.parent_id,
                 name = excluded.name, order_key = excluded.order_key",
        )
        .map_err(sql)?;
    for folder in folders {
        same_library(library_id, folder.library_id())?;
        let parent = folder.parent_id().map(|p| p.as_str());
        stmt.execute(params![
            folder.id().as_str(),
            parent,
            folder.name(),
            folder.order_key()
        ])
        .map_err(sql)?;
    }
    Ok(())
}
