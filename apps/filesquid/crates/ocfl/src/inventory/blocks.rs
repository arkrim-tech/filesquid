//! Reading the inventory blocks: `manifest` (OCFL 1.1 section 3.5.2),
//! `versions` and each version (3.5.3, 3.5.3.1) and `fixity` (3.5.4).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::error::FormatError;
use super::paths::{first_conflict, path_fault, PathFault};
use super::read::{check_keys, string_at};
use super::timestamp::is_rfc3339;
use super::{DigestMap, User, Version, VersionNum};
use crate::digest::DigestAlgorithm;

/// Every key a version block may have.
const VERSION_KEYS: [&str; 4] = ["created", "state", "message", "user"];

/// Every key a user block may have.
const USER_KEYS: [&str; 2] = ["name", "address"];

/// The codes for a digest map that is not an object, and for an entry that
/// is not an array of strings.
#[derive(Clone, Copy)]
struct MapCodes {
    object: &'static str,
    entry: &'static str,
}

/// The codes for the two path faults of one kind of path.
#[derive(Clone, Copy)]
struct PathCodes {
    element: &'static str,
    edge: &'static str,
}

/// Content paths in the manifest and fixity blocks.
const CONTENT: PathCodes = PathCodes {
    element: "E099",
    edge: "E100",
};

/// Logical paths in a version state.
const LOGICAL: PathCodes = PathCodes {
    element: "E052",
    edge: "E053",
};

/// Read the `manifest` block: digests of the inventory's algorithm, each
/// once regardless of case, mapped to safe, unique content paths.
pub(super) fn read_manifest(
    value: &Value,
    algorithm: DigestAlgorithm,
) -> Result<DigestMap, FormatError> {
    let at = "/manifest";
    let codes = MapCodes {
        object: "E106",
        entry: "E092",
    };
    let manifest = digest_map(value, at, codes)?;
    check_digest_format(&manifest, algorithm, at)?;
    check_case_unique(&manifest, at, "E096")?;
    check_paths(&manifest, at, CONTENT)?;
    let all = manifest.values().flatten().map(String::as_str);
    if let Some(path) = first_conflict(all) {
        return Err(FormatError::invalid(
            "E101",
            format!("{at}: {path}"),
            "content paths must be unique and non-conflicting",
        ));
    }
    Ok(manifest)
}

/// Read the `versions` block: names first (E104, E105), then the naming
/// convention and sequence (E009 to E012), then each version block.
pub(super) fn read_versions(value: &Value) -> Result<BTreeMap<VersionNum, Version>, FormatError> {
    let object = value
        .as_object()
        .ok_or_else(|| FormatError::invalid("E045", "/versions", "must be a JSON object"))?;
    if object.is_empty() {
        return Err(FormatError::invalid(
            "E008",
            "/versions",
            "an object has at least one version",
        ));
    }
    let mut named = BTreeMap::new();
    for (name, block) in object {
        named.insert(VersionNum::parse(name)?, block);
    }
    check_sequence(named.keys())?;
    let mut versions = BTreeMap::new();
    for (num, block) in named {
        versions.insert(num, read_version(block, &format!("/versions/{num}"))?);
    }
    Ok(versions)
}

/// Every name follows the first one's convention, numbering starts at 1
/// and has no gaps. `names` is sorted by number.
fn check_sequence<'a>(names: impl Iterator<Item = &'a VersionNum>) -> Result<(), FormatError> {
    let names: Vec<&VersionNum> = names.collect();
    let Some(first) = names.first() else {
        return Ok(());
    };
    for name in &names {
        first.check_convention(name)?;
    }
    if first.number() != 1 {
        return Err(FormatError::invalid(
            "E009",
            "/versions",
            "versions start at 1",
        ));
    }
    match names
        .iter()
        .zip(1u64..)
        .find(|(name, n)| name.number() != *n)
    {
        Some((name, _)) => Err(FormatError::invalid(
            "E010",
            format!("/versions/{name}"),
            "version numbers are continuous",
        )),
        None => Ok(()),
    }
}

/// Read one version block at `at`.
fn read_version(value: &Value, at: &str) -> Result<Version, FormatError> {
    let object = value
        .as_object()
        .ok_or_else(|| FormatError::invalid("E047", at, "a version is a JSON object"))?;
    check_keys(object, &VERSION_KEYS, at)?;
    let created = read_created(object, at)?;
    let state_value = object
        .get("state")
        .ok_or_else(|| FormatError::invalid("E048", at, "a version must include state"))?;
    let state = read_state(state_value, &format!("{at}/state"))?;
    let message = object
        .get("message")
        .map(|m| string_at(m, &format!("{at}/message"), "E094").map(str::to_string))
        .transpose()?;
    let user = object
        .get("user")
        .map(|u| read_user(u, &format!("{at}/user")))
        .transpose()?;
    Ok(Version {
        created,
        state,
        message,
        user,
    })
}

/// The `created` value: present (E048) and an RFC 3339 date-time (E049).
fn read_created(object: &Map<String, Value>, at: &str) -> Result<String, FormatError> {
    let value = object
        .get("created")
        .ok_or_else(|| FormatError::invalid("E048", at, "a version must include created"))?;
    let at = format!("{at}/created");
    let text = string_at(value, &at, "E049")?;
    if !is_rfc3339(text) {
        return Err(FormatError::invalid(
            "E049",
            at,
            "must be an RFC 3339 date-time to the second with a time zone",
        ));
    }
    Ok(text.to_string())
}

/// Read a version state: safe logical paths, unique and non-conflicting
/// within the version (E052, E053, E095). Digests are checked against the
/// manifest afterwards.
fn read_state(value: &Value, at: &str) -> Result<DigestMap, FormatError> {
    let codes = MapCodes {
        object: "E033",
        entry: "E033",
    };
    let state = digest_map(value, at, codes)?;
    check_paths(&state, at, LOGICAL)?;
    if let Some(path) = first_conflict(state.values().flatten().map(String::as_str)) {
        return Err(FormatError::invalid(
            "E095",
            format!("{at}: {path}"),
            "logical paths must be unique and non-conflicting within a version",
        ));
    }
    Ok(state)
}

/// Read a user block: an object with a string `name` (E054) and an
/// optional string `address`.
fn read_user(value: &Value, at: &str) -> Result<User, FormatError> {
    let object = value
        .as_object()
        .ok_or_else(|| FormatError::invalid("E054", at, "user is a JSON object with a name"))?;
    check_keys(object, &USER_KEYS, at)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| FormatError::invalid("E054", at, "user must include a string name"))?;
    let address = object
        .get("address")
        .map(|a| string_at(a, &format!("{at}/address"), "E033").map(str::to_string))
        .transpose()?;
    Ok(User {
        name: name.to_string(),
        address,
    })
}

/// Read the `fixity` block: an object (E111) of algorithm names, each with
/// a manifest-shaped block (E057) whose digests occur once regardless of
/// case (E097) and whose content paths are safe (E099, E100).
pub(super) fn read_fixity(value: &Value) -> Result<BTreeMap<String, DigestMap>, FormatError> {
    let object = value
        .as_object()
        .ok_or_else(|| FormatError::invalid("E111", "/fixity", "must be a JSON object"))?;
    let codes = MapCodes {
        object: "E057",
        entry: "E057",
    };
    let mut fixity = BTreeMap::new();
    for (algorithm, block) in object {
        let at = format!("/fixity/{algorithm}");
        let map = digest_map(block, &at, codes)?;
        check_case_unique(&map, &at, "E097")?;
        check_paths(&map, &at, CONTENT)?;
        fixity.insert(algorithm.clone(), map);
    }
    Ok(fixity)
}

/// Read a JSON object of digests mapped to arrays of strings.
fn digest_map(value: &Value, at: &str, codes: MapCodes) -> Result<DigestMap, FormatError> {
    let object = value
        .as_object()
        .ok_or_else(|| FormatError::invalid(codes.object, at, "must be a JSON object"))?;
    let mut map = DigestMap::new();
    for (digest, paths) in object {
        let paths = string_array(paths).ok_or_else(|| {
            FormatError::invalid(
                codes.entry,
                format!("{at}/{digest}"),
                "must be an array of strings",
            )
        })?;
        map.insert(digest.clone(), paths);
    }
    Ok(map)
}

/// The strings of a JSON array of strings, or `None` for anything else.
fn string_array(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

/// Manifest digests are hexadecimal (E030 sha256, E031 sha512) of the
/// algorithm's length (E039).
fn check_digest_format(
    map: &DigestMap,
    algorithm: DigestAlgorithm,
    at: &str,
) -> Result<(), FormatError> {
    let hex_code = match algorithm {
        DigestAlgorithm::Sha256 => "E030",
        DigestAlgorithm::Sha512 => "E031",
    };
    for digest in map.keys() {
        let fault = if !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            Some((hex_code, "a digest is hexadecimal"))
        } else if digest.len() != algorithm.hex_len() {
            Some(("E039", "the digest length does not match digestAlgorithm"))
        } else {
            None
        };
        if let Some((code, reason)) = fault {
            return Err(FormatError::invalid(code, format!("{at}/{digest}"), reason));
        }
    }
    Ok(())
}

/// Each digest occurs once regardless of case (`code`: E096 or E097).
fn check_case_unique(map: &DigestMap, at: &str, code: &'static str) -> Result<(), FormatError> {
    let mut seen = BTreeSet::new();
    match map
        .keys()
        .find(|digest| !seen.insert(digest.to_ascii_lowercase()))
    {
        Some(digest) => Err(FormatError::invalid(
            code,
            format!("{at}/{digest}"),
            "a digest must occur only once regardless of case",
        )),
        None => Ok(()),
    }
}

/// Every path is well formed, with `codes` for the kind of path.
fn check_paths(map: &DigestMap, at: &str, codes: PathCodes) -> Result<(), FormatError> {
    for (digest, paths) in map {
        let Some(fault) = paths.iter().find_map(|path| path_fault(path)) else {
            continue;
        };
        let (code, reason) = match fault {
            PathFault::BadElement => (codes.element, "path elements must not be ., .. or empty"),
            PathFault::EdgeSlash => (codes.edge, "a path must not begin or end with /"),
        };
        return Err(FormatError::invalid(code, format!("{at}/{digest}"), reason));
    }
    Ok(())
}
