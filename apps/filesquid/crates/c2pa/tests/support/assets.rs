//! Test-only builders for JPEG, PNG and BMFF assets that carry a manifest
//! store, with hard-binding assertions whose hashes are computed here,
//! independently of the code under test.
//!
//! Each asset is a list of named pieces (the box names C2PA 2.4 section
//! 18.7 uses), so tests know every byte range and can build data-hash and
//! box-hash assertions from them. The pieces never depend on the store's
//! size except the store's own piece, so box hashes are stable.

use std::ops::Range;

use filesquid_c2pa::cbor::CborValue;
use sha2::{Digest, Sha256};

use super::fixtures::*;

/// SHA-256 of `bytes`, from the `sha2` crate directly, so the helpers
/// need neither the crate's crypto backend nor its `rust-crypto` feature.
pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

/// SHA-256 of `bytes` without the `excluded` ranges (sorted, in bounds).
pub fn sha256_without(bytes: &[u8], excluded: &[Range<u64>]) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut at = 0usize;
    for range in excluded {
        let start = (range.start as usize).min(bytes.len());
        kept.extend_from_slice(&bytes[at.min(start)..start]);
        at = at.max((range.end as usize).min(bytes.len()));
    }
    kept.extend_from_slice(&bytes[at.min(bytes.len())..]);
    sha256(&kept)
}

/// Which container format to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// JPEG with the store in APP11 segments.
    Jpeg,
    /// PNG with the store in a `caBX` chunk.
    Png,
}

/// A named piece of an asset.
pub type Piece = (String, Vec<u8>);

/// APP11 segments carrying `store` (a JUMBF box with an 8-byte header),
/// `chunk` bytes of box content per segment, box instance `en`.
pub fn app11_segments(store: &[u8], en: u16, chunk: usize) -> Vec<u8> {
    let (header, body) = store.split_at(8);
    let mut out = Vec::new();
    for (i, part) in body.chunks(chunk).enumerate() {
        let mut payload = b"JP".to_vec();
        payload.extend_from_slice(&en.to_be_bytes());
        payload.extend_from_slice(&(i as u32 + 1).to_be_bytes());
        payload.extend_from_slice(header);
        payload.extend_from_slice(part);
        out.extend_from_slice(&[0xFF, 0xEB]);
        out.extend_from_slice(
            &u16::try_from(payload.len() + 2)
                .expect("segment")
                .to_be_bytes(),
        );
        out.extend_from_slice(&payload);
    }
    out
}

/// A JPEG marker segment with a length field.
pub fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, marker];
    out.extend_from_slice(
        &u16::try_from(payload.len() + 2)
            .expect("segment")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out
}

/// The pieces of a small JPEG with `store_piece` after APP0.
pub fn jpeg_pieces(store_piece: Vec<u8>) -> Vec<Piece> {
    let app0 = segment(0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    let mut dqt_payload = vec![0u8];
    dqt_payload.extend((1..=64).map(|i| i as u8));
    let mut sos = segment(0xDA, &[1, 1, 0, 0, 0x3F, 0]);
    // Entropy-coded data with a stuffed 0xFF00.
    sos.extend_from_slice(&[0x12, 0xFF, 0x00, 0x34]);
    vec![
        ("SOI".into(), vec![0xFF, 0xD8]),
        ("APP0".into(), app0),
        ("C2PA".into(), store_piece),
        ("DQT".into(), segment(0xDB, &dqt_payload)),
        ("SOS".into(), sos),
        ("RST0".into(), vec![0xFF, 0xD0, 0x56, 0x78]),
        ("EOI".into(), vec![0xFF, 0xD9]),
    ]
}

/// CRC-32 (ISO 3309, as PNG uses it).
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// A PNG chunk with a correct CRC.
pub fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(data.len())
        .expect("chunk")
        .to_be_bytes()
        .to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// The PNG signature.
pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The pieces of a small PNG with `store_piece` after IHDR.
pub fn png_pieces(store_piece: Vec<u8>) -> Vec<Piece> {
    vec![
        ("PNGh".into(), PNG_SIGNATURE.to_vec()),
        (
            "IHDR".into(),
            chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]),
        ),
        ("C2PA".into(), store_piece),
        (
            "IDAT".into(),
            chunk(
                b"IDAT",
                &[0x78, 0x9C, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01],
            ),
        ),
        ("IEND".into(), chunk(b"IEND", &[])),
    ]
}

/// The pieces of an asset of `kind` carrying `store` (APP11 segments of
/// 200 content bytes for JPEG).
pub fn pieces(kind: Kind, store: &[u8]) -> Vec<Piece> {
    match kind {
        Kind::Jpeg => jpeg_pieces(app11_segments(store, 1, 200)),
        Kind::Png => png_pieces(chunk(b"caBX", store)),
    }
}

/// Concatenate `pieces`, returning the bytes and each piece's range.
pub fn assemble(pieces: &[Piece]) -> (Vec<u8>, Vec<Range<u64>>) {
    let mut bytes = Vec::new();
    let mut ranges = Vec::new();
    for (_, piece) in pieces {
        let start = bytes.len() as u64;
        bytes.extend_from_slice(piece);
        ranges.push(start..bytes.len() as u64);
    }
    (bytes, ranges)
}

/// The range of the `C2PA` piece.
pub fn store_range(pieces: &[Piece], ranges: &[Range<u64>]) -> Range<u64> {
    let index = pieces
        .iter()
        .position(|(name, _)| name == "C2PA")
        .expect("C2PA piece");
    ranges[index].clone()
}

/// A store with one standard manifest whose only assertion is `label`
/// holding `value` (claim `alg` is `sha256`).
pub fn binding_store(label: &str, value: &CborValue) -> Vec<u8> {
    store(&[binding_manifest(ACTIVE_LABEL, label, value)])
}

/// A standard manifest labelled `manifest_label` with one assertion.
pub fn binding_manifest(manifest_label: &str, label: &str, value: &CborValue) -> Vec<u8> {
    manifest(
        b"c2ma",
        manifest_label,
        &[
            assertion_store(&[cbor_assertion(label, value)]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&[label]))),
            signature_box(),
        ],
    )
}

/// A data-hash assertion map.
pub fn data_hash_map(exclusions: &[(u64, u64)], hash: &[u8]) -> CborValue {
    let ranges = exclusions
        .iter()
        .map(|(start, length)| {
            map(&[
                ("start", CborValue::Unsigned(*start)),
                ("length", CborValue::Unsigned(*length)),
            ])
        })
        .collect();
    map(&[
        ("exclusions", CborValue::Array(ranges)),
        ("alg", text("sha256")),
        ("hash", bytes(hash)),
        ("pad", bytes(&[0; 4])),
    ])
}

/// Build an asset of `kind` whose store holds a data-hash assertion with
/// the exclusions `exclusions(container)` returns for the store container
/// range, and a hash computed over the asset minus those exclusions.
/// `wrap` turns the data-hash map into the full store.
pub fn data_hash_asset_with(
    kind: Kind,
    exclusions: impl Fn(&Range<u64>) -> Vec<(u64, u64)>,
    wrap: impl Fn(&CborValue) -> Vec<u8>,
) -> Vec<u8> {
    // A non-empty first guess, so closures may offset from either end.
    let mut container = 100..1_000u64;
    for _ in 0..8 {
        let store = wrap(&data_hash_map(&exclusions(&container), &[0; 32]));
        let pieces = pieces(kind, &store);
        let (_, ranges) = assemble(&pieces);
        let next = store_range(&pieces, &ranges);
        if next == container {
            break;
        }
        container = next;
    }
    let excl = exclusions(&container);
    let placeholder = wrap(&data_hash_map(&excl, &[0; 32]));
    let (unhashed, _) = assemble(&pieces(kind, &placeholder));
    let ranges: Vec<_> = excl.iter().map(|(s, l)| *s..s + l).collect();
    let hash = sha256_without(&unhashed, &ranges);
    let (asset, final_ranges) = {
        let store = wrap(&data_hash_map(&excl, &hash));
        let pieces = pieces(kind, &store);
        let (asset, ranges) = assemble(&pieces);
        (asset, store_range(&pieces, &ranges))
    };
    assert_eq!(final_ranges, container, "the store size is stable");
    asset
}

/// An asset of `kind` with a valid data hash excluding exactly the store.
pub fn data_hash_asset(kind: Kind) -> Vec<u8> {
    data_hash_asset_with(
        kind,
        |c| vec![(c.start, c.end - c.start)],
        |map| binding_store("c2pa.hash.data", map),
    )
}

/// One box-hash entry: `names`, `hash`, optional extra fields.
pub fn box_entry(names: &[&str], hash: &[u8], extra: &[(&str, CborValue)]) -> CborValue {
    let mut fields = vec![
        (
            "names",
            CborValue::Array(names.iter().map(|n| text(n)).collect()),
        ),
        ("hash", bytes(hash)),
    ];
    fields.extend(extra.iter().cloned());
    map(&fields)
}

/// The default box-hash entries for `pieces`: one per piece, the store
/// named `C2PA` with hash `00`.
pub fn default_box_entries(pieces: &[Piece]) -> Vec<CborValue> {
    pieces
        .iter()
        .map(|(name, piece)| match name.as_str() {
            "C2PA" => box_entry(&["C2PA"], &[0], &[]),
            _ => box_entry(&[name], &sha256(piece), &[]),
        })
        .collect()
}

/// A box-hash assertion map.
pub fn box_hash_map(entries: Vec<CborValue>) -> CborValue {
    map(&[
        ("boxes", CborValue::Array(entries)),
        ("alg", text("sha256")),
    ])
}

/// Build an asset of `kind` whose store holds the box-hash assertion
/// `entries(pieces)` returns; box hashes never depend on the store.
pub fn box_hash_asset_with(kind: Kind, entries: impl Fn(&[Piece]) -> Vec<CborValue>) -> Vec<u8> {
    let skeleton = pieces(kind, &binding_store("c2pa.hash.boxes", &CborValue::Null));
    let store = binding_store("c2pa.hash.boxes", &box_hash_map(entries(&skeleton)));
    assemble(&pieces(kind, &store)).0
}

/// An asset of `kind` with a valid default box hash.
pub fn box_hash_asset(kind: Kind) -> Vec<u8> {
    box_hash_asset_with(kind, default_box_entries)
}

/// The C2PA BMFF `uuid` extended type (A.5).
pub const C2PA_BMFF_UUID: [u8; 16] = [
    0xD8, 0xFE, 0xC3, 0xD6, 0x1B, 0x0E, 0x48, 0x3C, 0x92, 0x97, 0x58, 0x28, 0x87, 0x7E, 0xC4, 0x81,
];

/// A BMFF box.
pub fn bmff_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(payload.len() + 8)
        .expect("box")
        .to_be_bytes()
        .to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

/// A small MP4: `ftyp`, the C2PA `uuid` box, `moov` (`mvhd` FullBox v1
/// flags 000003, `udta`), `free`, `mdat`. Returns the bytes and the ranges
/// of `ftyp`, `uuid`, `moov`, `mvhd`, `udta`, `free`, `mdat`.
pub fn mp4() -> (Vec<u8>, Vec<Range<u64>>) {
    let mut uuid_payload = C2PA_BMFF_UUID.to_vec();
    uuid_payload.extend_from_slice(&[0, 0, 0, 0]);
    uuid_payload.extend_from_slice(b"manifest\0");
    uuid_payload.extend_from_slice(&[0; 8]);
    uuid_payload.extend_from_slice(&[0xAA; 40]);
    let mvhd = bmff_box(b"mvhd", &[1, 0, 0, 3, 9, 9, 9, 9]);
    let udta = bmff_box(b"udta", &[7; 12]);
    let moov = bmff_box(b"moov", &[mvhd.clone(), udta.clone()].concat());
    let pieces = [
        bmff_box(b"ftyp", b"isom\0\0\0\0isom"),
        bmff_box(b"uuid", &uuid_payload),
        moov.clone(),
        bmff_box(b"free", &[0; 6]),
        bmff_box(b"mdat", &[5; 40]),
    ];
    let mut bytes = Vec::new();
    let mut ranges = Vec::new();
    for piece in &pieces {
        let start = bytes.len() as u64;
        bytes.extend_from_slice(piece);
        ranges.push(start..bytes.len() as u64);
    }
    let moov_start = ranges[2].start;
    let mvhd_range = moov_start + 8..moov_start + 8 + mvhd.len() as u64;
    let udta_range = mvhd_range.end..mvhd_range.end + udta.len() as u64;
    let mut all = vec![
        ranges[0].clone(),
        ranges[1].clone(),
        ranges[2].clone(),
        mvhd_range,
        udta_range,
    ];
    all.push(ranges[3].clone());
    all.push(ranges[4].clone());
    (bytes, all)
}

/// The BMFF hash input for root boxes `roots` of `bytes`: `offset || data`
/// for each, minus `excluded` absolute ranges (sorted).
pub fn bmff_expected_hash(bytes: &[u8], roots: &[Range<u64>], excluded: &[Range<u64>]) -> Vec<u8> {
    let mut input = Vec::new();
    for root in roots {
        input.extend_from_slice(&root.start.to_be_bytes());
        for at in root.clone() {
            if !excluded.iter().any(|r| r.contains(&at)) {
                input.push(bytes[at as usize]);
            }
        }
    }
    sha256(&input)
}

/// One BMFF exclusion entry for `xpath` with extra fields.
pub fn bmff_exclusion(xpath: &str, extra: &[(&str, CborValue)]) -> CborValue {
    let mut fields = vec![("xpath", text(xpath))];
    fields.extend(extra.iter().cloned());
    map(&fields)
}

/// The A.5.6 always-excluded entries: `/uuid` (C2PA data), `/ftyp`, `/mfra`.
pub fn bmff_required_exclusions() -> Vec<CborValue> {
    vec![
        bmff_exclusion(
            "/uuid",
            &[(
                "data",
                CborValue::Array(vec![map(&[
                    ("offset", CborValue::Unsigned(8)),
                    ("value", bytes(&C2PA_BMFF_UUID)),
                ])]),
            )],
        ),
        bmff_exclusion("/ftyp", &[]),
        bmff_exclusion("/mfra", &[]),
    ]
}

/// A BMFF hash map with `exclusions` and `hash`.
pub fn bmff_hash_map(exclusions: Vec<CborValue>, hash: &[u8]) -> CborValue {
    map(&[
        ("exclusions", CborValue::Array(exclusions)),
        ("alg", text("sha256")),
        ("hash", bytes(hash)),
    ])
}

/// An ingredient v3 assertion value pointing at `manifest_label` as the
/// parent.
pub fn parent_ingredient(manifest_label: &str) -> CborValue {
    map(&[
        ("relationship", text("parentOf")),
        (
            "activeManifest",
            hashed_uri(&format!("self#jumbf=/c2pa/{manifest_label}"), &[3; 32]),
        ),
        (
            "claimSignature",
            hashed_uri(
                &format!("self#jumbf=/c2pa/{manifest_label}/c2pa.signature"),
                &[4; 32],
            ),
        ),
    ])
}

/// An update manifest labelled `label` whose parent is `parent_label`.
pub fn update_manifest(label: &str, parent_label: &str) -> Vec<u8> {
    manifest(
        b"c2um",
        label,
        &[
            assertion_store(&[cbor_assertion(
                "c2pa.ingredient.v3",
                &parent_ingredient(parent_label),
            )]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["c2pa.ingredient.v3"]))),
            signature_box(),
        ],
    )
}
