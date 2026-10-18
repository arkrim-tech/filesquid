//! OCFL 1.1 inventories (OCFL 1.1 section 3.5), their digest sidecars
//! (section 3.6) and version names (section 3.3).
//!
//! [`Inventory::parse`] reads one `inventory.json` from untrusted bytes: it
//! caps the size ([`MAX_INVENTORY_BYTES`]), reads strict JSON (no repeated
//! keys, nesting at most [`MAX_JSON_DEPTH`]) and checks every rule that one
//! inventory's bytes can decide, refusing the first broken rule with its
//! OCFL 1.1 validation code ([`FormatError::code`]). Object-level checks
//! (files against the manifest, version directories, earlier version
//! inventories) are structural validation, built on these types.
//!
//! The types hold the inventory as written: digests keep their case, paths
//! are `/`-separated strings, and `created` is the RFC 3339 text.

mod blocks;
mod check;
mod error;
mod json;
mod paths;
mod read;
mod sidecar;
mod timestamp;
mod version;
mod write;

use std::collections::BTreeMap;

use crate::digest::DigestAlgorithm;
use crate::namaste::SpecVersion;

pub use error::FormatError;
pub use json::MAX_JSON_DEPTH;
pub use sidecar::{
    check_sidecar_name, parse_sidecar, sidecar_algorithm, sidecar_contents, sidecar_name,
    verify_sidecar,
};
pub use version::VersionNum;

/// The inventory file name.
pub const INVENTORY_FILE: &str = "inventory.json";

/// The content directory name when an inventory does not set one.
pub const DEFAULT_CONTENT_DIRECTORY: &str = "content";

/// Largest inventory [`Inventory::parse`] accepts: 64 MiB. Larger inputs are
/// refused before any parsing, so untrusted bytes cannot exhaust memory.
pub const MAX_INVENTORY_BYTES: usize = 64 * 1024 * 1024;

/// Digests mapped to the paths with that content, as in a manifest, a
/// version state or a fixity block. Keys keep the case they were written in.
pub type DigestMap = BTreeMap<String, Vec<String>>;

/// One OCFL 1.1 inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    /// The object identifier (`id`).
    pub id: String,
    /// The specification version the inventory's `type` names.
    pub spec: SpecVersion,
    /// The content-addressing algorithm (`digestAlgorithm`).
    pub digest_algorithm: DigestAlgorithm,
    /// The most recent version (`head`).
    pub head: VersionNum,
    /// The `contentDirectory` value when the inventory sets one; see
    /// [`Inventory::content_directory`] for the effective name.
    pub content_directory: Option<String>,
    /// Content digests mapped to content paths (`manifest`).
    pub manifest: DigestMap,
    /// Every version, by version number (`versions`).
    pub versions: BTreeMap<VersionNum, Version>,
    /// Additional fixity blocks by algorithm name (`fixity`), when present.
    /// Algorithms the engine does not compute are kept, never refused.
    pub fixity: Option<BTreeMap<String, DigestMap>>,
}

/// One version block of an inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// When the version was created: RFC 3339 text as written.
    pub created: String,
    /// Content digests mapped to logical paths: the version's logical state.
    pub state: DigestMap,
    /// Why the version was made, when given.
    pub message: Option<String>,
    /// Who made the version, when given.
    pub user: Option<User>,
}

/// The user or agent that created a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// A readable name.
    pub name: String,
    /// A URI for the user (such as `mailto:`), when given.
    pub address: Option<String>,
}

impl Inventory {
    /// Parse and check one inventory from untrusted bytes.
    ///
    /// # Errors
    ///
    /// [`FormatError::TooLarge`] above [`MAX_INVENTORY_BYTES`], or
    /// [`FormatError::Invalid`] with the OCFL 1.1 code of the first broken
    /// rule.
    pub fn parse(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() > MAX_INVENTORY_BYTES {
            return Err(FormatError::TooLarge {
                what: "inventory",
                size: bytes.len(),
                limit: MAX_INVENTORY_BYTES,
            });
        }
        let value = json::read_json(bytes)?;
        let inventory = read::read_inventory(&value)?;
        check::check_consistency(&inventory)?;
        Ok(inventory)
    }

    /// The effective content directory name: the inventory's
    /// `contentDirectory`, or [`DEFAULT_CONTENT_DIRECTORY`].
    pub fn content_directory(&self) -> &str {
        self.content_directory
            .as_deref()
            .unwrap_or(DEFAULT_CONTENT_DIRECTORY)
    }

    /// The head version's block. A parsed inventory always has one; an
    /// inventory built by hand may not.
    pub fn head_version(&self) -> Option<&Version> {
        self.versions.get(&self.head)
    }

    /// The inventory as pretty-printed JSON with sorted keys, ending in a
    /// newline. [`Inventory::parse`] reads it back to an equal inventory.
    pub fn to_json(&self) -> Vec<u8> {
        write::inventory_json(self)
    }
}
