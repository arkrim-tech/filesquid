//! The entities of a collection snapshot and their JSON encoding. Every function is pure.
//!
//! Each logical file holds `{"schema": 1, "entity": {"kind": ..., ...}}`. Titles and names
//! are stored as people gave them: the owner-readable local profile keeps the library
//! inspectable with ordinary tools (ADR 010); the encrypted profile seals the bytes as a
//! caller transform before staging ([`super::ProfileTransform`]).

use super::collection_path::check_collection_path;
use super::{file_object_id, CanonicalError, SnapshotKey};
use crate::library::{FileState, LibraryRecord, MAX_RETENTION_DAYS};
use filesquid_core::{Collection, File, Folder, VirtualCollection, VirtualMembership};
use serde::{Deserialize, Serialize};

/// Version of the entity encoding written by this host.
pub const ENTITY_SCHEMA: u32 = 1;

/// The library record with its revision counter and creation order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryHead {
    /// The library and its name.
    pub record: LibraryRecord,
    /// The catalog revision this snapshot version holds.
    pub revision: u64,
    /// Creation order (Unix milliseconds of the create command).
    pub created_order: i64,
    /// The library's own retention period of recoverably deleted files, in days (1 to
    /// [`MAX_RETENTION_DAYS`]), when the owner set one; the installation default applies
    /// otherwise (owner decision 2026-10-22, ADR 041 Decision 10). Absent in entities
    /// written before Task DE-1.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

/// A virtual collection with its members in membership order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualEntry {
    /// The virtual collection.
    pub collection: VirtualCollection,
    /// Its memberships, ordered by order key then file.
    pub members: Vec<VirtualMembership>,
}

/// A file's place in the library: its record, listing order, hide and delete state and,
/// since Task DE-1.7, its collection logical path and file object (ADR 041 Decision 5).
///
/// A recoverably deleted file keeps its home collection's identifier, even after that
/// collection is deleted, its path and its object: deletion is a state, nothing is moved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// The file (identifier, library and home collection).
    pub file: File,
    /// Listing order.
    pub order: i64,
    /// Hidden flag and recoverable deletion.
    pub state: FileState,
    /// The file's collection logical path inside its home collection
    /// ([`super::collection_path`]); absent in entities written before Task DE-1.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The OCFL object identifier of the file's own object (`urn:uuid:<file-id>`); absent in
    /// entities written before Task DE-1.7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    /// The title its home collection had when that collection was deleted (Task API-F2.15,
    /// ADR 041 Decision 8), so a rebuilt index still names it in Recently deleted; absent
    /// while the home collection exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_home_title: Option<String>,
}

impl SnapshotFile {
    /// Whether the entity registers the file at a path of its home collection and names its
    /// object. An entity of the earlier form does not, and a reader reports it as
    /// unregistered (API-S-026).
    pub fn is_registered(&self) -> bool {
        self.path.is_some() && self.object_id.is_some()
    }
}

/// One entity of a collection snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SnapshotEntity {
    /// The library entity.
    Library(LibraryHead),
    /// A collection.
    Collection(Collection),
    /// A virtual collection with its members.
    VirtualCollection(VirtualEntry),
    /// A folder.
    Folder(Folder),
    /// A file's place and state.
    File(SnapshotFile),
}

impl SnapshotEntity {
    /// The key that names this entity.
    pub fn key(&self) -> SnapshotKey {
        match self {
            Self::Library(_) => SnapshotKey::Library,
            Self::Collection(c) => SnapshotKey::Collection(c.id().clone()),
            Self::VirtualCollection(v) => SnapshotKey::VirtualCollection(v.collection.id().clone()),
            Self::Folder(f) => SnapshotKey::Folder(f.id().clone()),
            Self::File(f) => SnapshotKey::File(f.file.id().clone()),
        }
    }
}

/// The stored form of an entity.
#[derive(Serialize, Deserialize)]
struct Stored {
    schema: u32,
    entity: SnapshotEntity,
}

/// `entity` as the bytes of its logical file.
///
/// # Errors
/// [`CanonicalError::Encode`] when it cannot be serialized.
pub fn encode(entity: &SnapshotEntity) -> Result<Vec<u8>, CanonicalError> {
    let stored = Stored {
        schema: ENTITY_SCHEMA,
        entity: entity.clone(),
    };
    let mut bytes =
        serde_json::to_vec_pretty(&stored).map_err(|e| CanonicalError::Encode(e.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The entity stored at logical `path`.
///
/// # Errors
/// [`CanonicalError::Decode`] for bytes that are not an entity of [`ENTITY_SCHEMA`], an
/// entity whose identifier cannot name a path, or one whose key does not match its path.
pub fn decode(path: &str, bytes: &[u8]) -> Result<SnapshotEntity, CanonicalError> {
    let refuse = |reason: String| CanonicalError::Decode {
        path: path.to_string(),
        reason,
    };
    // A fixed reason: serde's message can quote a stored value, such as a title.
    let stored: Stored =
        serde_json::from_slice(bytes).map_err(|_| refuse("not a snapshot entity".into()))?;
    if stored.schema != ENTITY_SCHEMA {
        return Err(refuse(format!("unknown schema {}", stored.schema)));
    }
    let expected = stored
        .entity
        .key()
        .path()
        .map_err(|_| refuse("entity identifier is not a path segment".into()))?;
    if expected != path {
        return Err(refuse("entity does not match its path".into()));
    }
    check_entity(&stored.entity).map_err(|reason| refuse(reason.into()))?;
    Ok(stored.entity)
}

/// Check the fields Task DE-1.7 added: a file's path follows the collection-path rule and its
/// object is its own, both present or both absent; a library's retention period lies within
/// 1 to [`MAX_RETENTION_DAYS`] days. Answers a fixed reason, never a stored value.
fn check_entity(entity: &SnapshotEntity) -> Result<(), &'static str> {
    match entity {
        SnapshotEntity::File(file) => check_placement(file),
        SnapshotEntity::Library(head) => match head.retention_days {
            Some(days) if !(1..=MAX_RETENTION_DAYS).contains(&days) => {
                Err("retention period out of range")
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

/// Check a file entity's path and object.
fn check_placement(file: &SnapshotFile) -> Result<(), &'static str> {
    match (&file.path, &file.object_id) {
        (None, None) => Ok(()),
        (Some(path), Some(object_id)) => {
            check_collection_path(path)
                .map_err(|_| "file path is not a collection logical path")?;
            match *object_id == file_object_id(file.file.id()) {
                true => Ok(()),
                false => Err("file object is not the file's own"),
            }
        }
        _ => Err("file path and object must be recorded together"),
    }
}
