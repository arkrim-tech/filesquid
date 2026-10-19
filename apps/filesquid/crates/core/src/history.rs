//! FileSquid-facing PROV-O history contract (CORE-S-010;
//! [ADR 040](../../../../../resources/architecture/040-prov-o-history.md)
//! Decision 10).
//!
//! These types are what hosts, the API and UIs exchange about the history of
//! a library: the kind of a committed operation ([`OperationKind`]), the
//! structured OCFL version message every commit carries
//! ([`OperationMessage`], `filesquid.operation.v1`), the OCFL object version
//! a record describes ([`VersionRef`]), the installation that acted
//! ([`InstallationId`]), the journal entries of a commit ([`JournalLink`])
//! and one activity as the API and UI show it ([`HistoryEntry`]). The PROV-O
//! graph itself belongs to the `filesquid-prov` module; this crate holds no
//! graph types and does no I/O.

use std::fmt;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

use crate::{OperationId, Rfc3339Utc};

/// Prefix of the structured OCFL version message.
pub const OPERATION_MESSAGE_PREFIX: &str = "filesquid.operation.v1 ";

/// Largest operation message [`OperationMessage::parse`] reads, in bytes.
pub const MAX_OPERATION_MESSAGE_BYTES: usize = 4096;

/// Most members a `filesquid.operation.v1` object may hold.
const MAX_MESSAGE_MEMBERS: usize = 8;

/// Longest decimal revision accepted (fits `u64`).
const MAX_REVISION_DIGITS: usize = 19;

/// The kind of a committed operation: one activity of the PROV-O history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OperationKind {
    /// A file imported from outside the library.
    Import,
    /// A file uploaded through the API.
    Upload,
    /// A copy of a file (a new file ID).
    Copy,
    /// A display-name change.
    Rename,
    /// A move to another logical path or collection.
    Move,
    /// A file hidden from views.
    Hide,
    /// A hidden file shown again.
    Unhide,
    /// A file moved to the trash.
    Delete,
    /// A trashed file restored.
    Restore,
    /// A permanent delete.
    Purge,
    /// A metadata record edit.
    MetadataEdit,
    /// A version label set or changed.
    Label,
    /// A whole object returned to an earlier version.
    Revert,
    /// Chosen files returned to an earlier version.
    Reinstate,
    /// A library snapshot commit.
    Snapshot,
    /// Catalog organisation: collections, virtual collections, memberships.
    CatalogOrganisation,
    /// A consented OCFL repair.
    Repair,
    /// An export or download of library content.
    Export,
    /// A scrubbed (privacy-filtered) output.
    Scrub,
    /// A reconstruction of missing history records.
    Backfill,
    /// An operation whose kind was not recorded.
    Unknown,
}

impl OperationKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 21] = [
        Self::Import,
        Self::Upload,
        Self::Copy,
        Self::Rename,
        Self::Move,
        Self::Hide,
        Self::Unhide,
        Self::Delete,
        Self::Restore,
        Self::Purge,
        Self::MetadataEdit,
        Self::Label,
        Self::Revert,
        Self::Reinstate,
        Self::Snapshot,
        Self::CatalogOrganisation,
        Self::Repair,
        Self::Export,
        Self::Scrub,
        Self::Backfill,
        Self::Unknown,
    ];

    /// The kind's stable token, as records, messages and the API spell it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Upload => "upload",
            Self::Copy => "copy",
            Self::Rename => "rename",
            Self::Move => "move",
            Self::Hide => "hide",
            Self::Unhide => "unhide",
            Self::Delete => "delete",
            Self::Restore => "restore",
            Self::Purge => "purge",
            Self::MetadataEdit => "metadata.edit",
            Self::Label => "label",
            Self::Revert => "revert",
            Self::Reinstate => "reinstate",
            Self::Snapshot => "snapshot",
            Self::CatalogOrganisation => "catalog",
            Self::Repair => "repair",
            Self::Export => "export",
            Self::Scrub => "scrub",
            Self::Backfill => "backfill",
            Self::Unknown => "unknown",
        }
    }

    /// The kind whose token is `token`, if any.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == token)
    }
}

impl fmt::Display for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for OperationKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for OperationKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let token = String::deserialize(deserializer)?;
        Self::parse(&token).ok_or_else(|| de::Error::custom("unknown operation kind"))
    }
}

/// Why an operation message could not be read. Messages name no stored
/// value, so a refusal never leaks an identifier or a label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationMessageError {
    /// The message is longer than [`MAX_OPERATION_MESSAGE_BYTES`].
    TooLarge,
    /// The message has the `filesquid.operation.v1` prefix but its object
    /// cannot be read; `code` names the rule it broke.
    Unreadable {
        /// A fixed code, such as `missing-kind` or `duplicate-member`.
        code: &'static str,
    },
}

impl fmt::Display for OperationMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(
                f,
                "the filesquid.operation.v1 message exceeds {MAX_OPERATION_MESSAGE_BYTES} bytes"
            ),
            Self::Unreadable { code } => {
                write!(
                    f,
                    "the filesquid.operation.v1 message is unreadable: {code}"
                )
            }
        }
    }
}

impl std::error::Error for OperationMessageError {}

/// The structured OCFL version message of a FileSquid commit:
/// `filesquid.operation.v1 {"operation_id", "kind", "revision"?, "label"?}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationMessage {
    operation_id: OperationId,
    kind: OperationKind,
    revision: Option<u64>,
    label: Option<String>,
}

impl OperationMessage {
    /// A message for `operation_id` of `kind`, with no revision or label.
    pub fn new(operation_id: OperationId, kind: OperationKind) -> Self {
        Self {
            operation_id,
            kind,
            revision: None,
            label: None,
        }
    }

    /// The same message with the catalog `revision` the commit produced.
    #[must_use]
    pub fn with_revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    /// The same message with a version `label` (PRD F-065).
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The operation ID, shared by every object version one operation commits.
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// The operation kind.
    pub fn kind(&self) -> OperationKind {
        self.kind
    }

    /// The catalog revision, when given.
    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// The version label, when given.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The message text: the prefix and one compact JSON object with members
    /// in the order `operation_id`, `kind`, `revision`, `label`.
    pub fn format(&self) -> String {
        let mut out = String::from(OPERATION_MESSAGE_PREFIX);
        out.push_str("{\"operation_id\":");
        push_json_string(&mut out, self.operation_id.as_str());
        out.push_str(",\"kind\":");
        push_json_string(&mut out, self.kind.as_str());
        if let Some(revision) = self.revision {
            out.push_str(&format!(",\"revision\":{revision}"));
        }
        if let Some(label) = &self.label {
            out.push_str(",\"label\":");
            push_json_string(&mut out, label);
        }
        out.push('}');
        out
    }

    /// Read a version message. `Ok(None)` when it is not a FileSquid message
    /// (no `filesquid.operation.v1 ` prefix), such as one another tool wrote.
    ///
    /// # Errors
    ///
    /// [`OperationMessageError::TooLarge`] above
    /// [`MAX_OPERATION_MESSAGE_BYTES`]; [`OperationMessageError::Unreadable`]
    /// for a prefixed message whose object breaks the format, which is
    /// refused rather than read as unknown.
    pub fn parse(text: &str) -> Result<Option<Self>, OperationMessageError> {
        if text.len() > MAX_OPERATION_MESSAGE_BYTES {
            return Err(OperationMessageError::TooLarge);
        }
        let Some(body) = text.strip_prefix(OPERATION_MESSAGE_PREFIX) else {
            return Ok(None);
        };
        let members = flat_json::parse_object(body, MAX_MESSAGE_MEMBERS).map_err(unreadable)?;
        Self::from_members(members).map(Some).map_err(unreadable)
    }

    /// Build a message from the members of its JSON object.
    fn from_members(members: Vec<(String, flat_json::Value)>) -> Result<Self, &'static str> {
        let mut fields = MessageFields::default();
        for (name, value) in members {
            fields.set(&name, value)?;
        }
        let operation_id = fields.operation_id.ok_or("missing-operation-id")?;
        let operation_id = OperationId::new(operation_id).map_err(|_| "empty-operation-id")?;
        let kind = fields.kind.ok_or("missing-kind")?;
        let kind = OperationKind::parse(&kind).ok_or("unknown-kind")?;
        Ok(Self {
            operation_id,
            kind,
            revision: fields.revision,
            label: fields.label,
        })
    }
}

/// The members of a message object as they are read.
#[derive(Default)]
struct MessageFields {
    operation_id: Option<String>,
    kind: Option<String>,
    revision: Option<u64>,
    label: Option<String>,
}

impl MessageFields {
    /// Record member `name`, refusing an unknown or mistyped one.
    fn set(&mut self, name: &str, value: flat_json::Value) -> Result<(), &'static str> {
        use flat_json::Value::{Str, Uint};
        match (name, value) {
            ("operation_id", Str(text)) => self.operation_id = Some(text),
            ("kind", Str(text)) => self.kind = Some(text),
            ("revision", Uint(number)) => self.revision = Some(number),
            ("label", Str(text)) => self.label = Some(text),
            ("operation_id" | "kind" | "revision" | "label", _) => return Err("wrong-type"),
            _ => return Err("unknown-member"),
        }
        Ok(())
    }
}

/// An unreadable-message error carrying `code`.
fn unreadable(code: &'static str) -> OperationMessageError {
    OperationMessageError::Unreadable { code }
}

/// Append `text` as a JSON string literal.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A bounded reader of one flat JSON object whose members are strings or
/// unsigned integers: all an operation message needs, without a JSON
/// dependency in this crate.
mod flat_json {
    /// A member value.
    pub(super) enum Value {
        /// A string.
        Str(String),
        /// An unsigned integer.
        Uint(u64),
    }

    /// The bytes being read and the read position.
    struct Cursor<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    /// Read `text` as one object of at most `max_members` members with
    /// distinct names and nothing after it but whitespace.
    pub(super) fn parse_object(
        text: &str,
        max_members: usize,
    ) -> Result<Vec<(String, Value)>, &'static str> {
        let mut cursor = Cursor {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let members = cursor.object(max_members)?;
        cursor.skip_ws();
        if cursor.pos != cursor.bytes.len() {
            return Err("trailing-data");
        }
        Ok(members)
    }

    impl Cursor<'_> {
        /// Read `{ "name": value, ... }`.
        fn object(&mut self, max_members: usize) -> Result<Vec<(String, Value)>, &'static str> {
            self.skip_ws();
            self.expect(b'{')?;
            let mut members: Vec<(String, Value)> = Vec::new();
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(members);
            }
            for _ in 0..max_members {
                let (name, value) = self.member()?;
                if members.iter().any(|(seen, _)| *seen == name) {
                    return Err("duplicate-member");
                }
                members.push((name, value));
                self.skip_ws();
                match self.next() {
                    Some(b',') => continue,
                    Some(b'}') => return Ok(members),
                    _ => return Err("expected-comma-or-brace"),
                }
            }
            Err("too-many-members")
        }

        /// Read `"name": value`.
        fn member(&mut self) -> Result<(String, Value), &'static str> {
            self.skip_ws();
            let name = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = match self.peek() {
                Some(b'"') => Value::Str(self.string()?),
                Some(b'0'..=b'9') => Value::Uint(self.uint()?),
                _ => return Err("unsupported-value"),
            };
            Ok((name, value))
        }

        /// Read a string literal, decoding its escapes.
        fn string(&mut self) -> Result<String, &'static str> {
            self.expect(b'"')?;
            let mut out = String::new();
            for _ in 0..=self.bytes.len() {
                match self.next().ok_or("unterminated-string")? {
                    b'"' => return Ok(out),
                    b'\\' => out.push(self.escape()?),
                    byte if byte < 0x20 => return Err("control-character"),
                    byte => self.push_utf8(byte, &mut out)?,
                }
            }
            Err("unterminated-string")
        }

        /// Append the UTF-8 character that starts with `first`.
        fn push_utf8(&mut self, first: u8, out: &mut String) -> Result<(), &'static str> {
            let width = match first {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };
            let start = self.pos - 1;
            let end = start + width;
            let slice = self.bytes.get(start..end).ok_or("invalid-utf8")?;
            let text = std::str::from_utf8(slice).map_err(|_| "invalid-utf8")?;
            out.push_str(text);
            self.pos = end;
            Ok(())
        }

        /// Decode the escape after a backslash.
        fn escape(&mut self) -> Result<char, &'static str> {
            let c = match self.next().ok_or("unterminated-string")? {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{8}',
                b'f' => '\u{c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => return self.unicode_escape(),
                _ => return Err("invalid-escape"),
            };
            Ok(c)
        }

        /// Decode `\uXXXX`, joining a surrogate pair.
        fn unicode_escape(&mut self) -> Result<char, &'static str> {
            let high = self.hex4()?;
            if !(0xd800..0xdc00).contains(&high) {
                return char::from_u32(high).ok_or("invalid-escape");
            }
            if self.next() != Some(b'\\') || self.next() != Some(b'u') {
                return Err("invalid-escape");
            }
            let low = self.hex4()?;
            if !(0xdc00..0xe000).contains(&low) {
                return Err("invalid-escape");
            }
            char::from_u32(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00))
                .ok_or("invalid-escape")
        }

        /// Read four hexadecimal digits.
        fn hex4(&mut self) -> Result<u32, &'static str> {
            let slice = self
                .bytes
                .get(self.pos..self.pos + 4)
                .ok_or("invalid-escape")?;
            let text = std::str::from_utf8(slice).map_err(|_| "invalid-escape")?;
            let value = u32::from_str_radix(text, 16).map_err(|_| "invalid-escape")?;
            self.pos += 4;
            Ok(value)
        }

        /// Read an unsigned decimal integer without a leading zero.
        fn uint(&mut self) -> Result<u64, &'static str> {
            let start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) && self.pos - start <= MAX_DIGITS {
                self.pos += 1;
            }
            let digits = &self.bytes[start..self.pos];
            if digits.len() > MAX_DIGITS || (digits.len() > 1 && digits[0] == b'0') {
                return Err("invalid-number");
            }
            let text = std::str::from_utf8(digits).map_err(|_| "invalid-number")?;
            text.parse().map_err(|_| "invalid-number")
        }

        /// Skip JSON whitespace.
        fn skip_ws(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.pos += 1;
            }
        }

        /// Consume `byte` or fail.
        fn expect(&mut self, byte: u8) -> Result<(), &'static str> {
            match self.next() {
                Some(found) if found == byte => Ok(()),
                _ => Err("unexpected-character"),
            }
        }

        /// The byte at the read position.
        fn peek(&self) -> Option<u8> {
            self.bytes.get(self.pos).copied()
        }

        /// Consume and return the byte at the read position.
        fn next(&mut self) -> Option<u8> {
            let byte = self.peek()?;
            self.pos += 1;
            Some(byte)
        }
    }

    /// Longest decimal integer read.
    const MAX_DIGITS: usize = super::MAX_REVISION_DIGITS;
}

/// The UUID v4 that names one FileSquid installation: the owner agent of its
/// PROV-O records and the address of its OCFL version `user`. Stored in
/// lower case; parsed, never generated, by this crate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InstallationId(String);

/// Why a value is not an installation ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidInstallationId;

impl fmt::Display for InvalidInstallationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an installation ID must be a UUID version 4")
    }
}

impl std::error::Error for InvalidInstallationId {}

impl InstallationId {
    /// Parse a canonical UUID v4 (`8-4-4-4-12` hexadecimal digits, version
    /// nibble 4, RFC 4122 variant), optionally prefixed `urn:uuid:`.
    ///
    /// # Errors
    ///
    /// [`InvalidInstallationId`] for anything else.
    pub fn parse(text: &str) -> Result<Self, InvalidInstallationId> {
        let uuid = text.strip_prefix("urn:uuid:").unwrap_or(text);
        if !is_uuid_v4(uuid) {
            return Err(InvalidInstallationId);
        }
        Ok(Self(uuid.to_ascii_lowercase()))
    }

    /// The UUID in lower case.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The agent identifier `urn:uuid:<uuid>`.
    pub fn urn(&self) -> String {
        format!("urn:uuid:{}", self.0)
    }
}

/// Whether `text` is a canonical UUID v4.
fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    let dashes = [8, 13, 18, 23];
    let shape = bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| {
            if dashes.contains(&i) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        });
    shape
        && bytes[14] == b'4'
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

impl Serialize for InstallationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for InstallationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

/// Why a version reference or journal link is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidHistoryRef {
    /// The object ID is empty or holds `@` or whitespace.
    ObjectId,
    /// The version is not an OCFL version name (`v1`, `v2`, ... or
    /// zero-padded) above zero.
    Version,
    /// A journal entry ID is empty or the link names too many.
    JournalEntries,
}

impl fmt::Display for InvalidHistoryRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectId => f.write_str("the object ID is empty or holds '@' or whitespace"),
            Self::Version => f.write_str("the version is not an OCFL version name"),
            Self::JournalEntries => f.write_str("the journal entry IDs are empty or too many"),
        }
    }
}

impl std::error::Error for InvalidHistoryRef {}

/// One OCFL object version: what a PROV-O record describes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawVersionRef")]
pub struct VersionRef {
    object_id: String,
    version: String,
}

/// The unchecked serialized form of a [`VersionRef`].
#[derive(Deserialize)]
struct RawVersionRef {
    object_id: String,
    version: String,
}

impl TryFrom<RawVersionRef> for VersionRef {
    type Error = InvalidHistoryRef;

    fn try_from(raw: RawVersionRef) -> Result<Self, Self::Error> {
        Self::new(raw.object_id, raw.version)
    }
}

impl VersionRef {
    /// A reference to `version` of the object `object_id`.
    ///
    /// # Errors
    ///
    /// [`InvalidHistoryRef::ObjectId`] or [`InvalidHistoryRef::Version`].
    pub fn new(
        object_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, InvalidHistoryRef> {
        let (object_id, version) = (object_id.into(), version.into());
        let bad_id = object_id.is_empty()
            || object_id.contains('@')
            || object_id.chars().any(char::is_whitespace);
        if bad_id {
            return Err(InvalidHistoryRef::ObjectId);
        }
        if !is_version_name(&version) {
            return Err(InvalidHistoryRef::Version);
        }
        Ok(Self { object_id, version })
    }

    /// Parse an object reference `<object-id>@<version>`.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new); [`InvalidHistoryRef::Version`] without `@`.
    pub fn parse_object_ref(text: &str) -> Result<Self, InvalidHistoryRef> {
        let (object_id, version) = text.rsplit_once('@').ok_or(InvalidHistoryRef::Version)?;
        Self::new(object_id, version)
    }

    /// The OCFL object ID.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The OCFL version name.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The object reference `<object-id>@<version>`, which is
    /// `urn:uuid:<uuid>@vN` for FileSquid's UUID object IDs.
    pub fn object_ref(&self) -> String {
        format!("{}@{}", self.object_id, self.version)
    }
}

/// Whether `text` is an OCFL version name above zero.
fn is_version_name(text: &str) -> bool {
    let Some(digits) = text.strip_prefix('v') else {
        return false;
    };
    !digits.is_empty()
        && digits.len() <= 18
        && digits.bytes().all(|b| b.is_ascii_digit())
        && digits.bytes().any(|b| b != b'0')
}

/// The journal entries of one committed operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawJournalLink")]
pub struct JournalLink {
    operation_id: OperationId,
    journal_entry_ids: Vec<String>,
}

/// The unchecked serialized form of a [`JournalLink`].
#[derive(Deserialize)]
struct RawJournalLink {
    operation_id: OperationId,
    journal_entry_ids: Vec<String>,
}

impl TryFrom<RawJournalLink> for JournalLink {
    type Error = InvalidHistoryRef;

    fn try_from(raw: RawJournalLink) -> Result<Self, Self::Error> {
        Self::new(raw.operation_id, raw.journal_entry_ids)
    }
}

impl JournalLink {
    /// Most journal entry IDs one link names.
    pub const MAX_ENTRIES: usize = 256;

    /// A link from `operation_id` to its opaque journal entry IDs.
    ///
    /// # Errors
    ///
    /// [`InvalidHistoryRef::JournalEntries`] for an empty ID or more than
    /// [`MAX_ENTRIES`](Self::MAX_ENTRIES).
    pub fn new(
        operation_id: OperationId,
        journal_entry_ids: Vec<String>,
    ) -> Result<Self, InvalidHistoryRef> {
        let invalid = journal_entry_ids.len() > Self::MAX_ENTRIES
            || journal_entry_ids.iter().any(String::is_empty);
        if invalid {
            return Err(InvalidHistoryRef::JournalEntries);
        }
        Ok(Self {
            operation_id,
            journal_entry_ids,
        })
    }

    /// The operation ID.
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// The opaque journal entry IDs, in commit order.
    pub fn journal_entry_ids(&self) -> &[String] {
        &self.journal_entry_ids
    }
}

/// One activity of a file's history as the API and UI show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The operation kind.
    pub kind: OperationKind,
    /// The operation ID.
    pub operation_id: OperationId,
    /// The object version the activity committed.
    pub version: VersionRef,
    /// The agents, by opaque identifier (`urn:uuid:<installation>` and the
    /// host software agent); never a person identity.
    pub agents: Vec<String>,
    /// When the operation started, when recorded.
    pub started_at: Option<Rfc3339Utc>,
    /// When the version was committed (the inventory `created` time).
    pub ended_at: Rfc3339Utc,
    /// The journal entries of the commit.
    pub journal: JournalLink,
    /// The version label, when set.
    pub label: Option<String>,
    /// Whether the record was reconstructed from the inventory afterwards.
    pub reconstructed: bool,
}
