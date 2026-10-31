//! The tombstone of an expired file (Task API-F2.15; API-S-019, API-S-026, ADR 041 Decision
//! 11): what remains of a file object under `keep_version_history` once its retention ends.
//!
//! OCFL 1.1 cannot remove content from the past versions of an object, so the host deletes
//! the original object and recreates it, with the same identifier at the same path, as a new
//! object whose first version is the tombstone's creation (operation kind `tombstone`). That
//! version holds only history, never the file's bytes, metadata record or derivatives:
//!
//! - `tombstone/history.json` ([`TombstoneRecord`]): the facts of every former version
//!   (number, `created`, `user`, `message`, and each file's logical path, digest and size,
//!   without bytes), then the events of its end: the original object's deletion (`purge`)
//!   and the tombstone's creation (`tombstone`), each with its time, actor, kind, PROV-O
//!   activity and the operation ID that links it to its journal entries; and the purge
//!   record naming the expiry.
//! - `tombstone/provenance/<name>`: the former object's PROV-O records and journal extracts
//!   (`provenance/<version>.jsonld` and `.journal.jsonl`), copied as they were.
//! - `provenance/v1.jsonld` and `provenance/v1.journal.jsonl`: the tombstone version's own
//!   PROV-O record, written by the commit's recorder like every other version.
//!
//! A library recovered from OCFL alone reads such an object as purged history, never as a
//! file ([`is_tombstone`]). Every function here is either pure or names its storage I/O; none
//! takes a filesystem path.

use super::store::read_small;
use super::CanonicalError;
use crate::library::ExpiryMode;
use filesquid_core::OperationKind;
use filesquid_ocfl::change::{ChangeError, ChangeModel};
use filesquid_ocfl::read::{version_history, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageKey};
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};

/// Schema name of a tombstone record.
pub const TOMBSTONE_SCHEMA: &str = "filesquid.tombstone.v1";

/// Logical path of the tombstone record inside the tombstone object.
pub const TOMBSTONE_RECORD: &str = "tombstone/history.json";

/// Logical folder of the former object's PROV-O records inside the tombstone object.
pub const FORMER_PROVENANCE: &str = "tombstone/provenance";

/// Logical folder of PROV-O records in every object.
const PROVENANCE_DIR: &str = "provenance/";

/// Journal operation of the tombstone's creation; its kind is `tombstone`.
pub const TOMBSTONE_OPERATION: &str = "file.tombstone";

/// Most former versions a tombstone records (AGENTS.md §22).
pub const MAX_FORMER_VERSIONS: usize = 100_000;

/// Most former PROV-O files a tombstone copies.
pub const MAX_FORMER_PROVENANCE: usize = 20_000;

/// Largest former PROV-O file copied, in bytes.
pub const MAX_PROVENANCE_BYTES: u64 = 4 * 1024 * 1024;

/// Most stored keys one object deletion removes.
pub const MAX_OBJECT_KEYS: usize = 1_000_000;

/// Keys listed per page of an object deletion.
const KEY_PAGE: usize = 1_000;

/// One file of a former version: its logical path, digest and size, without its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormerFile {
    /// Its logical path in that version.
    pub path: String,
    /// Its content digest.
    pub digest: String,
    /// Its size in bytes, when the stored content could be measured.
    pub size: Option<u64>,
}

/// The facts of one former version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormerVersion {
    /// The version (`v1`, `v2`, ...).
    pub version: String,
    /// When it was created, as recorded.
    pub created: String,
    /// Who made it: the recorded name, when given.
    pub user_name: Option<String>,
    /// Who made it: the recorded address, when given.
    pub user_address: Option<String>,
    /// Its message, when given.
    pub message: Option<String>,
    /// Its files.
    pub files: Vec<FormerFile>,
}

/// One event of the tombstone's history: the original's deletion or the tombstone's creation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneEvent {
    /// `purge` or `tombstone`.
    pub kind: OperationKind,
    /// When it happened (RFC 3339 UTC).
    pub time: String,
    /// Who: `system` for the scheduled expiry, else the admin's opaque actor.
    pub actor: String,
    /// The operation ID shared with its journal entries.
    pub operation_id: String,
    /// Its PROV-O activity identifier (ADR 040).
    pub activity: String,
}

/// Why the original object's content was deleted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeRecord {
    /// Always `expiry`: the retention period ended.
    pub reason: String,
    /// The expiry setting that applied.
    pub expiry: ExpiryMode,
    /// The `purge` operation.
    pub operation_id: String,
    /// When the original object was deleted (RFC 3339 UTC).
    pub purged_at: String,
}

/// `tombstone/history.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneRecord {
    /// [`TOMBSTONE_SCHEMA`].
    pub schema: String,
    /// The object identifier, the same as the original's.
    pub object_id: String,
    /// The facts of every former version, oldest first.
    pub versions: Vec<FormerVersion>,
    /// The original's deletion, then the tombstone's creation.
    pub events: Vec<TombstoneEvent>,
    /// The purge record naming the expiry.
    pub purge: PurgeRecord,
}

/// What a tombstone keeps of the original object: its version facts and PROV-O files.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Former {
    /// The facts of every former version, oldest first.
    pub versions: Vec<FormerVersion>,
    /// The former PROV-O files by name (`v1.jsonld`, `v1.journal.jsonl`, ...).
    pub provenance: Vec<(String, Vec<u8>)>,
}

/// Facts of the expiry a tombstone records. Pure data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpiryFacts {
    /// The `purge` operation.
    pub operation_id: String,
    /// Who expired it.
    pub actor: String,
    /// When (RFC 3339 UTC).
    pub time: String,
    /// The PROV-O activity of the purge: the activity of the library snapshot version the
    /// purge committed.
    pub purge_activity: String,
}

/// The PROV-O activity identifier of `operation_id` on `object_id` at `version` (ADR 040), as
/// `filesquid-prov` writes it. Pure.
pub fn activity_id(operation_id: &str, object_id: &str, version: &str) -> String {
    filesquid_prov::activity_id(operation_id, object_id, version)
}

/// The record of a tombstone of `object_id` keeping `former`, for the expiry `facts`. Its
/// events are the original's deletion and the tombstone's creation, whose activity is that
/// of the tombstone's own first version. Pure.
pub fn tombstone_record(object_id: &str, former: &Former, facts: &ExpiryFacts) -> TombstoneRecord {
    let created = activity_id(&facts.operation_id, object_id, "v1");
    let event = |kind, activity: String| TombstoneEvent {
        kind,
        time: facts.time.clone(),
        actor: facts.actor.clone(),
        operation_id: facts.operation_id.clone(),
        activity,
    };
    TombstoneRecord {
        schema: TOMBSTONE_SCHEMA.to_string(),
        object_id: object_id.to_string(),
        versions: former.versions.clone(),
        events: vec![
            event(OperationKind::Purge, facts.purge_activity.clone()),
            event(OperationKind::Tombstone, created),
        ],
        purge: PurgeRecord {
            reason: "expiry".to_string(),
            expiry: ExpiryMode::KeepVersionHistory,
            operation_id: facts.operation_id.clone(),
            purged_at: facts.time.clone(),
        },
    }
}

/// Whether the object at `object_root` is a tombstone: its head holds [`TOMBSTONE_RECORD`].
/// A missing or unreadable object is not one. Storage I/O.
pub async fn is_tombstone<B: StorageBackend>(backend: &B, object_root: &str) -> bool {
    let Ok(root) = StorageRoot::open(backend).await else {
        return false;
    };
    let Ok(object) = root.open_object(object_root).await else {
        return false;
    };
    object
        .state(object.head())
        .is_ok_and(|state| state.get(TOMBSTONE_RECORD).is_some())
}

/// What a tombstone keeps of the object at `object_root`: every version's facts and the head's
/// PROV-O files; `None` when no readable object is there. Storage I/O.
///
/// # Errors
/// A storage failure, or more versions or PROV-O files than the bounds.
pub async fn read_former<B: StorageBackend>(
    backend: &B,
    object_root: &str,
) -> Result<Option<Former>, CanonicalError> {
    let root = StorageRoot::open(backend).await?;
    let Ok(object) = root.open_object(object_root).await else {
        return Ok(None);
    };
    let history = version_history(object.inventory());
    if history.len() > MAX_FORMER_VERSIONS {
        return Err(CanonicalError::Failed(
            "too many versions for a tombstone".into(),
        ));
    }
    let mut former = Former::default();
    for info in history {
        let state = object.state(&info.version)?;
        let mut files = Vec::with_capacity(state.len());
        for file in state.files() {
            let size = match object.content_key(file) {
                Ok(key) => backend.head(&key).await.ok().map(|meta| meta.size),
                Err(_) => None,
            };
            files.push(FormerFile {
                path: file.path.clone(),
                digest: file.digest.clone(),
                size,
            });
        }
        former.versions.push(FormerVersion {
            version: info.version.to_string(),
            created: info.created.clone(),
            user_name: info.user.as_ref().map(|u| u.name.clone()),
            user_address: info.user.as_ref().and_then(|u| u.address.clone()),
            message: info.message.clone(),
            files,
        });
    }
    former.provenance = read_provenance(&object, backend).await?;
    Ok(Some(former))
}

/// The head's PROV-O files of `object`, by name, each at most [`MAX_PROVENANCE_BYTES`].
async fn read_provenance<B: StorageBackend>(
    object: &filesquid_ocfl::read::Object<'_, B>,
    backend: &B,
) -> Result<Vec<(String, Vec<u8>)>, CanonicalError> {
    let state = object.state(object.head())?;
    let mut files = Vec::new();
    for file in state.files().filter(|f| f.path.starts_with(PROVENANCE_DIR)) {
        if files.len() >= MAX_FORMER_PROVENANCE {
            return Err(CanonicalError::Failed(
                "too many PROV-O files for a tombstone".into(),
            ));
        }
        let name = file.path.trim_start_matches(PROVENANCE_DIR).to_string();
        let bytes = read_small(object, backend, file, MAX_PROVENANCE_BYTES).await?;
        files.push((name, bytes));
    }
    Ok(files)
}

/// Delete every stored key of the object at `object_root`, at most [`MAX_OBJECT_KEYS`];
/// answer how many were deleted. Deleting what is already gone succeeds, so a deletion
/// stopped part-way is completed by running it again. Storage I/O.
///
/// # Errors
/// A storage failure, or an object larger than the bound (nothing past it is deleted).
pub async fn delete_object<B: StorageBackend>(
    backend: &B,
    object_root: &str,
) -> Result<usize, CanonicalError> {
    let prefix = format!("{object_root}/");
    let mut deleted = 0;
    for _ in 0..=MAX_OBJECT_KEYS / KEY_PAGE {
        let page = backend
            .list(&prefix, None, KEY_PAGE)
            .await
            .map_err(storage)?;
        if page.keys.is_empty() {
            return Ok(deleted);
        }
        for key in &page.keys {
            backend.delete(key).await.map_err(storage)?;
            deleted += 1;
        }
    }
    Err(CanonicalError::Failed(
        "the object holds too many stored files".into(),
    ))
}

/// Stage the tombstone's content in `model`: the record and the former PROV-O files. The
/// caller commits the version with the `tombstone` operation's metadata. Storage I/O.
///
/// # Errors
/// A staging failure.
pub async fn stage_tombstone<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    record: &TombstoneRecord,
    former: &Former,
) -> Result<(), CanonicalError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    model.reset().await.map_err(change)?;
    let bytes = encode_record(record)?;
    model
        .stage_bytes(TOMBSTONE_RECORD, &bytes)
        .await
        .map_err(change)?;
    for (name, bytes) in &former.provenance {
        let path = format!("{FORMER_PROVENANCE}/{name}");
        model.stage_bytes(&path, bytes).await.map_err(change)?;
    }
    Ok(())
}

/// The JSON bytes of `record`. Pure.
///
/// # Errors
/// An encoding failure.
pub fn encode_record(record: &TombstoneRecord) -> Result<Vec<u8>, CanonicalError> {
    let mut bytes =
        serde_json::to_vec_pretty(record).map_err(|e| CanonicalError::Encode(e.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The storage key of `text`, for callers that keep a tombstone's payload outside the root.
///
/// # Errors
/// A key the backend would refuse.
pub fn key(text: &str) -> Result<StorageKey, CanonicalError> {
    StorageKey::new(text).map_err(storage)
}

/// A canonical error of a storage failure, without its path.
fn storage(err: filesquid_ocfl::storage::StorageError) -> CanonicalError {
    CanonicalError::from(filesquid_ocfl::read::ReadError::Storage(err))
}

/// A canonical error of a staging failure.
fn change(err: ChangeError) -> CanonicalError {
    CanonicalError::from(err)
}
