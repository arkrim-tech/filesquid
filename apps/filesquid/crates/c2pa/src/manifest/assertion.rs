//! Assertions in an assertion store (C2PA 2.4 section 11.1.4.3).
//!
//! An assertion is a superbox whose type UUID names its content type. CBOR
//! assertions are decoded, JSON assertions are checked for well-formedness
//! and kept as text, embedded files and UUID boxes are located, and any
//! other content type is kept as [`AssertionContent::Other`] (the
//! specification permits every JUMBF content type). Padding (`free`) boxes
//! are allowed everywhere.

use crate::cbor::{decode, CborError, CborValue, ItemBudget};
use crate::jumbf::{
    types, BoxRange, BoxType, ByteRange, ContentBox, JumbfChild, Superbox, TypeUuid,
};

/// One assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    /// Its label, including any `__N` instance suffix.
    pub label: String,
    /// The assertion superbox. A hashed URI to the assertion covers
    /// [`BoxRange::payload`] of this range (C2PA 2.4 section 8.4.2.3).
    pub range: BoxRange,
    /// The content type UUID.
    pub content_type: TypeUuid,
    /// The content, or why it cannot be read.
    pub content: Result<AssertionContent, AssertionError>,
}

/// The content of an assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionContent {
    /// A decoded CBOR assertion.
    Cbor(CborValue),
    /// A JSON assertion's text, checked to be well-formed JSON.
    Json(String),
    /// An embedded file (ISO 19566-5 B.4).
    EmbeddedFile {
        /// Its media type.
        media_type: String,
        /// Its file name, if given.
        file_name: Option<String>,
        /// Whether the data box holds a URL to the file instead of the file.
        external: bool,
        /// The data box content.
        data: ByteRange,
    },
    /// A UUID content box: an application UUID and opaque data.
    Uuid {
        /// The application UUID.
        uuid: TypeUuid,
        /// The data after the UUID.
        data: ByteRange,
    },
    /// Any other content type; its boxes are in the store's JUMBF tree.
    Other,
}

/// Why an assertion's content cannot be read. [`AssertionError::status_code`]
/// gives the C2PA 2.4 failure code a validator reports when the claim
/// references the assertion.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssertionError {
    /// The CBOR is not valid (`assertion.cbor.invalid`).
    #[error("the assertion is not valid CBOR: {0}")]
    Cbor(CborError),
    /// A CBOR assertion does not hold exactly one `cbor` box
    /// (`assertion.cbor.invalid`).
    #[error("a CBOR assertion holds exactly one cbor box")]
    CborBoxes,
    /// The JSON is not well-formed UTF-8 JSON (`assertion.json.invalid`).
    #[error("the assertion is not valid JSON")]
    Json,
    /// A JSON assertion does not hold exactly one `json` box
    /// (`assertion.json.invalid`).
    #[error("a JSON assertion holds exactly one json box")]
    JsonBoxes,
    /// An embedded file is malformed.
    #[error("the embedded file is malformed: {reason}")]
    EmbeddedFile {
        /// Which rule was broken.
        reason: &'static str,
    },
    /// A UUID assertion does not hold exactly one `uuid` box of at least 16
    /// bytes.
    #[error("a UUID assertion holds exactly one uuid box of at least 16 bytes")]
    UuidBox,
}

impl AssertionError {
    /// The C2PA 2.4 failure code (section 15.10), or `None` where the
    /// specification defines none.
    pub fn status_code(&self) -> Option<&'static str> {
        match self {
            Self::Cbor(_) | Self::CborBoxes => Some("assertion.cbor.invalid"),
            Self::Json | Self::JsonBoxes => Some("assertion.json.invalid"),
            Self::EmbeddedFile { .. } | Self::UuidBox => None,
        }
    }
}

/// CBOR decoding settings shared by every claim and assertion of a store.
pub(crate) struct CborContext {
    /// Depth limit per document.
    pub(crate) max_depth: usize,
    /// Item budget for the whole store.
    pub(crate) budget: ItemBudget,
}

impl CborContext {
    /// Decode `bytes`. An engine-limit error is returned as `Err`; any other
    /// decoding error is `Ok(Err(..))`, a fault in the input.
    pub(crate) fn decode(
        &mut self,
        bytes: &[u8],
    ) -> Result<Result<CborValue, CborError>, CborError> {
        match decode(bytes, self.max_depth, &mut self.budget) {
            Err(err) if err.is_limit() => Err(err),
            other => Ok(other),
        }
    }
}

/// Parse the assertion superbox `sb` labelled `label`. Only an engine CBOR
/// limit is an `Err`.
pub(crate) fn parse_assertion(
    bytes: &[u8],
    sb: &Superbox,
    label: &str,
    cbor: &mut CborContext,
) -> Result<Assertion, CborError> {
    let content_type = sb.type_uuid();
    let content = match content_type {
        types::CBOR => cbor_content(bytes, sb, cbor)?,
        types::JSON => json_content(bytes, sb),
        types::EMBEDDED_FILE => embedded_content(bytes, sb),
        types::UUID => uuid_content(bytes, sb),
        _ => Ok(AssertionContent::Other),
    };
    Ok(Assertion {
        label: label.to_string(),
        range: sb.range,
        content_type,
        content,
    })
}

/// The content boxes of `sb` other than padding, or `None` when it also
/// holds a superbox.
fn data_boxes(sb: &Superbox) -> Option<Vec<&ContentBox>> {
    sb.children
        .iter()
        .filter_map(|child| match child {
            JumbfChild::Superbox(_) => Some(None),
            JumbfChild::Content(c) if c.box_type == BoxType::FREE => None,
            JumbfChild::Content(c) => Some(Some(c)),
        })
        .collect()
}

/// The payload of the single content box of `box_type` in `sb`.
fn single_box(sb: &Superbox, box_type: BoxType) -> Option<ByteRange> {
    match data_boxes(sb)?.as_slice() {
        [only] if only.box_type == box_type => Some(only.range.payload()),
        _ => None,
    }
}

/// Decode a CBOR assertion.
fn cbor_content(
    bytes: &[u8],
    sb: &Superbox,
    cbor: &mut CborContext,
) -> Result<Result<AssertionContent, AssertionError>, CborError> {
    let Some(range) = single_box(sb, BoxType::CBOR) else {
        return Ok(Err(AssertionError::CborBoxes));
    };
    let decoded = cbor.decode(&bytes[range.start..range.end])?;
    Ok(decoded
        .map(AssertionContent::Cbor)
        .map_err(AssertionError::Cbor))
}

/// Check and keep a JSON assertion. Deserializing into `IgnoredAny` builds
/// no value tree and `serde_json` skips nested values iteratively, so
/// neither memory nor stack grows beyond what the box size allows; no
/// separate depth limit is needed.
fn json_content(bytes: &[u8], sb: &Superbox) -> Result<AssertionContent, AssertionError> {
    let range = single_box(sb, BoxType::JSON).ok_or(AssertionError::JsonBoxes)?;
    let raw = &bytes[range.start..range.end];
    let text = std::str::from_utf8(raw).map_err(|_| AssertionError::Json)?;
    serde_json::from_str::<serde::de::IgnoredAny>(text).map_err(|_| AssertionError::Json)?;
    Ok(AssertionContent::Json(text.to_string()))
}

/// Embedded file description toggle: a file name follows the media type.
const FILE_NAME_PRESENT: u8 = 0x01;
/// Embedded file description toggle: the data box holds a URL.
const EXTERNAL_DATA: u8 = 0x02;

/// Reason text for [`AssertionError::EmbeddedFile`].
const BAD_LAYOUT: &str = "an embedded file holds a bfdb box followed by a bidb box";
/// Reason text for [`AssertionError::EmbeddedFile`].
const RESERVED_BITS: &str = "reserved toggle bits are set";
/// Reason text for [`AssertionError::EmbeddedFile`].
const BAD_STRING: &str = "a media type or file name is not NUL-terminated UTF-8";
/// Reason text for [`AssertionError::EmbeddedFile`].
const TRAILING: &str = "unexpected bytes after the description fields";

/// Read an embedded file assertion: `bfdb` then `bidb`.
fn embedded_content(bytes: &[u8], sb: &Superbox) -> Result<AssertionContent, AssertionError> {
    let bad = |reason| AssertionError::EmbeddedFile { reason };
    let boxes = data_boxes(sb).ok_or(bad(BAD_LAYOUT))?;
    let [description, data] = boxes.as_slice() else {
        return Err(bad(BAD_LAYOUT));
    };
    if description.box_type != BoxType::BFDB || data.box_type != BoxType::BIDB {
        return Err(bad(BAD_LAYOUT));
    }
    let payload = description.range.payload();
    let fields = &bytes[payload.start..payload.end];
    let (&toggles, mut rest) = fields.split_first().ok_or(bad(BAD_STRING))?;
    if toggles & !(FILE_NAME_PRESENT | EXTERNAL_DATA) != 0 {
        return Err(bad(RESERVED_BITS));
    }
    let media_type = nul_terminated(&mut rest).ok_or(bad(BAD_STRING))?;
    let file_name = match toggles & FILE_NAME_PRESENT {
        0 => None,
        _ => Some(nul_terminated(&mut rest).ok_or(bad(BAD_STRING))?),
    };
    if !rest.is_empty() {
        return Err(bad(TRAILING));
    }
    Ok(AssertionContent::EmbeddedFile {
        media_type,
        file_name,
        external: toggles & EXTERNAL_DATA != 0,
        data: data.range.payload(),
    })
}

/// Take a NUL-terminated UTF-8 string from the front of `rest`.
fn nul_terminated(rest: &mut &[u8]) -> Option<String> {
    let nul = rest.iter().position(|b| *b == 0)?;
    let text = std::str::from_utf8(&rest[..nul]).ok()?.to_string();
    *rest = &rest[nul + 1..];
    Some(text)
}

/// Read a UUID assertion: one `uuid` box of a 16-byte UUID and data.
fn uuid_content(bytes: &[u8], sb: &Superbox) -> Result<AssertionContent, AssertionError> {
    let range = single_box(sb, BoxType::UUID).ok_or(AssertionError::UuidBox)?;
    if range.len() < 16 {
        return Err(AssertionError::UuidBox);
    }
    let uuid: [u8; 16] = bytes[range.start..range.start + 16]
        .try_into()
        .map_err(|_| AssertionError::UuidBox)?;
    Ok(AssertionContent::Uuid {
        uuid: TypeUuid(uuid),
        data: ByteRange {
            start: range.start + 16,
            end: range.end,
        },
    })
}
