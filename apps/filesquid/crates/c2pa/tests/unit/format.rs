//! C2PA-S-005 and C2PA-S-001: the format-handler interface and the JPEG
//! (APP11, C2PA 2.4 A.3.1) and PNG (`caBX`, A.3.2) handlers that locate
//! embedded manifest stores, with every unsupported format reported as
//! such.

#[path = "../support/assets.rs"]
#[allow(dead_code)]
mod assets;
#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::ops::Range;

use assets::*;
use filesquid_c2pa::format::{
    asset_layout, detect_format, read_store_bytes, AssetFormat, AssetLayout, EmbeddedStore,
    FormatError, FormatHandler, PngHandler,
};
use filesquid_c2pa::io::{ByteIoError, ByteSource, MemorySource};
use filesquid_c2pa::jumbf::toggles;
use filesquid_c2pa::manifest::ParseLimits;
use filesquid_c2pa::read::{read_asset, AssetManifest, ReadError};
use fixtures::*;
use futures::executor::block_on;

/// Lay out `bytes` with a generous box limit.
fn layout(bytes: &[u8]) -> Result<Option<AssetLayout>, FormatError> {
    block_on(asset_layout(&mut MemorySource::new(bytes.to_vec()), 10_000))
}

/// The names of `layout`'s boxes.
fn names(layout: &AssetLayout) -> Vec<&str> {
    layout.boxes.iter().map(|b| b.name.as_str()).collect()
}

/// The ranges of `layout`'s boxes.
fn ranges(layout: &AssetLayout) -> Vec<Range<u64>> {
    layout.boxes.iter().map(|b| b.range.clone()).collect()
}

/// The reassembled bytes of `layout`'s only store.
fn store_bytes(bytes: &[u8], layout: &AssetLayout) -> Vec<u8> {
    let store = layout.stores.first().expect("a store");
    block_on(read_store_bytes(
        &mut MemorySource::new(bytes.to_vec()),
        store,
        1 << 20,
    ))
    .expect("store bytes")
}

#[test]
fn formats_are_detected_by_signature() {
    let detect = |bytes: &[u8]| block_on(detect_format(&mut MemorySource::new(bytes.to_vec())));
    assert_eq!(
        detect(&[0xFF, 0xD8, 0xFF, 0xE0]),
        Ok(Some(AssetFormat::Jpeg))
    );
    assert_eq!(detect(&PNG_SIGNATURE), Ok(Some(AssetFormat::Png)));
    assert_eq!(detect(b"GIF89a.."), Ok(None));
    assert_eq!(detect(&[0xFF, 0xD8]), Ok(None), "too short to be a JPEG");
    assert_eq!(detect(&[]), Ok(None));
}

#[test]
fn a_jpeg_store_is_located_across_app11_segments() {
    let store = sample_store();
    let pieces = jpeg_pieces(app11_segments(&store, 1, 50));
    let (bytes, expected) = assemble(&pieces);
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(layout.format, AssetFormat::Jpeg);
    assert_eq!(
        names(&layout),
        ["SOI", "APP0", "C2PA", "DQT", "SOS", "RST0", "EOI"]
    );
    assert_eq!(ranges(&layout), expected);
    assert_eq!(layout.size, bytes.len() as u64);
    assert_eq!(layout.stores.len(), 1);
    assert_eq!(layout.stores[0].container, expected[2]);
    assert_eq!(store_bytes(&bytes, &layout), store);
}

#[test]
fn fill_bytes_and_stray_bytes_before_markers_are_skipped() {
    let store = sample_store();
    let mut pieces = jpeg_pieces(app11_segments(&store, 1, 400));
    // Two 0xFF fill bytes before DQT and two stray bytes before SOS, as an
    // understated segment length leaves them.
    pieces[3].1.splice(0..0, [0xFF, 0xFF]);
    pieces[4].1.splice(0..0, [0x00, 0x00]);
    let (bytes, expected) = assemble(&pieces);
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(
        names(&layout),
        ["SOI", "APP0", "C2PA", "DQT", "SOS", "RST0", "EOI"]
    );
    assert_eq!(
        layout.boxes[3].range,
        expected[3].start + 2..expected[3].end
    );
    assert_eq!(
        layout.boxes[4].range,
        expected[4].start + 2..expected[4].end
    );
    assert_eq!(store_bytes(&bytes, &layout), store);
}

#[test]
fn other_app11_segments_are_ordinary_boxes() {
    let not_jumbf = segment(0xEB, b"XY other APP11 data");
    let other_store = superbox(uuid(b"c2pa"), "jpeg360", &[]);
    let other_jumbf = app11_segments(&other_store, 7, 200);
    let mut pieces = jpeg_pieces(not_jumbf);
    pieces.insert(3, ("APP11".into(), other_jumbf));
    let (bytes, _) = assemble(&pieces);
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(
        names(&layout),
        ["SOI", "APP0", "APP11", "APP11", "DQT", "SOS", "RST0", "EOI"]
    );
    assert!(layout.stores.is_empty());
}

#[test]
fn every_jpeg_marker_has_its_iso_10918_name() {
    let mut bytes = vec![0xFF, 0xD8];
    for marker in [
        0xC0u8, 0xC4, 0xC8, 0xCC, 0xCF, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE5, 0xF0, 0xFD, 0xFE,
    ] {
        bytes.extend_from_slice(&segment(marker, &[0]));
    }
    bytes.extend_from_slice(&[0xFF, 0x01, 0xFF, 0xD9]);
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(
        names(&layout),
        [
            "SOI", "SOF0", "DHT", "JPG", "DAC", "SOF15", "DQT", "DNL", "DRI", "DHP", "EXP", "APP5",
            "JPG0", "JPG13", "COM", "TEM", "EOI"
        ]
    );
}

#[test]
fn scan_data_runs_to_the_next_marker_and_restart_markers_are_boxes() {
    let mut sos = segment(0xDA, &[1, 1, 0, 0, 0x3F, 0]);
    sos.extend_from_slice(&[0x01, 0xFF, 0x00, 0x02]);
    let mut bytes = vec![0xFF, 0xD8];
    bytes.extend_from_slice(&sos);
    let rst_at = bytes.len() as u64;
    bytes.extend_from_slice(&[0xFF, 0xD3, 0x03, 0xFF, 0xD7, 0x04]);
    let eoi_at = bytes.len() as u64;
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(names(&layout), ["SOI", "SOS", "RST3", "RST7", "EOI"]);
    assert_eq!(layout.boxes[2].range, rst_at..rst_at + 3);
    assert_eq!(layout.boxes[3].range, rst_at + 3..eoi_at);
}

#[test]
fn a_short_jp_packet_is_malformed() {
    let mut bytes = vec![0xFF, 0xD8];
    bytes.extend_from_slice(&segment(0xEB, &[b'J', b'P', 0, 1, 0, 0, 0, 1]));
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    assert!(matches!(layout(&bytes), Err(FormatError::Malformed { .. })));
}

#[test]
fn truncated_segments_are_errors() {
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&sample_store(), 1, 200)));
    for cut in [5usize, 30, 300] {
        assert!(
            matches!(layout(&bytes[..cut]), Err(FormatError::Truncated { .. })),
            "cut at {cut}"
        );
    }
}

#[test]
fn c2pa_segments_must_be_contiguous_and_in_sequence() {
    let store = sample_store();
    let segments = app11_segments(&store, 1, 400);
    let first_len = 2 + usize::from(u16::from_be_bytes([segments[2], segments[3]]));
    let (first, rest) = segments.split_at(first_len);
    let mut split = first.to_vec();
    split.extend_from_slice(&segment(0xE1, b"Exif\0\0"));
    split.extend_from_slice(rest);
    let (bytes, _) = assemble(&jpeg_pieces(split));
    assert!(
        matches!(layout(&bytes), Err(FormatError::Malformed { .. })),
        "gap"
    );
    let mut swapped = segments.clone();
    // Packet sequence number of the first packet: 2 instead of 1.
    swapped[11] = 2;
    let (bytes, _) = assemble(&jpeg_pieces(swapped));
    assert!(
        matches!(layout(&bytes), Err(FormatError::Malformed { .. })),
        "sequence"
    );
}

#[test]
fn a_reassembled_store_must_match_its_box_length() {
    let mut store = sample_store();
    store.extend_from_slice(&[0; 3]);
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&store, 1, 400)));
    assert!(matches!(layout(&bytes), Err(FormatError::Malformed { .. })));
}

#[test]
fn two_embedded_stores_are_reported_as_multiple() {
    let store = sample_store();
    let mut both = app11_segments(&store, 1, 4000);
    both.extend_from_slice(&app11_segments(&store, 2, 4000));
    let (bytes, _) = assemble(&jpeg_pieces(both));
    let layout = layout(&bytes).expect("layout").expect("JPEG");
    assert_eq!(layout.stores.len(), 2);
    let read = block_on(read_asset(
        &mut MemorySource::new(bytes),
        &ParseLimits::default(),
    ));
    assert!(
        matches!(read, Ok(AssetManifest::MultipleStores { .. })),
        "{read:?}"
    );
}

#[test]
fn the_asset_box_limit_holds_at_the_boundary() {
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&sample_store(), 1, 4000)));
    let lay = |max| block_on(asset_layout(&mut MemorySource::new(bytes.clone()), max));
    assert!(lay(7).is_ok());
    assert!(matches!(
        lay(6),
        Err(FormatError::TooManyBoxes { max: 6, .. })
    ));
    let (png, _) = assemble(&png_pieces(chunk(b"caBX", &sample_store())));
    let lay = |max| block_on(asset_layout(&mut MemorySource::new(png.clone()), max));
    assert!(lay(5).is_ok());
    assert!(matches!(
        lay(4),
        Err(FormatError::TooManyBoxes { max: 4, .. })
    ));
}

/// An in-memory source that records the range of every read.
struct RecordingSource {
    /// The bytes.
    bytes: Vec<u8>,
    /// `offset..offset + len` of each `read_at` call.
    reads: Vec<Range<u64>>,
}

impl ByteSource for RecordingSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Ok(self.bytes.len() as u64)
    }

    async fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        let size = self.bytes.len() as u64;
        if offset > size {
            return Err(ByteIoError::OutOfRange { offset, size });
        }
        // `offset <= size`, which came from a `usize`, so this cannot truncate.
        let available = &self.bytes[offset as usize..];
        let n = buf.len().min(available.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.reads.push(offset..offset + n as u64);
        Ok(n)
    }
}

#[test]
fn a_store_over_the_size_limit_is_refused_without_reading_its_bytes() {
    // The layout reads the APP11 segments to find the store; reading the
    // store itself must then stop at the size check, with no read at all.
    let store = sample_store();
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&store, 1, 200)));
    let mut source = RecordingSource {
        bytes: bytes.clone(),
        reads: Vec::new(),
    };
    let found = block_on(asset_layout(&mut source, 10_000))
        .expect("layout")
        .expect("JPEG");
    let embedded = found.stores.first().expect("a store").clone();
    source.reads.clear();
    let size = store.len() as u64;
    let over = block_on(read_store_bytes(&mut source, &embedded, size - 1));
    assert_eq!(
        over,
        Err(FormatError::StoreTooLarge {
            size,
            max: size - 1
        })
    );
    assert!(source.reads.is_empty(), "{:?}", source.reads);
    let at_limit = block_on(read_store_bytes(&mut source, &embedded, size));
    assert_eq!(at_limit, Ok(store.clone()));
    // Every read lies wholly inside one store part.
    let in_parts = |r: &Range<u64>| {
        embedded
            .parts
            .iter()
            .any(|p| p.start <= r.start && r.end <= p.end)
    };
    assert!(
        source.reads.iter().all(in_parts),
        "only store parts are read"
    );
    // The recorder keeps the ByteSource contract past the end.
    let past = source.bytes.len() as u64 + 1;
    assert!(matches!(
        block_on(source.read_at(past, &mut [0; 4])),
        Err(ByteIoError::OutOfRange { .. })
    ));
}

#[test]
fn a_store_over_the_size_limit_is_reported_by_read_asset() {
    let store = sample_store();
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&store, 1, 200)));
    let limits = |max_store_bytes| ParseLimits {
        max_store_bytes,
        ..ParseLimits::default()
    };
    let size = store.len() as u64;
    let ok = block_on(read_asset(
        &mut MemorySource::new(bytes.clone()),
        &limits(size),
    ));
    assert!(matches!(ok, Ok(AssetManifest::Embedded { .. })));
    let over = block_on(read_asset(&mut MemorySource::new(bytes), &limits(size - 1)));
    assert!(
        matches!(
            over,
            Err(ReadError::Format(FormatError::StoreTooLarge { .. }))
        ),
        "{over:?}"
    );
}

#[test]
fn a_png_store_is_located_in_its_cabx_chunk() {
    let store = sample_store();
    let pieces = png_pieces(chunk(b"caBX", &store));
    let (mut bytes, expected) = assemble(&pieces);
    bytes.extend_from_slice(b"trailing data after IEND");
    let layout = layout(&bytes).expect("layout").expect("PNG");
    assert_eq!(layout.format, AssetFormat::Png);
    assert_eq!(names(&layout), ["PNGh", "IHDR", "C2PA", "IDAT", "IEND"]);
    assert_eq!(ranges(&layout), expected);
    assert_eq!(layout.size, bytes.len() as u64);
    assert_eq!(layout.stores[0].container, expected[2]);
    assert_eq!(store_bytes(&bytes, &layout), store);
}

#[test]
fn malformed_pngs_are_errors() {
    let (bytes, _) = assemble(&png_pieces(chunk(b"caBX", &sample_store())));
    assert!(matches!(
        layout(&bytes[..20]),
        Err(FormatError::Truncated { .. })
    ));
    let mut huge = PNG_SIGNATURE.to_vec();
    huge.extend_from_slice(&[0x80, 0, 0, 0]);
    huge.extend_from_slice(b"IHDR");
    assert!(
        matches!(layout(&huge), Err(FormatError::Malformed { .. })),
        "length over 2^31-1"
    );
    let mut no_end = PNG_SIGNATURE.to_vec();
    no_end.extend_from_slice(&chunk(b"IHDR", &[0; 13]));
    assert!(
        matches!(layout(&no_end), Err(FormatError::Truncated { .. })),
        "no IEND"
    );
}

#[test]
fn two_cabx_chunks_are_two_stores() {
    let store = sample_store();
    let mut pieces = png_pieces(chunk(b"caBX", &store));
    pieces.insert(3, ("C2PA".into(), chunk(b"caBX", &store)));
    let (bytes, _) = assemble(&pieces);
    assert_eq!(
        layout(&bytes).expect("layout").expect("PNG").stores.len(),
        2
    );
}

#[test]
fn embedded_stores_are_read_and_parsed() {
    for kind in [Kind::Jpeg, Kind::Png] {
        let (bytes, _) = assemble(&pieces(kind, &sample_store()));
        let read = block_on(read_asset(
            &mut MemorySource::new(bytes),
            &ParseLimits::default(),
        ));
        let Ok(AssetManifest::Embedded { store, layout }) = read else {
            panic!("{kind:?}: {read:?}");
        };
        assert_eq!(store.active_manifest().label, ACTIVE_LABEL);
        assert_eq!(layout.stores.len(), 1);
    }
}

#[test]
fn assets_without_a_store_and_unsupported_formats_are_reported() {
    let (jpeg, _) = assemble(&jpeg_pieces(segment(0xE1, b"Exif\0\0")));
    let read = block_on(read_asset(
        &mut MemorySource::new(jpeg),
        &ParseLimits::default(),
    ));
    assert!(
        matches!(read, Ok(AssetManifest::NotEmbedded { .. })),
        "{read:?}"
    );
    let gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec();
    let read = block_on(read_asset(
        &mut MemorySource::new(gif),
        &ParseLimits::default(),
    ));
    assert_eq!(read, Ok(AssetManifest::Unsupported));
}

#[test]
fn truncated_and_corrupted_assets_never_panic() {
    for kind in [Kind::Jpeg, Kind::Png] {
        let (bytes, _) = assemble(&pieces(kind, &sample_store()));
        for len in 0..bytes.len() {
            let _ = layout(&bytes[..len]);
        }
        let mut rng = Rng::new(0x0C2_0003);
        for _ in 0..2_000 {
            let mut mutated = bytes.clone();
            for _ in 0..1 + rng.below(4) {
                let at = rng.below(mutated.len());
                mutated[at] = rng.next_u64() as u8;
            }
            let _ = block_on(read_asset(
                &mut MemorySource::new(mutated),
                &ParseLimits::default(),
            ));
        }
    }
}

/// A JPEG whose only APP11 data is the `JP` packet carrying `body`.
fn jpeg_with_packet(body: &[u8]) -> Vec<u8> {
    let mut payload = vec![b'J', b'P', 0, 1, 0, 0, 0, 1];
    payload.extend_from_slice(body);
    let mut bytes = vec![0xFF, 0xD8];
    bytes.extend_from_slice(&segment(0xEB, &payload));
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    bytes
}

#[test]
fn a_c2pa_store_without_the_requestable_and_label_toggles_is_malformed() {
    // Byte 32 is the store description box's toggles (8-byte `jumb`
    // header, then `jumd` length, type and 16-byte UUID).
    let store = sample_store();
    assert_eq!(store[32] & 0x03, 0x03, "fixture sets both toggles");
    let mut unrequestable = store.clone();
    unrequestable[32] &= !toggles::REQUESTABLE;
    let (bytes, _) = assemble(&jpeg_pieces(app11_segments(&unrequestable, 1, 200)));
    assert!(matches!(layout(&bytes), Err(FormatError::Malformed { .. })));
}

#[test]
fn a_first_packet_cut_inside_a_c2pa_description_box_is_malformed() {
    // Cut before the end of the type UUID, and after the UUID but inside
    // the label: neither is passed over as an ordinary APP11 segment.
    let store = sample_store();
    for cut in [30usize, 36] {
        assert!(
            matches!(
                layout(&jpeg_with_packet(&store[..cut])),
                Err(FormatError::Malformed { .. })
            ),
            "cut at {cut}"
        );
    }
}

#[test]
fn a_continuation_packet_must_repeat_the_first_box_header() {
    let store = sample_store();
    let mut segments = app11_segments(&store, 1, 400);
    let first_len = 2 + usize::from(u16::from_be_bytes([segments[2], segments[3]]));
    // Marker and length (4), JP/En/Z (8), then the repeated LBox.
    segments[first_len + 4 + 8 + 3] ^= 0x01;
    let (bytes, _) = assemble(&jpeg_pieces(segments));
    assert!(matches!(layout(&bytes), Err(FormatError::Malformed { .. })));
}

#[test]
fn the_png_handler_checks_the_signature_itself() {
    let (jpeg, _) = assemble(&jpeg_pieces(app11_segments(&sample_store(), 1, 200)));
    let direct = block_on(PngHandler.layout(&mut MemorySource::new(jpeg), 10_000));
    assert!(matches!(
        direct,
        Err(FormatError::Malformed { offset: 0, .. })
    ));
}

#[test]
fn an_iend_chunk_with_data_is_malformed() {
    let mut pieces = png_pieces(chunk(b"caBX", &sample_store()));
    let last = pieces.len() - 1;
    pieces[last] = ("IEND".into(), chunk(b"IEND", &[1]));
    let (bytes, _) = assemble(&pieces);
    assert!(matches!(layout(&bytes), Err(FormatError::Malformed { .. })));
}

#[test]
fn store_parts_must_be_well_formed_and_fill_the_store() {
    let read = |store: EmbeddedStore| {
        block_on(read_store_bytes(
            &mut MemorySource::new(vec![0; 64]),
            &store,
            u64::MAX,
        ))
    };
    let reversed = EmbeddedStore {
        container: 0..64,
        parts: vec![Range { start: 10, end: 4 }],
        len: 6,
    };
    assert!(matches!(read(reversed), Err(FormatError::Malformed { .. })));
    let short = EmbeddedStore {
        container: 0..64,
        parts: vec![Range { start: 4, end: 10 }],
        len: 8,
    };
    assert!(matches!(read(short), Err(FormatError::Malformed { .. })));
    let exact = EmbeddedStore {
        container: 0..64,
        parts: vec![4..10, 20..22],
        len: 8,
    };
    assert_eq!(read(exact), Ok(vec![0; 8]));
}

#[test]
fn an_unreservable_store_is_an_allocation_error_not_an_abort() {
    // More than `isize::MAX` bytes can never be reserved, on any platform.
    let size = isize::MAX as u64 + 1;
    let huge = EmbeddedStore {
        container: 0..64,
        parts: vec![Range { start: 0, end: 64 }],
        len: size,
    };
    let result = block_on(read_store_bytes(
        &mut MemorySource::new(vec![0; 64]),
        &huge,
        u64::MAX,
    ));
    let expected = if usize::try_from(size).is_ok() {
        FormatError::Allocation { size }
    } else {
        FormatError::StoreTooLarge {
            size,
            max: u64::MAX,
        }
    };
    assert_eq!(result, Err(expected));
    assert_eq!(FormatError::Allocation { size }.category(), "allocation");
}

#[test]
fn no_bytes_may_sit_between_c2pa_app11_packets() {
    // A stray byte or a 0xFF fill byte between the store's packets would
    // leave bytes inside the store's container that no part covers.
    let store = sample_store();
    let segments = app11_segments(&store, 1, 400);
    let first_len = 2 + usize::from(u16::from_be_bytes([segments[2], segments[3]]));
    let (first, rest) = segments.split_at(first_len);
    for gap in [0x00u8, 0xFF] {
        let gapped = [first, &[gap], rest].concat();
        let (bytes, _) = assemble(&jpeg_pieces(gapped));
        assert!(
            matches!(layout(&bytes), Err(FormatError::Malformed { .. })),
            "gap byte {gap:#04x}"
        );
    }
    // The same bytes after the last packet are skipped as before.
    let (bytes, _) = assemble(&jpeg_pieces([&segments[..], &[0x00]].concat()));
    assert_eq!(
        layout(&bytes).expect("layout").expect("JPEG").stores.len(),
        1
    );
}
