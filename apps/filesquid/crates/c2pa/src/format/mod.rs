//! Format handlers: where an asset's manifest store and boxes are
//! (C2PA-S-005, C2PA 2.4 Appendix A).
//!
//! A [`FormatHandler`] recognises its format from the first bytes and lays
//! the asset out as an [`AssetLayout`]: the asset's boxes in order, named
//! as C2PA 2.4 section 18.7 names them for box hashing, and every embedded
//! manifest store's location. JPEG (APP11, A.3.1) and PNG (`caBX`, A.3.2)
//! are supported. [`asset_layout`] returns `None` for any other format, so
//! callers report it as unsupported rather than skipping it. Handlers read
//! only through a [`ByteSource`], move forward through the asset, and cap
//! the number of boxes.

mod jpeg;
mod png;
pub(crate) mod reader;

pub use jpeg::JpegHandler;
pub use png::PngHandler;

use std::future::Future;
use std::ops::Range;

use crate::io::{read_exact_at, try_zeroed, ByteIoError, ByteSource};

/// A format with a handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetFormat {
    /// JPEG (ISO 10918-1); the store is in APP11 segments.
    Jpeg,
    /// PNG; the store is a `caBX` chunk.
    Png,
}

/// One box of an asset, named as a box-hash assertion lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBox {
    /// The name: a JPEG marker symbol (ISO 10918-1 Table B.1, for example
    /// `APP0`, `SOS`, `RST3`), a PNG chunk type, `PNGh` for the PNG
    /// signature, or `C2PA` for the embedded manifest store.
    pub name: String,
    /// Where the box is in the asset, header included. A JPEG `SOS` or
    /// `RSTn` box includes the entropy-coded data after it.
    pub range: Range<u64>,
}

/// Where an embedded manifest store lies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedStore {
    /// The bytes that carry the store, format extras included: every APP11
    /// segment of the store (markers and lengths), or the whole `caBX`
    /// chunk (length, type, data and CRC). A data-hash exclusion for the
    /// store must cover exactly this range.
    pub container: Range<u64>,
    /// The ranges holding the store's own bytes, in order.
    pub parts: Vec<Range<u64>>,
    /// The store's length: the sum of `parts`.
    pub len: u64,
}

/// An asset's boxes and embedded stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLayout {
    /// The format.
    pub format: AssetFormat,
    /// The asset's size in bytes.
    pub size: u64,
    /// Every box, in order.
    pub boxes: Vec<AssetBox>,
    /// Every embedded C2PA manifest store, in order. More than one means
    /// none of them is valid (C2PA 2.4 section 15.5.2.1).
    pub stores: Vec<EmbeddedStore>,
}

/// Why an asset could not be laid out. Offsets are byte positions in the
/// asset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// The byte source failed.
    #[error("byte source error: {0}")]
    Io(ByteIoError),
    /// The asset ends inside a box, or before its end marker.
    #[error("the asset ends early at offset {offset}")]
    Truncated {
        /// Where more bytes were needed.
        offset: u64,
    },
    /// The asset's structure is invalid.
    #[error("malformed asset at offset {offset}: {reason}")]
    Malformed {
        /// Where the problem is.
        offset: u64,
        /// Which rule was broken.
        reason: &'static str,
    },
    /// More boxes than the limit.
    #[error("more than {max} boxes at offset {offset}")]
    TooManyBoxes {
        /// Where the limit was reached.
        offset: u64,
        /// The limit.
        max: usize,
    },
    /// The embedded store is larger than the store size limit.
    #[error("the embedded manifest store is {size} bytes, over the {max}-byte limit")]
    StoreTooLarge {
        /// The store's size.
        size: u64,
        /// The limit.
        max: u64,
    },
    /// The memory for the embedded store could not be reserved.
    #[error("could not reserve {size} bytes for the embedded manifest store")]
    Allocation {
        /// The store's size.
        size: u64,
    },
}

impl FormatError {
    /// A fixed category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Truncated { .. } => "truncated",
            Self::Malformed { .. } => "malformed_asset",
            Self::TooManyBoxes { .. } => "too_many_boxes",
            Self::StoreTooLarge { .. } => "store_too_large",
            Self::Allocation { .. } => "allocation",
        }
    }
}

impl From<ByteIoError> for FormatError {
    fn from(err: ByteIoError) -> Self {
        Self::Io(err)
    }
}

/// How many leading bytes [`FormatHandler::recognizes`] sees.
pub const HEAD_BYTES: usize = 8;

/// A handler for one asset format.
pub trait FormatHandler {
    /// The format handled.
    fn format(&self) -> AssetFormat;

    /// Whether `head` (up to [`HEAD_BYTES`] leading bytes) starts an asset
    /// of this format.
    fn recognizes(&self, head: &[u8]) -> bool;

    /// Lay out the asset in `source`, with at most `max_boxes` boxes.
    fn layout<S: ByteSource + ?Sized>(
        &self,
        source: &mut S,
        max_boxes: usize,
    ) -> impl Future<Output = Result<AssetLayout, FormatError>>;
}

/// The leading bytes of `source`, at most [`HEAD_BYTES`].
async fn head<S: ByteSource + ?Sized>(source: &mut S) -> Result<Vec<u8>, ByteIoError> {
    let size = source.size().await?;
    let len = size.min(HEAD_BYTES as u64) as usize;
    let mut buf = vec![0u8; len];
    read_exact_at(source, 0, &mut buf).await?;
    Ok(buf)
}

/// The format of `source`, or `None` when no handler recognises it.
pub async fn detect_format<S: ByteSource + ?Sized>(
    source: &mut S,
) -> Result<Option<AssetFormat>, ByteIoError> {
    let head = head(source).await?;
    if JpegHandler.recognizes(&head) {
        return Ok(Some(AssetFormat::Jpeg));
    }
    if PngHandler.recognizes(&head) {
        return Ok(Some(AssetFormat::Png));
    }
    Ok(None)
}

/// Lay out `source` with the handler for its format, or `None` when the
/// format has no handler.
pub async fn asset_layout<S: ByteSource + ?Sized>(
    source: &mut S,
    max_boxes: usize,
) -> Result<Option<AssetLayout>, FormatError> {
    let layout = match detect_format(source).await? {
        None => return Ok(None),
        Some(AssetFormat::Jpeg) => JpegHandler.layout(source, max_boxes).await?,
        Some(AssetFormat::Png) => PngHandler.layout(source, max_boxes).await?,
    };
    Ok(Some(layout))
}

/// Read the bytes of the embedded `store` from `source`, refusing a store
/// over `max_bytes` before reading or allocating anything. The buffer is
/// reserved fallibly ([`FormatError::Allocation`]), and the parts must be
/// well-formed ranges that fill the store exactly.
pub async fn read_store_bytes<S: ByteSource + ?Sized>(
    source: &mut S,
    store: &EmbeddedStore,
    max_bytes: u64,
) -> Result<Vec<u8>, FormatError> {
    if store.len > max_bytes || usize::try_from(store.len).is_err() {
        return Err(FormatError::StoreTooLarge {
            size: store.len,
            max: max_bytes,
        });
    }
    let mut bytes = try_zeroed(store.len).ok_or(FormatError::Allocation { size: store.len })?;
    let mut at = 0usize;
    for part in &store.parts {
        let len = part
            .end
            .checked_sub(part.start)
            .and_then(|n| usize::try_from(n).ok());
        let len = len.ok_or(FormatError::Malformed {
            offset: part.start,
            reason: "a store part range is reversed or too long",
        })?;
        let end = at.checked_add(len).filter(|end| *end <= bytes.len());
        let end = end.ok_or(FormatError::Malformed {
            offset: part.start,
            reason: "store parts are longer than the store",
        })?;
        read_exact_at(source, part.start, &mut bytes[at..end]).await?;
        at = end;
    }
    if at != bytes.len() {
        return Err(FormatError::Malformed {
            offset: store.container.start,
            reason: "store parts are shorter than the store",
        });
    }
    Ok(bytes)
}

/// Counts boxes against a limit.
pub(crate) struct BoxBudget {
    /// The limit.
    max: usize,
    /// Boxes counted so far.
    used: usize,
}

impl BoxBudget {
    /// A budget of `max` boxes.
    pub(crate) fn new(max: usize) -> Self {
        Self { max, used: 0 }
    }

    /// Count one box at `offset`.
    pub(crate) fn take(&mut self, offset: u64) -> Result<(), FormatError> {
        if self.used >= self.max {
            return Err(FormatError::TooManyBoxes {
                offset,
                max: self.max,
            });
        }
        self.used += 1;
        Ok(())
    }
}
