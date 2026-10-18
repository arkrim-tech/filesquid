//! Reading the top level of an inventory: its keys, `id`, `type`,
//! `digestAlgorithm`, `contentDirectory` and `head`, then the blocks
//! (OCFL 1.1 section 3.5.1). Blocks are read in `blocks.rs`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::blocks::{read_fixity, read_manifest, read_versions};
use super::error::FormatError;
use super::{Inventory, Version, VersionNum};
use crate::digest::DigestAlgorithm;
use crate::namaste::SpecVersion;

/// Every key an inventory may have (E102 for any other).
const TOP_KEYS: [&str; 8] = [
    "id",
    "type",
    "digestAlgorithm",
    "head",
    "contentDirectory",
    "manifest",
    "versions",
    "fixity",
];

/// The keys every inventory must have, with the code for a missing one.
const REQUIRED: [(&str, &str); 6] = [
    ("id", "E036"),
    ("type", "E036"),
    ("digestAlgorithm", "E036"),
    ("head", "E036"),
    ("manifest", "E041"),
    ("versions", "E043"),
];

/// Read a parsed JSON document as an inventory, in rule order: keys, the
/// scalar fields, the manifest, the versions, the head, the fixity block.
pub(super) fn read_inventory(root: &Value) -> Result<Inventory, FormatError> {
    let Value::Object(object) = root else {
        return Err(FormatError::invalid(
            "E033",
            "",
            "an inventory is a JSON object",
        ));
    };
    check_keys(object, &TOP_KEYS, "")?;
    check_required(object)?;
    let spec = read_type(&object["type"])?;
    let digest_algorithm = read_algorithm(&object["digestAlgorithm"])?;
    let content_directory = object
        .get("contentDirectory")
        .map(read_content_directory)
        .transpose()?;
    let id = string_at(&object["id"], "/id", "E033")?.to_string();
    let manifest = read_manifest(&object["manifest"], digest_algorithm)?;
    let versions = read_versions(&object["versions"])?;
    let head = read_head(&object["head"], &versions)?;
    let fixity = object.get("fixity").map(read_fixity).transpose()?;
    Ok(Inventory {
        id,
        spec,
        digest_algorithm,
        head,
        content_directory,
        manifest,
        versions,
        fixity,
    })
}

/// Refuse any key of `object` outside `allowed` (E102).
pub(super) fn check_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    at: &str,
) -> Result<(), FormatError> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(FormatError::invalid(
            "E102",
            format!("{at}/{key}"),
            "an inventory must not contain keys the specification does not define",
        )),
        None => Ok(()),
    }
}

/// Refuse a missing required key with its code (E036, E041, E043).
fn check_required(object: &Map<String, Value>) -> Result<(), FormatError> {
    match REQUIRED.iter().find(|(key, _)| !object.contains_key(*key)) {
        Some((key, code)) => Err(FormatError::invalid(
            code,
            format!("/{key}"),
            format!("an inventory must include {key}"),
        )),
        None => Ok(()),
    }
}

/// The JSON string at `at`, or `code` when it is another type.
pub(super) fn string_at<'v>(
    value: &'v Value,
    at: &str,
    code: &'static str,
) -> Result<&'v str, FormatError> {
    value
        .as_str()
        .ok_or_else(|| FormatError::invalid(code, at, "must be a JSON string"))
}

/// The specification version the inventory `type` names (E038).
fn read_type(value: &Value) -> Result<SpecVersion, FormatError> {
    let uri = string_at(value, "/type", "E038")?;
    SpecVersion::from_inventory_type(uri)
        .ok_or_else(|| FormatError::invalid("E038", "/type", "must be the OCFL inventory type URI"))
}

/// The content-addressing algorithm: `sha512` or `sha256` (E025).
fn read_algorithm(value: &Value) -> Result<DigestAlgorithm, FormatError> {
    let name = string_at(value, "/digestAlgorithm", "E025")?;
    DigestAlgorithm::from_name(name)
        .ok_or_else(|| FormatError::invalid("E025", "/digestAlgorithm", "must be sha512 or sha256"))
}

/// The `contentDirectory` name: a direct child directory (E017, E018,
/// E108).
fn read_content_directory(value: &Value) -> Result<String, FormatError> {
    let at = "/contentDirectory";
    let name = string_at(value, at, "E033")?;
    let fault = match name {
        _ if name.contains('/') => Some(("E017", "must not contain /")),
        "." | ".." => Some(("E018", "must not be . or ..")),
        "" => Some(("E108", "must name a direct child directory")),
        _ => None,
    };
    match fault {
        Some((code, reason)) => Err(FormatError::invalid(code, at, reason)),
        None => Ok(name.to_string()),
    }
}

/// The `head` version: a version name in the object's naming convention
/// that names the highest version (E011, E012, E040).
fn read_head(
    value: &Value,
    versions: &BTreeMap<VersionNum, Version>,
) -> Result<VersionNum, FormatError> {
    let not_highest = || FormatError::invalid("E040", "/head", "must name the highest version");
    let name = value.as_str().ok_or_else(not_highest)?;
    let head = VersionNum::parse(name).map_err(|_| not_highest())?;
    let (first, highest) = match (versions.keys().next(), versions.keys().next_back()) {
        (Some(first), Some(highest)) => (first, highest),
        _ => return Err(not_highest()),
    };
    first.check_convention(&head)?;
    if head != *highest {
        return Err(not_highest());
    }
    Ok(head)
}
