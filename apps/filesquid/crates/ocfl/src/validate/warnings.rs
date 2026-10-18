//! What the OCFL 1.1 inventory SHOULD do (W001, W004, W005, W007, W008,
//! W009). Warnings never make an object invalid.

use super::report::ValidationReport;
use crate::digest::DigestAlgorithm;
use crate::inventory::{Inventory, Version};

/// Report every inventory-level warning of the root inventory, located at
/// `at` (its storage key).
pub(crate) fn inventory_warnings(inventory: &Inventory, at: &str, report: &mut ValidationReport) {
    if inventory.head.is_zero_padded() {
        let message = "version directory names should not be zero-padded";
        report.add("W001", format!("{at}/head"), message);
    }
    algorithm_warning(inventory.digest_algorithm, at, report);
    if !is_uri(&inventory.id) {
        report.add("W005", format!("{at}/id"), "the object id should be a URI");
    }
    for (version, block) in &inventory.versions {
        version_warnings(block, &format!("{at}/versions/{version}"), report);
    }
}

/// W004: objects should use sha512 for content addressing. Reported for
/// the root inventory and for every version inventory.
pub(crate) fn algorithm_warning(
    algorithm: DigestAlgorithm,
    at: &str,
    report: &mut ValidationReport,
) {
    if algorithm != DigestAlgorithm::Sha512 {
        let message = "objects should use sha512 for content addressing";
        report.add("W004", format!("{at}/digestAlgorithm"), message);
    }
}

/// W007, W008 and W009 for one version block at `at`.
fn version_warnings(block: &Version, at: &str, report: &mut ValidationReport) {
    if block.message.is_none() || block.user.is_none() {
        let message = "a version should include message and user";
        report.add("W007", at.to_string(), message);
    }
    let Some(user) = &block.user else {
        return;
    };
    match &user.address {
        None => report.add(
            "W008",
            format!("{at}/user"),
            "a user should have an address",
        ),
        Some(address) if !is_uri(address) => {
            let message = "a user address should be a URI, such as mailto: or https:";
            report.add("W009", format!("{at}/user/address"), message);
        }
        Some(_) => {}
    }
}

/// Whether `text` is a URI by RFC 3986 section 3: a scheme (a letter, then
/// letters, digits, `+`, `-` or `.`), a colon, and something after it.
pub(crate) fn is_uri(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let scheme_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    starts_with_letter && scheme_ok && !rest.is_empty() && !text.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::is_uri;

    #[test]
    fn uris_need_a_scheme_and_a_rest() {
        for uri in [
            "ark:123/abc",
            "urn:example:x",
            "https://orcid.org/0",
            "info:a/b",
        ] {
            assert!(is_uri(uri), "{uri}");
        }
        for text in ["not_a_uri", "1 Wonky Way, WW", ":x", "a:", "1a:b", "a b:c"] {
            assert!(!is_uri(text), "{text}");
        }
    }
}
