//! Version directories and the inventories they hold.
//!
//! A version directory holds its inventory, its sidecar and the content
//! directory only (E015; another directory is W002 and ignored). When the
//! inventory parses, only the sidecar for its `digestAlgorithm` is allowed;
//! when it does not, its error is reported and any recognised sidecar is
//! allowed; with no inventory, no sidecar is. Its inventory SHOULD be there
//! (W010) and, when it is, must be valid, name its own version as `head`
//! (E040), keep the object id (E037) and content directory (E019), never go
//! back to an older specification (E103), equal the root inventory for the
//! head version (E064), describe every earlier version as the root
//! inventory does (E066; `created`, `message` and `user` differences are
//! W011), keep every manifest path inside a content directory (E042) and
//! list every content file of its versions (E023).

use std::collections::{BTreeMap, BTreeSet};

use super::content::{check_listed, check_manifest_paths, expect_manifest, Expectations};
use super::listing::Listing;
use super::report::ValidationReport;
use super::root::{read_inventory, ReadInventory};
use super::warnings::algorithm_warning;
use crate::digest::DigestAlgorithm;
use crate::inventory::{
    sidecar_algorithm, DigestMap, Inventory, Version, VersionNum, INVENTORY_FILE,
};
use crate::namaste::SpecVersion;
use crate::read::small::child;
use crate::read::ReadError;
use crate::storage::StorageBackend;

/// Check every version directory the root inventory lists, oldest first,
/// and add the manifest digests of each version inventory to
/// `expectations`. A missing directory is reported with the root entries
/// (E010) and skipped here.
///
/// # Errors
///
/// An engine bound or a backend error.
pub(crate) async fn check_versions<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    root: &ReadInventory,
    expectations: &mut Expectations,
    report: &mut ValidationReport,
) -> Result<(), ReadError> {
    let content = root.inventory.content_directory();
    let mut newest: Option<SpecVersion> = None;
    for version in root.inventory.versions.keys() {
        let dir = version.to_string();
        if !listing.has_dir(&dir) {
            continue;
        }
        let prior = read_version_inventory(backend, listing, &dir, report).await?;
        let sidecars = sidecar_rule(listing, &dir, prior.as_ref());
        check_version_entries(listing, &dir, content, sidecars, report);
        let Some(prior) = prior else {
            continue;
        };
        let at = listing.key(&child(&dir, INVENTORY_FILE));
        check_spec_order(&mut newest, prior.inventory.spec, &at, report);
        compare_with_root(&prior, version, root, &at, report);
        check_manifest_paths(listing, &prior.inventory, report);
        check_listed(
            listing,
            &prior.inventory,
            &format!("{dir}/{INVENTORY_FILE}"),
            report,
        );
        expect_manifest(expectations, &prior.inventory);
    }
    Ok(())
}

/// Which inventory sidecars a version directory may hold.
#[derive(Debug, Clone, Copy)]
enum Sidecars {
    /// No inventory, so no sidecar either.
    None,
    /// An inventory whose algorithm is unknown (it breaks a rule, already
    /// reported): any content-addressing sidecar.
    Any,
    /// The sidecar of the inventory's `digestAlgorithm` only.
    Only(DigestAlgorithm),
}

impl Sidecars {
    /// Whether a sidecar stating `algorithm` is allowed.
    fn allows(self, algorithm: Option<DigestAlgorithm>) -> bool {
        match (self, algorithm) {
            (_, None) | (Self::None, _) => false,
            (Self::Any, Some(_)) => true,
            (Self::Only(expected), Some(stated)) => expected == stated,
        }
    }
}

/// The sidecars allowed in the version directory `dir`, from its inventory.
fn sidecar_rule(listing: &Listing, dir: &str, prior: Option<&ReadInventory>) -> Sidecars {
    match prior {
        Some(read) => Sidecars::Only(read.inventory.digest_algorithm),
        None if listing.contains(&child(dir, INVENTORY_FILE)) => Sidecars::Any,
        None => Sidecars::None,
    }
}

/// A version directory holds only its inventory, the sidecars `sidecars`
/// allows (the one for a parsed inventory's `digestAlgorithm`, any
/// recognised one beside a malformed inventory, none without an inventory)
/// and the content directory (E015); other directories are W002 and
/// ignored.
fn check_version_entries(
    listing: &Listing,
    dir: &str,
    content: &str,
    sidecars: Sidecars,
    report: &mut ValidationReport,
) {
    let children = listing.children(dir);
    for name in &children.files {
        let stated = sidecar_algorithm(name).and_then(DigestAlgorithm::from_name);
        if name != INVENTORY_FILE && !sidecars.allows(stated) {
            let message = "a version directory holds only its inventory, sidecar and content";
            report.add("E015", listing.key(&child(dir, name)), message);
        }
    }
    for name in children.dirs.iter().filter(|name| *name != content) {
        let message = "a version directory should hold no directory but the content directory";
        report.add("W002", listing.key(&child(dir, name)), message);
    }
}

/// Read the inventory of the version directory `dir`: `None` when it is
/// missing (W010) or breaks a rule (in the report).
///
/// # Errors
///
/// An engine bound or a backend error.
async fn read_version_inventory<B: StorageBackend>(
    backend: &B,
    listing: &Listing,
    dir: &str,
    report: &mut ValidationReport,
) -> Result<Option<ReadInventory>, ReadError> {
    let rel = child(dir, INVENTORY_FILE);
    if !listing.contains(&rel) {
        let message = "every version directory should hold an inventory";
        report.add("W010", listing.key(&rel), message);
        return Ok(None);
    }
    read_inventory(backend, listing, dir, report).await
}

/// Each version inventory conforms to the same or a later specification
/// than the one before it (E103).
fn check_spec_order(
    newest: &mut Option<SpecVersion>,
    spec: SpecVersion,
    at: &str,
    report: &mut ValidationReport,
) {
    if newest.is_some_and(|before| spec < before) {
        let message = "a version must not conform to an older specification than the one before";
        report.add("E103", format!("{at}/type"), message);
    }
    *newest = (*newest).max(Some(spec));
}

/// Compare one version inventory with the root inventory.
fn compare_with_root(
    prior: &ReadInventory,
    version: &VersionNum,
    root: &ReadInventory,
    at: &str,
    report: &mut ValidationReport,
) {
    let (inventory, current) = (&prior.inventory, &root.inventory);
    if inventory.head != *version {
        report.add(
            "E040",
            format!("{at}/head"),
            "head must name this version directory",
        );
    }
    if inventory.id != current.id {
        report.add("E037", format!("{at}/id"), "the object id must not change");
    }
    if inventory.content_directory() != current.content_directory() {
        let message = "contentDirectory must not change between versions";
        report.add("E019", format!("{at}/contentDirectory"), message);
    }
    algorithm_warning(inventory.digest_algorithm, at, report);
    if *version == current.head && prior.bytes != root.bytes {
        let message = "the head version inventory must equal the root inventory";
        report.add("E064", at.to_string(), message);
    }
    compare_blocks(inventory, current, at, report);
}

/// Every version block of an earlier inventory must describe the same
/// state as the root inventory's block (E066), and SHOULD have the same
/// `created`, `message` and `user` (W011).
fn compare_blocks(prior: &Inventory, current: &Inventory, at: &str, report: &mut ValidationReport) {
    for (number, block) in &prior.versions {
        let block_at = format!("{at}/versions/{number}");
        let Some(current_block) = current.versions.get(number) else {
            report.add("E066", block_at, "the root inventory has no such version");
            continue;
        };
        if !same_state(prior, block, current, current_block) {
            let message = "the version state differs from the root inventory's";
            report.add("E066", format!("{block_at}/state"), message);
        }
        if !same_metadata(block, current_block) {
            let message = "created, message and user should equal the root inventory's";
            report.add("W011", block_at, message);
        }
    }
}

/// Whether two version blocks have equal `created`, `message` and `user`.
fn same_metadata(a: &Version, b: &Version) -> bool {
    a.created == b.created && a.message == b.message && a.user == b.user
}

/// Whether two version blocks give every logical path the same content.
/// With one algorithm the digests must be equal (case aside); across
/// algorithms each logical path must resolve to a common content path.
fn same_state(prior: &Inventory, a: &Version, current: &Inventory, b: &Version) -> bool {
    if prior.digest_algorithm == current.digest_algorithm {
        return path_digests(&a.state) == path_digests(&b.state);
    }
    let (left, right) = (
        path_contents(&a.state, &prior.manifest),
        path_contents(&b.state, &current.manifest),
    );
    left.len() == right.len()
        && left.iter().all(|(path, contents)| {
            right
                .get(path)
                .is_some_and(|other| !contents.is_disjoint(other))
        })
}

/// Logical paths mapped to their lowercase digests.
fn path_digests(state: &DigestMap) -> BTreeMap<&str, String> {
    state
        .iter()
        .flat_map(|(digest, paths)| {
            paths
                .iter()
                .map(move |p| (p.as_str(), digest.to_ascii_lowercase()))
        })
        .collect()
}

/// Logical paths mapped to the content paths their digest has.
fn path_contents<'a>(
    state: &'a DigestMap,
    manifest: &'a DigestMap,
) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut map = BTreeMap::new();
    for (digest, paths) in state {
        let contents: BTreeSet<&str> = manifest
            .get(digest)
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        for path in paths {
            map.insert(path.as_str(), contents.clone());
        }
    }
    map
}
