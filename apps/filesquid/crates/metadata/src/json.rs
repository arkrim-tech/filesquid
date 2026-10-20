//! JSON sidecar interchange (MD-S-010).
//!
//! A JSON sidecar is one UTF-8 JSON document per OCFL object. It binds
//! `{namespace, term, value}` statements to the OCFL object identifier and
//! the SHA-256 digest of the described logical file:
//!
//! ```json
//! {
//!   "objectId": "urn:example:object-1",
//!   "digestAlgorithm": "sha256",
//!   "digest": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
//!   "statements": [
//!     { "namespace": "dcterms", "term": "title", "value": "Harbour" }
//!   ]
//! }
//! ```
//!
//! A sidecar is interchange only: canonical statements stay in `metadata.md`
//! ([ADR 022](../../../../../resources/architecture/022-dublin-core-descriptive-metadata.md),
//! [ADR 024](../../../../../resources/architecture/024-filesquid-metadata-engine.md)).
//! Every function here is pure: callers read the file bytes and the sidecar
//! text, and write the results, themselves.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::namespace::NamespaceRegistry;
use crate::record::{resolve_term, Record, Statement};
use crate::MetadataError;

/// The only digest algorithm a sidecar may name (OCFL inventory spelling).
pub const DIGEST_ALGORITHM: &str = "sha256";

/// One `{namespace, term, value}` statement as written in a sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonStatement {
    /// Namespace prefix: `dcterms`, `exif`, `schema` or a registered
    /// extension prefix (`dc` maps to `dcterms` on import).
    pub namespace: String,
    /// Local name within the namespace, for example `title`.
    pub term: String,
    /// Literal value.
    pub value: String,
}

/// A parsed JSON sidecar document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonSidecar {
    /// OCFL object identifier the statements describe.
    pub object_id: String,
    /// Digest algorithm of [`JsonSidecar::digest`]; always [`DIGEST_ALGORITHM`].
    pub digest_algorithm: String,
    /// Hex SHA-256 digest of the described logical file.
    pub digest: String,
    /// Statements in record order.
    pub statements: Vec<JsonStatement>,
}

/// The object and file a caller expects a sidecar to describe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonTarget<'a> {
    /// OCFL object identifier of the target object.
    pub object_id: &'a str,
    /// Hex SHA-256 digest of the target logical file (see [`sha256_hex`]).
    pub digest: &'a str,
}

/// Errors from reading or applying a JSON sidecar.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum JsonError {
    /// The text is not one JSON sidecar object with the required fields.
    #[error("malformed JSON sidecar: {0}")]
    Malformed(String),
    /// The sidecar names a digest algorithm other than SHA-256.
    #[error("unsupported digest algorithm {0}; a JSON sidecar MUST use sha256")]
    UnsupportedDigestAlgorithm(String),
    /// The sidecar digest does not match the target file.
    #[error("digest mismatch: the JSON sidecar does not describe this file")]
    DigestMismatch,
    /// The sidecar names a different OCFL object.
    #[error("object identifier mismatch: the JSON sidecar describes another object")]
    ObjectIdMismatch,
    /// A statement uses a namespace prefix that is not registered.
    #[error("namespace prefix {0} is not registered")]
    UnregisteredNamespace(String),
    /// A statement fails the `/terms/` register (MD-S-002).
    #[error(transparent)]
    Register(#[from] MetadataError),
}

/// Return the lowercase hex SHA-256 digest of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

/// Lowercase hex of `digest`.
pub(crate) fn lower_hex(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Serialize `record` as a pretty-printed UTF-8 JSON sidecar.
///
/// `object_id` is the OCFL object identifier and `digest` the hex SHA-256 of
/// the described logical file. The Markdown body of `metadata.md` is not
/// part of the sidecar.
pub fn to_json(object_id: &str, digest: &str, record: &Record) -> String {
    let sidecar = JsonSidecar {
        object_id: object_id.to_string(),
        digest_algorithm: DIGEST_ALGORITHM.to_string(),
        digest: digest.to_ascii_lowercase(),
        statements: record.statements.iter().map(to_json_statement).collect(),
    };
    let mut text =
        serde_json::to_string_pretty(&sidecar).expect("a sidecar of strings always serializes");
    text.push('\n');
    text
}

/// Convert a record statement into its sidecar form.
fn to_json_statement(statement: &Statement) -> JsonStatement {
    JsonStatement {
        namespace: statement.prefix().to_string(),
        term: statement.term.clone(),
        value: statement.value.clone(),
    }
}

/// Parse one JSON sidecar document without checking its binding.
pub fn parse_json(text: &str) -> Result<JsonSidecar, JsonError> {
    serde_json::from_str(text).map_err(|err| JsonError::Malformed(err.to_string()))
}

/// Apply a JSON sidecar onto `record` after every check passes. Without a
/// registry no extension prefix is registered.
///
/// # Errors
///
/// As [`apply_json_with`].
pub fn apply_json(text: &str, target: &JsonTarget, record: &mut Record) -> Result<(), JsonError> {
    apply_json_with(text, target, &NamespaceRegistry::default(), record)
}

/// Apply a JSON sidecar onto `record` after every check passes, resolving
/// extension prefixes through `registry` (MD-S-010, MD-S-012).
///
/// Checks, in order: the document parses, the algorithm is SHA-256, the
/// digest matches `target.digest` (hex, case-insensitive), the object
/// identifier matches `target.object_id`, and every statement passes the
/// namespace registry and `/terms/` register checks. Statements are staged
/// on a copy, so a rejected sidecar leaves `record` unchanged.
///
/// # Errors
///
/// [`JsonError`] for the first check that fails.
pub fn apply_json_with(
    text: &str,
    target: &JsonTarget,
    registry: &NamespaceRegistry,
    record: &mut Record,
) -> Result<(), JsonError> {
    let sidecar = parse_json(text)?;
    check_binding(&sidecar, target)?;
    let mut staged = record.clone();
    for statement in &sidecar.statements {
        stage_statement(statement, registry, &mut staged)?;
    }
    *record = staged;
    Ok(())
}

/// Reject a sidecar bound to another algorithm, file or object.
fn check_binding(sidecar: &JsonSidecar, target: &JsonTarget) -> Result<(), JsonError> {
    if sidecar.digest_algorithm != DIGEST_ALGORITHM {
        return Err(JsonError::UnsupportedDigestAlgorithm(
            sidecar.digest_algorithm.clone(),
        ));
    }
    if !sidecar.digest.eq_ignore_ascii_case(target.digest) {
        return Err(JsonError::DigestMismatch);
    }
    if sidecar.object_id != target.object_id {
        return Err(JsonError::ObjectIdMismatch);
    }
    Ok(())
}

/// Register-check one statement and set it on `record`.
fn stage_statement(
    statement: &JsonStatement,
    registry: &NamespaceRegistry,
    record: &mut Record,
) -> Result<(), JsonError> {
    let (namespace, term) =
        resolve_term(&statement.namespace, &statement.term, registry).map_err(json_error)?;
    record.set_registered(registry, namespace, term, statement.value.as_str())?;
    Ok(())
}

/// The JSON form of a register or registry refusal: an unregistered prefix
/// is named on its own.
fn json_error(err: MetadataError) -> JsonError {
    match err {
        MetadataError::UnregisteredNamespace(prefix) => JsonError::UnregisteredNamespace(prefix),
        other => JsonError::Register(other),
    }
}
