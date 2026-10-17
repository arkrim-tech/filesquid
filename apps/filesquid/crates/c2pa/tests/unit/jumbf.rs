//! C2PA-S-001: JUMBF superboxes and description boxes (ISO 19566-5 as
//! profiled by C2PA 2.4 section 11.1).

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_c2pa::jumbf::{
    parse_superbox, toggles, types, BoxType, JumbfChild, JumbfError, JumbfLimits, TypeUuid,
    MAX_SUPPORTED_DEPTH,
};
use fixtures::{boxed, boxed_xl, jumd, jumd_payload, sample_store, superbox, uuid};

/// Generous limits.
fn limits() -> JumbfLimits {
    JumbfLimits {
        max_depth: 16,
        max_boxes: 10_000,
    }
}

/// Parse with generous limits.
fn parse(bytes: &[u8]) -> Result<filesquid_c2pa::jumbf::Superbox, JumbfError> {
    parse_superbox(bytes, &limits())
}

#[test]
fn superbox_description_and_content_boxes_parse_with_ranges() {
    let input = superbox(uuid(b"cbor"), "c2pa.actions", &[boxed(b"cbor", &[0xA0])]);
    let sb = parse(&input).expect("parse");
    assert_eq!(sb.label(), Some("c2pa.actions"));
    assert_eq!(sb.type_uuid(), types::CBOR);
    assert_eq!(
        sb.description.toggles,
        toggles::REQUESTABLE | toggles::LABEL
    );
    assert_eq!(
        (sb.range.start, sb.range.header_len, sb.range.end),
        (0, 8, input.len())
    );
    assert_eq!(
        sb.range.payload().start,
        8,
        "hashing excludes the superbox header"
    );
    assert_eq!(sb.children.len(), 1);
    let JumbfChild::Content(content) = &sb.children[0] else {
        panic!("content box expected");
    };
    assert_eq!(content.box_type, BoxType::CBOR);
    assert_eq!(
        &input[content.range.payload().start..content.range.end],
        &[0xA0]
    );
    assert_eq!(content.range.whole().len(), 9);
}

#[test]
fn extended_length_header_is_parsed() {
    let mut payload = jumd(uuid(b"cbor"), "x");
    payload.extend_from_slice(&boxed_xl(b"cbor", &[0x01]));
    let input = boxed_xl(b"jumb", &payload);
    let sb = parse(&input).expect("parse");
    assert_eq!(sb.range.header_len, 16);
    let JumbfChild::Content(content) = &sb.children[0] else {
        panic!("content box expected");
    };
    assert_eq!(content.range.header_len, 16);
    assert_eq!(
        &input[content.range.payload().start..content.range.end],
        &[0x01]
    );
}

#[test]
fn zero_length_box_runs_to_the_end_of_its_container() {
    let mut payload = jumd(uuid(b"json"), "j");
    let mut last = 0u32.to_be_bytes().to_vec();
    last.extend_from_slice(b"json");
    last.extend_from_slice(b"{}");
    payload.extend_from_slice(&last);
    let input = boxed(b"jumb", &payload);
    let sb = parse(&input).expect("parse");
    let JumbfChild::Content(content) = &sb.children[0] else {
        panic!("content box expected");
    };
    assert_eq!(content.range.end, input.len());
    assert_eq!(
        &input[content.range.payload().start..content.range.end],
        b"{}"
    );
}

#[test]
fn lengths_two_to_seven_are_invalid() {
    for lbox in 2u32..8 {
        let mut input = boxed(b"jumb", &jumd(uuid(b"cbor"), "x"));
        let child_at = input.len();
        input.extend_from_slice(&lbox.to_be_bytes());
        input.extend_from_slice(b"cbor");
        let total = u32::try_from(input.len()).expect("len");
        input[..4].copy_from_slice(&total.to_be_bytes());
        assert_eq!(
            parse(&input).map(|_| ()),
            Err(JumbfError::InvalidLength { offset: child_at }),
            "LBox {lbox}"
        );
    }
}

#[test]
fn extended_length_below_sixteen_is_invalid() {
    let mut input = 1u32.to_be_bytes().to_vec();
    input.extend_from_slice(b"jumb");
    input.extend_from_slice(&15u64.to_be_bytes());
    assert_eq!(
        parse(&input).map(|_| ()),
        Err(JumbfError::InvalidLength { offset: 0 })
    );
}

#[test]
fn a_box_longer_than_its_container_is_truncated() {
    let mut input = superbox(uuid(b"cbor"), "x", &[boxed(b"cbor", &[1, 2, 3])]);
    let child_at = input.len() - 11;
    input[child_at + 3] = 200;
    assert_eq!(
        parse(&input).map(|_| ()),
        Err(JumbfError::Truncated { offset: child_at })
    );
    assert_eq!(
        parse(&input[..4]).map(|_| ()),
        Err(JumbfError::Truncated { offset: 0 })
    );
}

#[test]
fn the_top_box_must_be_a_superbox_with_a_description_first() {
    assert_eq!(
        parse(&boxed(b"cbor", &[0])).map(|_| ()),
        Err(JumbfError::NotASuperbox { offset: 0 })
    );
    assert_eq!(
        parse(&boxed(b"jumb", &boxed(b"cbor", &[0]))).map(|_| ()),
        Err(JumbfError::MissingDescription { offset: 8 })
    );
    assert_eq!(
        parse(&boxed(b"jumb", &[])).map(|_| ()),
        Err(JumbfError::MissingDescription { offset: 8 })
    );
}

#[test]
fn a_second_description_box_is_invalid() {
    let input = superbox(uuid(b"cbor"), "x", &[jumd(uuid(b"cbor"), "y")]);
    assert!(matches!(
        parse(&input),
        Err(JumbfError::InvalidDescription { .. })
    ));
}

#[test]
fn description_boxes_need_a_uuid_and_toggles() {
    let input = boxed(b"jumb", &boxed(b"jumd", &[0u8; 16]));
    assert!(matches!(
        parse(&input),
        Err(JumbfError::InvalidDescription { .. })
    ));
}

#[test]
fn reserved_toggle_bits_are_rejected() {
    for bit in [0x20u8, 0x40, 0x80] {
        let d = boxed(b"jumd", &jumd_payload(uuid(b"cbor"), 0x03 | bit, Some("x")));
        let input = boxed(b"jumb", &d);
        assert!(
            matches!(parse(&input), Err(JumbfError::InvalidDescription { .. })),
            "{bit:#x}"
        );
    }
}

#[test]
fn an_unterminated_label_is_rejected() {
    let mut payload = uuid(b"cbor").to_vec();
    payload.push(toggles::LABEL);
    payload.extend_from_slice(b"no-terminator");
    let input = boxed(b"jumb", &boxed(b"jumd", &payload));
    assert!(matches!(
        parse(&input),
        Err(JumbfError::InvalidDescription { .. })
    ));
}

#[test]
fn forbidden_label_characters_are_rejected() {
    let bad = [
        "",
        "a/b",
        "a;b",
        "a?b",
        "a#b",
        "a\u{1}b",
        "a\u{1f}b",
        "a\u{7f}b",
        "a\u{80}b",
        "a\u{9f}b",
        "a\u{feff}b",
        "a\u{ffff}b",
    ];
    for label in bad {
        let input = superbox(uuid(b"cbor"), label, &[]);
        assert_eq!(
            parse(&input).map(|_| ()),
            Err(JumbfError::InvalidLabel { offset: 8 }),
            "{label:?}"
        );
    }
    for label in [
        "urn:c2pa:1-2",
        "c2pa.ingredient.v3__1",
        "\u{e9}t\u{e9}",
        "a\u{a0}b",
    ] {
        assert!(
            parse(&superbox(uuid(b"cbor"), label, &[])).is_ok(),
            "{label:?}"
        );
    }
}

#[test]
fn a_label_that_is_not_utf8_is_rejected() {
    let mut payload = uuid(b"cbor").to_vec();
    payload.push(toggles::LABEL);
    payload.extend_from_slice(&[0xC3, 0x28, 0x00]);
    let input = boxed(b"jumb", &boxed(b"jumd", &payload));
    assert_eq!(
        parse(&input).map(|_| ()),
        Err(JumbfError::InvalidLabel { offset: 8 })
    );
}

#[test]
fn optional_id_signature_and_private_fields_are_parsed() {
    let mut payload = jumd_payload(uuid(b"cbor"), 0x1F, Some("x"));
    payload.extend_from_slice(&7u32.to_be_bytes());
    payload.extend_from_slice(&[0xAB; 32]);
    payload.extend_from_slice(&boxed(b"c2sh", &[0x55; 16]));
    let input = boxed(b"jumb", &boxed(b"jumd", &payload));
    let sb = parse(&input).expect("parse");
    let d = &sb.description;
    assert_eq!(d.id, Some(7));
    assert_eq!(d.signature, Some([0xAB; 32]));
    let private = d.private_box.as_ref().expect("private box");
    assert_eq!(private.box_type, BoxType(*b"c2sh"));
    assert_eq!(
        &input[private.range.payload().start..private.range.end],
        &[0x55; 16]
    );
}

#[test]
fn missing_or_extra_description_fields_are_rejected() {
    let mut short_id = jumd_payload(uuid(b"cbor"), 0x07, Some("x"));
    short_id.extend_from_slice(&[0, 0]);
    let mut short_sig = jumd_payload(uuid(b"cbor"), 0x0B, Some("x"));
    short_sig.extend_from_slice(&[0; 31]);
    let mut bad_private = jumd_payload(uuid(b"cbor"), 0x13, Some("x"));
    bad_private.extend_from_slice(&[0, 0, 0, 40, b'c', b'2', b's', b'h']);
    let mut trailing = jumd_payload(uuid(b"cbor"), 0x03, Some("x"));
    trailing.push(0);
    for payload in [short_id, short_sig, bad_private, trailing] {
        let input = boxed(b"jumb", &boxed(b"jumd", &payload));
        assert!(
            matches!(parse(&input), Err(JumbfError::InvalidDescription { .. })),
            "{payload:?}"
        );
    }
}

#[test]
fn trailing_bytes_after_the_superbox_are_rejected() {
    let mut input = superbox(uuid(b"cbor"), "x", &[]);
    let end = input.len();
    input.push(0);
    assert_eq!(
        parse(&input).map(|_| ()),
        Err(JumbfError::TrailingBytes { offset: end })
    );
}

/// `depth` nested superboxes.
fn nested(depth: usize) -> Vec<u8> {
    let mut inner = superbox(uuid(b"c2pa"), "d", &[]);
    for _ in 1..depth {
        inner = superbox(uuid(b"c2pa"), "d", &[inner]);
    }
    inner
}

#[test]
fn superbox_depth_limit_holds_at_the_boundary() {
    let lim = JumbfLimits {
        max_depth: 5,
        max_boxes: 1_000,
    };
    assert!(parse_superbox(&nested(5), &lim).is_ok());
    // Each level is an 8-byte superbox header and a 27-byte description box
    // before its child, so the sixth superbox starts at 5 * 35.
    assert_eq!(
        parse_superbox(&nested(6), &lim).map(|_| ()),
        Err(JumbfError::TooDeep {
            offset: 5 * 35,
            max: 5
        })
    );
}

#[test]
fn hostile_nesting_does_not_overflow_the_stack() {
    assert!(matches!(
        parse(&deep_nesting(10_000)),
        Err(JumbfError::TooDeep { max: 16, .. })
    ));
}

/// `levels` nested superboxes written in one pass (35 bytes per level).
fn deep_nesting(levels: usize) -> Vec<u8> {
    let description = jumd(uuid(b"c2pa"), "d");
    let mut out = Vec::with_capacity(levels * 35);
    for level in 0..levels {
        let len = u32::try_from((levels - level) * 35).expect("fits");
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(b"jumb");
        out.extend_from_slice(&description);
    }
    out
}

#[test]
fn a_caller_depth_above_the_supported_ceiling_is_lowered_to_it() {
    let unlimited = JumbfLimits {
        max_depth: usize::MAX,
        max_boxes: usize::MAX,
    };
    assert_eq!(
        nested(MAX_SUPPORTED_DEPTH),
        deep_nesting(MAX_SUPPORTED_DEPTH)
    );
    assert!(parse_superbox(&deep_nesting(MAX_SUPPORTED_DEPTH), &unlimited).is_ok());
    assert_eq!(
        parse_superbox(&deep_nesting(100_000), &unlimited).map(|_| ()),
        Err(JumbfError::TooDeep {
            offset: MAX_SUPPORTED_DEPTH * 35,
            max: MAX_SUPPORTED_DEPTH
        })
    );
}

/// Count every box header in `bytes[start..end]`, recursing into `jumb`
/// and into description boxes' private boxes (an independent count).
fn count_boxes(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut at = start;
    let mut count = 0;
    while at < end {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4")) as usize;
        let kind = &bytes[at + 4..at + 8];
        count += 1;
        if kind == b"jumb" {
            count += count_boxes(bytes, at + 8, at + len);
        }
        at += len;
    }
    count
}

#[test]
fn box_count_limit_holds_at_the_boundary() {
    let store = sample_store();
    let total = count_boxes(&store, 0, store.len());
    let exact = JumbfLimits {
        max_depth: 16,
        max_boxes: total,
    };
    assert!(parse_superbox(&store, &exact).is_ok());
    let under = JumbfLimits {
        max_depth: 16,
        max_boxes: total - 1,
    };
    assert!(matches!(
        parse_superbox(&store, &under),
        Err(JumbfError::TooManyBoxes { max, .. }) if max == total - 1
    ));
}

#[test]
fn private_boxes_count_toward_the_box_limit() {
    let mut payload = jumd_payload(uuid(b"cbor"), 0x13, Some("x"));
    payload.extend_from_slice(&boxed(b"c2sh", &[0; 16]));
    let input = boxed(b"jumb", &boxed(b"jumd", &payload));
    let two = JumbfLimits {
        max_depth: 4,
        max_boxes: 2,
    };
    assert!(matches!(
        parse_superbox(&input, &two),
        Err(JumbfError::TooManyBoxes { .. })
    ));
    let three = JumbfLimits {
        max_depth: 4,
        max_boxes: 3,
    };
    assert!(parse_superbox(&input, &three).is_ok());
}

#[test]
fn every_prefix_of_a_store_is_rejected_without_panicking() {
    let store = sample_store();
    for len in 0..store.len() {
        assert!(parse(&store[..len]).is_err(), "prefix {len}");
    }
}

#[test]
fn every_single_byte_corruption_is_handled_without_panicking() {
    let store = sample_store();
    for at in 0..store.len() {
        let mut mutated = store.clone();
        mutated[at] ^= 0xFF;
        let _ = parse(&mutated);
    }
}

#[test]
fn type_uuids_and_box_types_display_safely() {
    assert_eq!(
        types::MANIFEST_STORE.to_string(),
        "63327061-0011-0010-8000-00AA00389B71"
    );
    assert_eq!(TypeUuid(uuid(b"c2ma")), types::STANDARD_MANIFEST);
    assert_eq!(
        types::EMBEDDED_FILE.to_string(),
        "40CB0C32-BB8A-489D-A70B-2AD6F47F4369"
    );
    assert_eq!(BoxType(*b"jumb").to_string(), "jumb");
    assert_eq!(
        BoxType([b'a', 0x0A, 0xFF, b'b']).to_string(),
        "a\\x0a\\xffb"
    );
}

#[test]
fn errors_carry_offsets_and_categories() {
    let err = JumbfError::Truncated { offset: 12 };
    assert!(err.to_string().contains("12"), "{err}");
    assert_eq!(err.category(), "truncated");
    assert!(JumbfError::TooDeep { offset: 0, max: 1 }.is_limit());
    assert!(JumbfError::TooManyBoxes { offset: 0, max: 1 }.is_limit());
    assert!(!err.is_limit());
}
