//! The JSON-LD profile `filesquid-prov/1` (PROV-S-003) and bounded parsing
//! (PROV-S-004).
//!
//! A record is one UTF-8 JSON-LD document with an embedded `@context`
//! (readers never fetch a context over the network), a `fsq:profile` member
//! and a `@graph`:
//!
//! ```json
//! {"@context":{"prov":"http://www.w3.org/ns/prov#","xsd":"http://www.w3.org/2001/XMLSchema#","fsq":"https://filesquid.tech/ns/prov/v1#"},
//!  "fsq:profile":"filesquid-prov/1",
//!  "@graph":[ <activity>, <agents sorted by @id>..., <entities sorted by @id>... ]}
//! ```
//!
//! [`to_json_ld`] always writes the current profile ([`Profile::CURRENT`]).
//! [`parse_record`] reads every profile in [`Profile::ALL`]: it reads the
//! document with the bounds below, then hands it to that profile's reader.
//! A later `filesquid-prov/2` adds a variant, a module next to `v1` and a
//! match arm; `/1` documents keep being read by `v1`.

mod strict;
mod v1;

use crate::error::ProvError;
use crate::model::Record;

/// `prov:` prefix: the W3C PROV namespace IRI.
pub const CONTEXT_PROV: &str = "http://www.w3.org/ns/prov#";

/// `xsd:` prefix: the XML Schema datatypes namespace IRI.
pub const CONTEXT_XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// `fsq:` prefix: the FileSquid PROV profile vocabulary, version 1.
pub const CONTEXT_FSQ: &str = "https://filesquid.tech/ns/prov/v1#";

/// The `fsq:profile` value of profile version 1.
pub const PROFILE_V1: &str = "filesquid-prov/1";

/// Largest record a reader accepts: 1 MiB (1,048,576 bytes).
pub const MAX_RECORD_BYTES: usize = 1_048_576;

/// Deepest nesting of JSON arrays and objects a reader accepts (the root
/// object is level 1).
pub const MAX_DEPTH: usize = 16;

/// Most nodes a `@graph` may hold.
pub const MAX_GRAPH_NODES: usize = 100_000;

/// A version of the FileSquid PROV-O profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    /// `filesquid-prov/1`.
    V1,
}

impl Profile {
    /// The profile [`to_json_ld`] writes.
    pub const CURRENT: Profile = Profile::V1;

    /// Every profile [`parse_record`] reads, oldest first.
    pub const ALL: [Profile; 1] = [Profile::V1];

    /// The `fsq:profile` value, for example `filesquid-prov/1`.
    pub fn name(self) -> &'static str {
        match self {
            Self::V1 => PROFILE_V1,
        }
    }

    /// The profile whose `fsq:profile` value is `name`.
    pub fn from_name(name: &str) -> Option<Profile> {
        Self::ALL.into_iter().find(|profile| profile.name() == name)
    }
}

/// Serialize `record` as a JSON-LD document in the current profile.
///
/// The output is deterministic: members are written in a fixed order, the
/// activity comes first, then the agents and the entities each sorted by
/// identifier, and the document ends with one newline. Equal records give
/// equal bytes. The text is compact JSON in UTF-8.
pub fn to_json_ld(record: &Record) -> String {
    match Profile::CURRENT {
        Profile::V1 => v1::write(record),
    }
}

/// Parse and validate a stored record of any known profile.
///
/// # Errors
/// - [`ProvError::TooLarge`] above [`MAX_RECORD_BYTES`];
/// - [`ProvError::InvalidJson`] for malformed JSON or UTF-8;
/// - [`ProvError::TooDeep`] for nesting deeper than [`MAX_DEPTH`];
/// - [`ProvError::DuplicateKey`] for a member name repeated in one object;
/// - [`ProvError::UnknownProfile`] for a missing or unknown `fsq:profile`;
/// - [`ProvError::InvalidNode`] for anything else that breaks the profile.
pub fn parse_record(bytes: &[u8]) -> Result<Record, ProvError> {
    let root = strict::read(bytes)?;
    let map = root.as_object().ok_or(ProvError::InvalidNode {
        reason: "the document is not a JSON object",
    })?;
    let profile = map
        .get("fsq:profile")
        .and_then(serde_json::Value::as_str)
        .and_then(Profile::from_name)
        .ok_or(ProvError::UnknownProfile)?;
    match profile {
        Profile::V1 => v1::read(map),
    }
}
