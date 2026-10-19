//! PROV-S-002 / PROV-S-003: identifier forms, percent-encoding, UUIDs,
//! SHA-256 digests and RFC 3339 UTC timestamps.

use filesquid_prov::{
    activity_id, entity_id, percent_encode, InstallationId, Sha256Digest, Timestamp,
};

#[test]
fn unreserved_characters_stay_as_they_are() {
    let unreserved = "AZaz09-._~";
    assert_eq!(percent_encode(unreserved), unreserved);
}

#[test]
fn every_other_byte_is_upper_case_percent_encoded_utf8() {
    assert_eq!(percent_encode("a b/c"), "a%20b%2Fc");
    assert_eq!(percent_encode("é"), "%C3%A9");
    assert_eq!(
        percent_encode("%:?#[]@!$&'()*+,;="),
        "%25%3A%3F%23%5B%5D%40%21%24%26%27%28%29%2A%2B%2C%3B%3D"
    );
    assert_eq!(percent_encode("😀"), "%F0%9F%98%80");
}

#[test]
fn activity_identifier_form() {
    assert_eq!(
        activity_id("op-1", "urn:uuid:abc", "v2"),
        "urn:filesquid:activity:op-1:urn:uuid:abc:v2"
    );
}

#[test]
fn activity_identifiers_are_injective_when_parts_hold_colons() {
    let split_one = activity_id("a:b", "c", "v1");
    let split_two = activity_id("a", "b:c", "v1");
    assert_ne!(split_one, split_two);
    assert_eq!(split_one, "urn:filesquid:activity:a%3Ab:c:v1");
    assert_eq!(split_two, "urn:filesquid:activity:a:b:c:v1");
}

#[test]
fn entity_identifier_form_encodes_the_logical_path() {
    assert_eq!(
        entity_id("urn:uuid:abc", "v2", "Fotos/été 1.jpg"),
        "urn:filesquid:entity:urn:uuid:abc:v2:Fotos%2F%C3%A9t%C3%A9%201.jpg"
    );
}

#[test]
fn installation_ids_are_uuid_v4_rendered_as_urn_uuid() {
    let id = InstallationId::parse("0F8E2B9A-4C1D-4E7F-9A3B-5D6C7E8F9A0B").expect("v4");
    assert_eq!(id.as_str(), "0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b");
    assert_eq!(id.urn(), "urn:uuid:0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b");
    for bad in [
        "",
        "0f8e2b9a-4c1d-1e7f-9a3b-5d6c7e8f9a0b",
        "0f8e2b9a-4c1d-4e7f-7a3b-5d6c7e8f9a0b",
        "0f8e2b9a4c1d4e7f9a3b5d6c7e8f9a0b",
        "0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0g",
        "urn:uuid:0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b",
    ] {
        assert!(InstallationId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn sha256_digests_are_64_hex_characters_in_lower_case() {
    let digest = Sha256Digest::parse(&"AB".repeat(32)).expect("digest");
    assert_eq!(digest.as_str(), "ab".repeat(32));
    assert!(Sha256Digest::parse(&"a".repeat(63)).is_err());
    assert!(Sha256Digest::parse(&"g".repeat(64)).is_err());
}

#[test]
fn timestamps_must_be_rfc3339_utc() {
    for good in [
        "2026-10-19T10:00:00Z",
        "2026-10-19T10:00:00.123456789Z",
        "2024-02-29T23:59:60Z",
    ] {
        assert_eq!(Timestamp::parse(good).expect(good).as_str(), good);
    }
    for bad in [
        "2026-10-19T10:00:00+00:00",
        "2026-10-19 10:00:00Z",
        "2026-10-19t10:00:00z",
        "2026-13-01T00:00:00Z",
        "2025-02-29T00:00:00Z",
        "2026-10-19T24:00:00Z",
        "2026-10-19T10:00:00.Z",
        "2026-10-19T10:00:00.1234567891Z",
        "2026-10-19",
        "2026-10-19T10:00:60Z",
        "2026-10-19T23:58:60Z",
        "2026-10-19T22:59:60Z",
    ] {
        assert!(Timestamp::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_long_all_digit_fraction_is_refused_without_panicking() {
    for digits in [10, 11, 20, 64] {
        let text = format!("2026-10-19T10:00:00.{}Z", "9".repeat(digits));
        assert!(Timestamp::parse(&text).is_err(), "{digits} digits");
    }
}

#[test]
fn a_leap_second_is_accepted_only_at_the_end_of_a_utc_day() {
    for good in ["2016-12-31T23:59:60Z", "2016-12-31T23:59:60.5Z"] {
        assert!(Timestamp::parse(good).is_ok(), "{good}");
    }
}

#[test]
fn timestamps_order_by_instant_not_by_text() {
    let whole = Timestamp::parse("2026-10-19T10:00:00Z").expect("whole");
    let half = Timestamp::parse("2026-10-19T10:00:00.5Z").expect("half");
    let next = Timestamp::parse("2026-10-19T10:00:01Z").expect("next");
    assert!(whole < half && half < next);
    let padded = Timestamp::parse("2026-10-19T10:00:00.900Z").expect("padded");
    assert!(half < padded && padded < next);
}
