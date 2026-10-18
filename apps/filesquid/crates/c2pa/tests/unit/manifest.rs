//! C2PA-S-001: the manifest store, manifests, claim signatures and
//! assertion stores (C2PA 2.4 sections 11.1.2 to 11.2 and 15.6).

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_c2pa::cbor::{CborError, CborValue};
use filesquid_c2pa::jumbf::{types, BoxType, ByteRange, JumbfError, TypeUuid};
use filesquid_c2pa::manifest::{
    AssertionContent, AssertionError, ClaimVersion, IgnoreReason, Manifest, ManifestError,
    ManifestKind, ManifestStore, ParseLimits, StoreError,
};
use fixtures::*;

/// Parse with the default limits.
fn parse(bytes: Vec<u8>) -> Result<ManifestStore, StoreError> {
    ManifestStore::from_bytes(bytes, &ParseLimits::default())
}

/// Parse a store holding one manifest of `kind` built from `children`.
fn one_manifest(kind: &[u8; 4], children: &[Vec<u8>]) -> ManifestStore {
    parse(store(&[manifest(kind, ACTIVE_LABEL, children)])).expect("store")
}

/// The parsed content of the store's only manifest.
fn content(store: &ManifestStore) -> &Result<Manifest, ManifestError> {
    &store.active_manifest().content
}

/// A standard manifest's children with the claim box replaced.
fn with_claim(claim: Vec<u8>) -> Vec<Vec<u8>> {
    vec![
        assertion_store(&standard_assertions()),
        claim,
        signature_box(),
    ]
}

#[test]
fn sample_store_parses_every_manifest_in_order_and_the_last_is_active() {
    let store = parse(sample_store()).expect("store");
    let labels: Vec<_> = store.manifests().iter().map(|m| m.label.as_str()).collect();
    assert_eq!(labels, [INGREDIENT_LABEL, ACTIVE_LABEL]);
    assert_eq!(store.active_manifest().label, ACTIVE_LABEL);
    assert_eq!(
        store.manifest_by_label(INGREDIENT_LABEL).map(|m| m.kind),
        Some(ManifestKind::Standard)
    );
    assert!(store.manifest_by_label("urn:c2pa:absent").is_none());
    assert!(store.ignored().is_empty());
    assert_eq!(store.root().label(), Some("c2pa"));
    for entry in store.manifests() {
        assert!(entry.content.is_ok(), "{:?}", entry.content);
    }
}

#[test]
fn claim_and_signature_bytes_are_kept_for_signature_validation() {
    let bytes = sample_store();
    let store = parse(bytes).expect("store");
    let manifest = store.active_manifest().content.as_ref().expect("manifest");
    assert_eq!(manifest.claim.version, ClaimVersion::V2);
    assert_eq!(
        store.slice(manifest.claim_cbor).expect("range"),
        &encode(&claim_v2(&STANDARD_LABELS))[..],
        "the signed payload is the claim's CBOR box content"
    );
    assert_eq!(
        store.slice(manifest.signature.cose_sign1).expect("range"),
        &cose_placeholder()[..]
    );
    let sig_box = signature_box();
    assert_eq!(
        store
            .slice(manifest.signature.range.whole())
            .expect("range"),
        &sig_box[..]
    );
}

#[test]
fn slicing_a_range_from_elsewhere_returns_none() {
    let store = parse(sample_store()).expect("store");
    let len = store.bytes().len();
    let outside = ByteRange {
        start: len - 1,
        end: len + 1,
    };
    assert_eq!(store.slice(outside), None);
    let reversed = ByteRange { start: 2, end: 1 };
    assert_eq!(store.slice(reversed), None);
}

#[test]
fn assertions_are_parsed_by_content_type() {
    let store = parse(sample_store()).expect("store");
    let manifest = store.active_manifest().content.as_ref().expect("manifest");
    let labels: Vec<_> = manifest
        .assertions
        .iter()
        .map(|a| a.label.as_str())
        .collect();
    assert_eq!(labels, STANDARD_LABELS);
    let [actions, hash, json, thumb] = &manifest.assertions[..] else {
        panic!("four assertions");
    };
    assert_eq!(actions.content_type, types::CBOR);
    assert_eq!(actions.content, Ok(AssertionContent::Cbor(actions_value())));
    assert_eq!(hash.content, Ok(AssertionContent::Cbor(hash_data_value())));
    assert_eq!(json.content_type, types::JSON);
    assert_eq!(
        json.content,
        Ok(AssertionContent::Json(
            r#"{"@type":"CreativeWork"}"#.to_string()
        ))
    );
    assert_eq!(thumb.content_type, types::EMBEDDED_FILE);
    let Ok(AssertionContent::EmbeddedFile {
        media_type,
        file_name,
        external,
        data,
    }) = &thumb.content
    else {
        panic!("embedded file: {:?}", thumb.content);
    };
    assert_eq!(
        (media_type.as_str(), file_name, *external),
        ("image/jpeg", &None, false)
    );
    assert_eq!(
        store.slice(*data).expect("range"),
        &[0xFF, 0xD8, 0xFF, 0xD9]
    );
}

#[test]
fn assertion_hash_ranges_exclude_the_superbox_header() {
    let store = parse(sample_store()).expect("store");
    let manifest = store.active_manifest().content.as_ref().expect("manifest");
    let actions = &manifest.assertions[0];
    let expected = cbor_assertion("c2pa.actions.v2", &actions_value());
    assert_eq!(
        store.slice(actions.range.whole()).expect("range"),
        &expected[..]
    );
    assert_eq!(
        store.slice(actions.range.payload()).expect("range"),
        &expected[8..]
    );
    assert_eq!(
        store
            .slice(manifest.assertion_store.whole())
            .expect("range")[4..8],
        *b"jumb"
    );
}

#[test]
fn legacy_v1_claims_are_accepted() {
    let store = parse(store(&[legacy_manifest(ACTIVE_LABEL)])).expect("store");
    let manifest = content(&store).as_ref().expect("manifest");
    assert_eq!(manifest.claim.version, ClaimVersion::V1);
    assert_eq!(
        manifest.claim.claim_generator.as_deref(),
        Some("FileSquid-test/0.1")
    );
    assert_eq!(manifest.claim.format.as_deref(), Some("image/jpeg"));
    assert_eq!(manifest.claim.created_assertions.len(), 1);
    assert_eq!(manifest.assertions[0].label, "c2pa.hash.data");
}

#[test]
fn manifest_types_are_recognised() {
    let children = with_claim(claim_box(
        "c2pa.claim.v2",
        &encode(&claim_v2(&STANDARD_LABELS)),
    ));
    assert_eq!(
        one_manifest(b"c2ma", &children).active_manifest().kind,
        ManifestKind::Standard
    );
    assert_eq!(
        one_manifest(b"c2md", &children).active_manifest().kind,
        ManifestKind::Standard
    );
    assert_eq!(
        one_manifest(b"c2um", &children).active_manifest().kind,
        ManifestKind::Update
    );
    let compressed = one_manifest(b"c2cm", &[boxed(b"brob", &[1, 2, 3])]);
    assert_eq!(compressed.active_manifest().kind, ManifestKind::Compressed);
    assert_eq!(
        compressed.active_manifest().content,
        Err(ManifestError::CompressedUnsupported)
    );
    assert_eq!(ManifestError::CompressedUnsupported.status_code(), None);
}

#[test]
fn unknown_and_historical_boxes_are_reported_not_dropped() {
    let unknown = superbox(uuid(b"zzzz"), "vendor.box", &[boxed(b"cbor", &[0])]);
    let timestamp = superbox(uuid(b"c2tm"), "urn:c2pa:ts", &[]);
    let databoxes = superbox(uuid(b"c2db"), "c2pa.databoxes", &[]);
    let mut children = with_claim(claim_box(
        "c2pa.claim.v2",
        &encode(&claim_v2(&STANDARD_LABELS)),
    ));
    children.push(databoxes);
    children.push(boxed(b"free", &[0; 4]));
    let bytes = store(&[
        unknown,
        boxed(b"xml ", b"<x/>"),
        manifest(b"c2ma", ACTIVE_LABEL, &children),
        timestamp,
    ]);
    let store = parse(bytes).expect("store");
    assert_eq!(store.manifests().len(), 1, "c2tm is not a manifest to read");
    let reasons: Vec<_> = store
        .ignored()
        .iter()
        .map(|i| (i.path.as_str(), i.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            ("c2pa/vendor.box", IgnoreReason::UnknownType),
            ("c2pa", IgnoreReason::ContentBox),
            (
                "c2pa/urn:c2pa:9a7b2c6e-5d4f-4e3a-8b1c-2d0e6f5a4b39/c2pa.databoxes",
                IgnoreReason::Historical
            ),
            (
                "c2pa/urn:c2pa:9a7b2c6e-5d4f-4e3a-8b1c-2d0e6f5a4b39",
                IgnoreReason::ContentBox
            ),
            ("c2pa/urn:c2pa:ts", IgnoreReason::Historical),
        ]
    );
    assert_eq!(store.ignored()[0].type_uuid, Some(TypeUuid(uuid(b"zzzz"))));
    assert_eq!(store.ignored()[1].box_type, BoxType(*b"xml "));
    assert_eq!(store.ignored()[1].type_uuid, None);
}

#[test]
fn a_known_box_with_an_unexpected_label_is_reported() {
    let children = vec![
        assertion_store(&standard_assertions()),
        claim_box("c2pa.claim.v3", &encode(&claim_v2(&STANDARD_LABELS))),
        signature_box(),
    ];
    let store = one_manifest(b"c2ma", &children);
    assert_eq!(content(&store), &Err(ManifestError::ClaimMissing));
    assert_eq!(store.ignored()[0].reason, IgnoreReason::UnexpectedLabel);
}

#[test]
fn the_store_label_and_type_are_checked() {
    let wrong_label = superbox(uuid(b"c2pa"), "c2pb", &[standard_manifest(ACTIVE_LABEL)]);
    assert_eq!(
        parse(wrong_label).map(|_| ()),
        Err(StoreError::NotAManifestStore)
    );
    let wrong_type = superbox(uuid(b"c2ma"), "c2pa", &[standard_manifest(ACTIVE_LABEL)]);
    assert_eq!(
        parse(wrong_type).map(|_| ()),
        Err(StoreError::NotAManifestStore)
    );
}

#[test]
fn a_store_without_manifests_is_rejected() {
    assert_eq!(parse(store(&[])).map(|_| ()), Err(StoreError::NoManifests));
    let only_unknown = store(&[superbox(uuid(b"zzzz"), "x", &[])]);
    assert_eq!(
        parse(only_unknown).map(|_| ()),
        Err(StoreError::NoManifests)
    );
}

#[test]
fn duplicate_manifest_labels_are_rejected() {
    let bytes = store(&[
        standard_manifest(ACTIVE_LABEL),
        standard_manifest(ACTIVE_LABEL),
    ]);
    assert!(matches!(
        parse(bytes),
        Err(StoreError::DuplicateLabel { .. })
    ));
}

#[test]
fn recognised_boxes_need_a_label() {
    let mut payload = boxed(b"jumd", &jumd_payload(uuid(b"c2ma"), 0x01, None));
    payload.extend_from_slice(&assertion_store(&standard_assertions()));
    let unlabelled = boxed(b"jumb", &payload);
    assert!(matches!(
        parse(store(&[unlabelled])),
        Err(StoreError::MissingLabel { .. })
    ));
}

#[test]
fn jumbf_framing_errors_reject_the_whole_store() {
    let mut bytes = sample_store();
    bytes.truncate(bytes.len() - 1);
    assert!(matches!(
        parse(bytes),
        Err(StoreError::Jumbf(JumbfError::Truncated { .. }))
    ));
}

#[test]
fn claim_location_errors_map_to_status_codes() {
    let claim = || claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS)));
    let none = one_manifest(
        b"c2ma",
        &[assertion_store(&standard_assertions()), signature_box()],
    );
    assert_eq!(content(&none), &Err(ManifestError::ClaimMissing));
    assert_eq!(
        ManifestError::ClaimMissing.status_code(),
        Some("claim.missing")
    );
    let two = one_manifest(
        b"c2ma",
        &[
            assertion_store(&standard_assertions()),
            claim(),
            claim(),
            signature_box(),
        ],
    );
    assert_eq!(content(&two), &Err(ManifestError::ClaimMultiple));
    assert_eq!(
        ManifestError::ClaimMultiple.status_code(),
        Some("claim.multiple")
    );
    let v1_and_v2 = one_manifest(
        b"c2ma",
        &[
            assertion_store(&standard_assertions()),
            claim(),
            claim_box("c2pa.claim", &encode(&claim_v1(&["c2pa.hash.data"]))),
            signature_box(),
        ],
    );
    assert_eq!(content(&v1_and_v2), &Err(ManifestError::ClaimMultiple));
}

#[test]
fn a_claim_box_must_hold_exactly_one_cbor_box() {
    let cbor = encode(&claim_v2(&STANDARD_LABELS));
    let shapes = [
        superbox(uuid(b"c2cl"), "c2pa.claim.v2", &[]),
        superbox(
            uuid(b"c2cl"),
            "c2pa.claim.v2",
            &[boxed(b"cbor", &cbor), boxed(b"cbor", &cbor)],
        ),
        superbox(uuid(b"c2cl"), "c2pa.claim.v2", &[boxed(b"json", b"{}")]),
    ];
    for shape in shapes {
        let store = one_manifest(b"c2ma", &with_claim(shape));
        let err = content(&store).as_ref().expect_err("claim box");
        assert!(matches!(err, ManifestError::ClaimBox { .. }), "{err:?}");
        assert_eq!(err.status_code(), Some("claim.malformed"));
    }
}

#[test]
fn invalid_claim_cbor_is_claim_cbor_invalid() {
    let store = one_manifest(
        b"c2ma",
        &with_claim(claim_box("c2pa.claim.v2", &[0x62, 0xC3, 0x28])),
    );
    let err = content(&store).as_ref().expect_err("claim cbor");
    assert!(
        matches!(err, ManifestError::ClaimCbor(CborError::InvalidUtf8 { .. })),
        "{err:?}"
    );
    assert_eq!(err.status_code(), Some("claim.cbor.invalid"));
}

#[test]
fn signature_location_errors_are_claim_signature_missing() {
    let claim = || claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS)));
    let missing = one_manifest(b"c2ma", &[assertion_store(&standard_assertions()), claim()]);
    let two = one_manifest(
        b"c2ma",
        &[
            assertion_store(&standard_assertions()),
            claim(),
            signature_box(),
            signature_box(),
        ],
    );
    let empty = one_manifest(
        b"c2ma",
        &[
            assertion_store(&standard_assertions()),
            claim(),
            superbox(uuid(b"c2cs"), "c2pa.signature", &[]),
        ],
    );
    let expected = [
        ManifestError::SignatureMissing,
        ManifestError::SignatureMultiple,
        ManifestError::SignatureBox {
            reason: "a claim signature box is requestable and holds exactly one cbor box",
        },
    ];
    for (store, want) in [missing, two, empty].iter().zip(expected) {
        assert_eq!(content(store), &Err(want.clone()));
        assert_eq!(want.status_code(), Some("claimSignature.missing"));
    }
}

#[test]
fn assertion_store_errors_are_assertion_missing() {
    let claim = || claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS)));
    let missing = one_manifest(b"c2ma", &[claim(), signature_box()]);
    let empty = one_manifest(b"c2ma", &[assertion_store(&[]), claim(), signature_box()]);
    let two = one_manifest(
        b"c2ma",
        &[
            assertion_store(&standard_assertions()),
            assertion_store(&standard_assertions()),
            claim(),
            signature_box(),
        ],
    );
    let expected = [
        ManifestError::AssertionStoreMissing,
        ManifestError::AssertionStoreEmpty,
        ManifestError::AssertionStoreMultiple,
    ];
    for (store, want) in [missing, empty, two].iter().zip(expected) {
        assert_eq!(content(store), &Err(want.clone()));
        assert_eq!(want.status_code(), Some("assertion.missing"));
    }
}

#[test]
fn duplicate_assertion_labels_reject_the_manifest() {
    let a = cbor_assertion("c2pa.actions.v2", &actions_value());
    let store = one_manifest(
        b"c2ma",
        &[
            assertion_store(&[a.clone(), a]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["c2pa.actions.v2"]))),
            signature_box(),
        ],
    );
    let err = content(&store).as_ref().expect_err("duplicate");
    assert!(
        matches!(err, ManifestError::DuplicateAssertionLabel { .. }),
        "{err:?}"
    );
    assert_eq!(err.status_code(), None);
}

/// Parse one manifest whose assertion store holds `assertion` and return
/// that assertion's content.
fn assertion_content(assertion: Vec<u8>) -> Result<AssertionContent, AssertionError> {
    let store = one_manifest(
        b"c2ma",
        &[
            assertion_store(&[assertion]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["a"]))),
            signature_box(),
        ],
    );
    let manifest = content(&store).as_ref().expect("manifest");
    manifest.assertions[0].content.clone()
}

#[test]
fn invalid_assertion_content_is_reported_per_assertion() {
    let bad_cbor = superbox(uuid(b"cbor"), "a", &[boxed(b"cbor", &[0xFF])]);
    let err = assertion_content(bad_cbor).expect_err("cbor");
    assert!(
        matches!(err, AssertionError::Cbor(CborError::Malformed { .. })),
        "{err:?}"
    );
    assert_eq!(err.status_code(), Some("assertion.cbor.invalid"));
    let two_boxes = superbox(
        uuid(b"cbor"),
        "a",
        &[boxed(b"cbor", &[0]), boxed(b"cbor", &[0])],
    );
    let err = assertion_content(two_boxes).expect_err("boxes");
    assert!(matches!(err, AssertionError::CborBoxes), "{err:?}");
    assert_eq!(err.status_code(), Some("assertion.cbor.invalid"));
    let bad_json = superbox(uuid(b"json"), "a", &[boxed(b"json", b"{\"a\":}")]);
    let err = assertion_content(bad_json).expect_err("json");
    assert_eq!(err, AssertionError::Json);
    assert_eq!(err.status_code(), Some("assertion.json.invalid"));
    let not_utf8 = superbox(
        uuid(b"json"),
        "a",
        &[boxed(b"json", &[b'"', 0xC3, 0x28, b'"'])],
    );
    assert_eq!(assertion_content(not_utf8), Err(AssertionError::Json));
    let no_json_box = superbox(uuid(b"json"), "a", &[]);
    assert_eq!(
        assertion_content(no_json_box),
        Err(AssertionError::JsonBoxes)
    );
}

#[test]
fn one_bad_assertion_does_not_hide_the_others() {
    let store = one_manifest(
        b"c2ma",
        &[
            assertion_store(&[
                superbox(uuid(b"cbor"), "bad", &[boxed(b"cbor", &[0xFF])]),
                cbor_assertion("good", &actions_value()),
            ]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["good"]))),
            signature_box(),
        ],
    );
    let manifest = content(&store).as_ref().expect("manifest");
    assert!(manifest.assertions[0].content.is_err());
    assert_eq!(
        manifest.assertions[1].content,
        Ok(AssertionContent::Cbor(actions_value()))
    );
}

#[test]
fn padding_boxes_are_allowed_in_assertions() {
    let padded = superbox(
        uuid(b"cbor"),
        "a",
        &[boxed(b"cbor", &[0x01]), boxed(b"free", &[0; 8])],
    );
    assert_eq!(
        assertion_content(padded),
        Ok(AssertionContent::Cbor(CborValue::Unsigned(1)))
    );
}

/// Nested JSON arrays `depth` deep.
fn json_nested(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

#[test]
fn deeply_nested_json_is_checked_without_recursion() {
    // JSON is only checked for well-formedness, iteratively, so nesting
    // costs no stack; the box size bounds the work.
    let deep = superbox(
        uuid(b"json"),
        "a",
        &[boxed(b"json", json_nested(200_000).as_bytes())],
    );
    assert!(matches!(
        assertion_content(deep),
        Ok(AssertionContent::Json(_))
    ));
    let unbalanced = format!("{}{}", "[".repeat(200_000), "]".repeat(199_999));
    let bad = superbox(uuid(b"json"), "a", &[boxed(b"json", unbalanced.as_bytes())]);
    assert_eq!(assertion_content(bad), Err(AssertionError::Json));
}

#[test]
fn embedded_files_need_a_description_and_a_data_box() {
    let named = embedded_assertion("a", "image/png", Some("thumb.png"), &[1, 2]);
    let Ok(AssertionContent::EmbeddedFile {
        media_type,
        file_name,
        ..
    }) = assertion_content(named)
    else {
        panic!("embedded file");
    };
    assert_eq!(
        (media_type.as_str(), file_name.as_deref()),
        ("image/png", Some("thumb.png"))
    );
    let shapes = [
        superbox(EMBEDDED_FILE_UUID, "a", &[boxed(b"bfdb", b"\0image/png\0")]),
        superbox(EMBEDDED_FILE_UUID, "a", &[boxed(b"bidb", &[1])]),
        superbox(
            EMBEDDED_FILE_UUID,
            "a",
            &[boxed(b"bfdb", b"\0image/png"), boxed(b"bidb", &[1])],
        ),
        superbox(
            EMBEDDED_FILE_UUID,
            "a",
            &[boxed(b"bfdb", b"\x04image/png\0"), boxed(b"bidb", &[1])],
        ),
        superbox(
            EMBEDDED_FILE_UUID,
            "a",
            &[boxed(b"bfdb", b"\x01image/png\0"), boxed(b"bidb", &[1])],
        ),
        superbox(
            EMBEDDED_FILE_UUID,
            "a",
            &[boxed(b"bfdb", b"\0image/png\0x"), boxed(b"bidb", &[1])],
        ),
    ];
    for shape in shapes {
        let err = assertion_content(shape).expect_err("embedded");
        assert!(
            matches!(err, AssertionError::EmbeddedFile { .. }),
            "{err:?}"
        );
        assert_eq!(err.status_code(), None);
    }
    let external = superbox(
        EMBEDDED_FILE_UUID,
        "a",
        &[
            boxed(b"bfdb", b"\x02image/png\0"),
            boxed(b"bidb", b"https://example.org/a.png"),
        ],
    );
    assert!(matches!(
        assertion_content(external),
        Ok(AssertionContent::EmbeddedFile { external: true, .. })
    ));
}

#[test]
fn uuid_and_other_content_types_are_kept() {
    let mut payload = [0x11u8; 16].to_vec();
    payload.extend_from_slice(b"opaque");
    let custom = superbox(uuid(b"uuid"), "a", &[boxed(b"uuid", &payload)]);
    let Ok(AssertionContent::Uuid { uuid: id, .. }) = assertion_content(custom) else {
        panic!("uuid content");
    };
    assert_eq!(id, TypeUuid([0x11; 16]));
    let short = superbox(uuid(b"uuid"), "a", &[boxed(b"uuid", &[0; 15])]);
    assert_eq!(assertion_content(short), Err(AssertionError::UuidBox));
    let other = superbox(uuid(b"xml "), "a", &[boxed(b"xml ", b"<x/>")]);
    assert_eq!(assertion_content(other), Ok(AssertionContent::Other));
}

#[test]
fn one_bad_ingredient_manifest_does_not_hide_the_active_manifest() {
    // Validation locates the claim first (C2PA 2.4 section 15.6), so a
    // manifest with only a signature reports the missing claim.
    let broken = manifest(b"c2ma", INGREDIENT_LABEL, &[signature_box()]);
    let store = parse(store(&[broken, standard_manifest(ACTIVE_LABEL)])).expect("store");
    assert_eq!(
        store.manifests()[0].content,
        Err(ManifestError::ClaimMissing)
    );
    assert!(store.active_manifest().content.is_ok());
}

#[test]
fn box_depth_limit_holds_at_the_boundary() {
    let limits = |max_box_depth| ParseLimits {
        max_box_depth,
        ..ParseLimits::default()
    };
    assert!(ManifestStore::from_bytes(sample_store(), &limits(4)).is_ok());
    assert!(matches!(
        ManifestStore::from_bytes(sample_store(), &limits(3)),
        Err(StoreError::Jumbf(JumbfError::TooDeep { max: 3, .. }))
    ));
}

#[test]
fn cbor_item_budget_is_shared_across_the_store_and_holds_at_the_boundary() {
    let needed = 2 * standard_manifest_items();
    let limits = |max_cbor_items| ParseLimits {
        max_cbor_items,
        ..ParseLimits::default()
    };
    assert!(ManifestStore::from_bytes(sample_store(), &limits(needed)).is_ok());
    assert!(matches!(
        ManifestStore::from_bytes(sample_store(), &limits(needed - 1)),
        Err(StoreError::CborLimit(CborError::TooManyItems { .. }))
    ));
}

#[test]
fn cbor_depth_limit_rejects_the_whole_store() {
    let deep = superbox(
        uuid(b"cbor"),
        "a",
        &[boxed(b"cbor", &[0x81, 0x81, 0x81, 0x00])],
    );
    let bytes = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[deep]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["a"]))),
            signature_box(),
        ],
    )]);
    let limits = |max_cbor_depth| ParseLimits {
        max_cbor_depth,
        ..ParseLimits::default()
    };
    assert!(ManifestStore::from_bytes(bytes.clone(), &limits(4)).is_ok());
    assert!(matches!(
        ManifestStore::from_bytes(bytes, &limits(3)),
        Err(StoreError::CborLimit(CborError::TooDeep { max: 3, .. }))
    ));
}

#[test]
fn every_prefix_and_byte_corruption_is_handled_without_panicking() {
    let bytes = sample_store();
    for len in 0..bytes.len() {
        assert!(parse(bytes[..len].to_vec()).is_err(), "prefix {len}");
    }
    for at in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[at] ^= 0xFF;
        let _ = parse(mutated);
    }
}

#[test]
fn error_display_escapes_untrusted_labels() {
    let hostile = "urn:c2pa:\u{202e}gpj.exe\u{2066}";
    let bytes = store(&[standard_manifest(hostile), standard_manifest(hostile)]);
    let err = parse(bytes).expect_err("duplicate");
    let shown = err.to_string();
    assert!(
        !shown.contains('\u{202e}') && !shown.contains('\u{2066}'),
        "{shown}"
    );
    assert!(shown.contains("\\u{202e}"), "{shown}");
    let long = format!("urn:c2pa:{}", "x".repeat(500));
    let err = parse(store(&[standard_manifest(&long), standard_manifest(&long)])).expect_err("dup");
    assert!(
        err.to_string().len() < 200,
        "labels are truncated in errors"
    );
    assert_eq!(err.category(), "duplicate_label");
}

#[test]
fn default_limits_are_documented_values() {
    let limits = ParseLimits::default();
    assert_eq!(limits.max_store_bytes, 64 * 1024 * 1024);
    assert_eq!(limits.max_box_depth, 16);
    assert_eq!(limits.max_boxes, 65_536);
    assert_eq!(limits.max_cbor_depth, 64);
    assert_eq!(limits.max_cbor_items, 1_048_576);
}

#[test]
fn random_multi_byte_corruption_is_handled_without_panicking() {
    let originals = [sample_store(), store(&[legacy_manifest(ACTIVE_LABEL)])];
    let mut rng = Rng::new(0x00C2_9A00_0002);
    for round in 0..4_000 {
        let mut bytes = originals[round % 2].clone();
        for _ in 0..1 + rng.below(8) {
            let at = rng.below(bytes.len());
            bytes[at] = rng.next_u64() as u8;
        }
        if rng.below(4) == 0 {
            bytes.truncate(rng.below(bytes.len()));
        }
        let _ = parse(bytes);
    }
}

/// Label-only toggles: present but not requestable.
const NOT_REQUESTABLE: u8 = 0x02;

#[test]
fn c2pa_boxes_must_be_requestable() {
    let claim = || claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS)));
    let cbor_claim = encode(&claim_v2(&STANDARD_LABELS));
    let store_box = assertion_store(&standard_assertions());
    let cases = [
        (
            vec![
                store_box.clone(),
                superbox_toggled(
                    uuid(b"c2cl"),
                    "c2pa.claim.v2",
                    NOT_REQUESTABLE,
                    &[boxed(b"cbor", &cbor_claim)],
                ),
                signature_box(),
            ],
            "claim.malformed",
        ),
        (
            vec![
                store_box.clone(),
                claim(),
                superbox_toggled(
                    uuid(b"c2cs"),
                    "c2pa.signature",
                    NOT_REQUESTABLE,
                    &[boxed(b"cbor", &cose_placeholder())],
                ),
            ],
            "claimSignature.missing",
        ),
        (
            vec![
                superbox_toggled(
                    uuid(b"c2as"),
                    "c2pa.assertions",
                    NOT_REQUESTABLE,
                    &standard_assertions(),
                ),
                claim(),
                signature_box(),
            ],
            "assertion.missing",
        ),
    ];
    for (children, code) in cases {
        let store = one_manifest(b"c2ma", &children);
        let err = content(&store).as_ref().expect_err("not requestable");
        assert_eq!(err.status_code(), Some(code), "{err:?}");
    }
}

#[test]
fn a_manifest_box_must_be_requestable() {
    let children = with_claim(claim_box(
        "c2pa.claim.v2",
        &encode(&claim_v2(&STANDARD_LABELS)),
    ));
    let unrequestable = superbox_toggled(uuid(b"c2ma"), ACTIVE_LABEL, NOT_REQUESTABLE, &children);
    let store = parse(store(&[unrequestable])).expect("store");
    let err = content(&store).as_ref().expect_err("not requestable");
    assert_eq!(err, &ManifestError::NotRequestable);
    assert_eq!(err.status_code(), Some("general.error"));
}

#[test]
fn an_assertion_box_must_be_requestable() {
    let hidden = superbox_toggled(
        uuid(b"cbor"),
        "a",
        NOT_REQUESTABLE,
        &[boxed(b"cbor", &encode(&actions_value()))],
    );
    let err = assertion_content(hidden).expect_err("not requestable");
    assert_eq!(err, AssertionError::NotRequestable);
    assert_eq!(err.status_code(), Some("assertion.missing"));
}

#[test]
fn the_store_box_must_be_requestable() {
    let bytes = superbox_toggled(
        uuid(b"c2pa"),
        "c2pa",
        NOT_REQUESTABLE,
        &[standard_manifest(ACTIVE_LABEL)],
    );
    assert_eq!(parse(bytes).map(|_| ()), Err(StoreError::NotAManifestStore));
}

#[test]
fn from_bytes_enforces_the_store_size_limit() {
    let bytes = sample_store();
    let size = bytes.len() as u64;
    let limits = |max_store_bytes| ParseLimits {
        max_store_bytes,
        ..ParseLimits::default()
    };
    assert!(ManifestStore::from_bytes(bytes.clone(), &limits(size)).is_ok());
    let err = ManifestStore::from_bytes(bytes, &limits(size - 1)).expect_err("too large");
    assert_eq!(
        err,
        StoreError::TooLarge {
            size,
            max: size - 1
        }
    );
    assert_eq!(err.category(), "too_large");
}
