//! Opening one OCFL object and reading its committed versions (OCFL-S-004;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Published head (readers)").
//!
//! [`Object::open`] does all the I/O of opening: the Git and hierarchy
//! guards on the object root (OCFL-S-001), the object declaration, the root
//! inventory with its sidecar, and the head version inventory when there is
//! one. Everything after that is either pure ([`Object::state`],
//! [`Object::history`], [`Object::diff`]) or reads content
//! ([`Object::stream_file`], [`Object::copy_file`]).

use std::fmt;

use super::committed::read_committed;
use super::copy_guard::refuse_copy_into_ocfl_root;
use super::declaration::read_declaration;
use super::error::ReadError;
use super::history::{diff_versions, version_history, Change, VersionInfo};
use super::small::{key_in, read_small, root_dir};
use super::state::{resolve_state, LogicalFile, LogicalState};
use super::stream::{copy_to, LogicalFileReader};
use crate::guard::reject_git_in_root;
use crate::inventory::{Inventory, VersionNum, INVENTORY_FILE, MAX_INVENTORY_BYTES};
use crate::namaste::{Declaration, DeclarationKind};
use crate::storage::{
    ObjectMeta, Precondition, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES,
};

/// One opened OCFL object: its root, declaration and committed inventory,
/// read once. Versions above the committed `head` are unpublished and
/// invisible here.
pub struct Object<'a, B: StorageBackend> {
    backend: &'a B,
    root: String,
    declaration: Declaration,
    inventory: Inventory,
    unpublished: Option<VersionNum>,
}

impl<'a, B: StorageBackend> Object<'a, B> {
    /// Open the object whose root is the directory `root` of `backend`
    /// (empty for the backend's own root, with or without a trailing `/`).
    ///
    /// Runs the Git and hierarchy guards on the object root, checks the one
    /// object declaration, reads the root inventory and its sidecar
    /// (re-reading both, at most [`READ_ATTEMPTS`](crate::storage::READ_ATTEMPTS)
    /// times, when a writer replaced the inventory between the two reads),
    /// and checks that the head version's inventory, when present, equals
    /// the committed inventory.
    ///
    /// A root inventory whose sidecar still holds the digest of the previous
    /// version's inventory names a head that is not committed yet (a writer
    /// is publishing it, or crashed before its sidecar). The object then
    /// opens at the previous version, and
    /// [`unpublished_head`](Self::unpublished_head) names the claimed head
    /// (ADR 016 "Published head (readers)").
    ///
    /// # Errors
    ///
    /// [`ReadError::Guard`] for a `.git` in the object root;
    /// [`ReadError::Storage`] for a link, empty directory or backend error;
    /// [`ReadError::UnsupportedVersion`] for an object that is not OCFL 1.1;
    /// [`ReadError::Format`] with E003 or E007 (declaration), E063 (no root
    /// inventory), E058, E060 or E061 (sidecar), E064 (head version
    /// inventory differs), any code [`Inventory::parse`] gives, or an engine
    /// size bound.
    pub async fn open(backend: &'a B, root: &str) -> Result<Self, ReadError> {
        let root = root_dir(root)?;
        guard_object_root(backend, &root).await?;
        let declaration = read_declaration(backend, &root, DeclarationKind::Object).await?;
        let committed = read_committed(backend, &root).await?;
        let inventory = committed.inventory;
        check_head_inventory(backend, &root, &inventory, &committed.bytes).await?;
        tracing::debug!(object = %inventory.id, head = %inventory.head, "opened OCFL object");
        Ok(Self {
            backend,
            root,
            declaration,
            inventory,
            unpublished: committed.unpublished,
        })
    }

    /// The object root, as a key prefix without a trailing `/`.
    pub fn root(&self) -> &str {
        &self.root
    }

    /// The object's conformance declaration.
    pub fn declaration(&self) -> Declaration {
        self.declaration
    }

    /// The root inventory as read and checked when the object was opened.
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// The object identifier.
    pub fn id(&self) -> &str {
        &self.inventory.id
    }

    /// The most recent committed version.
    pub fn head(&self) -> &VersionNum {
        &self.inventory.head
    }

    /// The head the root inventory names when its sidecar does not commit
    /// it: a version being published, or left unpublished by a crash. It
    /// is not readable; `None` when the root inventory is committed.
    pub fn unpublished_head(&self) -> Option<&VersionNum> {
        self.unpublished.as_ref()
    }

    /// The logical state of the caller-pinned `version`.
    ///
    /// # Errors
    ///
    /// As [`resolve_state`](super::resolve_state).
    pub fn state(&self, version: &VersionNum) -> Result<LogicalState, ReadError> {
        resolve_state(&self.inventory, version)
    }

    /// Every committed version, oldest first.
    pub fn history(&self) -> Vec<VersionInfo> {
        version_history(&self.inventory)
    }

    /// The changes from version `from` to version `to`.
    ///
    /// # Errors
    ///
    /// As [`diff_versions`](super::diff_versions).
    pub fn diff(&self, from: &VersionNum, to: &VersionNum) -> Result<Vec<Change>, ReadError> {
        diff_versions(&self.inventory, from, to)
    }

    /// The storage key that holds `file`'s bytes.
    ///
    /// # Errors
    ///
    /// [`ReadError::Storage`] when the content path cannot be a key.
    pub fn content_key(&self, file: &LogicalFile) -> Result<StorageKey, ReadError> {
        key_in(&self.root, &file.content_path)
    }

    /// Prepare to stream the logical file `path` of the pinned `version` in
    /// chunks of `chunk_bytes` (1 to [`MAX_CHUNK_BYTES`]). No I/O happens
    /// until the first [`LogicalFileReader::next_chunk`].
    ///
    /// # Errors
    ///
    /// [`ReadError::VersionNotFound`], [`ReadError::PathNotFound`], or
    /// [`ReadError::Storage`] for a chunk size outside the bound.
    pub fn stream_file(
        &self,
        version: &VersionNum,
        path: &str,
        chunk_bytes: u64,
    ) -> Result<LogicalFileReader<'a, B>, ReadError> {
        let state = self.state(version)?;
        let file = state.get(path).ok_or_else(|| ReadError::PathNotFound {
            version: version.to_string(),
            path: path.to_string(),
        })?;
        let key = self.content_key(file)?;
        let algorithm = self.inventory.digest_algorithm;
        LogicalFileReader::new(self.backend, key, file, algorithm, chunk_bytes)
    }

    /// Copy the logical file `path` of the pinned `version` to `dest_key` on
    /// `dest` (any backend, this one included), streaming it in chunks of
    /// [`MAX_CHUNK_BYTES`] through a multipart upload. The copy is published
    /// only when every byte matched the inventory digest and `precondition`,
    /// when given, holds; otherwise nothing is published at `dest_key`.
    ///
    /// A copy never edits an OCFL object (OCFL-S-019): `dest_key` MUST NOT
    /// lie inside an OCFL object root or storage root on `dest`, which is
    /// checked before anything is read or written. New versions are
    /// published by [`ObjectWriter`](crate::write::ObjectWriter) only.
    ///
    /// # Errors
    ///
    /// [`ReadError::CopyIntoOcflRoot`] for a destination inside an OCFL
    /// root; as [`stream_file`](Self::stream_file) and
    /// [`LogicalFileReader::next_chunk`]; a destination backend error; or
    /// [`ReadError::Storage`] with [`StorageError::LimitExceeded`] for a
    /// `dest_key` deeper than 64 directory levels, or for a directory above
    /// it holding more than [`MAX_LIST_PAGE`](crate::storage::MAX_LIST_PAGE)
    /// keys named like an OCFL declaration, refused before any copy.
    pub async fn copy_file<D: StorageBackend>(
        &self,
        version: &VersionNum,
        path: &str,
        dest: &D,
        dest_key: &StorageKey,
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, ReadError> {
        let source = self.stream_file(version, path, MAX_CHUNK_BYTES)?;
        refuse_copy_into_ocfl_root(dest, dest_key).await?;
        copy_to(source, dest, dest_key.clone(), precondition).await
    }
}

impl<B: StorageBackend> fmt::Debug for Object<'_, B> {
    /// Show the object; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Object")
            .field("root", &self.root)
            .field("id", &self.inventory.id)
            .field("head", &self.inventory.head)
            .field("unpublished", &self.unpublished)
            .finish_non_exhaustive()
    }
}

/// Run the Git guard and the hierarchy check on one object root (OCFL-S-001).
async fn guard_object_root<B: StorageBackend>(backend: &B, root: &str) -> Result<(), ReadError> {
    reject_git_in_root(backend, root).await?;
    backend.check_hierarchy(root).await?;
    Ok(())
}

/// When the head version directory holds an inventory, it must be the same
/// file as the root inventory (E064). A missing one is allowed (OCFL
/// recommends it, W010). It is read directly, without an existence check
/// first, so one that disappears during the read counts as missing.
async fn check_head_inventory<B: StorageBackend>(
    backend: &B,
    root: &str,
    inventory: &Inventory,
    root_bytes: &[u8],
) -> Result<(), ReadError> {
    let key = key_in(root, &format!("{}/{INVENTORY_FILE}", inventory.head))?;
    let bytes = match read_small(backend, &key, MAX_INVENTORY_BYTES, "inventory").await {
        Ok(bytes) => bytes,
        Err(ReadError::Storage(StorageError::NotFound { .. })) => return Ok(()),
        Err(err) => return Err(err),
    };
    if bytes == root_bytes {
        return Ok(());
    }
    Err(ReadError::invalid(
        "E064",
        key.as_str(),
        "the head version inventory differs from the root inventory",
    ))
}
