//! Checks across an inventory's blocks: every state digest is a manifest
//! key, written exactly (E050), and every manifest digest is used by some
//! version (E107).

use std::collections::BTreeSet;

use super::error::FormatError;
use super::Inventory;

/// Check that the versions and the manifest agree.
pub(super) fn check_consistency(inventory: &Inventory) -> Result<(), FormatError> {
    check_state_digests(inventory)?;
    check_manifest_used(inventory)
}

/// Each state digest exactly matches a manifest key. OCFL 1.1 section
/// 3.5.3.1 requires an exact match, so a digest in another case is refused.
fn check_state_digests(inventory: &Inventory) -> Result<(), FormatError> {
    for (version, block) in &inventory.versions {
        let missing = block
            .state
            .keys()
            .find(|digest| !inventory.manifest.contains_key(*digest));
        if let Some(digest) = missing {
            return Err(FormatError::invalid(
                "E050",
                format!("/versions/{version}/state/{digest}"),
                "a state digest must exactly match a manifest digest",
            ));
        }
    }
    Ok(())
}

/// Each manifest digest appears in at least one version state.
fn check_manifest_used(inventory: &Inventory) -> Result<(), FormatError> {
    let used: BTreeSet<&String> = inventory
        .versions
        .values()
        .flat_map(|block| block.state.keys())
        .collect();
    match inventory
        .manifest
        .keys()
        .find(|digest| !used.contains(digest))
    {
        Some(digest) => Err(FormatError::invalid(
            "E107",
            format!("/manifest/{digest}"),
            "every manifest digest must be used by a version state",
        )),
        None => Ok(()),
    }
}
