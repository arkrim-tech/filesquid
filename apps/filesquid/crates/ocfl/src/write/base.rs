//! What a commit builds on: the object's committed inventory, and the root
//! inventory as stored, whose entity tag the head compare-and-swap needs.
//!
//! Three states are possible: no object (no root inventory), a committed
//! object (possibly with an unpublished head a crashed writer left, see
//! [`crate::read`]), or an object whose first version was never committed
//! (root inventory for `v1` written, sidecar not yet: E058).

use super::error::CommitError;
use crate::inventory::{Inventory, VersionNum, INVENTORY_FILE, MAX_INVENTORY_BYTES};
use crate::read::committed::{at_inventory, read_committed, Committed};
use crate::read::small::{key_in, read_small_tagged};
use crate::read::ReadError;
use crate::storage::{ETag, StorageBackend, StorageError};

/// The base of a commit.
#[derive(Debug, Clone, Default)]
pub(crate) struct Base {
    /// The committed inventory, or `None` when no version is committed.
    pub committed: Option<Committed>,
    /// The root inventory's entity tag, or `None` when there is none.
    pub root_tag: Option<ETag>,
}

impl Base {
    /// The committed inventory.
    pub fn inventory(&self) -> Option<&Inventory> {
        self.committed.as_ref().map(|c| &c.inventory)
    }

    /// The committed head.
    pub fn head(&self) -> Option<&VersionNum> {
        self.inventory().map(|i| &i.head)
    }

    /// Whether the root inventory names a head its sidecar does not commit.
    pub fn root_is_ahead(&self) -> bool {
        self.committed
            .as_ref()
            .is_some_and(|c| c.unpublished.is_some())
    }
}

/// Read the base of a commit to the object at `root`.
///
/// # Errors
///
/// [`CommitError::Read`] for an object that cannot be read, or a backend
/// error.
pub(crate) async fn read_base<B: StorageBackend>(
    storage: &B,
    root: &str,
) -> Result<Base, CommitError> {
    let key = key_in(root, INVENTORY_FILE)?;
    match storage.head(&key).await {
        Ok(_) => {}
        Err(StorageError::NotFound { .. }) => return Ok(Base::default()),
        Err(err) => return Err(err.into()),
    }
    match read_committed(storage, root).await {
        Ok(committed) => Ok(Base {
            root_tag: Some(committed.root_tag.clone()),
            committed: Some(committed),
        }),
        Err(err) if err.code() == Some("E058") => uncommitted_first(storage, root, err).await,
        Err(err) => Err(err.into()),
    }
}

/// A root inventory without a sidecar is an uncommitted object when it
/// names only a first version; otherwise the missing sidecar `err` stands.
async fn uncommitted_first<B: StorageBackend>(
    storage: &B,
    root: &str,
    err: ReadError,
) -> Result<Base, CommitError> {
    let key = key_in(root, INVENTORY_FILE)?;
    let (bytes, tag) = read_small_tagged(storage, &key, MAX_INVENTORY_BYTES, "inventory").await?;
    let inventory = Inventory::parse(&bytes).map_err(|e| at_inventory(e, &key))?;
    if inventory.versions.len() != 1 {
        return Err(err.into());
    }
    tracing::warn!(object = %inventory.id, "the first version was never committed");
    Ok(Base {
        committed: None,
        root_tag: Some(tag),
    })
}
