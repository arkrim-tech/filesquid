//! PROV-S-004: parsing refuses oversize, deep, duplicate-key, unknown-profile
//! and profile-breaking records with fixed codes that name no stored value.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_prov::{
    build_record, parse_record, to_json_ld, ProvError, MAX_DEPTH, MAX_RECORD_BYTES,
};
use fixtures::*;
use serde_json::{json, Value};

/// Marker planted in tampered values; no error message may repeat it.
const SECRET: &str = "SECRET-7731";

/// Serialized standard record.
fn standard_text() -> String {
    to_json_ld(&build_record(&standard_facts()).expect("build"))
}

/// Standard record as a mutable JSON value.
fn standard_value() -> Value {
    serde_json::from_str(&standard_text()).expect("json")
}

/// Parse `text`, expect a refusal with `code`, and check the message.
fn refused(text: &str, code: &str) -> ProvError {
    let err = parse_record(text.as_bytes()).expect_err("must be refused");
    assert_eq!(err.code(), code, "{err}");
    assert!(err.to_string().starts_with(code), "{err}");
    assert!(!err.to_string().contains(SECRET), "{err}");
    err
}

/// Parse a tampered JSON value and expect `PROV_INVALID_NODE`.
fn invalid_node(doc: &Value) {
    refused(
        &serde_json::to_string(doc).expect("text"),
        "PROV_INVALID_NODE",
    );
}

#[test]
fn bounds_are_one_mebibyte_and_sixteen_levels() {
    assert_eq!(MAX_RECORD_BYTES, 1_048_576);
    assert_eq!(MAX_DEPTH, 16);
}

#[test]
fn a_record_of_exactly_one_mebibyte_is_read() {
    let mut text = standard_text();
    text.push_str(&" ".repeat(MAX_RECORD_BYTES - text.len()));
    assert_eq!(text.len(), MAX_RECORD_BYTES);
    parse_record(text.as_bytes()).expect("at the limit");
}

#[test]
fn a_record_above_one_mebibyte_is_refused() {
    let mut text = standard_text();
    text.push_str(&" ".repeat(MAX_RECORD_BYTES + 1 - text.len()));
    refused(&text, "PROV_TOO_LARGE");
}

#[test]
fn nesting_deeper_than_sixteen_is_refused() {
    let deep = format!("{}{}", "[".repeat(17), "]".repeat(17));
    refused(&deep, "PROV_TOO_DEEP");
    let mut doc = standard_value();
    let mut nested = json!(SECRET);
    for _ in 0..14 {
        nested = json!({ "k": nested });
    }
    doc["@graph"][0]["fsq:kind"] = nested;
    refused(&serde_json::to_string(&doc).unwrap(), "PROV_TOO_DEEP");
}

#[test]
fn nesting_of_exactly_sixteen_is_not_a_depth_refusal() {
    let at_limit = format!("{}{}", "[".repeat(16), "]".repeat(16));
    let err = parse_record(at_limit.as_bytes()).expect_err("not a record");
    assert_ne!(err.code(), "PROV_TOO_DEEP");
}

#[test]
fn repeated_keys_are_refused_at_every_level() {
    let text = standard_text();
    let root_dup = text.replacen(
        "\"fsq:profile\":\"filesquid-prov/1\"",
        &format!("\"fsq:profile\":\"filesquid-prov/1\",\"fsq:profile\":\"{SECRET}\""),
        1,
    );
    refused(&root_dup, "PROV_DUPLICATE_KEY");
    let node_dup = text.replacen(
        "\"fsq:kind\":\"move\"",
        &format!("\"fsq:kind\":\"move\",\"fsq:kind\":\"{SECRET}\""),
        1,
    );
    refused(&node_dup, "PROV_DUPLICATE_KEY");
}

#[test]
fn unknown_or_missing_profiles_are_refused() {
    let text = standard_text();
    refused(
        &text.replacen("filesquid-prov/1", SECRET, 1),
        "PROV_UNKNOWN_PROFILE",
    );
    refused(
        &text.replacen("filesquid-prov/1", "filesquid-prov/2", 1),
        "PROV_UNKNOWN_PROFILE",
    );
    let mut doc = standard_value();
    doc.as_object_mut().unwrap().remove("fsq:profile");
    refused(
        &serde_json::to_string(&doc).unwrap(),
        "PROV_UNKNOWN_PROFILE",
    );
}

#[test]
fn malformed_json_and_invalid_utf8_are_refused() {
    refused(&format!("{{\"{SECRET}\": "), "PROV_INVALID_JSON");
    let err = parse_record(&[b'{', 0xff, b'}']).expect_err("utf-8");
    assert_eq!(err.code(), "PROV_INVALID_JSON");
}

#[test]
fn a_forged_entity_identifier_breaks_the_profile() {
    let mut doc = standard_value();
    doc["@graph"][3]["fsq:path"] = json!(SECRET);
    invalid_node(&doc);
}

#[test]
fn a_wrong_context_breaks_the_profile() {
    let mut doc = standard_value();
    doc["@context"]["prov"] = json!("https://www.w3.org/ns/prov#");
    invalid_node(&doc);
}

#[test]
fn unknown_members_and_wrong_types_break_the_profile() {
    let mut doc = standard_value();
    doc["@graph"][1]["schema:name"] = json!(SECRET);
    invalid_node(&doc);
    let mut doc = standard_value();
    doc["@graph"][1]["@type"] = json!("prov:Person");
    invalid_node(&doc);
    let mut doc = standard_value();
    doc["@graph"][3]["fsq:sha256"] = json!(SECRET);
    invalid_node(&doc);
}

#[test]
fn non_utc_times_break_the_profile() {
    let mut doc = standard_value();
    doc["@graph"][0]["prov:endedAtTime"]["@value"] = json!("2026-10-19T12:00:05+02:00");
    invalid_node(&doc);
}

#[test]
fn dangling_references_break_the_profile() {
    let mut doc = standard_value();
    doc["@graph"][0]["prov:used"][0]["@id"] = json!(format!("urn:filesquid:entity:{SECRET}"));
    invalid_node(&doc);
}

#[test]
fn exactly_one_activity_is_required() {
    let mut doc = standard_value();
    let activity = doc["@graph"][0].clone();
    doc["@graph"].as_array_mut().unwrap().push(activity);
    invalid_node(&doc);
    let mut doc = standard_value();
    doc["@graph"].as_array_mut().unwrap().remove(0);
    invalid_node(&doc);
}

#[test]
fn a_person_agent_is_never_accepted() {
    let mut doc = standard_value();
    doc["@graph"].as_array_mut().unwrap().push(json!({
        "@id": "urn:example:person",
        "@type": "prov:Person",
        "schema:name": SECRET
    }));
    invalid_node(&doc);
}
