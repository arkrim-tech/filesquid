//! Opening an OCFL storage root and discovering its objects (OCFL 1.1
//! section 4; ADR 023 Decision 2).

use std::fmt;

use super::declaration::read_declaration;
use super::error::ReadError;
use super::object::Object;
use crate::guard::checked_layout;
use crate::namaste::{Declaration, DeclarationKind};
use crate::storage::StorageBackend;

/// One opened storage root: its declaration and the object roots found in
/// it when it was opened.
pub struct StorageRoot<'a, B: StorageBackend> {
    backend: &'a B,
    declaration: Declaration,
    objects: Vec<String>,
}

impl<'a, B: StorageBackend> StorageRoot<'a, B> {
    /// Open the storage root at the root of `backend`.
    ///
    /// Checks its declaration, then runs the storage-root guard
    /// ([`check_storage_root`](crate::check_storage_root): no `.git` above
    /// the objects or in an object root, no links or empty directories) and
    /// discovers the object roots in the same paged listing: every
    /// directory holding an `0=ocfl_object_*` declaration that is not inside
    /// another object. The listing reads at most
    /// [`MAX_WALK_ENTRIES`](crate::storage::MAX_WALK_ENTRIES) keys; a larger
    /// root is refused as over the limit, never partly discovered.
    ///
    /// # Errors
    ///
    /// [`ReadError::Format`] with E069 or E080 for the declaration;
    /// [`ReadError::Guard`] for forbidden Git metadata;
    /// [`ReadError::ObjectAtStorageRoot`] when the storage root also declares
    /// an object; [`ReadError::Storage`] for a link, an empty directory, the
    /// key bound or a backend error.
    pub async fn open(backend: &'a B) -> Result<Self, ReadError> {
        let declaration = read_declaration(backend, "", DeclarationKind::StorageRoot).await?;
        let objects = checked_layout(backend).await?.outermost_object_roots();
        if objects.iter().any(String::is_empty) {
            return Err(ReadError::ObjectAtStorageRoot);
        }
        tracing::debug!(objects = objects.len(), "opened OCFL storage root");
        Ok(Self {
            backend,
            declaration,
            objects,
        })
    }

    /// The storage root's conformance declaration.
    pub fn declaration(&self) -> Declaration {
        self.declaration
    }

    /// The object roots discovered when the storage root was opened, as key
    /// prefixes in lexical order.
    pub fn objects(&self) -> &[String] {
        &self.objects
    }

    /// Open the object whose root is `root` (one of [`objects`](Self::objects)
    /// or any other object root in this storage root).
    ///
    /// # Errors
    ///
    /// As [`Object::open`].
    pub async fn open_object(&self, root: &str) -> Result<Object<'a, B>, ReadError> {
        Object::open(self.backend, root).await
    }
}

impl<B: StorageBackend> fmt::Debug for StorageRoot<'_, B> {
    /// Show the storage root; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageRoot")
            .field("declaration", &self.declaration)
            .field("objects", &self.objects)
            .finish_non_exhaustive()
    }
}
