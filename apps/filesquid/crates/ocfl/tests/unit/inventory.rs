//! OCFL 1.1 inventory parsing (OCFL-S-002; OCFL 1.1 section 3.5): a valid
//! inventory becomes typed data, and every rule decidable from one
//! inventory's bytes is refused with its OCFL 1.1 validation code.

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{
    FormatError, Inventory, VersionNum, DEFAULT_CONTENT_DIRECTORY, MAX_INVENTORY_BYTES,
};
use filesquid_ocfl::namaste::SpecVersion;
use serde_json::{json, Value};

#[path = "../support/inventory_example.rs"]
mod example;
use example::{base, A, C, EMPTY};

/// Parse a JSON value as inventory bytes.
fn parse(value: &Value) -> Result<Inventory, FormatError> {
    Inventory::parse(&serde_json::to_vec(value).expect("serialise"))
}

/// Parse `base()` after `change`, and return the refusal's OCFL code.
fn code_after(change: impl FnOnce(&mut Value)) -> &'static str {
    let mut value = base();
    change(&mut value);
    let err = parse(&value).expect_err("the inventory must be refused");
    err.code()
        .unwrap_or_else(|| panic!("no OCFL code in {err:?}"))
}

/// The OCFL code for raw bytes that are refused.
fn code_of_bytes(bytes: &[u8]) -> &'static str {
    let err = Inventory::parse(bytes).expect_err("refused");
    err.code()
        .unwrap_or_else(|| panic!("no OCFL code in {err:?}"))
}

/// Remove a key from the JSON object at `pointer`.
fn remove(value: &mut Value, pointer: &str, key: &str) {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .expect("object")
        .remove(key);
}

/// Set the value at `pointer`, adding the last key when it is absent. Keys
/// in these tests never contain `/` or `~`.
fn set(value: &mut Value, pointer: &str, new: Value) {
    let (parent, key) = pointer.rsplit_once('/').expect("a JSON pointer");
    let object = value
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {parent:?}"));
    object.insert(key.to_string(), new);
}

#[test]
fn a_valid_inventory_becomes_typed_data() {
    let inventory = parse(&base()).expect("valid");
    assert_eq!(inventory.id, "ark:/12345/bcd987");
    assert_eq!(inventory.spec, SpecVersion::V1_1);
    assert_eq!(inventory.digest_algorithm, DigestAlgorithm::Sha256);
    assert_eq!(inventory.head.to_string(), "v2");
    assert_eq!(inventory.content_directory(), "content");
    assert_eq!(inventory.manifest.len(), 3);
    assert_eq!(inventory.versions.len(), 2);
    let head = inventory.head_version().expect("head version");
    assert_eq!(head.created, "2018-02-02T02:02:02.5+01:00");
    assert_eq!(head.message.as_deref(), Some("Fix bar.xml, add image"));
    assert_eq!(
        head.user.as_ref().map(|u| u.name.as_str()),
        Some("A Person")
    );
    assert_eq!(head.user.as_ref().and_then(|u| u.address.clone()), None);
    assert_eq!(head.state[EMPTY], ["empty.txt", "empty2.txt"]);
    let fixity = inventory.fixity.as_ref().expect("fixity");
    assert!(
        fixity.contains_key("crc32"),
        "unsupported fixity algorithms are kept"
    );
}

#[test]
fn content_directory_defaults_to_content() {
    let mut value = base();
    remove(&mut value, "", "contentDirectory");
    let inventory = parse(&value).expect("valid");
    assert_eq!(inventory.content_directory, None);
    assert_eq!(inventory.content_directory(), DEFAULT_CONTENT_DIRECTORY);
}

#[test]
fn optional_blocks_may_be_absent_or_empty() {
    let mut value = base();
    remove(&mut value, "", "fixity");
    remove(&mut value, "/versions/v1", "message");
    remove(&mut value, "/versions/v1", "user");
    assert!(parse(&value).expect("valid").fixity.is_none());
    set(&mut value, "/fixity", json!({}));
    assert!(parse(&value).expect("valid").fixity.is_some());
}

#[test]
fn a_written_inventory_parses_back_to_the_same_data() {
    let inventory = parse(&base()).expect("valid");
    let written = inventory.to_json();
    assert_eq!(Inventory::parse(&written).expect("reparse"), inventory);
}

#[test]
fn an_ocfl_1_0_inventory_type_is_recognised() {
    let mut value = base();
    set(
        &mut value,
        "/type",
        json!("https://ocfl.io/1.0/spec/#inventory"),
    );
    assert_eq!(parse(&value).expect("valid").spec, SpecVersion::V1_0);
}

#[test]
fn required_keys_are_enforced() {
    for key in ["id", "type", "digestAlgorithm", "head"] {
        assert_eq!(code_after(|v| remove(v, "", key)), "E036", "{key}");
    }
    assert_eq!(code_after(|v| remove(v, "", "manifest")), "E041");
    assert_eq!(code_after(|v| remove(v, "", "versions")), "E043");
    assert_eq!(code_after(|v| remove(v, "/versions/v1", "created")), "E048");
    assert_eq!(code_after(|v| remove(v, "/versions/v1", "state")), "E048");
}

#[test]
fn unknown_keys_are_refused() {
    assert_eq!(code_after(|v| set(v, "/extra", json!(1))), "E102");
    assert_eq!(
        code_after(|v| set(v, "/versions/v1/extra", json!(1))),
        "E102"
    );
    assert_eq!(
        code_after(|v| set(v, "/versions/v1/user/extra", json!(1))),
        "E102"
    );
}

#[test]
fn type_and_digest_algorithm_are_checked() {
    assert_eq!(
        code_after(|v| set(v, "/type", json!("https://example.org/"))),
        "E038"
    );
    for name in ["sha1", "md5", "SHA256", "blake2b-512"] {
        assert_eq!(
            code_after(|v| set(v, "/digestAlgorithm", json!(name))),
            "E025",
            "{name}"
        );
    }
}

#[test]
fn content_directory_must_be_a_direct_child_name() {
    assert_eq!(
        code_after(|v| set(v, "/contentDirectory", json!("content/dir"))),
        "E017"
    );
    assert_eq!(
        code_after(|v| set(v, "/contentDirectory", json!("."))),
        "E018"
    );
    assert_eq!(
        code_after(|v| set(v, "/contentDirectory", json!(".."))),
        "E018"
    );
    assert_eq!(
        code_after(|v| set(v, "/contentDirectory", json!(""))),
        "E108"
    );
    assert_eq!(
        code_after(|v| set(v, "/contentDirectory", json!(1))),
        "E033"
    );
}

#[test]
fn head_must_name_the_highest_version() {
    assert_eq!(code_after(|v| set(v, "/head", json!("v1"))), "E040");
    assert_eq!(code_after(|v| set(v, "/head", json!("v3"))), "E040");
    assert_eq!(code_after(|v| set(v, "/head", json!("2"))), "E040");
    assert_eq!(
        code_after(|v| set(v, "/head", json!({"not": "a name"}))),
        "E040"
    );
}

/// Replace the versions block with `names`, each a minimal valid version
/// whose state uses every manifest digest, and set head to the last name.
fn with_versions(value: &mut Value, names: &[&str]) {
    let state = json!({A: ["foo/bar.xml"], EMPTY: ["empty.txt"], C: ["image.tiff"]});
    let block = json!({"created": "2018-01-01T01:01:01Z", "state": state});
    let versions = names
        .iter()
        .map(|n| (n.to_string(), block.clone()))
        .collect::<serde_json::Map<_, _>>();
    set(value, "/versions", Value::Object(versions));
    set(value, "/head", json!(names.last().expect("a name")));
}

#[test]
fn versions_must_be_a_non_empty_object() {
    assert_eq!(code_after(|v| set(v, "/versions", json!([]))), "E045");
    assert_eq!(code_after(|v| set(v, "/versions", json!({}))), "E008");
    assert_eq!(code_after(|v| set(v, "/versions/v1", json!("x"))), "E047");
}

#[test]
fn version_names_are_v_and_a_positive_integer() {
    assert_eq!(code_after(|v| with_versions(v, &["1"])), "E104");
    for name in ["v0", "v1a", "v+1", "v-1", "v", "v١"] {
        assert_eq!(code_after(|v| with_versions(v, &[name])), "E105", "{name}");
    }
}

#[test]
fn version_sequences_start_at_one_without_gaps() {
    assert_eq!(code_after(|v| with_versions(v, &["v2"])), "E009");
    assert_eq!(code_after(|v| with_versions(v, &["v1", "v3"])), "E010");
}

#[test]
fn zero_padded_versions_keep_one_width() {
    let mut value = base();
    with_versions(&mut value, &["v001", "v002", "v003"]);
    let inventory = parse(&value).expect("valid padded");
    assert_eq!(inventory.head.to_string(), "v003");
    assert!(inventory.head.is_zero_padded());
    assert_eq!(code_after(|v| with_versions(v, &["v01", "v2"])), "E012");
    assert_eq!(code_after(|v| with_versions(v, &["v1", "v02"])), "E012");
    let ten: Vec<String> = (1..=10).map(|n| format!("v{n:02}")).collect();
    let names: Vec<&str> = ten.iter().map(String::as_str).collect();
    assert_eq!(code_after(|v| with_versions(v, &names)), "E011");
}

#[test]
fn a_padded_head_outside_the_width_is_refused() {
    let mut value = base();
    with_versions(&mut value, &["v01", "v02"]);
    set(&mut value, "/head", json!("v10"));
    assert_eq!(parse(&value).expect_err("refused").code(), Some("E011"));
}

#[test]
fn created_is_an_rfc3339_date_time_to_the_second() {
    for good in [
        "2018-01-01T01:01:01Z",
        "2018-01-01t01:01:01z",
        "2018-01-01T01:01:01.123456789+05:30",
        "2016-12-31T23:59:60Z",
        "2016-12-31T18:59:60-05:00",
        "2017-01-01T05:29:60+05:30",
        "2020-02-29T00:00:00-00:00",
    ] {
        let mut value = base();
        set(&mut value, "/versions/v1/created", json!(good));
        assert!(parse(&value).is_ok(), "{good}");
    }
    for bad in [
        "2018-01-01T01:01Z",
        "2018-01-01T01:01:01",
        "2018-01-01 01:01:01Z",
        "2019-02-29T00:00:00Z",
        "2018-13-01T00:00:00Z",
        "2018-01-01T24:00:00Z",
        "2018-01-01T01:01:01.Z",
        "2018-01-01T01:01:01+0100",
        "18-01-01T01:01:01Z",
        "2018-01-01T12:00:60Z",
        "2016-12-31T23:59:60+01:00",
    ] {
        assert_eq!(
            code_after(|v| set(v, "/versions/v1/created", json!(bad))),
            "E049",
            "{bad}"
        );
    }
}

#[test]
fn message_and_user_have_their_shapes() {
    assert_eq!(
        code_after(|v| set(v, "/versions/v1/message", json!(1))),
        "E094"
    );
    assert_eq!(
        code_after(|v| set(v, "/versions/v1/user", json!("x"))),
        "E054"
    );
    assert_eq!(
        code_after(|v| remove(v, "/versions/v1/user", "name")),
        "E054"
    );
    assert_eq!(
        code_after(|v| set(v, "/versions/v1/user/name", json!(1))),
        "E054"
    );
}

#[test]
fn manifest_structure_is_checked() {
    assert_eq!(code_after(|v| set(v, "/manifest", json!([]))), "E106");
    assert_eq!(
        code_after(|v| set(v, &format!("/manifest/{A}"), json!("x"))),
        "E092"
    );
    assert_eq!(
        code_after(|v| set(v, &format!("/manifest/{A}"), json!([1]))),
        "E092"
    );
}

#[test]
fn manifest_digests_match_the_algorithm() {
    let not_hex = "z".repeat(64);
    let short = "a".repeat(63);
    let rename = |v: &mut Value, to: &str| {
        let manifest = v["manifest"].as_object_mut().expect("manifest");
        let paths = manifest.remove(A).expect("A");
        manifest.insert(to.to_string(), paths);
    };
    assert_eq!(code_after(|v| rename(v, &not_hex)), "E030");
    assert_eq!(code_after(|v| rename(v, &short)), "E039");
    let mut value = base();
    set(&mut value, "/digestAlgorithm", json!("sha512"));
    assert_eq!(
        parse(&value).expect_err("sha256 digests").code(),
        Some("E039")
    );
}

/// `base()` rewritten for sha512: every stand-in digest becomes 128 digits.
fn sha512_inventory() -> Value {
    let text = serde_json::to_string(&base())
        .expect("serialise")
        .replace("sha256", "sha512")
        .replace(A, &A.repeat(2))
        .replace(C, &C.repeat(2))
        .replace(EMPTY, &EMPTY.repeat(2));
    serde_json::from_str(&text).expect("json")
}

#[test]
fn a_valid_sha512_inventory_parses() {
    let inventory = parse(&sha512_inventory()).expect("valid sha512 inventory");
    assert_eq!(inventory.digest_algorithm, DigestAlgorithm::Sha512);
    assert!(inventory.manifest.keys().all(|d| d.len() == 128));
}

#[test]
fn a_non_hex_sha512_digest_is_e031() {
    let mut value = sha512_inventory();
    let manifest = value["manifest"].as_object_mut().expect("manifest");
    let paths = manifest.remove(&A.repeat(2)).expect("A");
    manifest.insert("z".repeat(128), paths);
    assert_eq!(parse(&value).expect_err("not hex").code(), Some("E031"));
}

#[test]
fn errors_name_a_location_only_when_there_is_one() {
    let whole = Inventory::parse(b"{").expect_err("invalid");
    assert!(!whole.to_string().contains("at \"\""), "{whole}");
    let mut value = base();
    set(&mut value, "/head", json!("v9"));
    let located = parse(&value).expect_err("head is not the highest version");
    assert_eq!(located.code(), Some("E040"));
    assert!(located.to_string().contains("at \"/head\""), "{located}");
}

#[test]
fn a_repeated_key_is_not_echoed() {
    let key = "k".repeat(10_000);
    let text = format!("{{\"{key}\":1,\"{key}\":2}}");
    let err = Inventory::parse(text.as_bytes()).expect_err("repeated key");
    assert_eq!(err.code(), Some("E033"));
    assert!(!err.to_string().contains(&key[..100]), "the key was echoed");
}

#[test]
fn a_digest_appears_once_regardless_of_case() {
    let upper = A.to_uppercase();
    let code = code_after(|v| set(v, &format!("/manifest/{upper}"), json!(["v1/content/x"])));
    assert_eq!(code, "E096");
}

#[test]
fn content_paths_are_safe_and_unique() {
    let path = |p: &str| {
        let p = p.to_string();
        move |v: &mut Value| set(v, &format!("/manifest/{C}"), json!([p]))
    };
    assert_eq!(code_after(path("v2/content/../x")), "E099");
    assert_eq!(code_after(path("v2//x")), "E099");
    assert_eq!(code_after(path("v2/./x")), "E099");
    assert_eq!(code_after(path("/v2/x")), "E100");
    assert_eq!(code_after(path("v2/x/")), "E100");
    assert_eq!(code_after(path("v1/content/foo/bar.xml")), "E101");
    assert_eq!(code_after(path("v1/content/foo/bar.xml/deeper")), "E101");
    assert_eq!(code_after(path("v1/content/foo")), "E101");
}

#[test]
fn every_manifest_digest_is_used_by_a_version() {
    let unused = "b".repeat(64);
    let code = code_after(|v| set(v, &format!("/manifest/{unused}"), json!(["v2/content/z"])));
    assert_eq!(code, "E107");
}

#[test]
fn state_digests_exactly_match_the_manifest() {
    let state = |v: &mut Value, digest: &str| {
        let state = v["versions"]["v1"]["state"].as_object_mut().expect("state");
        let paths = state.remove(A).expect("A");
        state.insert(digest.to_string(), paths);
    };
    assert_eq!(code_after(|v| state(v, &"d".repeat(64))), "E050");
    assert_eq!(code_after(|v| state(v, &A.to_uppercase())), "E050");
}

#[test]
fn mixed_case_digests_are_kept_as_written() {
    let mixed = "AaAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaA";
    let text = serde_json::to_string(&base())
        .expect("serialise")
        .replace(A, mixed);
    let inventory = Inventory::parse(text.as_bytes()).expect("valid");
    assert!(inventory.manifest.contains_key(mixed));
}

#[test]
fn logical_paths_are_safe_and_unique() {
    let path =
        |paths: Value| move |v: &mut Value| set(v, &format!("/versions/v2/state/{C}"), paths);
    assert_eq!(code_after(path(json!(["../x"]))), "E052");
    assert_eq!(code_after(path(json!(["a//b"]))), "E052");
    assert_eq!(code_after(path(json!(["/x"]))), "E053");
    assert_eq!(code_after(path(json!(["x/"]))), "E053");
    assert_eq!(code_after(path(json!(["empty.txt"]))), "E095");
    assert_eq!(code_after(path(json!(["foo"]))), "E095");
    assert_eq!(
        code_after(path(json!(["image.tiff", "image.tiff"]))),
        "E095"
    );
}

#[test]
fn fixity_follows_the_manifest_structure() {
    assert_eq!(code_after(|v| set(v, "/fixity", json!([]))), "E111");
    assert_eq!(code_after(|v| set(v, "/fixity/md5", json!([]))), "E057");
    let md5 = "/fixity/md5/184f84e28cbe75e050e9c25ea7f2e939";
    assert_eq!(code_after(|v| set(v, md5, json!("x"))), "E057");
    let upper = "/fixity/md5/184F84E28CBE75E050E9C25EA7F2E939";
    assert_eq!(
        code_after(|v| set(v, upper, json!(["v1/content/x"]))),
        "E097"
    );
    assert_eq!(
        code_after(|v| set(v, md5, json!(["/v1/content/x"]))),
        "E100"
    );
    assert_eq!(code_after(|v| set(v, md5, json!(["v1/../x"]))), "E099");
}

#[test]
fn malformed_json_is_refused() {
    assert_eq!(code_of_bytes(b"{"), "E033");
    assert_eq!(code_of_bytes(b"[]"), "E033");
    assert_eq!(code_of_bytes(b"\xff\xfe"), "E033");
    let mut text = serde_json::to_vec(&base()).expect("serialise");
    text.extend_from_slice(b" {}");
    assert_eq!(code_of_bytes(&text), "E033");
}

#[test]
fn duplicate_json_keys_are_refused() {
    let text = serde_json::to_string(&base()).expect("serialise");
    let doubled = text.replacen(
        "{\"contentDirectory\"",
        "{\"id\":\"other\",\"contentDirectory\"",
        1,
    );
    assert_ne!(doubled, text, "the test must add a duplicate key");
    assert_eq!(code_of_bytes(doubled.as_bytes()), "E033");
}

#[test]
fn deep_nesting_is_refused_without_exhausting_the_stack() {
    let depth = 100_000;
    let mut text = String::from("{\"id\":");
    text.push_str(&"[".repeat(depth));
    text.push_str(&"]".repeat(depth));
    text.push('}');
    assert_eq!(code_of_bytes(text.as_bytes()), "E033");
}

#[test]
fn oversized_input_is_refused_before_parsing() {
    let bytes = vec![b' '; MAX_INVENTORY_BYTES + 1];
    let err = Inventory::parse(&bytes).expect_err("too large");
    assert!(matches!(err, FormatError::TooLarge { .. }), "{err:?}");
    assert_eq!(err.code(), None);
}

#[test]
fn version_numbers_parse_format_and_advance() {
    let v1 = VersionNum::parse("v1").expect("v1");
    assert_eq!((v1.number(), v1.is_zero_padded()), ("1", false));
    assert_eq!(v1.next().expect("next").to_string(), "v2");
    let padded = VersionNum::parse("v0009").expect("padded");
    assert_eq!(padded.number(), "9");
    assert_eq!(padded.as_u64(), Some(9));
    assert_eq!(padded.next().expect("next").to_string(), "v0010");
    let last = VersionNum::parse("v0999").expect("last at width 4");
    assert_eq!(last.next(), None, "v1000 does not fit the padded width");
    let long = VersionNum::parse("v123456789012345678901234567890").expect("no digit limit");
    assert_eq!(long.as_u64(), None);
    assert_eq!(
        long.next().expect("next").to_string(),
        "v123456789012345678901234567891"
    );
    let nines = VersionNum::parse("v9999999999999999999").expect("nineteen nines");
    let after = nines.next().expect("next");
    assert_eq!(
        after,
        VersionNum::parse("v10000000000000000000").expect("parses")
    );
    assert!(nines < after);
    let name = |n: &str| VersionNum::parse(n).expect(n);
    assert!(name("v1") < name("v2"));
    assert!(name("v2") < name("v10"));
}
