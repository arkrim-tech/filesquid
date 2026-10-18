//! Test-only location of the manifest store in an MP4's C2PA `uuid` box
//! (C2PA 2.4 A.5), until the BMFF format handler exists (CP-2.2).
//!
//! The box is a FullBox with the C2PA extended type, a NUL-terminated
//! `box_purpose`, and for `manifest` a 64-bit Merkle offset followed by the
//! manifest store. Writers may pad the box after the store with zero
//! bytes; the padding is checked and dropped.

/// The C2PA extended type.
const C2PA_UUID: [u8; 16] = [
    0xD8, 0xFE, 0xC3, 0xD6, 0x1B, 0x0E, 0x48, 0x3C, 0x92, 0x97, 0x58, 0x28, 0x87, 0x7E, 0xC4, 0x81,
];

/// Most root boxes walked.
const MAX_BOXES: usize = 10_000;

/// The manifest store bytes of the first C2PA `uuid` box with purpose
/// `manifest`, or `None`.
pub fn mp4_manifest_store(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut at = 0usize;
    for _ in 0..MAX_BOXES {
        if at == bytes.len() {
            return Ok(None);
        }
        let header = bytes.get(at..at + 8).ok_or("truncated box header")?;
        let size = u32::from_be_bytes(header[..4].try_into().expect("4")) as usize;
        let end = match size {
            0 => bytes.len(),
            1 => return Err("64-bit box sizes are not handled by this test helper".into()),
            n => at.checked_add(n).ok_or("box size overflow")?,
        };
        let body = bytes.get(at + 8..end).ok_or("box runs past the end")?;
        if &header[4..] == b"uuid" && body.get(..16) == Some(&C2PA_UUID[..]) {
            if let Some(store) = manifest_payload(&body[16..])? {
                return Ok(Some(store));
            }
        }
        at = end;
    }
    Err("too many boxes".into())
}

/// The store inside a C2PA `uuid` box body after its extended type, or
/// `None` when the box has another purpose (such as `merkle`).
fn manifest_payload(body: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let rest = body.get(4..).ok_or("short FullBox")?;
    let nul = rest
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated purpose")?;
    if &rest[..nul] != b"manifest" {
        return Ok(None);
    }
    let store = rest.get(nul + 1 + 8..).ok_or("short merkle offset")?;
    let declared = store.get(..4).ok_or("short JUMBF header")?;
    let len = u32::from_be_bytes(declared.try_into().expect("4")) as usize;
    let (jumbf, padding) = store
        .split_at_checked(len)
        .ok_or("JUMBF runs past the box")?;
    if padding.iter().any(|b| *b != 0) {
        return Err("non-zero bytes after the manifest store".into());
    }
    Ok(Some(jumbf.to_vec()))
}
