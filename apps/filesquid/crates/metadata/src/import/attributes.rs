//! Extended attributes of an import entry and the bounds on inspecting them
//! ([ADR 038](../../../../../resources/architecture/038-import-file-record.md)
//! Decision 4, MD-S-013).
//!
//! The host reads the attributes; this module does the bookkeeping the bounds
//! need. [`AttributeCollector`] tells the host how much it may still read,
//! refuses what exceeds a bound, and marks everything it cut, so nothing is
//! dropped silently. [`Attributes::validate`] re-checks the same bounds on a
//! record read back.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ImportError;
use crate::json::lower_hex;
use crate::raw::Raw;

/// Most attribute names inspected per acquisition entry.
pub const MAX_ATTRIBUTE_NAMES: usize = 256;

/// Most attribute value bytes read per acquisition entry: 1 MiB.
pub const MAX_INSPECTED_BYTES: u64 = 1024 * 1024;

/// Largest value stored whole: 64 KiB. A larger one is kept as size and
/// SHA-256 only.
pub const MAX_STORED_VALUE_BYTES: u64 = 64 * 1024;

/// What the record keeps of one attribute's value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttrValue {
    /// The whole value (at most 64 KiB).
    Full {
        /// The value as read.
        value: Raw,
    },
    /// A value over 64 KiB: its size is on the attribute, its digest here.
    Digest {
        /// Lowercase hex SHA-256 of the whole value.
        sha256: String,
    },
    /// The read budget ran out or the read failed: the digest is incomplete.
    Incomplete,
    /// The outgoing disclosure policy removed the value.
    Withheld,
}

/// One extended attribute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribute {
    /// The attribute name as reported.
    pub name: Raw,
    /// The value's size in bytes as the source reported it.
    pub size: u64,
    /// What the record keeps of the value.
    pub value: AttrValue,
    /// Known attributes decoded: each key (for example `agent`, `urls`) with
    /// its values in order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub decoded: BTreeMap<String, Vec<String>>,
}

/// The extended attributes of one acquisition entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// The attributes inspected, in the order the host listed them.
    pub entries: Vec<Attribute>,
    /// How many attribute names the source has, as far as the host counted.
    pub names_enumerated: u64,
    /// Whether names were left out because more than 256 exist.
    pub truncated: bool,
    /// Value bytes read from the source for this entry.
    pub bytes_inspected: u64,
}

impl Attributes {
    /// Check the bounds of MD-S-013.
    ///
    /// # Errors
    ///
    /// [`ImportError::TooManyAttributeNames`] over 256 names,
    /// [`ImportError::AttributeReadOverBudget`] over 1 MiB read,
    /// [`ImportError::AttributeValueTooLarge`] for a whole value over 64 KiB,
    /// [`ImportError::AttributeBytesUnderReported`] when the whole values
    /// stored add up to more bytes than `bytes_inspected` claims,
    /// and [`ImportError::TruncationNotMarked`] when names were left out
    /// without the `truncated` mark.
    pub fn validate(&self) -> Result<(), ImportError> {
        if self.entries.len() > MAX_ATTRIBUTE_NAMES {
            return Err(ImportError::TooManyAttributeNames);
        }
        if self.bytes_inspected > MAX_INSPECTED_BYTES {
            return Err(ImportError::AttributeReadOverBudget);
        }
        let whole: Vec<u64> = self
            .entries
            .iter()
            .filter_map(|a| match &a.value {
                AttrValue::Full { value } => Some(value.as_bytes().len() as u64),
                _ => None,
            })
            .collect();
        if whole.iter().any(|len| *len > MAX_STORED_VALUE_BYTES) {
            return Err(ImportError::AttributeValueTooLarge);
        }
        if whole.iter().sum::<u64>() > self.bytes_inspected {
            return Err(ImportError::AttributeBytesUnderReported);
        }
        if self.names_enumerated > self.entries.len() as u64 && !self.truncated {
            return Err(ImportError::TruncationNotMarked);
        }
        Ok(())
    }
}

/// Collects attributes within the bounds, for a host that reads them.
///
/// The host asks [`names_remaining`](Self::names_remaining) and
/// [`read_budget`](Self::read_budget) before each read, passes what it read
/// to [`add`](Self::add), reports how many names the source has with
/// [`note_names_enumerated`](Self::note_names_enumerated), and takes the
/// result from [`finish`](Self::finish). The collector reads nothing itself.
#[derive(Debug, Default)]
pub struct AttributeCollector {
    /// Attributes accepted so far.
    entries: Vec<Attribute>,
    /// Value bytes the host has read so far.
    bytes: u64,
    /// Names the source has, as the host counted.
    enumerated: u64,
}

impl AttributeCollector {
    /// An empty collector with the full budget.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many more names may be added.
    pub fn names_remaining(&self) -> usize {
        MAX_ATTRIBUTE_NAMES - self.entries.len()
    }

    /// How many more value bytes the host may read.
    pub fn read_budget(&self) -> u64 {
        MAX_INSPECTED_BYTES - self.bytes
    }

    /// Record the attribute `name` whose value is `size` bytes at the
    /// source, of which the host read `read` (at most
    /// [`read_budget`](Self::read_budget) bytes; empty when it read none).
    ///
    /// A whole value of at most 64 KiB is stored; a whole larger one as its
    /// SHA-256; a value only partly read, or not read, is marked incomplete.
    ///
    /// # Errors
    ///
    /// [`ImportError::TooManyAttributeNames`] for a 257th name and
    /// [`ImportError::AttributeReadOverBudget`] when `read` exceeds the
    /// remaining budget. Nothing is added then.
    pub fn add(&mut self, name: &[u8], size: u64, read: &[u8]) -> Result<(), ImportError> {
        if self.names_remaining() == 0 {
            return Err(ImportError::TooManyAttributeNames);
        }
        let taken = read.len() as u64;
        if taken > self.read_budget() {
            return Err(ImportError::AttributeReadOverBudget);
        }
        self.bytes += taken;
        self.enumerated = self.enumerated.max(self.entries.len() as u64 + 1);
        self.entries.push(Attribute {
            name: Raw::from_bytes(name),
            size,
            value: keep(size, read),
            decoded: BTreeMap::new(),
        });
        Ok(())
    }

    /// Attach decoded fields to the attribute added last.
    pub fn decode_last(&mut self, decoded: BTreeMap<String, Vec<String>>) {
        if let Some(last) = self.entries.last_mut() {
            last.decoded = decoded;
        }
    }

    /// Report how many attribute names the source has. A number above the
    /// names added marks the entry truncated.
    pub fn note_names_enumerated(&mut self, total: u64) {
        self.enumerated = self.enumerated.max(total);
    }

    /// The attributes, marked truncated when names were left out.
    pub fn finish(self) -> Attributes {
        let truncated = self.enumerated > self.entries.len() as u64;
        Attributes {
            entries: self.entries,
            names_enumerated: self.enumerated,
            truncated,
            bytes_inspected: self.bytes,
        }
    }
}

/// What to keep of a value of `size` bytes of which `read` was read.
fn keep(size: u64, read: &[u8]) -> AttrValue {
    if read.len() as u64 != size {
        return AttrValue::Incomplete;
    }
    if size <= MAX_STORED_VALUE_BYTES {
        return AttrValue::Full {
            value: Raw::from_bytes(read),
        };
    }
    AttrValue::Digest {
        sha256: lower_hex(&Sha256::digest(read)),
    }
}
