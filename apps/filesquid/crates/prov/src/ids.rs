//! Identifier forms of the profile `filesquid-prov/1` (PROV-S-003).
//!
//! | Node | Identifier |
//! |---|---|
//! | Activity | `urn:filesquid:activity:<operation-id with `:` as `%3A`>:<object-id>:<version>` |
//! | Entity | `urn:filesquid:entity:<object-id>:<version>:<percent-encoded logical path>` |
//! | Installation agent | `urn:uuid:<installation-uuid>` |
//! | Host software agent | `urn:filesquid:software:<product>:<distribution>:<version>` (each part percent-encoded) |
//! | Tool software agent (reserved) | `urn:filesquid:tool:<name>:<version>` (each part percent-encoded) |
//! | Unrecorded agent | `urn:filesquid:agent:unrecorded:<role>` |
//!
//! Object, operation and version identifiers are validated by the builder
//! and the reader to RFC 3986 path characters without `%`, so they are used
//! verbatim, except that every `:` of the operation identifier is written as
//! `%3A`. Then the first `:` after the prefix ends the operation, a version
//! (`v` and digits) holds no `:`, so the last `:` starts the version, and
//! the activity identifier names exactly one operation, object and version
//! even when an object identifier holds `:` (`urn:uuid:...`).

use crate::model::{AgentRole, HostFacts, ToolFacts};
use crate::scalar::{check_identifier, check_version};

/// Prefix of every activity identifier.
const ACTIVITY_PREFIX: &str = "urn:filesquid:activity:";

/// Hexadecimal digits for percent-encoding, upper case as RFC 3986 recommends.
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encode `text`: RFC 3986 unreserved characters (`A-Z a-z 0-9 - . _ ~`)
/// stay as they are, and every other UTF-8 byte becomes `%XX` in upper case.
///
/// ```
/// assert_eq!(filesquid_prov::percent_encode("Fotos/été 1.jpg"), "Fotos%2F%C3%A9t%C3%A9%201.jpg");
/// ```
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
            continue;
        }
        out.push('%');
        out.push(char::from(HEX_UPPER[usize::from(byte >> 4)]));
        out.push(char::from(HEX_UPPER[usize::from(byte & 0x0f)]));
    }
    out
}

/// Identifier of the activity that `operation_id` committed as `version` of
/// `object_id`. Every `:` of the operation identifier is written as `%3A`,
/// so distinct (operation, object, version) triples never share an
/// identifier.
///
/// ```
/// assert_eq!(
///     filesquid_prov::activity_id("op:1", "urn:uuid:abc", "v2"),
///     "urn:filesquid:activity:op%3A1:urn:uuid:abc:v2"
/// );
/// ```
pub fn activity_id(operation_id: &str, object_id: &str, version: &str) -> String {
    let operation = operation_id.replace(':', "%3A");
    format!("{ACTIVITY_PREFIX}{operation}:{object_id}:{version}")
}

/// Whether `text` is an activity identifier in the profile's form: valid
/// operation, object and version parts, spelt exactly as [`activity_id`]
/// writes them. The parts are checked one by one, so an identifier longer
/// than one part's bound is still accepted.
pub(crate) fn check_activity_id(text: &str) -> bool {
    let Some((operation, rest)) = text
        .strip_prefix(ACTIVITY_PREFIX)
        .and_then(|rest| rest.split_once(':'))
    else {
        return false;
    };
    let Some((object_id, version)) = rest.rsplit_once(':') else {
        return false;
    };
    let operation = operation.replace("%3A", ":");
    check_identifier(&operation)
        && check_identifier(object_id)
        && check_version(version)
        && activity_id(&operation, object_id, version) == text
}

/// Identifier of the state of logical path `path` in `version` of `object_id`.
pub fn entity_id(object_id: &str, version: &str, path: &str) -> String {
    format!(
        "urn:filesquid:entity:{object_id}:{version}:{}",
        percent_encode(path)
    )
}

/// Identifier of a FileSquid host software agent.
pub(crate) fn host_agent_id(host: &HostFacts) -> String {
    format!(
        "urn:filesquid:software:{}:{}:{}",
        percent_encode(&host.product),
        percent_encode(&host.distribution),
        percent_encode(&host.version)
    )
}

/// Identifier of a tool or model software agent (reserved kind).
pub(crate) fn tool_agent_id(tool: &ToolFacts) -> String {
    format!(
        "urn:filesquid:tool:{}:{}",
        percent_encode(&tool.name),
        percent_encode(&tool.version)
    )
}

/// Identifier of the agent that stands for an unobserved agent in `role`.
pub(crate) fn unrecorded_agent_id(role: AgentRole) -> String {
    format!("urn:filesquid:agent:unrecorded:{}", role.as_str())
}
