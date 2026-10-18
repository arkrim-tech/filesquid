//! A bounded CBOR decoder for C2PA claims and assertions (RFC 8949).
//!
//! C2PA claims, claim signatures and most assertions are CBOR, and every
//! byte comes from a file an attacker may control. [`decode`] therefore
//! accepts only well-formed CBOR (RFC 8949 Appendix C) and also rejects
//! invalid CBOR that C2PA cannot interpret safely: text that is not UTF-8
//! and maps with duplicate keys. It never trusts a declared length: a
//! string, array or map longer than the bytes that remain fails as
//! [`CborError::Truncated`] before anything is allocated. Nesting depth is
//! capped by the caller, and every decoded data item is charged to an
//! [`ItemBudget`] that can be shared across all the CBOR in a manifest store,
//! so neither recursion nor memory grows without bound.
//!
//! Definite and indefinite lengths, tags, simple values and half, single and
//! double precision floats are supported. The decoder does not require the
//! Core Deterministic Encoding that C2PA asks claim generators to produce
//! (C2PA 2.4 section 10.1); readers only need well-formed CBOR.

use std::collections::HashSet;

/// A float, stored by its IEEE 754 binary64 bit pattern so that values can
/// be compared and hashed (half and single precision widen exactly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CborFloat(u64);

impl CborFloat {
    /// Wrap `value`.
    pub fn from_f64(value: f64) -> Self {
        Self(value.to_bits())
    }

    /// The float value.
    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// One decoded CBOR data item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CborValue {
    /// Major type 0: an unsigned integer.
    Unsigned(u64),
    /// Major type 1: the negative integer `-1 - n`, holding `n`.
    Negative(u64),
    /// Major type 2: a byte string.
    Bytes(Vec<u8>),
    /// Major type 3: a UTF-8 text string.
    Text(String),
    /// Major type 4: an array.
    Array(Vec<CborValue>),
    /// Major type 5: a map, in encoded order, with unique keys.
    Map(Vec<(CborValue, CborValue)>),
    /// Major type 6: a tag number and its content.
    Tag(u64, Box<CborValue>),
    /// Simple value 20 or 21.
    Bool(bool),
    /// Simple value 22.
    Null,
    /// Simple value 23.
    Undefined,
    /// Any other simple value (0 to 19 or 32 to 255).
    Simple(u8),
    /// A half, single or double precision float.
    Float(CborFloat),
}

impl CborValue {
    /// The text, if this is a text string.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The bytes, if this is a byte string.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The items, if this is an array.
    pub fn as_array(&self) -> Option<&[CborValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The key-value pairs, if this is a map.
    pub fn as_map(&self) -> Option<&[(CborValue, CborValue)]> {
        match self {
            Self::Map(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// The integer value, if this is an integer (major type 0 or 1).
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Self::Unsigned(n) => Some(i128::from(*n)),
            Self::Negative(n) => Some(-1 - i128::from(*n)),
            _ => None,
        }
    }

    /// The value stored under the text key `key`, if this is a map that has
    /// one. Decoded maps have unique keys, so there is at most one match.
    pub fn get(&self, key: &str) -> Option<&CborValue> {
        self.as_map()?
            .iter()
            .find(|(k, _)| k.as_text() == Some(key))
            .map(|(_, v)| v)
    }
}

/// Errors from [`decode`]. Offsets are byte positions in the decoded input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CborError {
    /// The input ended inside an item, or an item declares a length longer
    /// than the bytes that remain.
    #[error("CBOR input ends early at offset {offset}")]
    Truncated {
        /// Where more bytes were needed.
        offset: usize,
    },
    /// Bytes follow the single top-level item.
    #[error("unexpected bytes after the CBOR item at offset {offset}")]
    TrailingBytes {
        /// First trailing byte.
        offset: usize,
    },
    /// The input is not well-formed CBOR.
    #[error("malformed CBOR at offset {offset}: {reason}")]
    Malformed {
        /// Start of the offending item.
        offset: usize,
        /// Which rule was broken.
        reason: &'static str,
    },
    /// A text string is not valid UTF-8.
    #[error("invalid UTF-8 in a CBOR text string at offset {offset}")]
    InvalidUtf8 {
        /// Start of the text string (or chunk).
        offset: usize,
    },
    /// A map has two equal keys (invalid CBOR, RFC 8949 section 5.6).
    #[error("duplicate key in the CBOR map at offset {offset}")]
    DuplicateKey {
        /// Start of the map.
        offset: usize,
    },
    /// Nesting exceeds the caller's depth limit.
    #[error("CBOR nesting deeper than {max} levels at offset {offset}")]
    TooDeep {
        /// Start of the item past the limit.
        offset: usize,
        /// The depth limit.
        max: usize,
    },
    /// More data items than the caller's [`ItemBudget`] allows.
    #[error("more than {max} CBOR data items at offset {offset}")]
    TooManyItems {
        /// Start of the first item over budget.
        offset: usize,
        /// The budget's limit.
        max: usize,
    },
}

impl CborError {
    /// A fixed category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Truncated { .. } => "truncated",
            Self::TrailingBytes { .. } => "trailing_bytes",
            Self::Malformed { .. } => "malformed",
            Self::InvalidUtf8 { .. } => "invalid_utf8",
            Self::DuplicateKey { .. } => "duplicate_key",
            Self::TooDeep { .. } => "too_deep",
            Self::TooManyItems { .. } => "too_many_items",
        }
    }

    /// Whether this is an engine limit (depth or item budget) rather than a
    /// fault in the CBOR itself.
    pub fn is_limit(&self) -> bool {
        matches!(self, Self::TooDeep { .. } | Self::TooManyItems { .. })
    }
}

/// A count of data items that one or more [`decode`] calls may still
/// produce. Every item, including each chunk of an indefinite-length
/// string, costs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemBudget {
    /// The limit the budget started with.
    max: usize,
    /// Items still allowed.
    remaining: usize,
}

impl ItemBudget {
    /// A budget of `max` items.
    pub fn new(max: usize) -> Self {
        Self {
            max,
            remaining: max,
        }
    }

    /// Items still allowed.
    pub fn remaining(&self) -> usize {
        self.remaining
    }

    /// Charge one item starting at `offset`.
    fn take(&mut self, offset: usize) -> Result<(), CborError> {
        if self.remaining == 0 {
            return Err(CborError::TooManyItems {
                offset,
                max: self.max,
            });
        }
        self.remaining -= 1;
        Ok(())
    }
}

/// The deepest nesting [`decode`] ever allows, whatever the caller asks
/// for: decoding recurses once per level, and this keeps the stack use well
/// inside a 1 MiB `wasm32` stack.
pub const MAX_SUPPORTED_DEPTH: usize = 256;

/// Decode exactly one CBOR data item from `bytes`.
///
/// `max_depth` caps nesting: a top-level scalar is depth 1, and each array,
/// map or tag adds one level. A `max_depth` above [`MAX_SUPPORTED_DEPTH`]
/// is lowered to it, and [`CborError::TooDeep`] then reports that value as
/// its `max`. Every decoded item is charged to `budget`. Bytes after the
/// item are [`CborError::TrailingBytes`].
pub fn decode(
    bytes: &[u8],
    max_depth: usize,
    budget: &mut ItemBudget,
) -> Result<CborValue, CborError> {
    let mut decoder = Decoder {
        bytes,
        pos: 0,
        max_depth: max_depth.min(MAX_SUPPORTED_DEPTH),
        budget,
    };
    let value = decoder.item(1)?;
    if decoder.pos != bytes.len() {
        return Err(CborError::TrailingBytes {
            offset: decoder.pos,
        });
    }
    Ok(value)
}

/// Most container slots reserved up front; larger containers grow as their
/// items are decoded.
const MAX_PREALLOCATED_ITEMS: usize = 1024;

/// Reason text for [`CborError::Malformed`].
const RESERVED_INFO: &str = "reserved additional information value";
/// Reason text for [`CborError::Malformed`].
const UNEXPECTED_BREAK: &str = "break outside an indefinite-length item";
/// Reason text for [`CborError::Malformed`].
const INDEFINITE_NOT_ALLOWED: &str = "indefinite length is not allowed for this major type";
/// Reason text for [`CborError::Malformed`].
const BAD_CHUNK: &str = "indefinite-length string chunk is not a definite string of the same type";
/// Reason text for [`CborError::Malformed`].
const BAD_SIMPLE: &str = "two-byte simple value below 32";

/// The head of a data item.
struct Head {
    /// Major type, 0 to 7.
    major: u8,
    /// Additional information, 0 to 31.
    info: u8,
    /// The argument; `None` for an indefinite length (or a break).
    arg: Option<u64>,
    /// Offset of the initial byte.
    start: usize,
}

/// Decoder state over one input.
struct Decoder<'a, 'b> {
    /// The input.
    bytes: &'a [u8],
    /// Next byte to read.
    pos: usize,
    /// Depth limit.
    max_depth: usize,
    /// Item budget.
    budget: &'b mut ItemBudget,
}

impl Decoder<'_, '_> {
    /// Bytes not yet read.
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// Decode one item at nesting level `depth`.
    fn item(&mut self, depth: usize) -> Result<CborValue, CborError> {
        let start = self.pos;
        if depth > self.max_depth {
            return Err(CborError::TooDeep {
                offset: start,
                max: self.max_depth,
            });
        }
        self.budget.take(start)?;
        let head = self.head()?;
        match head.major {
            0 => Ok(CborValue::Unsigned(definite(&head)?)),
            1 => Ok(CborValue::Negative(definite(&head)?)),
            2 => self.byte_string(&head).map(CborValue::Bytes),
            3 => self.text_string(&head).map(CborValue::Text),
            4 => self.array(&head, depth),
            5 => self.map(&head, depth),
            6 => self.tag(&head, depth),
            _ => simple(&head),
        }
    }

    /// Read one byte.
    fn byte(&mut self) -> Result<u8, CborError> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or(CborError::Truncated { offset: self.pos })?;
        self.pos += 1;
        Ok(byte)
    }

    /// Read `len` bytes, failing before any allocation when they are not
    /// all present.
    fn slice(&mut self, len: u64) -> Result<&[u8], CborError> {
        let truncated = CborError::Truncated { offset: self.pos };
        let len = usize::try_from(len).map_err(|_| truncated.clone())?;
        if len > self.remaining() {
            return Err(truncated);
        }
        let out = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }

    /// Read a big-endian unsigned integer of `width` bytes.
    fn uint(&mut self, width: u64) -> Result<u64, CborError> {
        let bytes = self.slice(width)?;
        Ok(bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)))
    }

    /// Read an item head.
    fn head(&mut self) -> Result<Head, CborError> {
        let start = self.pos;
        let initial = self.byte()?;
        let (major, info) = (initial >> 5, initial & 0x1F);
        let arg = match info {
            0..=23 => Some(u64::from(info)),
            24 => Some(self.uint(1)?),
            25 => Some(self.uint(2)?),
            26 => Some(self.uint(4)?),
            27 => Some(self.uint(8)?),
            31 => None,
            _ => return Err(malformed(start, RESERVED_INFO)),
        };
        Ok(Head {
            major,
            info,
            arg,
            start,
        })
    }

    /// Consume a break byte if one is next. The end of input is truncation.
    fn at_break(&mut self) -> Result<bool, CborError> {
        match self.bytes.get(self.pos) {
            None => Err(CborError::Truncated { offset: self.pos }),
            Some(0xFF) => {
                self.pos += 1;
                Ok(true)
            }
            Some(_) => Ok(false),
        }
    }

    /// Decode a byte string (major type 2).
    fn byte_string(&mut self, head: &Head) -> Result<Vec<u8>, CborError> {
        match head.arg {
            Some(len) => Ok(self.slice(len)?.to_vec()),
            None => self.chunks(2),
        }
    }

    /// Decode a text string (major type 3).
    fn text_string(&mut self, head: &Head) -> Result<String, CborError> {
        let bytes = match head.arg {
            Some(len) => {
                let raw = self.slice(len)?;
                utf8(raw, head.start)?;
                raw.to_vec()
            }
            None => self.chunks(3)?,
        };
        String::from_utf8(bytes).map_err(|_| CborError::InvalidUtf8 { offset: head.start })
    }

    /// Concatenate the chunks of an indefinite-length string of `major`
    /// type up to its break. Text chunks must each be valid UTF-8.
    ///
    /// Every chunk consumes at least one byte, so the loop is bounded by the
    /// remaining input; the explicit cap only makes that visible.
    fn chunks(&mut self, major: u8) -> Result<Vec<u8>, CborError> {
        let mut out = Vec::new();
        for _ in 0..=self.remaining() {
            if self.at_break()? {
                return Ok(out);
            }
            self.budget.take(self.pos)?;
            let chunk = self.head()?;
            let len = match chunk.arg {
                Some(len) if chunk.major == major => len,
                _ => return Err(malformed(chunk.start, BAD_CHUNK)),
            };
            let bytes = self.slice(len)?;
            if major == 3 {
                utf8(bytes, chunk.start)?;
            }
            out.extend_from_slice(bytes);
        }
        Err(CborError::Truncated { offset: self.pos })
    }

    /// Decode an array (major type 4).
    fn array(&mut self, head: &Head, depth: usize) -> Result<CborValue, CborError> {
        let Some(len) = head.arg else {
            return self.indefinite_array(depth);
        };
        // Every element needs at least one byte.
        if len > self.remaining() as u64 {
            return Err(CborError::Truncated { offset: self.pos });
        }
        let mut items = Vec::with_capacity(self.capacity_for(len, head.start)?);
        for _ in 0..len {
            items.push(self.item(depth + 1)?);
        }
        Ok(CborValue::Array(items))
    }

    /// Decode the elements of an indefinite-length array up to its break.
    fn indefinite_array(&mut self, depth: usize) -> Result<CborValue, CborError> {
        let mut items = Vec::new();
        for _ in 0..=self.remaining() {
            if self.at_break()? {
                return Ok(CborValue::Array(items));
            }
            items.push(self.item(depth + 1)?);
        }
        Err(CborError::Truncated { offset: self.pos })
    }

    /// Decode a map (major type 5) and check that its keys are unique.
    fn map(&mut self, head: &Head, depth: usize) -> Result<CborValue, CborError> {
        let pairs = match head.arg {
            Some(len) => self.definite_pairs(len, head.start, depth)?,
            None => self.indefinite_pairs(depth)?,
        };
        let mut seen = HashSet::with_capacity(pairs.len());
        if pairs.iter().any(|(key, _)| !seen.insert(key_repr(key))) {
            return Err(CborError::DuplicateKey { offset: head.start });
        }
        Ok(CborValue::Map(pairs))
    }

    /// Decode `len` key-value pairs of the map whose head is at `start`.
    fn definite_pairs(
        &mut self,
        len: u64,
        start: usize,
        depth: usize,
    ) -> Result<Vec<(CborValue, CborValue)>, CborError> {
        // Every pair needs at least two bytes.
        if len > (self.remaining() / 2) as u64 {
            return Err(CborError::Truncated { offset: self.pos });
        }
        let mut pairs = Vec::with_capacity(self.capacity_for(len.saturating_mul(2), start)? / 2);
        for _ in 0..len {
            let key = self.item(depth + 1)?;
            let value = self.item(depth + 1)?;
            pairs.push((key, value));
        }
        Ok(pairs)
    }

    /// Decode key-value pairs up to a break. A break where a value is
    /// expected reaches [`simple`] and is malformed.
    fn indefinite_pairs(&mut self, depth: usize) -> Result<Vec<(CborValue, CborValue)>, CborError> {
        let mut pairs = Vec::new();
        for _ in 0..=self.remaining() {
            if self.at_break()? {
                return Ok(pairs);
            }
            let key = self.item(depth + 1)?;
            let value = self.item(depth + 1)?;
            pairs.push((key, value));
        }
        Err(CborError::Truncated { offset: self.pos })
    }

    /// The capacity to reserve for a container at `start` that declares
    /// `items` child items. Fails at once when the item budget cannot cover
    /// them, and never reserves more than [`MAX_PREALLOCATED_ITEMS`], so a
    /// declared count costs no memory before its items are decoded.
    fn capacity_for(&self, items: u64, start: usize) -> Result<usize, CborError> {
        if items > self.budget.remaining as u64 {
            return Err(CborError::TooManyItems {
                offset: start,
                max: self.budget.max,
            });
        }
        Ok((items as usize).min(MAX_PREALLOCATED_ITEMS))
    }

    /// Decode a tag (major type 6) and its content.
    fn tag(&mut self, head: &Head, depth: usize) -> Result<CborValue, CborError> {
        let number = definite(head)?;
        let content = self.item(depth + 1)?;
        Ok(CborValue::Tag(number, Box::new(content)))
    }
}

/// The definite argument of `head`, or malformed when it is indefinite.
fn definite(head: &Head) -> Result<u64, CborError> {
    head.arg
        .ok_or_else(|| malformed(head.start, INDEFINITE_NOT_ALLOWED))
}

/// Decode a major type 7 item: a simple value or a float. A break here is
/// outside any indefinite-length item.
fn simple(head: &Head) -> Result<CborValue, CborError> {
    let Some(arg) = head.arg else {
        return Err(malformed(head.start, UNEXPECTED_BREAK));
    };
    match head.info {
        20 => Ok(CborValue::Bool(false)),
        21 => Ok(CborValue::Bool(true)),
        22 => Ok(CborValue::Null),
        23 => Ok(CborValue::Undefined),
        24 if arg < 32 => Err(malformed(head.start, BAD_SIMPLE)),
        25 => Ok(float(half_to_f64(arg as u16))),
        26 => Ok(float(single_to_f64(arg as u32))),
        27 => Ok(float(f64::from_bits(arg))),
        // 0 to 19 inline, or 32 to 255 in the following byte.
        _ => Ok(CborValue::Simple(arg as u8)),
    }
}

/// A float value.
fn float(value: f64) -> CborValue {
    CborValue::Float(CborFloat::from_f64(value))
}

/// Widen an IEEE 754 binary16 bit pattern to `f64` (RFC 8949 Appendix D).
fn half_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let mantissa = f64::from(bits & 0x03FF);
    match exponent {
        0 => sign * mantissa * 2f64.powi(-24),
        31 if mantissa == 0.0 => sign * f64::INFINITY,
        31 => nan(u64::from(bits >> 15), u64::from(bits & 0x03FF) << 42),
        _ => sign * (1.0 + mantissa / 1024.0) * 2f64.powi(exponent - 15),
    }
}

/// Widen an IEEE 754 binary32 bit pattern to `f64`, keeping a NaN's
/// significand (zero-extended) so NaN keys compare as RFC 8949 section
/// 5.6.1 requires.
fn single_to_f64(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return f64::from(value);
    }
    nan(u64::from(bits >> 31), u64::from(bits & 0x007F_FFFF) << 29)
}

/// The `f64` NaN with `sign` (0 or 1) and the 52-bit `significand`.
fn nan(sign: u64, significand: u64) -> f64 {
    f64::from_bits((sign << 63) | (0x7FF << 52) | significand)
}

/// A map key in the form RFC 8949 section 5.6.1 compares: `0.0` and `-0.0`
/// are one key, NaNs are equal when their significands are, and maps are
/// equal whatever the order of their pairs. Integers and floats, and byte
/// and text strings, stay distinct.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord)]
enum KeyRepr {
    /// An integer: `true` for major type 1.
    Int(bool, u64),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A text string.
    Text(String),
    /// An array, element by element.
    Array(Vec<KeyRepr>),
    /// A map, pairs sorted.
    Map(Vec<(KeyRepr, KeyRepr)>),
    /// A tag and its content.
    Tag(u64, Box<KeyRepr>),
    /// A simple value (false, true, null and undefined are 20 to 23).
    Simple(u8),
    /// A float's bits, with `-0.0` folded into `0.0`.
    Float(u64),
}

/// The comparable form of `value`. Recursion is bounded by the decode
/// depth limit.
fn key_repr(value: &CborValue) -> KeyRepr {
    match value {
        CborValue::Unsigned(n) => KeyRepr::Int(false, *n),
        CborValue::Negative(n) => KeyRepr::Int(true, *n),
        CborValue::Bytes(b) => KeyRepr::Bytes(b.clone()),
        CborValue::Text(t) => KeyRepr::Text(t.clone()),
        CborValue::Array(items) => KeyRepr::Array(items.iter().map(key_repr).collect()),
        CborValue::Map(pairs) => {
            let mut sorted: Vec<_> = pairs
                .iter()
                .map(|(k, v)| (key_repr(k), key_repr(v)))
                .collect();
            sorted.sort();
            KeyRepr::Map(sorted)
        }
        CborValue::Tag(tag, inner) => KeyRepr::Tag(*tag, Box::new(key_repr(inner))),
        CborValue::Bool(b) => KeyRepr::Simple(20 + u8::from(*b)),
        CborValue::Null => KeyRepr::Simple(22),
        CborValue::Undefined => KeyRepr::Simple(23),
        CborValue::Simple(n) => KeyRepr::Simple(*n),
        CborValue::Float(f) if f.value() == 0.0 => KeyRepr::Float(0),
        CborValue::Float(f) => KeyRepr::Float(f.value().to_bits()),
    }
}

/// Check that `bytes` is UTF-8; `offset` locates the error.
fn utf8(bytes: &[u8], offset: usize) -> Result<(), CborError> {
    std::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|_| CborError::InvalidUtf8 { offset })
}

/// A malformed-input error.
fn malformed(offset: usize, reason: &'static str) -> CborError {
    CborError::Malformed { offset, reason }
}
