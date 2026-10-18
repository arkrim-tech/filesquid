//! Test-only builders for JUMBF boxes, CBOR values and C2PA manifest stores.
//!
//! The library has no encoder yet (manifest building is CP-0.6), so tests
//! build their inputs here: a definite-length CBOR encoder with preferred
//! (shortest) argument encoding, a JUMBF box writer and helpers that put
//! together claims, assertions, manifests and stores as C2PA 2.4 section
//! 11.1 lays them out.

use filesquid_c2pa::cbor::{CborFloat, CborValue};

/// The common tail of every C2PA and JUMBF ISO type UUID:
/// `-0011-0010-8000-00AA00389B71`.
const UUID_TAIL: [u8; 12] = [
    0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

/// The JUMBF type UUID whose first four bytes are `tag`.
pub fn uuid(tag: &[u8; 4]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(tag);
    out[4..].copy_from_slice(&UUID_TAIL);
    out
}

/// Embedded File content type UUID `40CB0C32-BB8A-489D-A70B-2AD6F47F4369`.
pub const EMBEDDED_FILE_UUID: [u8; 16] = [
    0x40, 0xCB, 0x0C, 0x32, 0xBB, 0x8A, 0x48, 0x9D, 0xA7, 0x0B, 0x2A, 0xD6, 0xF4, 0x7F, 0x43, 0x69,
];

/// Description box toggles: requestable and label present.
pub const TOGGLES: u8 = 0x03;

/// A box with an 8-byte header.
pub fn boxed(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len() + 8).expect("box fits in LBox");
    let mut out = len.to_be_bytes().to_vec();
    out.extend_from_slice(box_type);
    out.extend_from_slice(payload);
    out
}

/// A box with a 16-byte header (`LBox = 1` and `XLBox`).
pub fn boxed_xl(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = 1u32.to_be_bytes().to_vec();
    out.extend_from_slice(box_type);
    out.extend_from_slice(&((payload.len() + 16) as u64).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// A description box payload with `toggles` and an optional label.
pub fn jumd_payload(type_uuid: [u8; 16], toggles: u8, label: Option<&str>) -> Vec<u8> {
    let mut out = type_uuid.to_vec();
    out.push(toggles);
    if let Some(label) = label {
        out.extend_from_slice(label.as_bytes());
        out.push(0);
    }
    out
}

/// A labelled, requestable description box.
pub fn jumd(type_uuid: [u8; 16], label: &str) -> Vec<u8> {
    boxed(b"jumd", &jumd_payload(type_uuid, TOGGLES, Some(label)))
}

/// A superbox of `type_uuid` and `label` holding `children`.
pub fn superbox(type_uuid: [u8; 16], label: &str, children: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = jumd(type_uuid, label);
    for child in children {
        payload.extend_from_slice(child);
    }
    boxed(b"jumb", &payload)
}

/// A superbox whose description box has `toggles` (a label is written when
/// the label bit is set).
pub fn superbox_toggled(
    type_uuid: [u8; 16],
    label: &str,
    toggles: u8,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let label = (toggles & 0x02 != 0).then_some(label);
    let mut payload = boxed(b"jumd", &jumd_payload(type_uuid, toggles, label));
    for child in children {
        payload.extend_from_slice(child);
    }
    boxed(b"jumb", &payload)
}

/// Text value.
pub fn text(value: &str) -> CborValue {
    CborValue::Text(value.to_string())
}

/// Byte-string value.
pub fn bytes(value: &[u8]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

/// Map with text keys, in the given order.
pub fn map(entries: &[(&str, CborValue)]) -> CborValue {
    CborValue::Map(
        entries
            .iter()
            .map(|(key, value)| (text(key), value.clone()))
            .collect(),
    )
}

/// Encode a CBOR head: major type and argument, shortest form.
pub fn head(major: u8, arg: u64) -> Vec<u8> {
    let m = major << 5;
    match arg {
        0..=23 => vec![m | arg as u8],
        24..=0xFF => vec![m | 24, arg as u8],
        0x100..=0xFFFF => [vec![m | 25], (arg as u16).to_be_bytes().to_vec()].concat(),
        0x1_0000..=0xFFFF_FFFF => [vec![m | 26], (arg as u32).to_be_bytes().to_vec()].concat(),
        _ => [vec![m | 27], arg.to_be_bytes().to_vec()].concat(),
    }
}

/// Encode `value` as definite-length CBOR (floats as 64-bit).
pub fn encode(value: &CborValue) -> Vec<u8> {
    match value {
        CborValue::Unsigned(n) => head(0, *n),
        CborValue::Negative(n) => head(1, *n),
        CborValue::Bytes(b) => [head(2, b.len() as u64), b.clone()].concat(),
        CborValue::Text(t) => [head(3, t.len() as u64), t.as_bytes().to_vec()].concat(),
        CborValue::Array(items) => encode_array(items),
        CborValue::Map(pairs) => encode_map(pairs),
        CborValue::Tag(tag, inner) => [head(6, *tag), encode(inner)].concat(),
        CborValue::Bool(false) => vec![0xF4],
        CborValue::Bool(true) => vec![0xF5],
        CborValue::Null => vec![0xF6],
        CborValue::Undefined => vec![0xF7],
        CborValue::Simple(n) if *n < 20 => vec![0xE0 | n],
        CborValue::Simple(n) if *n < 32 => panic!("simple value {n} has no encoding of its own"),
        CborValue::Simple(n) => vec![0xF8, *n],
        CborValue::Float(f) => [vec![0xFB], f.value().to_bits().to_be_bytes().to_vec()].concat(),
    }
}

/// Encode an array.
fn encode_array(items: &[CborValue]) -> Vec<u8> {
    let mut out = head(4, items.len() as u64);
    for item in items {
        out.extend_from_slice(&encode(item));
    }
    out
}

/// Encode a map.
fn encode_map(pairs: &[(CborValue, CborValue)]) -> Vec<u8> {
    let mut out = head(5, pairs.len() as u64);
    for (key, value) in pairs {
        out.extend_from_slice(&encode(key));
        out.extend_from_slice(&encode(value));
    }
    out
}

/// A float value.
pub fn float(value: f64) -> CborValue {
    CborValue::Float(CborFloat::from_f64(value))
}

/// A CBOR assertion superbox.
pub fn cbor_assertion(label: &str, value: &CborValue) -> Vec<u8> {
    superbox(uuid(b"cbor"), label, &[boxed(b"cbor", &encode(value))])
}

/// A JSON assertion superbox holding `json` verbatim.
pub fn json_assertion(label: &str, json: &str) -> Vec<u8> {
    superbox(uuid(b"json"), label, &[boxed(b"json", json.as_bytes())])
}

/// An embedded-file assertion superbox.
pub fn embedded_assertion(
    label: &str,
    media_type: &str,
    name: Option<&str>,
    data: &[u8],
) -> Vec<u8> {
    let mut bfdb = vec![u8::from(name.is_some())];
    bfdb.extend_from_slice(media_type.as_bytes());
    bfdb.push(0);
    if let Some(name) = name {
        bfdb.extend_from_slice(name.as_bytes());
        bfdb.push(0);
    }
    superbox(
        EMBEDDED_FILE_UUID,
        label,
        &[boxed(b"bfdb", &bfdb), boxed(b"bidb", data)],
    )
}

/// A hashed URI map.
pub fn hashed_uri(url: &str, hash: &[u8]) -> CborValue {
    map(&[("url", text(url)), ("hash", bytes(hash))])
}

/// A valid `c2pa.claim.v2` map referencing `assertions`.
pub fn claim_v2(assertions: &[&str]) -> CborValue {
    let created = assertions
        .iter()
        .map(|label| hashed_uri(&format!("self#jumbf=c2pa.assertions/{label}"), &[7u8; 32]))
        .collect();
    map(&[
        (
            "instanceID",
            text("xmp:iid:5c2f0b2b-0c9c-4b8b-9a28-6c3e4e2c0f11"),
        ),
        (
            "claim_generator_info",
            map(&[("name", text("FileSquid test")), ("version", text("0.2"))]),
        ),
        ("signature", text("self#jumbf=c2pa.signature")),
        ("created_assertions", CborValue::Array(created)),
        ("alg", text("sha256")),
    ])
}

/// A valid legacy `c2pa.claim` (v1) map referencing `assertions`.
pub fn claim_v1(assertions: &[&str]) -> CborValue {
    let refs = assertions
        .iter()
        .map(|label| hashed_uri(&format!("self#jumbf=c2pa.assertions/{label}"), &[9u8; 32]))
        .collect();
    map(&[
        ("claim_generator", text("FileSquid-test/0.1")),
        ("signature", text("self#jumbf=c2pa.signature")),
        ("assertions", CborValue::Array(refs)),
        ("dc:format", text("image/jpeg")),
        ("instanceID", text("xmp:iid:legacy")),
        ("alg", text("sha256")),
    ])
}

/// Stand-in claim signature bytes: a `COSE_Sign1_Tagged` shape (tag 18,
/// four-element array). Not a real signature; verification is CP-0.4.
pub fn cose_placeholder() -> Vec<u8> {
    encode(&CborValue::Tag(
        18,
        Box::new(CborValue::Array(vec![
            bytes(&[0xA1, 0x01, 0x26]),
            CborValue::Map(vec![]),
            CborValue::Null,
            bytes(&[0u8; 64]),
        ])),
    ))
}

/// A claim superbox with `label` holding `claim_cbor` verbatim.
pub fn claim_box(label: &str, claim_cbor: &[u8]) -> Vec<u8> {
    superbox(uuid(b"c2cl"), label, &[boxed(b"cbor", claim_cbor)])
}

/// The claim signature superbox.
pub fn signature_box() -> Vec<u8> {
    superbox(
        uuid(b"c2cs"),
        "c2pa.signature",
        &[boxed(b"cbor", &cose_placeholder())],
    )
}

/// An assertion store superbox.
pub fn assertion_store(assertions: &[Vec<u8>]) -> Vec<u8> {
    superbox(uuid(b"c2as"), "c2pa.assertions", assertions)
}

/// A manifest superbox of `kind` (for example `b"c2ma"`) with `children`.
pub fn manifest(kind: &[u8; 4], label: &str, children: &[Vec<u8>]) -> Vec<u8> {
    superbox(uuid(kind), label, children)
}

/// The `c2pa.actions.v2` assertion value of [`standard_assertions`].
pub fn actions_value() -> CborValue {
    map(&[(
        "actions",
        CborValue::Array(vec![map(&[("action", text("c2pa.created"))])]),
    )])
}

/// The `c2pa.hash.data` assertion value of [`standard_assertions`].
pub fn hash_data_value() -> CborValue {
    map(&[
        ("exclusions", CborValue::Array(vec![])),
        ("alg", text("sha256")),
        ("hash", bytes(&[1u8; 32])),
    ])
}

/// CBOR data items the parser decodes in one [`standard_manifest`]: the
/// claim and the two CBOR assertions (the signature is kept as raw bytes).
pub fn standard_manifest_items() -> usize {
    count_items(&claim_v2(&STANDARD_LABELS))
        + count_items(&actions_value())
        + count_items(&hash_data_value())
}

/// The standard assertions used by [`standard_manifest`].
pub fn standard_assertions() -> Vec<Vec<u8>> {
    vec![
        cbor_assertion("c2pa.actions.v2", &actions_value()),
        cbor_assertion("c2pa.hash.data", &hash_data_value()),
        json_assertion(
            "stds.schema-org.CreativeWork",
            r#"{"@type":"CreativeWork"}"#,
        ),
        embedded_assertion(
            "c2pa.thumbnail.claim",
            "image/jpeg",
            None,
            &[0xFF, 0xD8, 0xFF, 0xD9],
        ),
    ]
}

/// Labels of [`standard_assertions`], in order.
pub const STANDARD_LABELS: [&str; 4] = [
    "c2pa.actions.v2",
    "c2pa.hash.data",
    "stds.schema-org.CreativeWork",
    "c2pa.thumbnail.claim",
];

/// A complete standard manifest with a v2 claim over [`standard_assertions`].
pub fn standard_manifest(label: &str) -> Vec<u8> {
    manifest(
        b"c2ma",
        label,
        &[
            assertion_store(&standard_assertions()),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS))),
            signature_box(),
        ],
    )
}

/// A complete manifest with a legacy v1 claim.
pub fn legacy_manifest(label: &str) -> Vec<u8> {
    let assertion = cbor_assertion("c2pa.hash.data", &map(&[("hash", bytes(&[2u8; 32]))]));
    manifest(
        b"c2ma",
        label,
        &[
            assertion_store(&[assertion]),
            claim_box("c2pa.claim", &encode(&claim_v1(&["c2pa.hash.data"]))),
            signature_box(),
        ],
    )
}

/// A manifest store (`c2pa`) holding `children`.
pub fn store(children: &[Vec<u8>]) -> Vec<u8> {
    superbox(uuid(b"c2pa"), "c2pa", children)
}

/// Label of the ingredient manifest in [`sample_store`].
pub const INGREDIENT_LABEL: &str = "urn:c2pa:3f1c4ad1-1b0f-4a3e-9d6e-0b5b8f0e7a11";

/// Label of the active manifest in [`sample_store`].
pub const ACTIVE_LABEL: &str = "urn:c2pa:9a7b2c6e-5d4f-4e3a-8b1c-2d0e6f5a4b39";

/// A store with an ingredient manifest followed by the active manifest.
pub fn sample_store() -> Vec<u8> {
    store(&[
        standard_manifest(INGREDIENT_LABEL),
        standard_manifest(ACTIVE_LABEL),
    ])
}

/// The number of CBOR data items in `value` (a map pair is two items), as
/// RFC 8949 counts them for definite-length encodings.
pub fn count_items(value: &CborValue) -> usize {
    match value {
        CborValue::Array(items) => 1 + items.iter().map(count_items).sum::<usize>(),
        CborValue::Map(pairs) => {
            1 + pairs
                .iter()
                .map(|(k, v)| count_items(k) + count_items(v))
                .sum::<usize>()
        }
        CborValue::Tag(_, inner) => 1 + count_items(inner),
        _ => 1,
    }
}

/// Find the first occurrence of `needle` in `haystack`.
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A small deterministic pseudo-random generator (xorshift64*) for fuzz
/// tests, so every run explores the same inputs.
pub struct Rng(u64);

impl Rng {
    /// A generator seeded with `seed` (zero is replaced by one).
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// The next value.
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value below `bound`, which must be non-zero.
    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}
