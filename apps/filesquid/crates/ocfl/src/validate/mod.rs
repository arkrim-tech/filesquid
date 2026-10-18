//! Structural validation of OCFL 1.1 objects (OCFL-S-009;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md),
//! [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)).
//!
//! [`validate_object`] checks one object through any
//! [`StorageBackend`] and returns a [`ValidationReport`]: every broken rule
//! as a [`Finding`] with its OCFL 1.1 validation code, errors (`E###`) for
//! MUST rules and warnings (`W###`) for SHOULD rules. An object is valid
//! when it has no errors.
//!
//! Where reading stops at the first fault it meets ([`crate::read`]),
//! validation goes on and reports what it can:
//!
//! 1. the storage hierarchy under the object root (E024, E090);
//! 2. the declaration (E003, E007), the root inventory (E063, every code
//!    [`Inventory::parse`](crate::Inventory::parse) gives, E038) and its
//!    sidecar (E058, E060, E061), and the entries of the object root
//!    (E001, E010, E046, E067, W013);
//! 3. inventory warnings (W001, W004, W005, W007, W008, W009);
//! 4. each version directory and its inventory (E015, W002, W010, E040,
//!    E037, E019, E103, E064, E066, W011, E023);
//! 5. content: files against the manifest (E023, E042) and every stated
//!    digest against the bytes, the root manifest and fixity block and the
//!    manifests of earlier version inventories (E092, E093).
//!
//! Only the root inventory commits a version: a version directory it does
//! not list is E046 even when it holds an inventory and sidecar of its own
//! (ADR 016 "Published head (readers)").
//!
//! One inventory is checked to its first broken rule, so an object that
//! breaks several rules of one inventory reports the first. The OCFL
//! fixtures README allows this.
//!
//! The object is listed once ([`crate::storage::MAX_WALK_ENTRIES`] keys at
//! most), small files are read within the engine's parsing bounds
//! (OCFL-S-017), and content is streamed in chunks of at most
//! [`MAX_CHUNK_BYTES`](crate::storage::MAX_CHUNK_BYTES), once per file
//! whatever the number of digests it must match.

mod content;
mod listing;
mod report;
mod root;
mod versions;
mod warnings;

use content::{check_listed, check_manifest_paths, expect_fixity, expect_manifest, verify_content};
use listing::{check_hierarchy, Listing};
use root::{check_declaration, check_root_entries, read_root_inventory, ReadInventory};
use versions::check_versions;

pub use report::{Finding, ValidationReport};

use crate::read::small::root_dir;
use crate::read::ReadError;
use crate::storage::StorageBackend;

/// Validate the OCFL 1.1 object whose root is the directory `root` of
/// `backend` (empty for the backend's own root, with or without a trailing
/// `/`).
///
/// # Errors
///
/// Only when the object cannot be validated: [`ReadError::Storage`] for an
/// invalid root, a backend failure or a listing above its bound;
/// [`ReadError::Format`] with
/// [`FormatError::TooLarge`](crate::FormatError::TooLarge) for an
/// inventory, sidecar or declaration above the engine's parsing bound
/// (OCFL-S-017, no OCFL code); [`ReadError::UnsupportedVersion`] for an
/// object that declares an OCFL version other than 1.1. Every broken OCFL
/// rule is a finding in the report instead.
pub async fn validate_object<B: StorageBackend>(
    backend: &B,
    root: &str,
) -> Result<ValidationReport, ReadError> {
    let root = root_dir(root)?;
    let mut report = ValidationReport::new(&root);
    if !check_hierarchy(backend, &root, &mut report).await? {
        return Ok(report);
    }
    let listing = Listing::read(backend, &root).await?;
    check_declaration(backend, &listing, &mut report).await?;
    let inventory = read_root_inventory(backend, &listing, &mut report).await?;
    check_root_entries(
        &listing,
        inventory.as_ref().map(|r| &r.inventory),
        &mut report,
    );
    if let Some(inventory) = inventory {
        check_committed(backend, &listing, &inventory, &mut report).await?;
    }
    tracing::debug!(
        object = %root,
        errors = report.errors().len(),
        warnings = report.warnings().len(),
        "validated OCFL object"
    );
    Ok(report)
}

/// Everything that needs a valid root inventory: its warnings, the version
/// directories, and the content against every inventory.
///
/// # Errors
///
/// An engine bound or a backend error.
async fn check_committed<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    root: &ReadInventory,
    report: &mut ValidationReport,
) -> Result<(), ReadError> {
    let inventory = &root.inventory;
    let at = listing.key(crate::inventory::INVENTORY_FILE);
    warnings::inventory_warnings(inventory, &at, report);
    let mut expectations = content::Expectations::new();
    expect_manifest(&mut expectations, inventory);
    expect_fixity(&mut expectations, inventory);
    check_versions(backend, listing, root, &mut expectations, report).await?;
    check_listed(listing, inventory, "the root inventory", report);
    check_manifest_paths(listing, inventory, report);
    verify_content(backend, listing, &expectations, report).await
}
