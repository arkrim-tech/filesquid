//! Identifier forms of the profile `filesquid-prov/1` (PROV-S-003).
//!
//! | Node | Identifier |
//! |---|---|
//! | Activity | `urn:filesquid:activity:<operation-id>:<object-id>:<version>` |
//! | Entity | `urn:filesquid:entity:<object-id>:<version>:<percent-encoded logical path>` |
//! | Installation agent | `urn:uuid:<installation-uuid>` |
//! | Host software agent | `urn:filesquid:software:<product>:<distribution>:<version>` (each part percent-encoded) |
//! | Tool software agent (reserved) | `urn:filesquid:tool:<name>:<version>` (each part percent-encoded) |
//! | Unrecorded agent | `urn:filesquid:agent:unrecorded:<role>` |
//!
//! Object, operation and version identifiers are validated by the builder
//! and the reader to RFC 3986 path characters, so they are used verbatim.

use crate::model::{AgentRole, HostFacts, ToolFacts};

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

/// Identifier of the activity that `operation_id` committed as `version` of `object_id`.
pub fn activity_id(operation_id: &str, object_id: &str, version: &str) -> String {
    format!("urn:filesquid:activity:{operation_id}:{object_id}:{version}")
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
