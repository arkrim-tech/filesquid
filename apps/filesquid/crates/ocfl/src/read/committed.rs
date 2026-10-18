//! Reading the committed root inventory of an object (OCFL-S-004,
//! OCFL-S-018; [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Published head (readers)").
//!
//! A version is committed only when the root inventory names it as `head`
//! and the root sidecar matches the root inventory. A writer replaces the
//! root inventory first and its sidecar last, so between the two (or after
//! a crash between them) the sidecar still holds the digest of the previous
//! inventory, which is also the previous version's own inventory. A reader
//! then sees an **unpublished** head: it opens the previous version, whose
//! inventory the sidecar still commits, and reports the head the root
//! inventory claims as unpublished. Any other mismatch is refused (E060).

use super::error::ReadError;
use super::small::{at_key, key_in, missing_as, read_small, read_small_tagged};
use crate::inventory::{
    sidecar_name, verify_sidecar, FormatError, Inventory, VersionNum, INVENTORY_FILE,
    MAX_INVENTORY_BYTES, MAX_SIDECAR_BYTES,
};
use crate::storage::{ETag, StorageBackend, StorageError, StorageKey, READ_ATTEMPTS};

/// The committed inventory of one object, as read.
#[derive(Debug, Clone)]
pub(crate) struct Committed {
    /// The committed inventory.
    pub inventory: Inventory,
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// The entity tag of the root inventory as read (not the committed
    /// inventory's when the head is unpublished).
    pub root_tag: ETag,
    /// The head the root inventory names when its sidecar does not commit
    /// it yet.
    pub unpublished: Option<VersionNum>,
}

/// One read of the root inventory and its sidecar.
struct RootRead {
    inventory: Inventory,
    bytes: Vec<u8>,
    tag: ETag,
    sidecar: Vec<u8>,
    sidecar_key: StorageKey,
}

/// Read the committed inventory of the object at `root`. A sidecar that
/// does not match (E060) may mean a writer replaced the inventory between
/// the two reads, so both are read again, at most [`READ_ATTEMPTS`] times.
///
/// # Errors
///
/// [`ReadError::Format`] with E063 (no root inventory), E058, E060 or E061
/// (sidecar), any code [`Inventory::parse`] gives, an engine size bound,
/// or a backend error.
pub(crate) async fn read_committed<B: StorageBackend>(
    backend: &B,
    root: &str,
) -> Result<Committed, ReadError> {
    let mut last = None;
    for _ in 0..READ_ATTEMPTS {
        match read_attempt(backend, root).await {
            Err(err) if err.code() == Some("E060") => last = Some(err),
            other => return other,
        }
    }
    Err(last.unwrap_or_else(|| ReadError::invalid("E060", root, "no read attempt was made")))
}

/// Read the root inventory and its sidecar once, and decide what they
/// commit.
async fn read_attempt<B: StorageBackend>(backend: &B, root: &str) -> Result<Committed, ReadError> {
    let read = read_root(backend, root).await?;
    let algorithm = read.inventory.digest_algorithm;
    match verify_sidecar(algorithm, &read.bytes, &read.sidecar) {
        Ok(()) => Ok(Committed {
            inventory: read.inventory,
            bytes: read.bytes,
            root_tag: read.tag,
            unpublished: None,
        }),
        Err(err) if err.code() == Some("E060") => {
            let mismatch = at_key(err, &read.sidecar_key);
            previous_committed(backend, root, read, mismatch).await
        }
        Err(err) => Err(at_key(err, &read.sidecar_key)),
    }
}

/// Read and parse the root inventory and read its sidecar.
async fn read_root<B: StorageBackend>(backend: &B, root: &str) -> Result<RootRead, ReadError> {
    let key = key_in(root, INVENTORY_FILE)?;
    let (bytes, tag) = read_small_tagged(backend, &key, MAX_INVENTORY_BYTES, "inventory")
        .await
        .map_err(|err| missing_as(err, "E063", key.as_str(), "the object has no inventory"))?;
    let inventory = Inventory::parse(&bytes).map_err(|err| at_inventory(err, &key))?;
    let sidecar_key = key_in(root, &sidecar_name(inventory.digest_algorithm))?;
    let sidecar = read_small(
        backend,
        &sidecar_key,
        MAX_SIDECAR_BYTES,
        "inventory sidecar",
    )
    .await
    .map_err(|err| missing_as(err, "E058", sidecar_key.as_str(), "the sidecar is missing"))?;
    Ok(RootRead {
        inventory,
        bytes,
        tag,
        sidecar,
        sidecar_key,
    })
}

/// When the root sidecar still commits the previous version's inventory,
/// that version is the committed head and the root inventory's head is
/// unpublished. Otherwise the sidecar mismatch `mismatch` stands.
async fn previous_committed<B: StorageBackend>(
    backend: &B,
    root: &str,
    read: RootRead,
    mismatch: ReadError,
) -> Result<Committed, ReadError> {
    let claimed = read.inventory.head.clone();
    let Some(previous) = read.inventory.versions.range(..&claimed).next_back() else {
        return Err(mismatch);
    };
    let key = key_in(root, &format!("{}/{INVENTORY_FILE}", previous.0))?;
    let bytes = match read_small(backend, &key, MAX_INVENTORY_BYTES, "inventory").await {
        Ok(bytes) => bytes,
        Err(ReadError::Storage(StorageError::NotFound { .. })) => return Err(mismatch),
        Err(err) => return Err(err),
    };
    let algorithm = read.inventory.digest_algorithm;
    if verify_sidecar(algorithm, &bytes, &read.sidecar).is_err() {
        return Err(mismatch);
    }
    let inventory = Inventory::parse(&bytes).map_err(|err| at_inventory(err, &key))?;
    if &inventory.head != previous.0 || inventory.id != read.inventory.id {
        return Err(mismatch);
    }
    tracing::warn!(
        object = %inventory.id,
        head = %inventory.head,
        unpublished = %claimed,
        "the root inventory names a head its sidecar does not commit; reading the previous version"
    );
    Ok(Committed {
        inventory,
        bytes,
        root_tag: read.tag,
        unpublished: Some(claimed),
    })
}

/// Prefix the JSON pointer of an inventory refusal with the inventory's key,
/// so the error names the object (`obj/inventory.json/manifest`).
pub(crate) fn at_inventory(err: FormatError, key: &StorageKey) -> ReadError {
    match err {
        FormatError::Invalid { code, at, reason } => {
            ReadError::invalid(code, format!("{key}{at}"), reason)
        }
        other => other.into(),
    }
}
