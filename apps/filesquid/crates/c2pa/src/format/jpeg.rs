//! The JPEG handler (C2PA 2.4 A.3.1 and 18.7.3.1).
//!
//! The manifest store is a JUMBF box carried in APP11 segments. Each
//! segment holds the common identifier `JP`, a box instance number `En`, a
//! packet sequence number `Z` (from 1) and a slice of the box; every packet
//! after the first repeats the box header, which reassembly drops
//! (ISO 19566-5 D.2) after checking that it equals the first packet's. The
//! store's segments must be contiguous (no fill or stray bytes between
//! them) and in sequence. The store is
//! identified by its JUMBF type UUID and the label `c2pa` (section
//! 18.7.3.1); a store whose description box lacks the requestable and label
//! toggles (section 11.1.4.1.2), or a first packet cut off before those
//! fields, is malformed. APP11 segments for other uses (no `JP`, or JUMBF
//! of another type or label) are ordinary `APP11` boxes.
//!
//! Boxes are named by ISO 10918-1 Table B.1. `SOS` and `RSTn` boxes include
//! the entropy-coded data after them. Like libjpeg, the handler skips 0xFF
//! fill bytes and stray bytes before a marker; a segment that runs past the
//! end of the asset, or an asset without `EOI`, is truncated. Skipped bytes
//! belong to no box: section 18.7.1 hashes "only data within the listed
//! box" when boxes are listed separately, and the bytes between boxes when
//! one entry lists a run of boxes.

use super::reader::Reader;
use super::{
    AssetBox, AssetFormat, AssetLayout, BoxBudget, EmbeddedStore, FormatError, FormatHandler,
};
use crate::io::ByteSource;
use crate::jumbf::{toggles, types};

/// The JPEG handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct JpegHandler;

impl FormatHandler for JpegHandler {
    fn format(&self) -> AssetFormat {
        AssetFormat::Jpeg
    }

    fn recognizes(&self, head: &[u8]) -> bool {
        head.starts_with(&[0xFF, 0xD8, 0xFF])
    }

    async fn layout<S: ByteSource + ?Sized>(
        &self,
        source: &mut S,
        max_boxes: usize,
    ) -> Result<AssetLayout, FormatError> {
        let reader = Reader::new(source).await?;
        let mut parser = Parser {
            layout: AssetLayout {
                format: AssetFormat::Jpeg,
                size: reader.size(),
                boxes: Vec::new(),
                stores: Vec::new(),
            },
            reader,
            budget: BoxBudget::new(max_boxes),
            building: None,
        };
        parser.push("SOI".into(), 0..2)?;
        parser.walk().await?;
        Ok(parser.layout)
    }
}

/// Marker code of APP11.
const APP11: u8 = 0xEB;
/// Marker code of SOS.
const SOS: u8 = 0xDA;
/// Marker code of EOI.
const EOI: u8 = 0xD9;
/// JP, `En`, `Z` before the box bytes in a JUMBF APP11 packet.
const PACKET_PREFIX: usize = 8;
/// The shortest JUMBF packet: prefix and a box header.
const MIN_PACKET: usize = PACKET_PREFIX + 8;

/// A store whose APP11 packets are still arriving.
struct Building {
    /// Box instance number.
    en: u16,
    /// Expected next packet sequence number.
    next_z: u32,
    /// Start of the first segment.
    start: u64,
    /// The box header (8 bytes, or 16 with `XLBox`) that every
    /// continuation packet repeats.
    header: Vec<u8>,
    /// Box length the header declares.
    declared: u64,
    /// Bytes collected so far.
    collected: u64,
    /// Where the collected bytes are.
    parts: Vec<std::ops::Range<u64>>,
}

/// Layout state.
struct Parser<'a, S: ?Sized> {
    /// The asset.
    reader: Reader<'a, S>,
    /// Box limit.
    budget: BoxBudget,
    /// The layout so far.
    layout: AssetLayout,
    /// A store being reassembled.
    building: Option<Building>,
}

/// A malformed-asset error.
fn malformed(offset: u64, reason: &'static str) -> FormatError {
    FormatError::Malformed { offset, reason }
}

impl<S: ByteSource + ?Sized> Parser<'_, S> {
    /// Add a box, which may not split the store's segments.
    fn push(&mut self, name: String, range: std::ops::Range<u64>) -> Result<(), FormatError> {
        if self.building.is_some() {
            return Err(malformed(
                range.start,
                "C2PA APP11 segments are not contiguous",
            ));
        }
        self.budget.take(range.start)?;
        self.layout.boxes.push(AssetBox { name, range });
        Ok(())
    }

    /// Walk the markers after SOI up to EOI. Every step moves forward, so
    /// the asset size bounds the loop.
    async fn walk(&mut self) -> Result<(), FormatError> {
        let mut at = 2u64;
        for _ in 0..=self.reader.size() {
            let Some((marker, code)) = self.next_marker(at).await? else {
                return Err(FormatError::Truncated { offset: at });
            };
            if marker != at && self.building.is_some() {
                return Err(malformed(at, "C2PA APP11 segments are not contiguous"));
            }
            if code == EOI {
                return self.push("EOI".into(), marker..marker + 2);
            }
            at = self.marker(marker, code).await?;
        }
        Err(FormatError::Truncated { offset: at })
    }

    /// The next marker at or after `at`, skipping stray bytes and 0xFF fill
    /// bytes: its offset and code, or `None` at the end of the asset.
    async fn next_marker(&mut self, mut at: u64) -> Result<Option<(u64, u8)>, FormatError> {
        for _ in 0..=self.reader.size() {
            let window = self.reader.window(at).await?;
            let Some(i) = window.iter().position(|b| *b == 0xFF) else {
                if window.is_empty() {
                    return Ok(None);
                }
                at += window.len() as u64;
                continue;
            };
            let ff = at + i as u64;
            match self.reader.byte(ff + 1).await? {
                None => return Ok(None),
                Some(0xFF | 0x00) => at = ff + 1,
                Some(code) => return Ok(Some((ff, code))),
            }
        }
        Ok(None)
    }

    /// Handle the marker `code` at `at`; return where to continue.
    async fn marker(&mut self, at: u64, code: u8) -> Result<u64, FormatError> {
        match code {
            0x01 | 0xD0..=0xD8 => {
                self.push(marker_name(code), at..at + 2)?;
                Ok(at + 2)
            }
            SOS => self.scan(at).await,
            _ => self.segment(at, code).await,
        }
    }

    /// The end of the length-prefixed segment at `at`.
    async fn segment_end(&mut self, at: u64) -> Result<u64, FormatError> {
        let len = self.reader.bytes(at + 2, 2).await?;
        let len = u64::from(u16::from_be_bytes([len[0], len[1]]));
        if len < 2 {
            return Err(malformed(at, "segment length below 2"));
        }
        let end = at.checked_add(2).and_then(|end| end.checked_add(len));
        match end {
            Some(end) if end <= self.reader.size() => Ok(end),
            _ => Err(FormatError::Truncated { offset: at }),
        }
    }

    /// A length-prefixed segment.
    async fn segment(&mut self, at: u64, code: u8) -> Result<u64, FormatError> {
        let end = self.segment_end(at).await?;
        if code == APP11 {
            self.app11(at, end).await?;
        } else {
            self.push(marker_name(code), at..end)?;
        }
        Ok(end)
    }

    /// SOS and its entropy-coded data, then each restart interval.
    async fn scan(&mut self, at: u64) -> Result<u64, FormatError> {
        let header_end = self.segment_end(at).await?;
        let (mut next, mut code) = self.scan_end(header_end).await?;
        self.push("SOS".into(), at..next)?;
        // Each restart box covers at least its two marker bytes.
        for _ in 0..=self.reader.size() / 2 {
            if !(0xD0..=0xD7).contains(&code) {
                return Ok(next);
            }
            let (end, end_code) = self.scan_end(next + 2).await?;
            self.push(marker_name(code), next..end)?;
            (next, code) = (end, end_code);
        }
        Ok(next)
    }

    /// The first marker after entropy-coded data starting at `from`:
    /// 0xFF followed by a byte other than 0x00 (stuffing) or 0xFF (fill).
    async fn scan_end(&mut self, from: u64) -> Result<(u64, u8), FormatError> {
        let mut at = from;
        for _ in 0..=self.reader.size() {
            let window = self.reader.window(at).await?;
            if window.is_empty() {
                return Err(FormatError::Truncated { offset: from });
            }
            let Some(i) = window.iter().position(|b| *b == 0xFF) else {
                at += window.len() as u64;
                continue;
            };
            let ff = at + i as u64;
            match self.reader.byte(ff + 1).await? {
                None => return Err(FormatError::Truncated { offset: from }),
                Some(0x00 | 0xFF) => at = ff + 1,
                Some(code) => return Ok((ff, code)),
            }
        }
        Err(FormatError::Truncated { offset: from })
    }

    /// An APP11 segment from `at` to `end`.
    async fn app11(&mut self, at: u64, end: u64) -> Result<(), FormatError> {
        let payload = self.reader.bytes(at + 4, (end - at - 4) as usize).await?;
        if !payload.starts_with(b"JP") {
            return self.push("APP11".into(), at..end);
        }
        if payload.len() < MIN_PACKET {
            return Err(malformed(at, "JP APP11 packet shorter than 16 bytes"));
        }
        let en = u16::from_be_bytes([payload[2], payload[3]]);
        let z = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        let body = &payload[PACKET_PREFIX..];
        match self.building.as_ref().map(|b| b.en) {
            Some(current) if current == en => self.continue_store(at, end, z, body),
            Some(_) => Err(malformed(at, "C2PA APP11 segments are not contiguous")),
            None => match store_header(body).map_err(|reason| malformed(at, reason))? {
                Some(declared) => self.start_store(at, end, (z, en), declared, body),
                None => self.push("APP11".into(), at..end),
            },
        }
    }

    /// The first packet of a C2PA store: sequence `z`, instance `en`, a box
    /// of `(header_len, declared)` bytes, and the packet's `body`.
    fn start_store(
        &mut self,
        at: u64,
        end: u64,
        (z, en): (u32, u16),
        (header_len, declared): (usize, u64),
        body: &[u8],
    ) -> Result<(), FormatError> {
        if z != 1 {
            return Err(malformed(at, "C2PA APP11 packets are out of sequence"));
        }
        self.budget.take(at)?;
        let first_part = at + 4 + PACKET_PREFIX as u64..end;
        self.building = Some(Building {
            en,
            next_z: 2,
            start: at,
            header: body[..header_len].to_vec(),
            declared,
            collected: body.len() as u64,
            parts: vec![first_part],
        });
        self.finish_store(end)
    }

    /// A continuation packet, whose repeated box header must equal the first
    /// packet's and is dropped.
    fn continue_store(
        &mut self,
        at: u64,
        end: u64,
        z: u32,
        body: &[u8],
    ) -> Result<(), FormatError> {
        self.budget.take(at)?;
        let Some(building) = self.building.as_mut() else {
            return Err(malformed(at, "no store is being reassembled"));
        };
        if z != building.next_z {
            return Err(malformed(at, "C2PA APP11 packets are out of sequence"));
        }
        if !body.starts_with(&building.header) {
            return Err(malformed(
                at,
                "a C2PA continuation packet repeats a different box header",
            ));
        }
        building.next_z = building.next_z.saturating_add(1);
        let data_start = at + 4 + (PACKET_PREFIX + building.header.len()) as u64;
        building.collected += end - data_start;
        building.parts.push(data_start..end);
        self.finish_store(end)
    }

    /// Record the store once all its bytes have arrived.
    fn finish_store(&mut self, end: u64) -> Result<(), FormatError> {
        let Some(building) = self.building.as_ref() else {
            return Ok(());
        };
        if building.collected > building.declared {
            return Err(malformed(
                building.start,
                "C2PA store is longer than its box length",
            ));
        }
        if building.collected < building.declared {
            return Ok(());
        }
        let Some(done) = self.building.take() else {
            return Ok(());
        };
        self.layout.stores.push(EmbeddedStore {
            container: done.start..end,
            parts: done.parts,
            len: done.declared,
        });
        self.layout.boxes.push(AssetBox {
            name: "C2PA".into(),
            range: done.start..end,
        });
        Ok(())
    }
}

/// Why a packet that starts a JUMBF superbox is too short to tell its type.
const SHORT: &str = "JUMBF APP11 packet ends inside its description box";

/// Whether the first JUMBF packet `body` starts a C2PA manifest store
/// (C2PA 2.4 sections 11.1.4.2 and 18.7.3.1): `Some((header_len,
/// declared))` for a store, `None` for another use of APP11, and an error
/// for a malformed store or a packet too short to tell.
fn store_header(body: &[u8]) -> Result<Option<(usize, u64)>, &'static str> {
    if body.get(4..8) != Some(&b"jumb"[..]) {
        return Ok(None);
    }
    let lbox = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let (header_len, declared) = match lbox {
        1 => {
            let xl = body.get(8..16).ok_or(SHORT)?;
            (16, u64::from_be_bytes(xl.try_into().map_err(|_| SHORT)?))
        }
        n if n >= 8 => (8, u64::from(n)),
        _ => return Ok(None),
    };
    let jumd = body.get(header_len..).unwrap_or_default();
    let (Some(kind), Some(uuid)) = (jumd.get(4..8), jumd.get(8..24)) else {
        return Err(SHORT);
    };
    if kind != b"jumd" || uuid != types::MANIFEST_STORE.0 {
        return Ok(None);
    }
    let (Some(found), Some(label)) = (jumd.get(24), jumd.get(25..30)) else {
        return Err(SHORT);
    };
    if label != b"c2pa\0" {
        return Ok(None);
    }
    let wanted = toggles::REQUESTABLE | toggles::LABEL;
    if found & wanted != wanted {
        return Err("C2PA store description box lacks the requestable and label toggles");
    }
    Ok(Some((header_len, declared)))
}

/// The ISO 10918-1 Table B.1 symbol of marker `code`.
fn marker_name(code: u8) -> String {
    let fixed = match code {
        0x01 => "TEM",
        0xC4 => "DHT",
        0xC8 => "JPG",
        0xCC => "DAC",
        0xD8 => "SOI",
        0xD9 => "EOI",
        0xDA => "SOS",
        0xDB => "DQT",
        0xDC => "DNL",
        0xDD => "DRI",
        0xDE => "DHP",
        0xDF => "EXP",
        0xFE => "COM",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.into();
    }
    match code {
        0xC0..=0xCF => format!("SOF{}", code - 0xC0),
        0xD0..=0xD7 => format!("RST{}", code - 0xD0),
        0xE0..=0xEF => format!("APP{}", code - 0xE0),
        0xF0..=0xFD => format!("JPG{}", code - 0xF0),
        _ => "RES".into(),
    }
}
