//! Where a library's canonical state lives in its OCFL storage root (the FileSquid
//! application profile, [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)).
//!
//! - One **collection snapshot** object per library at [`SNAPSHOT_ROOT`], object identifier
//!   `urn:uuid:<library-id>`. Each entity is one logical file, so a version changes only the
//!   entities a command touched: `library.json`, `collections/<id>.json`,
//!   `virtual-collections/<id>.json` (with its ordered members), `folders/<id>.json` and
//!   `files/<id>.json` (home collection, listing order, hide and delete state).
//! - One **file** object per FileSquid file at `files/<file-id>`, object identifier
//!   `urn:uuid:<file-id>`, its original under `original/<file-uuid>.<ext>` beside its metadata
//!   record `original/<file-uuid>.metadata.md` that holds the owner's original file name
//!   ([`super::original`]).
//!
//! Every function here is pure.

use super::CanonicalError;
use crate::library::CatalogChange;
use filesquid_core::{CollectionId, FileId, FolderId, LibraryId, VirtualCollectionId};

/// The object root of a library's collection snapshot inside its storage root.
pub const SNAPSHOT_ROOT: &str = "snapshot";

/// The logical path of the library entity.
pub const LIBRARY_PATH: &str = "library.json";

/// The logical directory of a file object's original.
pub const ORIGINAL_DIR: &str = "original";

/// Longest identifier or name accepted in a logical path.
pub const MAX_SEGMENT_BYTES: usize = 255;

/// One entity of a collection snapshot, named by its identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SnapshotKey {
    /// The library record and revision.
    Library,
    /// One collection.
    Collection(CollectionId),
    /// One virtual collection and its members.
    VirtualCollection(VirtualCollectionId),
    /// One folder.
    Folder(FolderId),
    /// One file's place and state.
    File(FileId),
}

impl SnapshotKey {
    /// The logical path of this entity in the snapshot object.
    ///
    /// # Errors
    /// [`CanonicalError::InvalidIdentifier`] when the identifier cannot name a path segment.
    pub fn path(&self) -> Result<String, CanonicalError> {
        let (dir, id) = match self {
            Self::Library => return Ok(LIBRARY_PATH.to_string()),
            Self::Collection(id) => ("collections", id.as_str()),
            Self::VirtualCollection(id) => ("virtual-collections", id.as_str()),
            Self::Folder(id) => ("folders", id.as_str()),
            Self::File(id) => ("files", id.as_str()),
        };
        let file = format!("{}.json", segment(id)?);
        Ok(format!("{dir}/{}", segment(&file)?))
    }

    /// The entity a logical path names, or `None` for a path outside the layout.
    pub fn from_path(path: &str) -> Option<Self> {
        if path == LIBRARY_PATH {
            return Some(Self::Library);
        }
        let (dir, file) = path.split_once('/')?;
        let id = file
            .strip_suffix(".json")
            .filter(|id| segment(id).is_ok())?;
        match dir {
            "collections" => CollectionId::new(id).ok().map(Self::Collection),
            "virtual-collections" => VirtualCollectionId::new(id)
                .ok()
                .map(Self::VirtualCollection),
            "folders" => FolderId::new(id).ok().map(Self::Folder),
            "files" => FileId::new(id).ok().map(Self::File),
            _ => None,
        }
    }
}

/// `text` when it can be one logical path segment: 1 to [`MAX_SEGMENT_BYTES`] bytes of
/// `[A-Za-z0-9._-]`, and not `.` or `..`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] otherwise.
pub fn segment(text: &str) -> Result<&str, CanonicalError> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    let ok = !text.is_empty()
        && text.len() <= MAX_SEGMENT_BYTES
        && text != "."
        && text != ".."
        && text.chars().all(allowed);
    ok.then_some(text)
        .ok_or_else(|| CanonicalError::InvalidIdentifier("not a path segment".into()))
}

/// The OCFL object identifier of a library's collection snapshot.
pub fn snapshot_object_id(library_id: &LibraryId) -> String {
    format!("urn:uuid:{}", library_id.as_str())
}

/// The OCFL object identifier of a file object.
pub fn file_object_id(file_id: &FileId) -> String {
    format!("urn:uuid:{}", file_id.as_str())
}

/// The object root of a file object inside its library's storage root.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when the identifier cannot name a path segment.
pub fn file_object_root(file_id: &FileId) -> Result<String, CanonicalError> {
    Ok(format!("files/{}", segment(file_id.as_str())?))
}

/// The logical path of a file object's original stored as `name`.
///
/// # Errors
/// [`CanonicalError::InvalidIdentifier`] when `name` cannot be one path segment.
pub fn original_path(name: &str) -> Result<String, CanonicalError> {
    Ok(format!("{ORIGINAL_DIR}/{}", segment(name)?))
}

/// The snapshot entities `change` writes or removes, each once. Every change also touches the
/// library entity, whose revision advances. A purged file also changes the virtual collections it
/// belonged to; only the catalog knows those, so the caller adds them.
pub fn touched(change: &CatalogChange) -> Vec<SnapshotKey> {
    let key = match change {
        CatalogChange::CreateLibrary(_) => return vec![SnapshotKey::Library],
        CatalogChange::PutCollection(c) => SnapshotKey::Collection(c.id().clone()),
        CatalogChange::RemoveCollection(id) => SnapshotKey::Collection(id.clone()),
        CatalogChange::PutVirtualCollection(v) => SnapshotKey::VirtualCollection(v.id().clone()),
        CatalogChange::RemoveVirtualCollection(id) => SnapshotKey::VirtualCollection(id.clone()),
        CatalogChange::AddVirtualMember(m) => {
            SnapshotKey::VirtualCollection(m.virtual_collection_id().clone())
        }
        CatalogChange::RemoveVirtualMember {
            virtual_collection_id,
            ..
        } => SnapshotKey::VirtualCollection(virtual_collection_id.clone()),
        CatalogChange::SetFileState { file_id, .. } => SnapshotKey::File(file_id.clone()),
        CatalogChange::PurgeFile(id) => SnapshotKey::File(id.clone()),
    };
    vec![SnapshotKey::Library, key]
}
