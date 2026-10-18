//! C2PA-S-001: the CBOR decoder that reads claims and assertions.
//!
//! Well-formed CBOR (RFC 8949 Appendix C) decodes; invalid or malformed
//! input, including truncated input and inputs that declare huge lengths,
//! fails with a typed error, and the depth and item limits hold exactly at
//! their boundary.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_c2pa::cbor::{
    decode, CborError, CborFloat, CborValue, ItemBudget, MAX_SUPPORTED_DEPTH,
};
use fixtures::{bytes, encode, map, text};

/// Decode with generous limits.
fn dec(input: &[u8]) -> Result<CborValue, CborError> {
    decode(input, 64, &mut ItemBudget::new(1 << 20))
}

/// Decode hex text.
fn dec_hex(hex: &str) -> Result<CborValue, CborError> {
    dec(&from_hex(hex))
}

/// Parse lowercase hex.
fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Whether `result` is a `Malformed` error.
fn is_malformed(result: &Result<CborValue, CborError>) -> bool {
    matches!(result, Err(CborError::Malformed { .. }))
}

#[test]
fn unsigned_integers_decode_at_every_argument_width() {
    let cases: [(&str, u64); 9] = [
        ("00", 0),
        ("17", 23),
        ("1818", 24),
        ("18ff", 255),
        ("190100", 256),
        ("19ffff", 65_535),
        ("1a00010000", 65_536),
        ("1affffffff", u32::MAX as u64),
        ("1bffffffffffffffff", u64::MAX),
    ];
    for (hex, value) in cases {
        assert_eq!(dec_hex(hex), Ok(CborValue::Unsigned(value)), "{hex}");
    }
}

#[test]
fn negative_integers_hold_the_rfc_argument() {
    assert_eq!(dec_hex("20"), Ok(CborValue::Negative(0)));
    assert_eq!(CborValue::Negative(0).as_i128(), Some(-1));
    assert_eq!(dec_hex("3863"), Ok(CborValue::Negative(99)));
    assert_eq!(CborValue::Negative(99).as_i128(), Some(-100));
    let min = dec_hex("3bffffffffffffffff").expect("decode");
    assert_eq!(min.as_i128(), Some(-(1i128 << 64)));
    assert_eq!(CborValue::Unsigned(7).as_i128(), Some(7));
    assert_eq!(text("x").as_i128(), None);
}

#[test]
fn strings_arrays_maps_and_tags_decode() {
    assert_eq!(dec_hex("4401020304"), Ok(bytes(&[1, 2, 3, 4])));
    assert_eq!(dec_hex("6449455446"), Ok(text("IETF")));
    assert_eq!(dec_hex("62c3bc"), Ok(text("\u{fc}")));
    assert_eq!(
        dec_hex("8301820203820405"),
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(3)]),
            CborValue::Array(vec![CborValue::Unsigned(4), CborValue::Unsigned(5)]),
        ]))
    );
    assert_eq!(
        dec_hex("a26161016162820203"),
        Ok(map(&[
            ("a", CborValue::Unsigned(1)),
            (
                "b",
                CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(3)])
            ),
        ]))
    );
    assert_eq!(
        dec_hex("c11a514b67b0"),
        Ok(CborValue::Tag(
            1,
            Box::new(CborValue::Unsigned(1_363_896_240))
        ))
    );
}

#[test]
fn indefinite_length_items_decode() {
    assert_eq!(dec_hex("5f42010243030405ff"), Ok(bytes(&[1, 2, 3, 4, 5])));
    assert_eq!(dec_hex("7f657374726561646d696e67ff"), Ok(text("streaming")));
    assert_eq!(dec_hex("5fff"), Ok(bytes(&[])));
    assert_eq!(
        dec_hex("9f018202039f0405ffff"),
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(3)]),
            CborValue::Array(vec![CborValue::Unsigned(4), CborValue::Unsigned(5)]),
        ]))
    );
    assert_eq!(
        dec_hex("bf6346756ef563416d7421ff"),
        Ok(map(&[
            ("Fun", CborValue::Bool(true)),
            ("Amt", CborValue::Negative(1)),
        ]))
    );
}

#[test]
fn simple_values_decode() {
    assert_eq!(dec_hex("f4"), Ok(CborValue::Bool(false)));
    assert_eq!(dec_hex("f5"), Ok(CborValue::Bool(true)));
    assert_eq!(dec_hex("f6"), Ok(CborValue::Null));
    assert_eq!(dec_hex("f7"), Ok(CborValue::Undefined));
    assert_eq!(dec_hex("f0"), Ok(CborValue::Simple(16)));
    assert_eq!(dec_hex("f820"), Ok(CborValue::Simple(32)));
    assert_eq!(dec_hex("f8ff"), Ok(CborValue::Simple(255)));
}

#[test]
fn two_byte_simple_values_below_32_are_malformed() {
    for hex in ["f800", "f814", "f81f"] {
        assert!(is_malformed(&dec_hex(hex)), "{hex}");
    }
}

#[test]
fn floats_decode_at_half_single_and_double_precision() {
    let f = |hex: &str| match dec_hex(hex) {
        Ok(CborValue::Float(value)) => value.value(),
        other => panic!("{hex}: {other:?}"),
    };
    assert_eq!(f("f93c00"), 1.0);
    assert_eq!(f("f9c400"), -4.0);
    assert_eq!(f("f97bff"), 65_504.0);
    assert_eq!(f("f90001"), 5.960_464_477_539_063e-8);
    assert_eq!(f("f90400"), 6.103_515_625e-5);
    assert_eq!(f("f97c00"), f64::INFINITY);
    assert_eq!(f("f9fc00"), f64::NEG_INFINITY);
    assert!(f("f97e00").is_nan());
    assert_eq!(f("fa47c35000"), 100_000.0);
    assert_eq!(f("fb3ff199999999999a"), 1.1);
    assert_eq!(f("f98000").to_bits(), (-0.0f64).to_bits());
    assert_eq!(CborFloat::from_f64(2.5).value(), 2.5);
}

#[test]
fn reserved_additional_information_is_malformed_for_every_major_type() {
    for major in 0u8..8 {
        for info in [28u8, 29, 30] {
            let byte = (major << 5) | info;
            assert!(is_malformed(&dec(&[byte])), "{byte:#04x}");
        }
    }
}

#[test]
fn indefinite_length_is_malformed_for_integers_and_tags() {
    for byte in [0x1Fu8, 0x3F, 0xDF] {
        assert!(is_malformed(&dec(&[byte, 0x00])), "{byte:#04x}");
    }
}

#[test]
fn a_break_outside_an_indefinite_item_is_malformed() {
    assert!(is_malformed(&dec_hex("ff")));
    assert!(is_malformed(&dec_hex("81ff")));
    assert!(is_malformed(&dec_hex("a1ff01")));
    assert!(
        is_malformed(&dec_hex("bf01ff")),
        "map break after a key only"
    );
}

#[test]
fn indefinite_string_chunks_must_be_definite_strings_of_the_same_type() {
    assert!(is_malformed(&dec_hex("5f6161ff")), "text chunk in bytes");
    assert!(is_malformed(&dec_hex("7f4161ff")), "bytes chunk in text");
    assert!(is_malformed(&dec_hex("5f5f4101ffff")), "nested indefinite");
    assert!(is_malformed(&dec_hex("5f01ff")), "integer chunk");
}

#[test]
fn invalid_utf8_is_rejected() {
    assert!(matches!(
        dec_hex("62c328"),
        Err(CborError::InvalidUtf8 { .. })
    ));
    assert!(
        matches!(dec_hex("7f61c3ff"), Err(CborError::InvalidUtf8 { .. })),
        "split code point"
    );
    assert!(
        matches!(dec_hex("63eda080"), Err(CborError::InvalidUtf8 { .. })),
        "surrogate"
    );
}

#[test]
fn duplicate_map_keys_are_rejected() {
    assert!(matches!(
        dec_hex("a2616101616102"),
        Err(CborError::DuplicateKey { offset: 0 })
    ));
    // 1 and 0x1801 encode the same value; equality is on decoded values.
    assert!(matches!(
        dec_hex("a2010118010a"),
        Err(CborError::DuplicateKey { .. })
    ));
    assert!(matches!(
        dec_hex("bf0101010aff"),
        Err(CborError::DuplicateKey { .. })
    ));
    assert_eq!(
        dec_hex("a20101200a").map(|v| v.as_map().map(<[_]>::len)),
        Ok(Some(2)),
        "1 and -1 differ"
    );
}

#[test]
fn trailing_bytes_are_rejected() {
    assert_eq!(dec_hex("0000"), Err(CborError::TrailingBytes { offset: 1 }));
}

#[test]
fn empty_input_is_truncated() {
    assert_eq!(dec(&[]), Err(CborError::Truncated { offset: 0 }));
}

#[test]
fn every_prefix_of_a_value_is_rejected_without_panicking() {
    let value = map(&[
        (
            "a",
            CborValue::Array(vec![CborValue::Unsigned(70_000), text("hello")]),
        ),
        ("b", bytes(&[9u8; 40])),
        ("c", CborValue::Tag(24, Box::new(CborValue::Negative(300)))),
    ]);
    let full = encode(&value);
    assert_eq!(dec(&full), Ok(value));
    for len in 0..full.len() {
        assert!(
            matches!(dec(&full[..len]), Err(CborError::Truncated { .. })),
            "prefix {len}"
        );
    }
}

#[test]
fn declared_lengths_longer_than_the_input_fail_without_allocating() {
    let huge = "ffffffffffffffff";
    for major in ["5b", "7b", "9b", "bb"] {
        let input = from_hex(&format!("{major}{huge}00"));
        assert!(
            matches!(dec(&input), Err(CborError::Truncated { .. })),
            "{major}"
        );
    }
    // A 2^32-element array in 6 bytes is rejected before decoding elements.
    assert!(matches!(
        dec_hex("9b00000001000000000101"),
        Err(CborError::Truncated { .. })
    ));
    // A map needs two bytes per pair.
    assert!(matches!(
        dec_hex("a30101010101"),
        Err(CborError::Truncated { .. })
    ));
}

/// `depth` nested one-element arrays around `0`.
fn nested_arrays(depth: usize) -> Vec<u8> {
    let mut out = vec![0x81u8; depth - 1];
    out.push(0x00);
    out
}

#[test]
fn nesting_depth_limit_holds_at_the_boundary() {
    let at_limit = nested_arrays(8);
    assert!(decode(&at_limit, 8, &mut ItemBudget::new(100)).is_ok());
    let over = nested_arrays(9);
    assert_eq!(
        decode(&over, 8, &mut ItemBudget::new(100)),
        Err(CborError::TooDeep { offset: 8, max: 8 })
    );
    assert!(
        decode(&[0x00], 1, &mut ItemBudget::new(1)).is_ok(),
        "a scalar is depth 1"
    );
    assert!(
        matches!(
            decode(&[0xC1, 0x00], 1, &mut ItemBudget::new(9)),
            Err(CborError::TooDeep { .. })
        ),
        "a tag adds a level"
    );
}

#[test]
fn deep_nesting_from_untrusted_input_does_not_overflow_the_stack() {
    let hostile = vec![0x81u8; 1_000_000];
    assert!(matches!(
        dec(&hostile),
        Err(CborError::TooDeep { max: 64, .. })
    ));
}

#[test]
fn a_caller_depth_above_the_supported_ceiling_is_lowered_to_it() {
    let at_ceiling = nested_arrays(MAX_SUPPORTED_DEPTH);
    assert!(decode(&at_ceiling, usize::MAX, &mut ItemBudget::new(1 << 20)).is_ok());
    let hostile = vec![0x81u8; 1_000_000];
    assert_eq!(
        decode(&hostile, usize::MAX, &mut ItemBudget::new(1 << 20)),
        Err(CborError::TooDeep {
            offset: MAX_SUPPORTED_DEPTH,
            max: MAX_SUPPORTED_DEPTH
        })
    );
}

#[test]
fn item_budget_holds_at_the_boundary_and_is_shared() {
    let input = from_hex("83010203");
    let mut exact = ItemBudget::new(4);
    assert!(decode(&input, 8, &mut exact).is_ok());
    assert_eq!(exact.remaining(), 0);
    // The array declares three items when only two remain in the budget,
    // so it fails at its head before any element is decoded.
    assert_eq!(
        decode(&input, 8, &mut ItemBudget::new(3)),
        Err(CborError::TooManyItems { offset: 0, max: 3 })
    );
    let mut shared = ItemBudget::new(6);
    assert!(decode(&input, 8, &mut shared).is_ok());
    assert_eq!(shared.remaining(), 2);
    assert!(matches!(
        decode(&input, 8, &mut shared),
        Err(CborError::TooManyItems { max: 6, .. })
    ));
}

#[test]
fn a_declared_count_beyond_the_item_budget_fails_at_the_container_head() {
    // A million-element array whose elements are all present: it fits the
    // input, so only the item budget can stop it, and it must do so before
    // reserving room for a million values.
    let mut array = from_hex("9a000f4240");
    array.extend(std::iter::repeat_n(0u8, 1_000_000));
    assert_eq!(
        decode(&array, 8, &mut ItemBudget::new(100)),
        Err(CborError::TooManyItems {
            offset: 0,
            max: 100
        })
    );
    // A map declaring 500 000 pairs needs a million items.
    let mut pairs = from_hex("ba0007a120");
    pairs.extend(std::iter::repeat_n(0u8, 1_000_000));
    assert_eq!(
        decode(&pairs, 8, &mut ItemBudget::new(100)),
        Err(CborError::TooManyItems {
            offset: 0,
            max: 100
        })
    );
    // Nested: the inner array's head is where the budget runs out.
    assert_eq!(
        decode(&from_hex("8283010203820405"), 8, &mut ItemBudget::new(4)),
        Err(CborError::TooManyItems { offset: 1, max: 4 })
    );
}

#[test]
fn map_keys_follow_rfc_8949_equivalence() {
    let dup = |hex: &str| matches!(dec_hex(hex), Err(CborError::DuplicateKey { .. }));
    assert!(dup("a2f9000001f9800002"), "0.0 and -0.0 are one key");
    assert!(dup("a2f93c0001fb3ff000000000000002"), "1.0 at two widths");
    assert!(
        dup("a2f97e0001fa7fc0000002"),
        "NaNs with the same significand"
    );
    assert!(
        !dup("a2f97e0001f97e0102"),
        "NaNs with different significands"
    );
    assert!(
        dup("a2a2616101616202 01 a2616202616101 02"
            .replace(' ', "")
            .as_str()),
        "maps with the same pairs in another order"
    );
    assert!(!dup("a201f93c0002"), "an integer and a float differ");
    assert!(
        !dup("a24161616102"),
        "a byte string and a text string differ"
    );
}

#[test]
fn nan_payloads_survive_widening() {
    let bits = |hex: &str| match dec_hex(hex) {
        Ok(CborValue::Float(f)) => f.value().to_bits(),
        other => panic!("{hex}: {other:?}"),
    };
    assert_eq!(bits("f97e01"), 0x7FF8_0400_0000_0000);
    assert_eq!(bits("fa7fc00001"), 0x7FF8_0000_2000_0000);
    assert_eq!(bits("f9fe00") >> 63, 1, "sign kept");
}

#[test]
fn map_lookup_and_accessors() {
    let value = map(&[
        ("t", text("x")),
        ("b", bytes(&[1])),
        ("a", CborValue::Array(vec![])),
    ]);
    assert_eq!(value.get("t").and_then(CborValue::as_text), Some("x"));
    assert_eq!(
        value.get("b").and_then(CborValue::as_bytes),
        Some(&[1u8][..])
    );
    assert_eq!(
        value.get("a").and_then(CborValue::as_array).map(<[_]>::len),
        Some(0)
    );
    assert_eq!(value.get("missing"), None);
    assert_eq!(text("x").get("t"), None);
    assert_eq!(CborValue::Unsigned(1).as_text(), None);
}

#[test]
fn errors_name_the_offset_and_a_category() {
    let err = dec_hex("62c328").expect_err("invalid");
    assert_eq!(err.category(), "invalid_utf8");
    assert!(err.to_string().contains("offset 0"), "{err}");
    assert!(CborError::TooDeep { offset: 0, max: 1 }.is_limit());
    assert!(CborError::TooManyItems { offset: 0, max: 1 }.is_limit());
    assert!(!err.is_limit());
}

#[test]
fn random_input_is_decoded_or_rejected_without_panicking() {
    let mut rng = fixtures::Rng::new(0xC0B0_2002);
    for _ in 0..50_000 {
        let len = rng.below(48);
        let input: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
        let _ = decode(&input, 16, &mut ItemBudget::new(256));
    }
}
