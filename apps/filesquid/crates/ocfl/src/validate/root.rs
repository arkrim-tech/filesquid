//! The object root: its declaration (E003, E007), its inventory and sidecar
//! (E063, every inventory code, E038, E058, E060, E061) and its entries
//! (E001, E046, E010, E067, W013).

use super::listing::Listing;
use super::report::ValidationReport;
use crate::digest::DigestAlgorithm;
use crate::inventory::{
    sidecar_algorithm, sidecar_name, verify_sidecar, FormatError, Inventory, VersionNum,
    INVENTORY_FILE, MAX_INVENTORY_BYTES, MAX_SIDECAR_BYTES,
};
use crate::namaste::{DeclarationKind, SpecVersion};
use crate::read::declaration::{check_declaration as check_contents, choose_declaration};
use crate::read::small::{child, key_in, read_small};
use crate::read::ReadError;
use crate::storage::StorageBackend;

/// The directory in an object root that holds object extensions.
pub(crate) const EXTENSIONS_DIR: &str = "extensions";

/// The directory in an object root that MAY hold logs (W012: any content).
const LOGS_DIR: &str = "logs";

/// Registered OCFL community extension names (W013), from the OCFL
/// extensions registry (https://ocfl.github.io/extensions/), plus
/// `initial`, which the registry reserves for an optional initial
/// extension. A name outside this list is a warning, never an error.
const REGISTERED_EXTENSIONS: [&str; 13] = [
    "0001-digest-algorithms",
    "0002-flat-direct-storage-layout",
    "0003-hash-and-id-n-tuple-storage-layout",
    "0004-hashed-n-tuple-storage-layout",
    "0005-mutable-head",
    "0006-flat-omit-prefix-storage-layout",
    "0007-n-tuple-omit-prefix-storage-layout",
    "0008-schema-registry",
    "0009-digest-algorithms",
    "0010-differential-n-tuple-omit-prefix-storage-layout",
    "0011-direct-clean-path-layout",
    "0012-hash-and-no-prefix-id-n-tuple-storage-layout",
    "initial",
];

/// A parsed inventory with the exact bytes it was read from.
#[derive(Debug, Clone)]
pub(crate) struct ReadInventory {
    /// The parsed inventory.
    pub inventory: Inventory,
    /// The bytes of `inventory.json`.
    pub bytes: Vec<u8>,
}

/// Check the one object declaration and its contents (E003, E007).
///
/// # Errors
///
/// [`ReadError::UnsupportedVersion`] for an object that is not OCFL 1.1, an
/// engine bound or a backend error.
pub(crate) async fn check_declaration<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    report: &mut ValidationReport,
) -> Result<(), ReadError> {
    let root = listing.key("");
    let children = listing.children("");
    let names = children.files.iter().map(String::as_str);
    let declaration = match choose_declaration(&root, DeclarationKind::Object, names) {
        Ok(declaration) => declaration,
        Err(err) => return report.record(err),
    };
    match check_contents(backend, &root, &declaration).await {
        Ok(()) => Ok(()),
        Err(err) => report.record(err),
    }
}

/// Read, parse and check the root inventory and its sidecar. `None` when
/// it is missing (E063) or breaks a rule, which is then in the report.
///
/// # Errors
///
/// An engine bound or a backend error.
pub(crate) async fn read_root_inventory<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    report: &mut ValidationReport,
) -> Result<Option<ReadInventory>, ReadError> {
    if !listing.contains(INVENTORY_FILE) {
        let at = listing.key(INVENTORY_FILE);
        report.add("E063", at, "every object must have a root inventory");
        return Ok(None);
    }
    let Some(read) = read_inventory(backend, listing, "", report).await? else {
        return Ok(None);
    };
    if read.inventory.spec != SpecVersion::V1_1 {
        let at = format!("{}/type", listing.key(INVENTORY_FILE));
        report.add(
            "E038",
            at,
            "the root inventory type must match the 1.1 declaration",
        );
    }
    Ok(Some(read))
}

/// Read and parse the inventory in `dir` (empty for the root) and check its
/// sidecar. `None` when the inventory breaks a rule (in the report).
///
/// # Errors
///
/// An engine bound or a backend error.
pub(crate) async fn read_inventory<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    dir: &str,
    report: &mut ValidationReport,
) -> Result<Option<ReadInventory>, ReadError> {
    let key = key_in(&listing.key(dir), INVENTORY_FILE)?;
    let bytes = read_small(backend, &key, MAX_INVENTORY_BYTES, "inventory").await?;
    let inventory = match Inventory::parse(&bytes) {
        Ok(inventory) => inventory,
        Err(FormatError::Invalid { code, at, reason }) => {
            report.add(code, format!("{key}{at}"), reason);
            return Ok(None);
        }
        Err(bound) => return Err(bound.into()),
    };
    let algorithm = inventory.digest_algorithm;
    check_sidecar(backend, listing, dir, algorithm, &bytes, report).await?;
    Ok(Some(ReadInventory { inventory, bytes }))
}

/// Check the sidecar of the inventory in `dir` (E058, E060, E061).
///
/// # Errors
///
/// An engine bound or a backend error.
async fn check_sidecar<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    dir: &str,
    algorithm: DigestAlgorithm,
    inventory: &[u8],
    report: &mut ValidationReport,
) -> Result<(), ReadError> {
    let rel = child(dir, &sidecar_name(algorithm));
    if !listing.contains(&rel) {
        let message = "every inventory must have a sidecar for its digestAlgorithm";
        report.add("E058", listing.key(&rel), message);
        return Ok(());
    }
    let key = key_in(&listing.key(""), &rel)?;
    let sidecar = read_small(backend, &key, MAX_SIDECAR_BYTES, "inventory sidecar").await?;
    match verify_sidecar(algorithm, inventory, &sidecar) {
        Ok(()) => Ok(()),
        Err(FormatError::Invalid { code, reason, .. }) => {
            report.add(code, key.as_str(), reason);
            Ok(())
        }
        Err(bound) => Err(bound.into()),
    }
}

/// Check every entry directly in the object root: the declaration, the
/// inventory and its sidecar, version directories, `logs` and `extensions`
/// only (E001). A version directory the root inventory does not list is
/// E046: only the root inventory commits a version, so a version directory
/// with its own inventory and sidecar is not a commit. Every listed version
/// must have its directory (E010). Without a root inventory, any
/// well-formed version name is accepted.
pub(crate) fn check_root_entries(
    listing: &Listing,
    inventory: Option<&Inventory>,
    report: &mut ValidationReport,
) {
    let children = listing.children("");
    for name in &children.files {
        if !is_root_file(name, inventory) {
            report.add(
                "E001",
                listing.key(name),
                "not a file an object root may hold",
            );
        }
    }
    for name in &children.dirs {
        check_root_dir(listing, name, inventory, report);
    }
    let Some(inventory) = inventory else {
        return;
    };
    for version in inventory.versions.keys() {
        let name = version.to_string();
        if !children.dirs.contains(&name) {
            let message = "the inventory lists a version that has no directory";
            report.add("E010", listing.key(&name), message);
        }
    }
}

/// Whether `name` may be a file directly in the object root.
fn is_root_file(name: &str, inventory: Option<&Inventory>) -> bool {
    if name == INVENTORY_FILE || name.starts_with("0=ocfl_object_") {
        return true;
    }
    match (sidecar_algorithm(name), inventory) {
        (Some(stated), Some(inventory)) => stated == inventory.digest_algorithm.name(),
        (Some(stated), None) => DigestAlgorithm::from_name(stated).is_some(),
        (None, _) => false,
    }
}

/// Check one directory directly in the object root.
fn check_root_dir(
    listing: &Listing,
    name: &str,
    inventory: Option<&Inventory>,
    report: &mut ValidationReport,
) {
    if name == LOGS_DIR {
        return;
    }
    if name == EXTENSIONS_DIR {
        return check_extensions(listing, report);
    }
    let at = listing.key(name);
    let Ok(version) = VersionNum::parse(name) else {
        return report.add("E001", at, "not a directory an object root may hold");
    };
    let listed = inventory.is_none_or(|inv| inv.versions.contains_key(&version));
    if !listed {
        let message = "the root inventory does not list this version directory; \
                       only the root inventory commits a version";
        report.add("E046", at, message);
    }
}

/// The object `extensions` directory holds only extension directories
/// (E067), each SHOULD be a registered extension name (W013).
fn check_extensions(listing: &Listing, report: &mut ValidationReport) {
    let children = listing.children(EXTENSIONS_DIR);
    for name in &children.files {
        let at = listing.key(&child(EXTENSIONS_DIR, name));
        report.add("E067", at, "extensions may hold only extension directories");
    }
    for name in &children.dirs {
        if !REGISTERED_EXTENSIONS.contains(&name.as_str()) {
            let at = listing.key(&child(EXTENSIONS_DIR, name));
            report.add("W013", at, "not a registered extension name");
        }
    }
}
