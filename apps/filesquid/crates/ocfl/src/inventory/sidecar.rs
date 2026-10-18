//! Inventory digest sidecars (OCFL 1.1 section 3.6).
//!
//! Every `inventory.json` has a sidecar `inventory.json.ALGORITHM`, where
//! ALGORITHM is the inventory's `digestAlgorithm`. Its text is the digest of
//! the inventory bytes, one or more spaces or tabs, and `inventory.json`.
//! The engine writes one space and a final newline; it reads any run of
//! spaces and tabs and an optional final newline.

use super::error::FormatError;
use super::INVENTORY_FILE;
use crate::digest::DigestAlgorithm;

/// The sidecar file name for `algorithm`, such as `inventory.json.sha256`.
pub fn sidecar_name(algorithm: DigestAlgorithm) -> String {
    format!("{INVENTORY_FILE}.{}", algorithm.name())
}

/// The algorithm a sidecar file name states, such as `sha512` for
/// `inventory.json.sha512`, or `None` when it is not a sidecar name.
pub fn sidecar_algorithm(file_name: &str) -> Option<&str> {
    file_name
        .strip_prefix(INVENTORY_FILE)?
        .strip_prefix('.')
        .filter(|name| !name.is_empty())
}

/// Check that a sidecar file name states the inventory's algorithm.
///
/// # Errors
///
/// [`FormatError::Invalid`] with E059 otherwise.
pub fn check_sidecar_name(algorithm: DigestAlgorithm, file_name: &str) -> Result<(), FormatError> {
    if sidecar_algorithm(file_name) == Some(algorithm.name()) {
        return Ok(());
    }
    Err(FormatError::invalid(
        "E059",
        file_name,
        format!("the sidecar must be {}", sidecar_name(algorithm)),
    ))
}

/// The sidecar text for `inventory`: its digest, one space,
/// `inventory.json` and a newline.
pub fn sidecar_contents(algorithm: DigestAlgorithm, inventory: &[u8]) -> String {
    format!("{} {INVENTORY_FILE}\n", algorithm.digest_hex(inventory))
}

/// The digest a sidecar states.
///
/// # Errors
///
/// [`FormatError::Invalid`] with E061 unless the bytes are UTF-8 text of the
/// form `DIGEST inventory.json`: a digest, one or more spaces or tabs, the
/// file name, and at most one final newline.
pub fn parse_sidecar(bytes: &[u8]) -> Result<&str, FormatError> {
    let malformed = || {
        FormatError::invalid(
            "E061",
            "",
            format!("a sidecar must read DIGEST {INVENTORY_FILE}"),
        )
    };
    let text = std::str::from_utf8(bytes).map_err(|_| malformed())?;
    let line = text.strip_suffix('\n').unwrap_or(text);
    let separator = line.find([' ', '\t']).ok_or_else(malformed)?;
    let (digest, rest) = line.split_at(separator);
    let name = rest.trim_start_matches([' ', '\t']);
    if digest.is_empty() || name != INVENTORY_FILE {
        return Err(malformed());
    }
    Ok(digest)
}

/// Check a sidecar against the inventory bytes it describes. Digests are
/// compared without regard to case.
///
/// # Errors
///
/// [`FormatError::Invalid`] with E061 for a malformed sidecar, or E060 when
/// the stated digest is not the digest of `inventory`.
pub fn verify_sidecar(
    algorithm: DigestAlgorithm,
    inventory: &[u8],
    sidecar: &[u8],
) -> Result<(), FormatError> {
    let stated = parse_sidecar(sidecar)?;
    let actual = algorithm.digest_hex(inventory);
    if algorithm.matches(stated, &actual) {
        return Ok(());
    }
    Err(FormatError::invalid(
        "E060",
        sidecar_name(algorithm),
        "the sidecar digest does not match the inventory",
    ))
}
