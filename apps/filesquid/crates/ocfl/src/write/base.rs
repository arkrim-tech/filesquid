//! What a commit builds on: the object's committed inventory, and the root
//! inventory as stored, whose entity tag the head compare-and-swap needs.
//!
//! Three states are possible: no object (no root inventory), a committed
//! object (possibly with an unpublished head a crashed writer left, see
//! [`crate::read`]), or an object whose first version was never committed
//! (root inventory for `v1` written, sidecar not yet: E058).

use super::error::CommitError;
use crate::inventory::{
    sidecar_name, verify_sidecar, Inventory, VersionNum, INVENTORY_FILE, MAX_INVENTORY_BYTES,
    MAX_SIDECAR_BYTES,
};
use crate::read::committed::{at_inventory, read_committed, Committed};
use crate::read::small::{key_in, read_small, read_small_tagged};
use crate::read::ReadError;
use crate::storage::{ETag, StorageBackend, StorageError, StorageKey};

/// The base of a commit.
#[derive(Debug, Clone, Default)]
pub(crate) struct Base {
    /// The committed inventory, or `None` when no version is committed.
    pub committed: Option<Committed>,
    /// The root inventory's entity tag, or `None` when there is none.
    pub root_tag: Option<ETag>,
    /// The object id of a first version that was never committed: its root
    /// inventory stays in place until a commit of the same object replaces
    /// it, so a commit for another id cannot take the object root over.
    pub pending_id: Option<String>,
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

    /// Refuse a draft for another object than the one an uncommitted first
    /// version names.
    ///
    /// # Errors
    ///
    /// [`CommitError::InvalidDraft`] when the ids differ.
    pub fn check_pending_id(&self, object_id: &str) -> Result<(), CommitError> {
        match &self.pending_id {
            Some(id) if id != object_id => Err(CommitError::draft(format!(
                "the draft is for {object_id:?}, the uncommitted object is {id:?}"
            ))),
            _ => Ok(()),
        }
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
            pending_id: None,
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
        pending_id: Some(inventory.id),
    })
}

/// Require the committed head's own version inventory, byte-identical to
/// the committed inventory, and a sidecar that matches it. A reader finds
/// the committed version through that inventory while a new root inventory
/// is not yet committed by its sidecar, so without it a crash
/// mid-publication would leave the object unreadable (E060). A head whose
/// version inventory has no sidecar (an interrupted version-inventory
/// repair, OCFL-S-020) is refused too, so the repair is completed before
/// the head becomes an older version.
///
/// # Errors
///
/// [`CommitError::MissingVersionInventory`] naming the first missing file,
/// [`CommitError::VersionInventoryMismatch`] for a version inventory that
/// differs from the committed inventory or a sidecar that does not match
/// it, or a backend error.
pub(crate) async fn require_head_inventory<B: StorageBackend>(
    storage: &B,
    root: &str,
    base: &Base,
) -> Result<(), CommitError> {
    let Some(committed) = base.committed.as_ref() else {
        return Ok(());
    };
    let inventory = &committed.inventory;
    let head = &inventory.head;
    let key = key_in(root, &format!("{head}/{INVENTORY_FILE}"))?;
    let bytes = read_head_file(storage, &key, MAX_INVENTORY_BYTES).await?;
    if bytes != committed.bytes {
        return Err(CommitError::VersionInventoryMismatch {
            key: key.to_string(),
        });
    }
    let algorithm = inventory.digest_algorithm;
    let sidecar_key = key_in(root, &format!("{head}/{}", sidecar_name(algorithm)))?;
    let sidecar = read_head_file(storage, &sidecar_key, MAX_SIDECAR_BYTES).await?;
    if verify_sidecar(algorithm, &bytes, &sidecar).is_err() {
        return Err(CommitError::VersionInventoryMismatch {
            key: sidecar_key.to_string(),
        });
    }
    Ok(())
}

/// Read one file of the head version directory, at most `limit` bytes.
///
/// # Errors
///
/// [`CommitError::MissingVersionInventory`] when it is absent,
/// [`CommitError::VersionInventoryMismatch`] when it is above `limit`, or a
/// backend error.
async fn read_head_file<B: StorageBackend>(
    storage: &B,
    key: &StorageKey,
    limit: usize,
) -> Result<Vec<u8>, CommitError> {
    match read_small(storage, key, limit, "head version inventory").await {
        Ok(bytes) => Ok(bytes),
        Err(ReadError::Storage(StorageError::NotFound { .. })) => {
            Err(CommitError::MissingVersionInventory {
                key: key.to_string(),
            })
        }
        Err(ReadError::Format(_)) => Err(CommitError::VersionInventoryMismatch {
            key: key.to_string(),
        }),
        Err(err) => Err(err.into()),
    }
}
