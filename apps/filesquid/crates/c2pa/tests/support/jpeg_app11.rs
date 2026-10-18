//! Test-only extraction of JUMBF from JPEG APP11 segments, so the manifest
//! store parser can be checked against the C2PA public test files before the
//! JPEG format handler exists (CP-0.3 implements it in the library).
//!
//! Each APP11 segment carries the common identifier `JP`, a 2-byte box
//! instance number `En`, a 4-byte packet sequence number `Z` and a slice of
//! one JUMBF box; packets after the first repeat the box header, which is
//! dropped when the box is reassembled (C2PA 2.4 section A.3.3, ISO 19566-5).

use std::collections::BTreeMap;

/// JPEG marker prefix.
const MARKER: u8 = 0xFF;

/// Start of scan: no APP segments follow.
const SOS: u8 = 0xDA;

/// APP11, the JUMBF carrier.
const APP11: u8 = 0xEB;

/// Upper bound on segments walked, so a hostile file cannot loop forever.
const MAX_SEGMENTS: usize = 100_000;

/// Reassemble every JUMBF box carried in APP11 segments, in order of their
/// box instance number.
pub fn jpeg_jumbf(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if bytes.get(..2) != Some(&[0xFF, 0xD8][..]) {
        return Err("not a JPEG".into());
    }
    let mut boxes: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut at = 2usize;
    for _ in 0..MAX_SEGMENTS {
        let Some(seg) = segment(bytes, at)? else {
            return Ok(boxes.into_values().collect());
        };
        if seg.marker == APP11 {
            add_packet(&mut boxes, seg.payload)?;
        }
        at = seg.next;
    }
    Err("too many JPEG segments".into())
}

/// One JPEG marker segment.
struct Segment<'a> {
    /// The marker byte after `0xFF`.
    marker: u8,
    /// The payload after the length field.
    payload: &'a [u8],
    /// Offset of the next marker.
    next: usize,
}

/// The segment at `at`, or `None` at the start of scan or the end of the
/// data.
fn segment(bytes: &[u8], at: usize) -> Result<Option<Segment<'_>>, String> {
    // Like libjpeg, skip extraneous bytes before a marker (some writers
    // understate a segment length) and any 0xFF fill bytes
    // (ITU-T T.81 B.1.1.2). Both scans only move forward through the input.
    let rest = &bytes[at.min(bytes.len())..];
    let extraneous = rest.iter().take_while(|b| **b != MARKER).count();
    let fill = rest[extraneous..]
        .windows(2)
        .take_while(|pair| pair == &[MARKER, MARKER])
        .count();
    let at = at + extraneous + fill;
    let Some(&[_, marker]) = bytes.get(at..at + 2) else {
        return Ok(None);
    };
    if marker == SOS {
        return Ok(None);
    }
    let len_bytes = bytes
        .get(at + 2..at + 4)
        .ok_or("truncated segment length")?;
    let len = usize::from(u16::from_be_bytes([len_bytes[0], len_bytes[1]]));
    let end = at + 2 + len;
    let payload = bytes.get(at + 4..end).ok_or("truncated segment")?;
    Ok(Some(Segment {
        marker,
        payload,
        next: end,
    }))
}

/// Append one APP11 packet to its box.
fn add_packet(boxes: &mut BTreeMap<u16, Vec<u8>>, payload: &[u8]) -> Result<(), String> {
    // APP11 segments without the JP identifier carry other data.
    if payload.get(..2) != Some(&b"JP"[..]) {
        return Ok(());
    }
    // JP, instance, sequence number and a box header take 16 bytes; less
    // is a broken store, never "no store".
    if payload.len() < 16 {
        return Err(format!(
            "JP APP11 packet of {} bytes is too short",
            payload.len()
        ));
    }
    let instance = u16::from_be_bytes([payload[2], payload[3]]);
    let body = &payload[8..];
    match boxes.get_mut(&instance) {
        None => {
            boxes.insert(instance, body.to_vec());
        }
        Some(existing) => {
            let header = if body[..4] == [0, 0, 0, 1] { 16 } else { 8 };
            let rest = body.get(header..).ok_or("short continuation packet")?;
            existing.extend_from_slice(rest);
        }
    }
    Ok(())
}
