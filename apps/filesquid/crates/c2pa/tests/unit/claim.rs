//! C2PA-S-001: claim fields for `c2pa.claim.v2` and the legacy `c2pa.claim`
//! (C2PA 2.4 sections 10.2 and 15.6.2).

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_c2pa::cbor::CborValue;
use filesquid_c2pa::manifest::{
    Claim, ClaimError, ClaimVersion, ManifestError, ManifestStore, ParseLimits,
};
use fixtures::*;

/// `value` with `key` removed (it must be a text-keyed map).
fn without(value: &CborValue, key: &str) -> CborValue {
    let CborValue::Map(pairs) = value else {
        panic!("map");
    };
    CborValue::Map(
        pairs
            .iter()
            .filter(|(k, _)| k.as_text() != Some(key))
            .cloned()
            .collect(),
    )
}

/// `value` with `key` set to `new` (appended when absent).
fn with(value: &CborValue, key: &str, new: CborValue) -> CborValue {
    let mut out = without(value, key);
    if let CborValue::Map(pairs) = &mut out {
        pairs.push((text(key), new));
    }
    out
}

/// The field named by a claim error.
fn failing_field(version: ClaimVersion, value: CborValue) -> &'static str {
    Claim::from_cbor(version, value)
        .expect_err("malformed")
        .field
}

#[test]
fn v2_claim_fields_are_read() {
    let claim = Claim::from_cbor(ClaimVersion::V2, claim_v2(&STANDARD_LABELS)).expect("claim");
    assert_eq!(claim.version, ClaimVersion::V2);
    assert_eq!(
        claim.instance_id,
        "xmp:iid:5c2f0b2b-0c9c-4b8b-9a28-6c3e4e2c0f11"
    );
    assert_eq!(claim.signature, "self#jumbf=c2pa.signature");
    assert_eq!(claim.created_assertions.len(), 4);
    assert_eq!(
        claim.created_assertions[0].url,
        "self#jumbf=c2pa.assertions/c2pa.actions.v2"
    );
    assert_eq!(claim.created_assertions[0].hash, vec![7u8; 32]);
    assert_eq!(claim.created_assertions[0].alg, None);
    assert!(claim.gathered_assertions.is_empty());
    assert!(claim.redacted_assertions.is_empty());
    assert_eq!(claim.claim_generator_info.len(), 1);
    assert_eq!(claim.claim_generator_info[0].name, "FileSquid test");
    assert_eq!(
        claim.claim_generator_info[0].version.as_deref(),
        Some("0.2")
    );
    assert_eq!(claim.alg.as_deref(), Some("sha256"));
    assert_eq!(claim.claim_generator, None);
    assert_eq!(claim.format, None);
    assert_eq!(claim.title, None);
    assert_eq!(claim.map, claim_v2(&STANDARD_LABELS));
}

#[test]
fn v2_optional_fields_are_read() {
    let value = with(
        &with(
            &with(
                &claim_v2(&["a"]),
                "gathered_assertions",
                CborValue::Array(vec![hashed_uri("self#jumbf=c2pa.assertions/b", &[1])]),
            ),
            "redacted_assertions",
            CborValue::Array(vec![text("self#jumbf=/c2pa/urn:c2pa:x/c2pa.assertions/c")]),
        ),
        "dc:title",
        text("photo.jpg"),
    );
    let value = with(
        &with(&value, "alg_soft", text("phash")),
        "specVersion",
        text("2.4.0"),
    );
    let claim = Claim::from_cbor(ClaimVersion::V2, value).expect("claim");
    assert_eq!(
        claim.gathered_assertions[0].url,
        "self#jumbf=c2pa.assertions/b"
    );
    assert_eq!(
        claim.redacted_assertions,
        ["self#jumbf=/c2pa/urn:c2pa:x/c2pa.assertions/c"]
    );
    assert_eq!(claim.title.as_deref(), Some("photo.jpg"));
    assert_eq!(claim.alg_soft.as_deref(), Some("phash"));
}

#[test]
fn every_required_v2_field_is_enforced() {
    for field in [
        "instanceID",
        "claim_generator_info",
        "signature",
        "created_assertions",
    ] {
        let value = without(&claim_v2(&["a"]), field);
        assert_eq!(failing_field(ClaimVersion::V2, value), field);
    }
}

#[test]
fn v2_field_types_and_sizes_are_enforced() {
    let base = claim_v2(&["a"]);
    let cases = [
        ("instanceID", text("")),
        ("instanceID", CborValue::Unsigned(1)),
        ("signature", bytes(b"self#jumbf=c2pa.signature")),
        ("created_assertions", CborValue::Array(vec![])),
        ("created_assertions", text("x")),
        ("gathered_assertions", CborValue::Array(vec![])),
        (
            "redacted_assertions",
            CborValue::Array(vec![CborValue::Unsigned(1)]),
        ),
        ("redacted_assertions", CborValue::Array(vec![])),
        ("dc:title", text("")),
        ("alg", text("")),
        ("alg_soft", CborValue::Null),
        ("specVersion", CborValue::Unsigned(2)),
        (
            "claim_generator_info",
            CborValue::Array(vec![map(&[("name", text("x"))])]),
        ),
        ("claim_generator_info", map(&[("version", text("1"))])),
        ("claim_generator_info", map(&[("name", text(""))])),
        (
            "claim_generator_info",
            map(&[("name", text("x")), ("version", CborValue::Unsigned(1))]),
        ),
    ];
    for (field, value) in cases {
        let claim = with(&base, field, value.clone());
        assert_eq!(
            failing_field(ClaimVersion::V2, claim),
            field,
            "{field} = {value:?}"
        );
    }
}

#[test]
fn hashed_uris_need_a_url_and_a_hash() {
    let base = claim_v2(&["a"]);
    let bad = [
        map(&[("hash", bytes(&[1]))]),
        map(&[("url", text("self#jumbf=c2pa.assertions/a"))]),
        map(&[("url", text("u")), ("hash", text("not bytes"))]),
        map(&[("url", CborValue::Unsigned(1)), ("hash", bytes(&[1]))]),
        map(&[("url", text("u")), ("hash", bytes(&[1])), ("alg", text(""))]),
        text("not a map"),
    ];
    for uri in bad {
        let claim = with(
            &base,
            "created_assertions",
            CborValue::Array(vec![uri.clone()]),
        );
        assert_eq!(
            failing_field(ClaimVersion::V2, claim),
            "created_assertions",
            "{uri:?}"
        );
    }
    let with_alg = map(&[
        ("url", text("u")),
        ("alg", text("sha384")),
        ("hash", bytes(&[1])),
    ]);
    let claim = with(
        &base,
        "created_assertions",
        CborValue::Array(vec![with_alg]),
    );
    let parsed = Claim::from_cbor(ClaimVersion::V2, claim).expect("claim");
    assert_eq!(parsed.created_assertions[0].alg.as_deref(), Some("sha384"));
}

#[test]
fn a_claim_must_be_a_map_with_text_keys() {
    let err = Claim::from_cbor(ClaimVersion::V2, CborValue::Array(vec![])).expect_err("not a map");
    assert_eq!(err.field, "claim");
    let mut value = claim_v2(&["a"]);
    if let CborValue::Map(pairs) = &mut value {
        pairs.push((CborValue::Unsigned(1), CborValue::Null));
    }
    assert_eq!(failing_field(ClaimVersion::V2, value), "claim");
}

#[test]
fn unknown_claim_fields_are_kept_in_the_map() {
    let value = with(&claim_v2(&["a"]), "vendor.extra", text("kept"));
    let claim = Claim::from_cbor(ClaimVersion::V2, value).expect("claim");
    assert_eq!(
        claim.map.get("vendor.extra").and_then(CborValue::as_text),
        Some("kept")
    );
}

#[test]
fn v1_claim_fields_are_read() {
    let claim = Claim::from_cbor(ClaimVersion::V1, claim_v1(&["a", "b"])).expect("claim");
    assert_eq!(claim.version, ClaimVersion::V1);
    assert_eq!(claim.claim_generator.as_deref(), Some("FileSquid-test/0.1"));
    assert_eq!(claim.format.as_deref(), Some("image/jpeg"));
    assert_eq!(
        claim.created_assertions.len(),
        2,
        "v1 `assertions` are the created assertions"
    );
    assert!(
        claim.claim_generator_info.is_empty(),
        "optional in legacy claims"
    );
    let info = CborValue::Array(vec![map(&[("name", text("Tool"))])]);
    let claim = Claim::from_cbor(
        ClaimVersion::V1,
        with(&claim_v1(&["a"]), "claim_generator_info", info),
    )
    .expect("claim");
    assert_eq!(claim.claim_generator_info[0].name, "Tool");
}

#[test]
fn every_required_v1_field_is_enforced() {
    for field in [
        "claim_generator",
        "signature",
        "assertions",
        "dc:format",
        "instanceID",
    ] {
        let value = without(&claim_v1(&["a"]), field);
        assert_eq!(failing_field(ClaimVersion::V1, value), field);
    }
    let empty_info = with(
        &claim_v1(&["a"]),
        "claim_generator_info",
        CborValue::Array(vec![]),
    );
    assert_eq!(
        failing_field(ClaimVersion::V1, empty_info),
        "claim_generator_info"
    );
}

#[test]
fn malformed_claims_are_claim_malformed_in_the_store() {
    let claim = encode(&without(&claim_v2(&STANDARD_LABELS), "instanceID"));
    let bytes = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&standard_assertions()),
            claim_box("c2pa.claim.v2", &claim),
            signature_box(),
        ],
    )]);
    let store = ManifestStore::from_bytes(bytes, &ParseLimits::default()).expect("store");
    let err = store
        .active_manifest()
        .content
        .as_ref()
        .expect_err("malformed");
    assert_eq!(
        err,
        &ManifestError::ClaimMalformed(ClaimError {
            field: "instanceID",
            reason: "required text field is missing"
        })
    );
    assert_eq!(err.status_code(), Some("claim.malformed"));
}
