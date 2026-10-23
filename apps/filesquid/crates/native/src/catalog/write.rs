//! Applying a [`CatalogCommit`] to one library in the order of ADR 005 and
//! `LibraryCatalog::commit` (Tasks DE-0.2, DE-0.3): record the intent; in one SQLite
//! transaction check the create or revision rule and apply every change; publish the
//! snapshot version of what changed through the library's OCFL root; then store the new
//! revision and snapshot version, the receipt, the intent's removal and the journal entry
//! (into the outbox) and commit. A commit with no change publishes nothing and only removes
//! its intent: it stores no receipt, no journal entry and no new revision (owner decision,
//! 2026-10-19). A failure before the commit rolls the transaction back.

use super::canonical::CanonicalRoot;
use super::history::{plan_commit, History};
use super::intent::PendingIntent;
use super::rows::{sort_name, sql};
use super::{failed, intent, outbox, project, reconcile, CommitProbe};
use filesquid_api::canonical::{operation_kind, OperationRecord, SnapshotDelta, SnapshotKey};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, CommitReceipt, FileState, LibraryRecord,
};
use filesquid_core::{Collection, File, Folder, LibraryId, VirtualCollection, VirtualMembership};
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

/// Where a commit stopped when it failed, which decides what happens to its intent.
#[derive(Debug)]
pub(crate) struct Stopped {
    /// The failure.
    pub error: CatalogError,
    /// Whether the intent stays: the version may be published (or a probe stopped the
    /// commit as a crash would), so reconcile must see it. A failure known to publish
    /// nothing discards it.
    pub keep_intent: bool,
}

impl Stopped {
    /// A failure before anything could be published.
    pub(crate) fn discard(error: CatalogError) -> Self {
        Self {
            error,
            keep_intent: false,
        }
    }

    /// A failure after which the version may be published.
    fn keep(error: CatalogError) -> Self {
        Self {
            error,
            keep_intent: true,
        }
    }
}

/// The canonical side of a commit: the library's OCFL root, the crash probe and who records
/// the commit's history.
pub(crate) struct Canonical<'a> {
    /// The library's canonical storage.
    pub root: &'a CanonicalRoot,
    /// Where tests stop a commit as a crash would.
    pub probe: &'a dyn CommitProbe,
    /// The installation that records commits and the journal that allocates their IDs;
    /// `None` when the host configured no provenance.
    pub history: Option<History<'a>>,
}

/// Apply `commit` in the order of ADR 005: record its intent, apply its changes in an open
/// transaction, publish the snapshot version of what they touched (when they changed
/// anything), then store the new revision, snapshot version, receipt, journal entry and the
/// intent's removal and commit. Database and filesystem I/O.
///
/// # Errors
/// [`CatalogError::Conflict`] for a failed create or revision rule or a moved head;
/// [`CatalogError::Failed`] for a change the schema or the engine refuses (a W010 head
/// included); [`CatalogError::Unavailable`] for storage trouble. The catalog is unchanged in
/// each case; the intent stays only when the version may have been published.
pub(crate) fn apply_commit(
    conn: &mut Connection,
    commit: &CatalogCommit,
    creates: bool,
    canonical: &Canonical<'_>,
) -> Result<CommitReceipt, Stopped> {
    record_intent(conn, commit).map_err(Stopped::discard)?;
    let result = commit_in_transaction(conn, commit, creates, canonical);
    if let Err(stopped) = &result {
        if !stopped.keep_intent {
            discard_intent(conn, commit.operation_id.as_str());
        }
    }
    result
}

/// Record `commit`'s intent in a transaction of its own, so it is durable before any OCFL
/// byte is written. Database I/O.
fn record_intent(conn: &Connection, commit: &CatalogCommit) -> Result<(), CatalogError> {
    let intent = PendingIntent {
        library_id: commit.library_id.clone(),
        operation_id: commit.operation_id.clone(),
        operation: commit.journal.operation.clone(),
        expected_revision: commit.expected_revision.clone(),
        recorded_at_ms: commit.journal.time_ms,
    };
    intent::record(conn, &intent)
}

/// Remove the intent of a commit that published nothing; a failure is logged to stdout and
/// leaves an intent that reconcile reports as pending. Database I/O.
fn discard_intent(conn: &Connection, operation_id: &str) {
    if let Err(err) = intent::discard(conn, operation_id) {
        tracing::warn!(
            target: super::LOG_TARGET,
            operation = "library.catalog.intent",
            error = ?err,
            "the intent of a failed commit was kept"
        );
    }
}

/// The transaction of [`apply_commit`]. Database and filesystem I/O.
fn commit_in_transaction(
    conn: &mut Connection,
    commit: &CatalogCommit,
    creates: bool,
    canonical: &Canonical<'_>,
) -> Result<CommitReceipt, Stopped> {
    let tx = conn.transaction().map_err(|e| Stopped::discard(sql(e)))?;
    let (receipt, purged) = apply_changes(&tx, commit, creates).map_err(Stopped::discard)?;
    if commit.changes.is_empty() {
        finish_unchanged(tx, commit).map_err(Stopped::discard)?;
        return Ok(receipt);
    }
    let entries = publish(&tx, commit, &receipt, purged, canonical)?;
    finish(tx, commit, &receipt, &entries).map_err(Stopped::keep)?;
    Ok(receipt)
}

/// Finish a checked commit with no change: remove its intent (no work remains) and the
/// intent's outbox entry, and commit `tx`, storing neither a receipt, a journal entry, a
/// snapshot version nor a new revision (owner decision, 2026-10-19). Database I/O.
fn finish_unchanged(tx: Transaction<'_>, commit: &CatalogCommit) -> Result<(), CatalogError> {
    intent::withdraw(&tx, commit.operation_id.as_str())?;
    tx.commit().map_err(sql)
}

/// Check the create or revision rule, apply every change and the new revision inside `tx`,
/// and answer the receipt with the virtual collections purged files were removed from (read
/// before the purge). Database I/O.
fn apply_changes(
    tx: &Transaction<'_>,
    commit: &CatalogCommit,
    creates: bool,
) -> Result<(CommitReceipt, Vec<SnapshotKey>), CatalogError> {
    let current = current_revision(tx)?;
    let revision = next_revision(creates, current, commit)?;
    let mut purged = project::purged_memberships(tx, &commit.changes)?;
    purged.extend(project::collection_deletions(tx, &commit.changes)?);
    for change in &commit.changes {
        apply_change(tx, &commit.library_id, change, commit.journal.time_ms)?;
    }
    tx.execute("UPDATE library SET revision = ?1", params![revision])
        .map_err(sql)?;
    let receipt = CommitReceipt {
        library_id: commit.library_id.clone(),
        revision: revision.to_string(),
        created_id: commit.created_id.clone(),
        deletion: commit.deletion,
    };
    Ok((receipt, purged))
}

/// Publish the snapshot version of what `commit` touched (with its PROV-O record and journal
/// entries when the host records history), between the probe's two stops, record its version
/// in `tx`, and answer the journal entries to store. Database, filesystem and journal-store
/// I/O.
fn publish(
    tx: &Transaction<'_>,
    commit: &CatalogCommit,
    receipt: &CommitReceipt,
    purged: Vec<SnapshotKey>,
    canonical: &Canonical<'_>,
) -> Result<Vec<JournalEntry>, Stopped> {
    let delta = snapshot_delta(tx, &commit.library_id, &commit.changes, purged)
        .map_err(Stopped::discard)?;
    project::check_entity_count(tx).map_err(Stopped::discard)?;
    let op = commit.operation_id.as_str();
    canonical.probe.before_snapshot(op).map_err(Stopped::keep)?;
    let record = OperationRecord {
        operation_id: op.to_string(),
        kind: Some(
            operation_kind(&commit.journal.operation)
                .as_str()
                .to_string(),
        ),
        revision: receipt.revision.clone(),
        created_id: receipt.created_id.clone(),
        deletion: receipt.deletion,
    };
    let plan = plan_commit(canonical.history.as_ref(), &record, &commit.journal);
    let published = canonical
        .root
        .commit(&delta, plan.meta(), plan.bound())
        .map_err(Stopped::discard)?
        .ok_or_else(|| Stopped::discard(failed("the changes left the snapshot as it was")))?;
    canonical.probe.after_snapshot(op).map_err(Stopped::keep)?;
    reconcile::set_snapshot_version(tx, &published.version.to_string()).map_err(Stopped::keep)?;
    Ok(plan.published(&published))
}

/// The snapshot delta of `changes` (every entity when the library has no snapshot yet).
/// Database I/O.
pub(crate) fn snapshot_delta(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    changes: &[CatalogChange],
    purged: Vec<SnapshotKey>,
) -> Result<SnapshotDelta, CatalogError> {
    match reconcile::snapshot_version(tx)? {
        Some(_) => project::changed_delta(tx, library_id, changes, purged),
        None => project::full_delta(tx, library_id),
    }
}

/// The snapshot delta of exactly `keys` (every entity when the library has no snapshot
/// yet). Database I/O.
pub(crate) fn delta_of_keys(
    tx: &Transaction<'_>,
    library_id: &LibraryId,
    keys: Vec<SnapshotKey>,
) -> Result<SnapshotDelta, CatalogError> {
    project::check_entity_count(tx)?;
    let has_snapshot = reconcile::snapshot_version(tx)?.is_some();
    let delta = if has_snapshot {
        project::delta_of(tx, library_id, keys)?
    } else {
        project::full_delta(tx, library_id)?
    };
    Ok(delta)
}

/// Store the receipt, remove the intent, push the journal `entries` and commit `tx`.
/// Database I/O.
fn finish(
    tx: Transaction<'_>,
    commit: &CatalogCommit,
    receipt: &CommitReceipt,
    entries: &[JournalEntry],
) -> Result<(), CatalogError> {
    store_receipt(&tx, commit.operation_id.as_str(), receipt)?;
    intent::clear(&tx, commit.operation_id.as_str())?;
    for entry in entries {
        outbox::push(&tx, entry)?;
    }
    tx.commit().map_err(sql)
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
pub(crate) fn same_library(library_id: &LibraryId, named: &LibraryId) -> Result<(), CatalogError> {
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
        CatalogChange::DeleteCollectionFiles {
            collection_id,
            deletion,
        } => tx
            .execute(
                "UPDATE files SET deleted_at_ms = ?2, purge_after_ms = ?3
                 WHERE home_collection_id = ?1 AND deleted_at_ms IS NULL",
                params![
                    collection_id.as_str(),
                    deletion.deleted_at_ms,
                    deletion.purge_after_ms
                ],
            )
            .map(drop)
            .map_err(sql),
        CatalogChange::PutVirtualCollection(v) => {
            same_library(library_id, v.library_id())?;
            put_virtual(tx, v)
        }
        CatalogChange::RemoveVirtualCollection(id) => exec(
            tx,
            "DELETE FROM virtual_collections WHERE id = ?1",
            id.as_str(),
        ),
        CatalogChange::AddVirtualMember(m) => add_member(tx, m),
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

/// Store one virtual-collection membership. Database I/O.
pub(crate) fn add_member(tx: &Transaction<'_>, m: &VirtualMembership) -> Result<(), CatalogError> {
    tx.execute(
        "INSERT INTO virtual_members (virtual_collection_id, file_id, order_key)
         VALUES (?1, ?2, ?3)",
        params![
            m.virtual_collection_id().as_str(),
            m.file_id().as_str(),
            m.order_key()
        ],
    )
    .map(drop)
    .map_err(sql)
}

/// Run a one-parameter statement. Database I/O.
fn exec(tx: &Transaction<'_>, statement: &str, id: &str) -> Result<(), CatalogError> {
    tx.execute(statement, params![id]).map(drop).map_err(sql)
}

/// Store the library row at revision 0 (the commit then sets its first revision).
/// Database I/O.
pub(crate) fn insert_library(
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

/// Cache the library's own retention period from its snapshot. Database I/O.
pub(crate) fn set_retention(tx: &Transaction<'_>, days: Option<u32>) -> Result<(), CatalogError> {
    tx.execute("UPDATE library SET retention_days = ?1", params![days])
        .map(drop)
        .map_err(sql)
}

/// Record the identity of the folder the index reflects. Database I/O.
pub(crate) fn set_folder_identity(conn: &Connection, identity: &str) -> Result<(), CatalogError> {
    conn.execute("UPDATE library SET folder_identity = ?1", params![identity])
        .map(drop)
        .map_err(sql)
}

/// Store a collection, replacing the one with the same identifier. Database I/O.
pub(crate) fn put_collection(tx: &Transaction<'_>, c: &Collection) -> Result<(), CatalogError> {
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
pub(crate) fn put_virtual(tx: &Transaction<'_>, v: &VirtualCollection) -> Result<(), CatalogError> {
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
pub(crate) fn set_file_state(
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
