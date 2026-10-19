//! The entities of a collection snapshot and their JSON encoding. Every function is pure.
//!
//! Each logical file holds `{"schema": 1, "entity": {"kind": ..., ...}}`. Titles and names
//! are stored as people gave them: the owner-readable local profile keeps the library
//! inspectable with ordinary tools (ADR 010); the encrypted profile seals the bytes as a
//! caller transform before staging ([`super::ProfileTransform`]).

use super::{CanonicalError, SnapshotKey};
use crate::library::{FileState, LibraryRecord};
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
}

/// A virtual collection with its members in membership order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualEntry {
    /// The virtual collection.
    pub collection: VirtualCollection,
    /// Its memberships, ordered by order key then file.
    pub members: Vec<VirtualMembership>,
}

/// A file's place in the library: its record, listing order and hide and delete state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// The file (identifier, library and home collection).
    pub file: File,
    /// Listing order.
    pub order: i64,
    /// Hidden flag and recoverable deletion.
    pub state: FileState,
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
    Ok(stored.entity)
}
