//! The staging index: what one staging area holds against which base.
//!
//! Format `filesquid-ocfl-staging` version 1, one UTF-8 JSON object:
//!
//! ```json
//! {
//!   "type": "filesquid-ocfl-staging",
//!   "version": 1,
//!   "objectId": "urn:example:obj",
//!   "base": "v2",
//!   "paths": {
//!     "a.txt": { "sha256": "<64 lowercase hex digits>" },
//!     "old.txt": { "removed": true }
//!   }
//! }
//! ```
//!
//! `base` is the object head the changes were staged against, or `null`
//! for an object with no version yet. `paths` lists only staged logical
//! paths: each one either holds the content with that sha256 digest or is
//! removed. Paths not listed keep their content from `base`. The bytes of
//! a digest the object's `manifest` lacks are in the staging area at
//! `blobs/<digest>`. The parser is strict and bounded: unknown keys, an
//! unknown type or version, a path that breaks the OCFL logical-path rules
//! (E052, E053) or a malformed digest are refused.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::delta::WorkingState;
use crate::inventory::json::read_json;
use crate::inventory::paths::{path_fault, PathFault};
use crate::inventory::VersionNum;

/// The index's `type` value.
pub(crate) const INDEX_TYPE: &str = "filesquid-ocfl-staging";

/// The index format version this engine reads and writes.
pub(crate) const INDEX_VERSION: u64 = 1;

/// The largest staging index read, in bytes.
pub(crate) const MAX_INDEX_BYTES: usize = 64 * 1024 * 1024;

/// What a staged logical path holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Staged {
    /// The content with this sha256 digest (lowercase hex).
    File(String),
    /// Nothing: the path is removed.
    Removed,
}

/// The staged changes of one object against one base head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagingIndex {
    /// The object the changes are for.
    pub object_id: String,
    /// The head they were staged against.
    pub base: Option<VersionNum>,
    /// Every staged logical path.
    pub paths: BTreeMap<String, Staged>,
}

impl StagingIndex {
    /// An index with nothing staged.
    pub fn new(object_id: &str, base: Option<VersionNum>) -> Self {
        Self {
            object_id: object_id.to_string(),
            base,
            paths: BTreeMap::new(),
        }
    }

    /// Stage `staged` at `path`, or unstage the path when that is what
    /// `base_state` already holds.
    pub fn set(&mut self, base_state: &WorkingState, path: &str, staged: Staged) {
        let unchanged = match &staged {
            Staged::File(digest) => base_state.get(path) == Some(digest),
            Staged::Removed => !base_state.contains_key(path),
        };
        if unchanged {
            self.paths.remove(path);
        } else {
            self.paths.insert(path.to_string(), staged);
        }
    }

    /// The working state: `base_state` with every staged path applied.
    pub fn working(&self, base_state: &WorkingState) -> WorkingState {
        let mut working = base_state.clone();
        for (path, staged) in &self.paths {
            match staged {
                Staged::File(digest) => working.insert(path.clone(), digest.clone()),
                Staged::Removed => working.remove(path),
            };
        }
        working
    }

    /// The index as JSON bytes.
    pub fn to_json(&self) -> Vec<u8> {
        let paths: Map<String, Value> = self
            .paths
            .iter()
            .map(|(path, staged)| (path.clone(), staged_json(staged)))
            .collect();
        let value = json!({
            "type": INDEX_TYPE,
            "version": INDEX_VERSION,
            "objectId": self.object_id,
            "base": self.base.as_ref().map(ToString::to_string),
            "paths": paths,
        });
        // A `Value` always formats; `{:#}` is serde_json's pretty printer.
        let mut text = format!("{value:#}");
        text.push('\n');
        text.into_bytes()
    }

    /// Parse index bytes the engine wrote earlier.
    ///
    /// # Errors
    ///
    /// The reason the bytes are not a version 1 staging index.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value = read_json(bytes).map_err(|err| err.to_string())?;
        let root = value.as_object().ok_or("not a JSON object")?;
        only_keys(root, &["type", "version", "objectId", "base", "paths"])?;
        check_header(root)?;
        let object_id = root
            .get("objectId")
            .and_then(Value::as_str)
            .ok_or("objectId must be a string")?;
        Ok(Self {
            object_id: object_id.to_string(),
            base: parse_base(root.get("base"))?,
            paths: parse_paths(root.get("paths"))?,
        })
    }
}

/// One staged entry as JSON.
fn staged_json(staged: &Staged) -> Value {
    match staged {
        Staged::File(digest) => json!({ "sha256": digest }),
        Staged::Removed => json!({ "removed": true }),
    }
}

/// Refuse any key of `object` not in `allowed`.
fn only_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("unknown key {:?}", truncate(key))),
        None => Ok(()),
    }
}

/// Check `type` and `version`.
fn check_header(root: &Map<String, Value>) -> Result<(), String> {
    if root.get("type").and_then(Value::as_str) != Some(INDEX_TYPE) {
        return Err(format!("type must be {INDEX_TYPE:?}"));
    }
    if root.get("version").and_then(Value::as_u64) != Some(INDEX_VERSION) {
        return Err(format!("version must be {INDEX_VERSION}"));
    }
    Ok(())
}

/// `base`: `null` or a version directory name.
fn parse_base(value: Option<&Value>) -> Result<Option<VersionNum>, String> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => VersionNum::parse(name)
            .map(Some)
            .map_err(|_| "base is not a version name".to_string()),
        _ => Err("base must be null or a version name".to_string()),
    }
}

/// `paths`: logical path → staged entry.
fn parse_paths(value: Option<&Value>) -> Result<BTreeMap<String, Staged>, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or("paths must be an object")?;
    let mut paths = BTreeMap::new();
    for (path, entry) in object {
        check_logical_path(path)?;
        paths.insert(path.clone(), parse_entry(path, entry)?);
    }
    Ok(paths)
}

/// One entry: `{"sha256": <digest>}` or `{"removed": true}`.
fn parse_entry(path: &str, entry: &Value) -> Result<Staged, String> {
    let object = entry.as_object().ok_or("an entry must be an object")?;
    if object.len() != 1 {
        return Err(format!(
            "{:?}: an entry has exactly one key",
            truncate(path)
        ));
    }
    if object.get("removed") == Some(&Value::Bool(true)) {
        return Ok(Staged::Removed);
    }
    let digest = object.get("sha256").and_then(Value::as_str);
    match digest {
        Some(digest) if is_sha256_hex(digest) => Ok(Staged::File(digest.to_string())),
        _ => Err(format!("{:?}: not a sha256 entry", truncate(path))),
    }
}

/// Refuse a logical path that breaks OCFL 1.1 E052 or E053.
pub(crate) fn check_logical_path(path: &str) -> Result<(), String> {
    match path_fault(path) {
        None => Ok(()),
        Some(PathFault::BadElement) => Err(format!(
            "{:?}: an element is empty, . or .. (E052)",
            truncate(path)
        )),
        Some(PathFault::EdgeSlash) => Err(format!(
            "{:?}: begins or ends with / (E053)",
            truncate(path)
        )),
    }
}

/// Whether `digest` is 64 lowercase hex digits.
pub(crate) fn is_sha256_hex(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// At most 64 characters of `text`, so errors never echo unbounded input.
fn truncate(text: &str) -> String {
    text.chars().take(64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_round_trips() {
        let mut index = StagingIndex::new("urn:x", VersionNum::parse("v3").ok());
        let base: WorkingState = [("old.txt".to_string(), "a".repeat(64))].into();
        index.set(&base, "old.txt", Staged::Removed);
        index.set(&base, "new.txt", Staged::File("b".repeat(64)));
        let parsed = StagingIndex::parse(&index.to_json()).expect("parse");
        assert_eq!(parsed, index);
    }

    #[test]
    fn staging_what_the_base_holds_unstages_the_path() {
        let base: WorkingState = [("a.txt".to_string(), "a".repeat(64))].into();
        let mut index = StagingIndex::new("urn:x", None);
        index.set(&base, "a.txt", Staged::File("b".repeat(64)));
        index.set(&base, "a.txt", Staged::File("a".repeat(64)));
        index.set(&base, "gone.txt", Staged::Removed);
        assert!(index.paths.is_empty());
    }

    #[test]
    fn a_new_object_index_has_a_null_base() {
        let index = StagingIndex::new("urn:x", None);
        let text = String::from_utf8(index.to_json()).expect("utf-8");
        assert!(text.contains("\"base\": null"), "{text}");
        assert_eq!(StagingIndex::parse(text.as_bytes()), Ok(index));
    }

    #[test]
    fn unknown_keys_and_bad_digests_are_refused() {
        let extra = br#"{"type":"filesquid-ocfl-staging","version":1,"objectId":"x","base":null,"paths":{},"x":1}"#;
        assert!(StagingIndex::parse(extra).is_err());
        let upper = format!(
            r#"{{"type":"filesquid-ocfl-staging","version":1,"objectId":"x","base":null,"paths":{{"a":{{"sha256":"{}"}}}}}}"#,
            "A".repeat(64)
        );
        assert!(StagingIndex::parse(upper.as_bytes()).is_err());
    }
}
