//! Redaction of PROV-O records for shared or published outputs (PRD F-046; ADR 040
//! Decision 8).
//!
//! A record read by its owner names the installation (`urn:uuid:<installation-uuid>`), the
//! version message (which may hold a label the owner typed) and every logical path, which
//! can carry names and titles. Before a record leaves the owner's control the
//! [`Redactor`]:
//!
//! - replaces the installation agent's identifier with [`REDACTED_INSTALLATION`];
//! - drops `fsq:message` from the activity;
//! - replaces each entity's `fsq:path` with an opaque `redacted-<n>` and its `@id` with the
//!   same token in place of the percent-encoded path, numbering paths consistently across
//!   every record one redactor handles, so derivation chains stay linked; an `fsq:path` it
//!   cannot number (a node without a string `@id`, or a path that is not a string) is
//!   dropped, so the redactor fails closed;
//! - rewrites every reference to a replaced identifier.
//!
//! Object IDs, version names, operation IDs, kinds, times, digests and journal entry IDs
//! are kept: they are opaque. The host software agent (product, distribution, version) is
//! kept: it names no person. The result is JSON-LD in the same shape, not a record the
//! strict profile reader accepts, because its paths and installation are no longer real.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// The identifier that replaces the installation agent's `urn:uuid:<installation-uuid>`.
pub const REDACTED_INSTALLATION: &str = "urn:filesquid:agent:redacted:installation";

/// Deepest JSON nesting walked; a record of the profile nests far less.
const MAX_DEPTH: usize = 32;

/// Prefix of entity identifiers (`urn:filesquid:entity:<object>:<version>:<path>`).
const ENTITY_PREFIX: &str = "urn:filesquid:entity:";

/// Why a record could not be redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedactError {
    /// The bytes are not a JSON object.
    NotJson,
    /// The document nests deeper than the walk allows.
    TooDeep,
}

impl std::fmt::Display for RedactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotJson => f.write_str("the record is not a JSON object"),
            Self::TooDeep => f.write_str("the record nests too deeply"),
        }
    }
}

impl std::error::Error for RedactError {}

/// Redacts records with one consistent numbering of logical paths.
#[derive(Debug, Default, Clone)]
pub struct Redactor {
    paths: BTreeMap<String, String>,
}

impl Redactor {
    /// A redactor that has numbered no path yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// `record` (JSON-LD bytes) with identities, messages and paths removed.
    ///
    /// # Errors
    ///
    /// [`RedactError`] when the bytes are not a JSON object or nest too deeply.
    pub fn redact(&mut self, record: &[u8]) -> Result<Value, RedactError> {
        let mut document: Value =
            serde_json::from_slice(record).map_err(|_| RedactError::NotJson)?;
        if !document.is_object() {
            return Err(RedactError::NotJson);
        }
        let mut renames = BTreeMap::new();
        self.collect(&document, 0, &mut renames)?;
        rewrite(&mut document, &renames, 0)?;
        Ok(document)
    }

    /// Record the new identifier of every installation agent and entity under `value`.
    fn collect(
        &mut self,
        value: &Value,
        depth: usize,
        renames: &mut BTreeMap<String, String>,
    ) -> Result<(), RedactError> {
        if depth > MAX_DEPTH {
            return Err(RedactError::TooDeep);
        }
        match value {
            Value::Object(node) => {
                self.rename_node(node, renames);
                for child in node.values() {
                    self.collect(child, depth + 1, renames)?;
                }
            }
            Value::Array(items) => {
                for child in items {
                    self.collect(child, depth + 1, renames)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The new identifier of `node` when it is an installation agent or an entity.
    fn rename_node(&mut self, node: &Map<String, Value>, renames: &mut BTreeMap<String, String>) {
        let Some(id) = node.get("@id").and_then(Value::as_str) else {
            return;
        };
        if node.get("fsq:agentKind").and_then(Value::as_str) == Some("installation") {
            renames.insert(id.to_string(), REDACTED_INSTALLATION.to_string());
            return;
        }
        let Some(path) = node.get("fsq:path").and_then(Value::as_str) else {
            return;
        };
        let token = self.path_token(path);
        renames.insert(id.to_string(), entity_id_without_path(id, &token));
    }

    /// The opaque token of `path`: `redacted-<n>`, the same for the same path.
    fn path_token(&mut self, path: &str) -> String {
        let next = format!("redacted-{}", self.paths.len() + 1);
        self.paths.entry(path.to_string()).or_insert(next).clone()
    }
}

/// `record` (JSON-LD bytes) redacted on its own, as compact JSON text.
///
/// # Errors
///
/// [`RedactError`] when the bytes are not a JSON object or nest too deeply.
pub fn redact_record(record: &[u8]) -> Result<String, RedactError> {
    let mut redactor = Redactor::new();
    let value = redactor.redact(record)?;
    Ok(value.to_string())
}

/// `id` (`urn:filesquid:entity:<object>:<version>:<path>`) with its path replaced by
/// `token`; an identifier of another form becomes `urn:filesquid:entity:redacted:<token>`.
fn entity_id_without_path(id: &str, token: &str) -> String {
    let Some(rest) = id.strip_prefix(ENTITY_PREFIX) else {
        return format!("{ENTITY_PREFIX}redacted:{token}");
    };
    match rest.rsplit_once(':') {
        Some((object_version, _)) => format!("{ENTITY_PREFIX}{object_version}:{token}"),
        None => format!("{ENTITY_PREFIX}redacted:{token}"),
    }
}

/// Apply `renames` to every string under `value`, drop `fsq:message` and replace each
/// `fsq:path` with its token, or drop it when it has none.
fn rewrite(
    value: &mut Value,
    renames: &BTreeMap<String, String>,
    depth: usize,
) -> Result<(), RedactError> {
    if depth > MAX_DEPTH {
        return Err(RedactError::TooDeep);
    }
    match value {
        Value::Object(node) => rewrite_node(node, renames, depth),
        Value::Array(items) => items
            .iter_mut()
            .try_for_each(|child| rewrite(child, renames, depth + 1)),
        Value::String(text) => {
            if let Some(new) = renames.get(text.as_str()) {
                *text = new.clone();
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// [`rewrite`] for one JSON object.
fn rewrite_node(
    node: &mut Map<String, Value>,
    renames: &BTreeMap<String, String>,
    depth: usize,
) -> Result<(), RedactError> {
    node.remove("fsq:message");
    let path = node.remove("fsq:path");
    let token = node
        .get("@id")
        .and_then(Value::as_str)
        .and_then(|id| renames.get(id))
        .and_then(|id| id.rsplit_once(':'))
        .map(|(_, token)| token.to_string());
    if let (Some(_), Some(token)) = (path, token) {
        node.insert("fsq:path".to_string(), Value::String(token));
    }
    node.values_mut()
        .try_for_each(|child| rewrite(child, renames, depth + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_keeps_its_token_across_records() {
        let mut redactor = Redactor::new();
        let first = redactor.path_token("Holiday/Anna.jpg");
        let second = redactor.path_token("other.txt");
        assert_eq!(redactor.path_token("Holiday/Anna.jpg"), first);
        assert_ne!(first, second);
        assert_eq!(redactor.path_token("other.txt"), second);
    }

    #[test]
    fn an_entity_id_loses_only_its_path() {
        let id = "urn:filesquid:entity:urn:uuid:a:v2:Holiday%2FAnna.jpg";
        assert_eq!(
            entity_id_without_path(id, "redacted-1"),
            "urn:filesquid:entity:urn:uuid:a:v2:redacted-1"
        );
        assert_eq!(
            entity_id_without_path("odd", "redacted-1"),
            "urn:filesquid:entity:redacted:redacted-1"
        );
    }

    #[test]
    fn a_path_without_a_numbered_identifier_is_dropped() {
        let record = br#"{"@graph":[{"fsq:path":"Holiday/Anna.jpg"},{"@id":7,"fsq:path":"x/Anna.txt"},{"@id":"urn:e","fsq:path":{"v":"Anna"}}]}"#;
        let text = redact_record(record).unwrap();
        assert!(!text.contains("Anna"), "{text}");
        assert!(!text.contains("fsq:path"), "{text}");
    }

    #[test]
    fn non_objects_and_deep_documents_are_refused() {
        assert_eq!(redact_record(b"[1]"), Err(RedactError::NotJson));
        assert_eq!(redact_record(b"nope"), Err(RedactError::NotJson));
        let deep = format!("{{\"a\":{}1{}}}", "[".repeat(40), "]".repeat(40));
        assert_eq!(redact_record(deep.as_bytes()), Err(RedactError::TooDeep));
    }
}
