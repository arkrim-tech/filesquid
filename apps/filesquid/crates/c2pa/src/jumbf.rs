//! JUMBF boxes (ISO 19566-5) as profiled by C2PA 2.4 section 11.1.
//!
//! A JUMBF superbox (`jumb`) holds one description box (`jumd`) followed by
//! content boxes and further superboxes. [`parse_superbox`] reads one
//! superbox tree from untrusted bytes and keeps the byte range of every box,
//! so later stages can hash a box's contents (C2PA 2.4 section 8.4.2.3
//! hashes a superbox without its own header: [`BoxRange::payload`]).
//!
//! Every length is checked against its container before it is used, the
//! superbox nesting depth and the total number of boxes are capped by
//! [`JumbfLimits`], and malformed input fails with a typed [`JumbfError`]
//! that carries a byte offset and never any of the input's text.

use std::fmt;

/// A JUMBF type UUID (the first 16 bytes of a description box).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeUuid(pub [u8; 16]);

impl fmt::Display for TypeUuid {
    /// Canonical 8-4-4-4-12 form in upper case, as the C2PA specification
    /// writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if matches!(i, 4 | 6 | 8 | 10) {
                f.write_str("-")?;
            }
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for TypeUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeUuid({self})")
    }
}

/// The type UUIDs C2PA 2.4 and ISO 19566-5 define.
pub mod types {
    use super::TypeUuid;

    /// The common tail `-0011-0010-8000-00AA00389B71` of ISO-style UUIDs.
    const TAIL: [u8; 12] = [
        0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
    ];

    /// The ISO-style UUID whose first four bytes are `tag`.
    const fn iso(tag: [u8; 4]) -> TypeUuid {
        let t = TAIL;
        TypeUuid([
            tag[0], tag[1], tag[2], tag[3], t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8],
            t[9], t[10], t[11],
        ])
    }

    /// C2PA Manifest Store, `c2pa` (11.1.4.2).
    pub const MANIFEST_STORE: TypeUuid = iso(*b"c2pa");
    /// Standard manifest, `c2ma` (11.2.2).
    pub const STANDARD_MANIFEST: TypeUuid = iso(*b"c2ma");
    /// Standard manifest under its older UUID, `c2md`; read, never written
    /// (11.2.2).
    pub const LEGACY_STANDARD_MANIFEST: TypeUuid = iso(*b"c2md");
    /// Update manifest, `c2um` (11.2.3).
    pub const UPDATE_MANIFEST: TypeUuid = iso(*b"c2um");
    /// Brotli-compressed manifest, `c2cm` (11.2.4).
    pub const COMPRESSED_MANIFEST: TypeUuid = iso(*b"c2cm");
    /// Time-stamp manifest, `c2tm`; historical and not read (11.2.5).
    pub const TIMESTAMP_MANIFEST: TypeUuid = iso(*b"c2tm");
    /// Assertion store, `c2as` (11.1.4.3).
    pub const ASSERTION_STORE: TypeUuid = iso(*b"c2as");
    /// Claim, `c2cl` (11.1.4.4).
    pub const CLAIM: TypeUuid = iso(*b"c2cl");
    /// Claim signature, `c2cs` (11.1.4.4).
    pub const CLAIM_SIGNATURE: TypeUuid = iso(*b"c2cs");
    /// Data box store, `c2db`; historical (11.1.4.6).
    pub const DATABOX_STORE: TypeUuid = iso(*b"c2db");
    /// CBOR content type (ISO 19566-5 B.2), content box `cbor`.
    pub const CBOR: TypeUuid = iso(*b"cbor");
    /// JSON content type (ISO 19566-5 B.1), content box `json`.
    pub const JSON: TypeUuid = iso(*b"json");
    /// UUID content type (ISO 19566-5 B.5), content box `uuid`.
    pub const UUID: TypeUuid = iso(*b"uuid");
    /// Embedded file content type (ISO 19566-5 B.4), boxes `bfdb` and `bidb`.
    pub const EMBEDDED_FILE: TypeUuid = TypeUuid([
        0x40, 0xCB, 0x0C, 0x32, 0xBB, 0x8A, 0x48, 0x9D, 0xA7, 0x0B, 0x2A, 0xD6, 0xF4, 0x7F, 0x43,
        0x69,
    ]);
}

/// A four-character box type (`TBox`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoxType(pub [u8; 4]);

impl BoxType {
    /// JUMBF superbox.
    pub const JUMB: Self = Self(*b"jumb");
    /// JUMBF description box.
    pub const JUMD: Self = Self(*b"jumd");
    /// CBOR content box.
    pub const CBOR: Self = Self(*b"cbor");
    /// JSON content box.
    pub const JSON: Self = Self(*b"json");
    /// UUID content box.
    pub const UUID: Self = Self(*b"uuid");
    /// Embedded file description box.
    pub const BFDB: Self = Self(*b"bfdb");
    /// Embedded file data box.
    pub const BIDB: Self = Self(*b"bidb");
    /// Padding box.
    pub const FREE: Self = Self(*b"free");
}

impl fmt::Display for BoxType {
    /// Printable ASCII as is; any other byte as `\xNN`, so a hostile type
    /// cannot inject control characters into logs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            if (0x20..0x7F).contains(&byte) && byte != b'\\' {
                write!(f, "{}", char::from(byte))?;
            } else {
                write!(f, "\\x{byte:02x}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for BoxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BoxType({self})")
    }
}

/// Description box toggle bits (ISO 19566-5 A.3).
pub mod toggles {
    /// The box can be referenced by a JUMBF URI.
    pub const REQUESTABLE: u8 = 0x01;
    /// A label follows the toggles.
    pub const LABEL: u8 = 0x02;
    /// A 4-byte ID follows.
    pub const ID: u8 = 0x04;
    /// A 32-byte signature follows.
    pub const SIGNATURE: u8 = 0x08;
    /// A private box follows.
    pub const PRIVATE: u8 = 0x10;
    /// Reserved bits; a description box with any of them set is rejected
    /// because its field layout is unknown.
    pub const RESERVED: u8 = 0xE0;
}

/// A half-open byte range `start..end` in the parsed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl ByteRange {
    /// Number of bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Where a box lies in the parsed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoxRange {
    /// First byte of the box header.
    pub start: usize,
    /// Header length: 8, or 16 with an extended length.
    pub header_len: usize,
    /// One past the last byte of the box.
    pub end: usize,
}

impl BoxRange {
    /// The whole box, header included.
    pub fn whole(&self) -> ByteRange {
        ByteRange {
            start: self.start,
            end: self.end,
        }
    }

    /// The box contents without its header: the bytes C2PA hashes when a
    /// URI references a superbox (C2PA 2.4 section 8.4.2.3).
    pub fn payload(&self) -> ByteRange {
        ByteRange {
            start: self.start + self.header_len,
            end: self.end,
        }
    }
}

/// A box that is not a superbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBox {
    /// Box type.
    pub box_type: BoxType,
    /// Location.
    pub range: BoxRange,
}

/// A parsed description box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionBox {
    /// Location of the `jumd` box.
    pub range: BoxRange,
    /// The superbox's type.
    pub type_uuid: TypeUuid,
    /// Toggle bits (see [`toggles`]).
    pub toggles: u8,
    /// The label, when the label toggle is set.
    pub label: Option<String>,
    /// The ID, when the ID toggle is set.
    pub id: Option<u32>,
    /// The signature, when the signature toggle is set.
    pub signature: Option<[u8; 32]>,
    /// The private box (for example a C2PA salt `c2sh`), when the private
    /// toggle is set.
    pub private_box: Option<ContentBox>,
}

/// A child of a superbox, after its description box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumbfChild {
    /// A nested superbox.
    Superbox(Superbox),
    /// A content box.
    Content(ContentBox),
}

/// A parsed superbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Superbox {
    /// Location of the `jumb` box.
    pub range: BoxRange,
    /// Its description box.
    pub description: DescriptionBox,
    /// Every box after the description box, in order.
    pub children: Vec<JumbfChild>,
}

impl Superbox {
    /// The label, if any.
    pub fn label(&self) -> Option<&str> {
        self.description.label.as_deref()
    }

    /// The type UUID.
    pub fn type_uuid(&self) -> TypeUuid {
        self.description.type_uuid
    }

    /// Child superboxes, in order.
    pub fn superboxes(&self) -> impl Iterator<Item = &Superbox> {
        self.children.iter().filter_map(|child| match child {
            JumbfChild::Superbox(sb) => Some(sb),
            JumbfChild::Content(_) => None,
        })
    }

    /// Child content boxes, in order.
    pub fn content_boxes(&self) -> impl Iterator<Item = &ContentBox> {
        self.children.iter().filter_map(|child| match child {
            JumbfChild::Content(content) => Some(content),
            JumbfChild::Superbox(_) => None,
        })
    }
}

/// Limits for [`parse_superbox`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumbfLimits {
    /// Largest superbox nesting depth; the outermost superbox is depth 1.
    pub max_depth: usize,
    /// Largest number of boxes, counting every header: superboxes,
    /// description boxes, content boxes and private boxes.
    pub max_boxes: usize,
}

/// Errors from [`parse_superbox`]. Offsets are byte positions in the input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JumbfError {
    /// A box header or body runs past the end of its container.
    #[error("JUMBF box at offset {offset} runs past the end of its container")]
    Truncated {
        /// Start of the box.
        offset: usize,
    },
    /// A box length is 2 to 7, or an extended length is below 16.
    #[error("JUMBF box at offset {offset} has an invalid length")]
    InvalidLength {
        /// Start of the box.
        offset: usize,
    },
    /// The input is not a `jumb` superbox.
    #[error("the box at offset {offset} is not a JUMBF superbox")]
    NotASuperbox {
        /// Start of the box.
        offset: usize,
    },
    /// A superbox does not start with a description box.
    #[error("JUMBF superbox content at offset {offset} does not start with a description box")]
    MissingDescription {
        /// Where the description box was expected.
        offset: usize,
    },
    /// A description box is malformed.
    #[error("JUMBF description box at offset {offset} is malformed: {reason}")]
    InvalidDescription {
        /// Start of the description box.
        offset: usize,
        /// Which rule was broken.
        reason: &'static str,
    },
    /// A label is empty, not UTF-8 or has a character C2PA forbids.
    #[error("JUMBF description box at offset {offset} has an invalid label")]
    InvalidLabel {
        /// Start of the description box.
        offset: usize,
    },
    /// Bytes follow the top-level superbox.
    #[error("unexpected bytes after the JUMBF superbox at offset {offset}")]
    TrailingBytes {
        /// First trailing byte.
        offset: usize,
    },
    /// Superboxes nest deeper than the limit.
    #[error("JUMBF superboxes nest deeper than {max} levels at offset {offset}")]
    TooDeep {
        /// Start of the superbox past the limit.
        offset: usize,
        /// The limit.
        max: usize,
    },
    /// More boxes than the limit.
    #[error("more than {max} JUMBF boxes at offset {offset}")]
    TooManyBoxes {
        /// Start of the first box over the limit.
        offset: usize,
        /// The limit.
        max: usize,
    },
}

impl JumbfError {
    /// A fixed category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Truncated { .. } => "truncated",
            Self::InvalidLength { .. } => "invalid_length",
            Self::NotASuperbox { .. } => "not_a_superbox",
            Self::MissingDescription { .. } => "missing_description",
            Self::InvalidDescription { .. } => "invalid_description",
            Self::InvalidLabel { .. } => "invalid_label",
            Self::TrailingBytes { .. } => "trailing_bytes",
            Self::TooDeep { .. } => "too_deep",
            Self::TooManyBoxes { .. } => "too_many_boxes",
        }
    }

    /// Whether this is an engine limit rather than a fault in the input.
    pub fn is_limit(&self) -> bool {
        matches!(self, Self::TooDeep { .. } | Self::TooManyBoxes { .. })
    }
}

/// The deepest superbox nesting [`parse_superbox`] ever allows, whatever
/// [`JumbfLimits::max_depth`] says: parsing recurses once per level, and
/// this keeps the stack use well inside a 1 MiB `wasm32` stack.
pub const MAX_SUPPORTED_DEPTH: usize = 256;

/// Parse `bytes` as exactly one JUMBF superbox and everything inside it.
///
/// A [`JumbfLimits::max_depth`] above [`MAX_SUPPORTED_DEPTH`] is lowered to
/// it, and [`JumbfError::TooDeep`] then reports that value as its `max`.
pub fn parse_superbox(bytes: &[u8], limits: &JumbfLimits) -> Result<Superbox, JumbfError> {
    let mut parser = Parser {
        bytes,
        max_depth: limits.max_depth.min(MAX_SUPPORTED_DEPTH),
        max_boxes: limits.max_boxes,
        boxes: 0,
    };
    let (box_type, range) = parser.header(0, bytes.len())?;
    if box_type != BoxType::JUMB {
        return Err(JumbfError::NotASuperbox { offset: 0 });
    }
    if range.end != bytes.len() {
        return Err(JumbfError::TrailingBytes { offset: range.end });
    }
    parser.superbox(range, 1)
}

/// Smallest box: an 8-byte header.
const MIN_BOX: usize = 8;

/// Description box fields before the label: UUID and toggles.
const DESCRIPTION_FIXED: usize = 17;

/// Parser state over one input.
struct Parser<'a> {
    /// The input.
    bytes: &'a [u8],
    /// Effective superbox depth limit.
    max_depth: usize,
    /// Box count limit.
    max_boxes: usize,
    /// Boxes seen so far.
    boxes: usize,
}

impl Parser<'_> {
    /// Read the box header at `at` inside a container ending at `end`, and
    /// count the box.
    fn header(&mut self, at: usize, end: usize) -> Result<(BoxType, BoxRange), JumbfError> {
        let avail = end - at;
        if avail < MIN_BOX {
            return Err(JumbfError::Truncated { offset: at });
        }
        let lbox = u32::from_be_bytes(self.array(at));
        let box_type = BoxType(self.array(at + 4));
        let (header_len, total) = match lbox {
            0 => (MIN_BOX, avail as u64),
            1 if avail < 16 => return Err(JumbfError::Truncated { offset: at }),
            1 => (16, u64::from_be_bytes(self.array(at + 8))),
            2..=7 => return Err(JumbfError::InvalidLength { offset: at }),
            n => (MIN_BOX, u64::from(n)),
        };
        if total < header_len as u64 {
            return Err(JumbfError::InvalidLength { offset: at });
        }
        if total > avail as u64 {
            return Err(JumbfError::Truncated { offset: at });
        }
        self.count(at)?;
        let range = BoxRange {
            start: at,
            header_len,
            end: at + total as usize,
        };
        Ok((box_type, range))
    }

    /// Copy `N` bytes at `at`; the caller has checked they exist.
    fn array<const N: usize>(&self, at: usize) -> [u8; N] {
        let mut out = [0u8; N];
        out.copy_from_slice(&self.bytes[at..at + N]);
        out
    }

    /// Count one box at `at` against the limit.
    fn count(&mut self, at: usize) -> Result<(), JumbfError> {
        if self.boxes >= self.max_boxes {
            return Err(JumbfError::TooManyBoxes {
                offset: at,
                max: self.max_boxes,
            });
        }
        self.boxes += 1;
        Ok(())
    }

    /// Parse the superbox at `range`, which is at nesting level `depth`.
    fn superbox(&mut self, range: BoxRange, depth: usize) -> Result<Superbox, JumbfError> {
        if depth > self.max_depth {
            return Err(JumbfError::TooDeep {
                offset: range.start,
                max: self.max_depth,
            });
        }
        let payload = range.payload();
        let missing = JumbfError::MissingDescription {
            offset: payload.start,
        };
        if payload.is_empty() {
            return Err(missing);
        }
        let (box_type, desc_range) = self.header(payload.start, payload.end)?;
        if box_type != BoxType::JUMD {
            return Err(missing);
        }
        let description = self.description(desc_range)?;
        let children = self.children(desc_range.end, payload.end, depth)?;
        Ok(Superbox {
            range,
            description,
            children,
        })
    }

    /// Parse the boxes from `at` to `end` inside a superbox at `depth`.
    ///
    /// Every box is at least [`MIN_BOX`] bytes, so the loop cap is never the
    /// reason it ends on valid input.
    fn children(
        &mut self,
        mut at: usize,
        end: usize,
        depth: usize,
    ) -> Result<Vec<JumbfChild>, JumbfError> {
        let mut children = Vec::new();
        for _ in 0..=(end - at) / MIN_BOX {
            if at == end {
                return Ok(children);
            }
            let (box_type, range) = self.header(at, end)?;
            let child = match box_type {
                BoxType::JUMB => JumbfChild::Superbox(self.superbox(range, depth + 1)?),
                BoxType::JUMD => return Err(invalid_description(at, SECOND_DESCRIPTION)),
                _ => JumbfChild::Content(ContentBox { box_type, range }),
            };
            children.push(child);
            at = range.end;
        }
        Err(JumbfError::Truncated { offset: at })
    }

    /// Parse the description box at `range`.
    fn description(&mut self, range: BoxRange) -> Result<DescriptionBox, JumbfError> {
        let payload = range.payload();
        if payload.len() < DESCRIPTION_FIXED {
            return Err(invalid_description(range.start, TOO_SHORT));
        }
        let type_uuid = TypeUuid(self.array(payload.start));
        let toggle_bits = self.bytes[payload.start + 16];
        if toggle_bits & toggles::RESERVED != 0 {
            return Err(invalid_description(range.start, RESERVED_TOGGLES));
        }
        let mut cursor = Cursor {
            at: payload.start + DESCRIPTION_FIXED,
            end: payload.end,
            offset: range.start,
        };
        let label = self.optional(toggle_bits, toggles::LABEL, |p| p.label(&mut cursor))?;
        let id = self.optional(toggle_bits, toggles::ID, |p| {
            cursor.take(4).map(|at| u32::from_be_bytes(p.array(at)))
        })?;
        let signature = self.optional(toggle_bits, toggles::SIGNATURE, |p| {
            cursor.take(32).map(|at| p.array(at))
        })?;
        let private_box = self.optional(toggle_bits, toggles::PRIVATE, |p| {
            p.private_box(&mut cursor)
        })?;
        if cursor.at != cursor.end {
            return Err(invalid_description(range.start, TRAILING_FIELDS));
        }
        Ok(DescriptionBox {
            range,
            type_uuid,
            toggles: toggle_bits,
            label,
            id,
            signature,
            private_box,
        })
    }

    /// Run `read` when `bit` is set in `bits`.
    fn optional<T>(
        &mut self,
        bits: u8,
        bit: u8,
        read: impl FnOnce(&mut Self) -> Result<T, JumbfError>,
    ) -> Result<Option<T>, JumbfError> {
        if bits & bit == 0 {
            return Ok(None);
        }
        read(self).map(Some)
    }

    /// Read a NUL-terminated label at the cursor and check it against the
    /// C2PA label rules (C2PA 2.4 section 11.1.4.1.1).
    fn label(&self, cursor: &mut Cursor) -> Result<String, JumbfError> {
        let field = &self.bytes[cursor.at..cursor.end];
        let nul = field
            .iter()
            .position(|b| *b == 0)
            .ok_or(invalid_description(cursor.offset, UNTERMINATED_LABEL))?;
        let invalid = JumbfError::InvalidLabel {
            offset: cursor.offset,
        };
        let text = std::str::from_utf8(&field[..nul]).map_err(|_| invalid.clone())?;
        if text.is_empty() || text.chars().any(forbidden_in_label) {
            return Err(invalid);
        }
        cursor.at += nul + 1;
        Ok(text.to_string())
    }

    /// Read the private box at the cursor.
    fn private_box(&mut self, cursor: &mut Cursor) -> Result<ContentBox, JumbfError> {
        let (box_type, range) = self
            .header(cursor.at, cursor.end)
            .map_err(|err| match err {
                JumbfError::Truncated { .. } | JumbfError::InvalidLength { .. } => {
                    invalid_description(cursor.offset, BAD_PRIVATE_BOX)
                }
                other => other,
            })?;
        cursor.at = range.end;
        Ok(ContentBox { box_type, range })
    }
}

/// Position inside a description box's fields.
struct Cursor {
    /// Next unread byte.
    at: usize,
    /// End of the description box.
    end: usize,
    /// Start of the description box, for errors.
    offset: usize,
}

impl Cursor {
    /// Take `len` bytes and return where they start.
    fn take(&mut self, len: usize) -> Result<usize, JumbfError> {
        if self.end - self.at < len {
            return Err(invalid_description(self.offset, SHORT_FIELD));
        }
        let start = self.at;
        self.at += len;
        Ok(start)
    }
}

/// Whether C2PA forbids `c` in a label: C0 and C1 controls, DEL, `/`, `;`,
/// `?`, `#`, U+FEFF and U+FFFF (surrogates cannot occur in UTF-8).
fn forbidden_in_label(c: char) -> bool {
    c <= '\u{1F}'
        || ('\u{7F}'..='\u{9F}').contains(&c)
        || matches!(c, '/' | ';' | '?' | '#' | '\u{FEFF}' | '\u{FFFF}')
}

/// An invalid-description error.
fn invalid_description(offset: usize, reason: &'static str) -> JumbfError {
    JumbfError::InvalidDescription { offset, reason }
}

/// Reason text for [`JumbfError::InvalidDescription`].
const TOO_SHORT: &str = "shorter than a type UUID and toggles";
/// Reason text for [`JumbfError::InvalidDescription`].
const RESERVED_TOGGLES: &str = "reserved toggle bits are set";
/// Reason text for [`JumbfError::InvalidDescription`].
const UNTERMINATED_LABEL: &str = "the label is not NUL-terminated";
/// Reason text for [`JumbfError::InvalidDescription`].
const SHORT_FIELD: &str = "an ID or signature field is cut short";
/// Reason text for [`JumbfError::InvalidDescription`].
const BAD_PRIVATE_BOX: &str = "the private box is malformed";
/// Reason text for [`JumbfError::InvalidDescription`].
const TRAILING_FIELDS: &str = "unexpected bytes after the description fields";
/// Reason text for [`JumbfError::InvalidDescription`].
const SECOND_DESCRIPTION: &str = "a superbox has exactly one description box";
