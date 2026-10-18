//! A forward-reading cache over a [`ByteSource`] for the format handlers.

use super::FormatError;
use crate::io::{read_exact_at, ByteSource};

/// Bytes read from the source at a time.
const WINDOW: usize = 64 * 1024;

/// Reads an asset through a window cache, so scanning entropy-coded data
/// byte by byte costs one source read per 64 KiB.
pub(crate) struct Reader<'a, S: ?Sized> {
    /// The asset.
    source: &'a mut S,
    /// The asset's size.
    size: u64,
    /// Cached bytes.
    window: Vec<u8>,
    /// Asset offset of `window[0]`.
    window_start: u64,
}

impl<'a, S: ByteSource + ?Sized> Reader<'a, S> {
    /// A reader over `source`.
    pub(crate) async fn new(source: &'a mut S) -> Result<Self, FormatError> {
        let size = source.size().await?;
        Ok(Self {
            source,
            size,
            window: Vec::new(),
            window_start: 0,
        })
    }

    /// The asset's size.
    pub(crate) fn size(&self) -> u64 {
        self.size
    }

    /// The cached bytes from `offset` to the end of the window (loading a
    /// new window when `offset` is outside it); empty at the end of the
    /// asset.
    pub(crate) async fn window(&mut self, offset: u64) -> Result<&[u8], FormatError> {
        let end = self.window_start + self.window.len() as u64;
        if offset < self.window_start || offset >= end {
            let len = (self.size.saturating_sub(offset)).min(WINDOW as u64) as usize;
            self.window = vec![0u8; len];
            self.window_start = offset;
            if len > 0 {
                read_exact_at(self.source, offset, &mut self.window).await?;
            }
        }
        let skip = (offset - self.window_start) as usize;
        Ok(&self.window[skip..])
    }

    /// The byte at `offset`, or `None` at the end of the asset.
    pub(crate) async fn byte(&mut self, offset: u64) -> Result<Option<u8>, FormatError> {
        Ok(self.window(offset).await?.first().copied())
    }

    /// `len` bytes at `offset`, or [`FormatError::Truncated`] when the
    /// asset ends first.
    pub(crate) async fn bytes(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        let end = offset.checked_add(len as u64);
        if end.is_none_or(|end| end > self.size) {
            return Err(FormatError::Truncated { offset });
        }
        let mut out = vec![0u8; len];
        read_exact_at(self.source, offset, &mut out).await?;
        Ok(out)
    }
}
