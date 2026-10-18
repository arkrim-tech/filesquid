//! The PNG handler (C2PA 2.4 A.3.2): the manifest store is the data of a
//! `caBX` chunk.
//!
//! Chunks are walked from the signature to `IEND`. Chunk CRCs are not
//! checked: a data or box hash covers the chunk bytes. Bytes after `IEND`
//! belong to no chunk (a box hash covers them with `c2pa.after`).

use super::reader::Reader;
use super::{
    AssetBox, AssetFormat, AssetLayout, BoxBudget, EmbeddedStore, FormatError, FormatHandler,
};
use crate::io::ByteSource;

/// The eight-byte PNG signature.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The largest chunk length PNG allows (2^31 - 1).
const MAX_CHUNK_LEN: u32 = 0x7FFF_FFFF;

/// Length, type and CRC around a chunk's data.
const CHUNK_OVERHEAD: u64 = 12;

/// The PNG format handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct PngHandler;

impl FormatHandler for PngHandler {
    fn format(&self) -> AssetFormat {
        AssetFormat::Png
    }

    fn recognizes(&self, head: &[u8]) -> bool {
        head.starts_with(&SIGNATURE)
    }

    async fn layout<S: ByteSource + ?Sized>(
        &self,
        source: &mut S,
        max_boxes: usize,
    ) -> Result<AssetLayout, FormatError> {
        let mut reader = Reader::new(source).await?;
        let mut budget = BoxBudget::new(max_boxes);
        let mut layout = AssetLayout {
            format: AssetFormat::Png,
            size: reader.size(),
            boxes: Vec::new(),
            stores: Vec::new(),
        };
        budget.take(0)?;
        layout.boxes.push(AssetBox {
            name: "PNGh".into(),
            range: 0..SIGNATURE.len() as u64,
        });
        let mut at = SIGNATURE.len() as u64;
        // Every chunk is at least 12 bytes, so the size bounds the loop.
        for _ in 0..=reader.size() / CHUNK_OVERHEAD {
            budget.take(at)?;
            let (name, end) = next_chunk(&mut reader, at, &mut layout).await?;
            let done = name == "IEND";
            layout.boxes.push(AssetBox {
                name,
                range: at..end,
            });
            if done {
                return Ok(layout);
            }
            at = end;
        }
        Err(FormatError::Truncated { offset: at })
    }
}

/// Read the chunk at `at`: its box name and end. A `caBX` chunk is
/// recorded in `layout` as a store and named `C2PA`.
async fn next_chunk<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    at: u64,
    layout: &mut AssetLayout,
) -> Result<(String, u64), FormatError> {
    let header = reader.bytes(at, 8).await?;
    let len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    if len > MAX_CHUNK_LEN {
        return Err(malformed(at, "chunk length over 2^31 - 1"));
    }
    let kind = &header[4..8];
    if !kind.iter().all(u8::is_ascii_alphabetic) {
        return Err(malformed(at + 4, "chunk type is not four ASCII letters"));
    }
    let end = at + CHUNK_OVERHEAD + u64::from(len);
    if end > reader.size() {
        return Err(FormatError::Truncated { offset: at });
    }
    if kind != b"caBX" {
        return Ok((String::from_utf8_lossy(kind).into_owned(), end));
    }
    let data = at + 8..at + 8 + u64::from(len);
    layout.stores.push(EmbeddedStore {
        container: at..end,
        len: u64::from(len),
        parts: vec![data],
    });
    Ok(("C2PA".into(), end))
}

/// A malformed-asset error.
fn malformed(offset: u64, reason: &'static str) -> FormatError {
    FormatError::Malformed { offset, reason }
}
