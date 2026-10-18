//! C2PA-S-002 (hard bindings): BMFF-hash validation (C2PA 2.4 sections
//! 15.12.2, 18.6 and A.5.6) on a small MP4 whose expected hash input
//! (`offset || data` per root box) is computed independently in the test.

#[path = "../support/assets.rs"]
#[allow(dead_code)]
mod assets;
#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::ops::Range;

use assets::*;
use filesquid_c2pa::binding::{validate_hard_binding, BindingKind, BindingReport, BindingStatus};
use filesquid_c2pa::cbor::CborValue;
use filesquid_c2pa::crypto::RustCryptoBackend;
use filesquid_c2pa::io::MemorySource;
use filesquid_c2pa::manifest::{ManifestStore, ParseLimits};
use fixtures::*;
use futures::executor::block_on;

/// Indices into the ranges [`mp4`] returns.
const FTYP: usize = 0;
const UUID: usize = 1;
const MOOV: usize = 2;
const MVHD: usize = 3;
const UDTA: usize = 4;
const FREE: usize = 5;
const MDAT: usize = 6;

/// Validate a BMFF-hash assertion `value` (label `label`) against `asset`.
fn check_labelled(label: &str, value: &CborValue, asset: &[u8]) -> BindingReport {
    let store = ManifestStore::from_bytes(binding_store(label, value), &ParseLimits::default())
        .expect("store");
    block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(asset.to_vec()),
        None,
        &RustCryptoBackend,
    ))
    .expect("asset readable")
}

/// Validate a `c2pa.hash.bmff.v3` assertion.
fn check(value: &CborValue, asset: &[u8]) -> BindingReport {
    check_labelled("c2pa.hash.bmff.v3", value, asset)
}

/// The status code, or `not checked`.
fn code(report: &BindingReport) -> String {
    match &report.status {
        BindingStatus::Valid { code } | BindingStatus::Invalid { code, .. } => code.to_string(),
        BindingStatus::NotChecked { reason } => format!("not checked: {reason}"),
    }
}

/// The hash with the required exclusions plus `extra` excluded ranges;
/// `roots` are the root boxes that are not wholly excluded.
fn expected(asset: &[u8], all: &[Range<u64>], roots: &[usize], extra: &[Range<u64>]) -> Vec<u8> {
    let roots: Vec<_> = roots.iter().map(|i| all[*i].clone()).collect();
    bmff_expected_hash(asset, &roots, extra)
}

/// The required exclusions plus `extra` entries.
fn exclusions(extra: Vec<CborValue>) -> Vec<CborValue> {
    let mut out = bmff_required_exclusions();
    out.extend(extra);
    out
}

#[test]
fn the_required_exclusions_alone_match() {
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    for label in ["c2pa.hash.bmff.v3", "c2pa.hash.bmff.v2"] {
        let report = check_labelled(
            label,
            &bmff_hash_map(bmff_required_exclusions(), &hash),
            &asset,
        );
        assert_eq!(code(&report), "assertion.bmffHash.match", "{label}");
        assert_eq!(report.kind, Some(BindingKind::BmffHash));
        assert!(report.informational.is_empty());
    }
}

#[test]
fn a_changed_byte_is_a_mismatch() {
    let (mut asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    asset[all[MDAT].end as usize - 1] ^= 1;
    let report = check(&bmff_hash_map(bmff_required_exclusions(), &hash), &asset);
    assert_eq!(code(&report), "assertion.bmffHash.mismatch");
}

#[test]
fn moving_a_root_box_changes_the_hash() {
    // The offset is part of the input, so the same asset elsewhere differ.
    let (asset, all) = mp4();
    let shifted = all[MOOV].start + 1..all[MOOV].end + 1;
    let wrong = bmff_expected_hash(&asset, &[shifted], &[]);
    let report = check(&bmff_hash_map(bmff_required_exclusions(), &wrong), &asset);
    assert_eq!(code(&report), "assertion.bmffHash.mismatch");
}

#[test]
fn nested_boxes_can_be_excluded_and_are_reported() {
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[all[UDTA].clone()]);
    let map = bmff_hash_map(
        exclusions(vec![bmff_exclusion("/moov[1]/udta", &[])]),
        &hash,
    );
    let report = check(&map, &asset);
    assert_eq!(code(&report), "assertion.bmffHash.match");
    assert_eq!(
        report.informational,
        ["assertion.bmffHash.additionalExclusionsPresent"]
    );
}

/// A subset entry.
fn subset(offset: u64, length: u64) -> CborValue {
    map(&[
        ("offset", CborValue::Unsigned(offset)),
        ("length", CborValue::Unsigned(length)),
    ])
}

#[test]
fn subsets_exclude_part_of_a_box() {
    let (asset, all) = mp4();
    let mdat = all[MDAT].clone();
    let tail = mdat.start + 16..mdat.end;
    // Length 0 runs to the end of the box; a range past the end is clipped.
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[tail]);
    let map = bmff_hash_map(
        exclusions(vec![bmff_exclusion(
            "/mdat",
            &[("subset", CborValue::Array(vec![subset(16, 0)]))],
        )]),
        &hash,
    );
    assert_eq!(code(&check(&map, &asset)), "assertion.bmffHash.match");
    let hash = expected(
        &asset,
        &all,
        &[MOOV, FREE, MDAT],
        &[mdat.start + 8..mdat.start + 10, mdat.start + 40..mdat.end],
    );
    let map = bmff_hash_map(
        exclusions(vec![bmff_exclusion(
            "/mdat",
            &[(
                "subset",
                CborValue::Array(vec![subset(8, 2), subset(40, 1_000)]),
            )],
        )]),
        &hash,
    );
    assert_eq!(code(&check(&map, &asset)), "assertion.bmffHash.match");
    let overlapping = bmff_hash_map(
        exclusions(vec![bmff_exclusion(
            "/mdat",
            &[(
                "subset",
                CborValue::Array(vec![subset(8, 4), subset(10, 1)]),
            )],
        )]),
        &hash,
    );
    assert_eq!(
        code(&check(&overlapping, &asset)),
        "assertion.bmffHash.malformed"
    );
}

#[test]
fn length_version_flags_and_data_conditions_select_boxes() {
    let (asset, all) = mp4();
    let free_len = all[FREE].end - all[FREE].start;
    let without_free = expected(&asset, &all, &[MOOV, MDAT], &[]);
    let with_free = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let free = |len: u64| bmff_exclusion("/free", &[("length", CborValue::Unsigned(len))]);
    assert_eq!(
        code(&check(
            &bmff_hash_map(exclusions(vec![free(free_len)]), &without_free),
            &asset
        )),
        "assertion.bmffHash.match"
    );
    assert_eq!(
        code(&check(
            &bmff_hash_map(exclusions(vec![free(free_len + 1)]), &with_free),
            &asset
        )),
        "assertion.bmffHash.match"
    );
    let mvhd_out = expected(&asset, &all, &[MOOV, FREE, MDAT], &[all[MVHD].clone()]);
    let mvhd = |fields: &[(&str, CborValue)]| bmff_exclusion("/moov/mvhd", fields);
    let cases = [
        (vec![("version", CborValue::Unsigned(1))], &mvhd_out),
        (vec![("version", CborValue::Unsigned(0))], &with_free),
        (vec![("flags", bytes(&[0, 0, 3]))], &mvhd_out),
        (vec![("flags", bytes(&[0, 0, 1]))], &with_free),
        (
            vec![
                ("flags", bytes(&[0, 0, 1])),
                ("exact", CborValue::Bool(false)),
            ],
            &mvhd_out,
        ),
        (
            vec![
                ("flags", bytes(&[0, 0, 4])),
                ("exact", CborValue::Bool(false)),
            ],
            &with_free,
        ),
    ];
    for (fields, hash) in cases {
        let map = bmff_hash_map(exclusions(vec![mvhd(&fields)]), hash);
        assert_eq!(
            code(&check(&map, &asset)),
            "assertion.bmffHash.match",
            "{fields:?}"
        );
    }
}

#[test]
fn a_uuid_box_with_other_data_is_not_excluded() {
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let wrong_uuid = bmff_exclusion(
        "/uuid",
        &[(
            "data",
            CborValue::Array(vec![map(&[
                ("offset", CborValue::Unsigned(8)),
                ("value", bytes(&[0; 16])),
            ])]),
        )],
    );
    let map = bmff_hash_map(vec![wrong_uuid, bmff_exclusion("/ftyp", &[])], &hash);
    assert_eq!(code(&check(&map, &asset)), "assertion.bmffHash.mismatch");
    let _ = (FTYP, UUID);
}

#[test]
fn an_index_selects_one_sibling() {
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let map = bmff_hash_map(exclusions(vec![bmff_exclusion("/free[2]", &[])]), &hash);
    assert_eq!(
        code(&check(&map, &asset)),
        "assertion.bmffHash.match",
        "no second free box"
    );
}

#[test]
fn malformed_bmff_maps_are_malformed() {
    let (asset, _) = mp4();
    let shapes = [
        map(&[("hash", bytes(&[0; 32]))]),
        map(&[
            ("exclusions", CborValue::Array(vec![])),
            ("hash", bytes(&[0; 32])),
        ]),
        bmff_hash_map(vec![map(&[("length", CborValue::Unsigned(1))])], &[0; 32]),
    ];
    for shape in shapes {
        assert_eq!(
            code(&check(&shape, &asset)),
            "assertion.bmffHash.malformed",
            "{shape:?}"
        );
    }
    let deep = format!("/{}", vec!["moov"; 17].join("/"));
    for xpath in [
        "moov",
        "/moov//udta",
        "/moov[0]",
        "/moov[01]",
        "/mo",
        "/moov[1",
        "/",
        deep.as_str(),
    ] {
        let map = bmff_hash_map(exclusions(vec![bmff_exclusion(xpath, &[])]), &[0; 32]);
        assert_eq!(
            code(&check(&map, &asset)),
            "assertion.bmffHash.malformed",
            "{xpath}"
        );
    }
}

#[test]
fn an_xpath_at_the_node_limit_is_accepted() {
    // 16 nodes is the limit; 17 is malformed (above). The path matches no
    // box, so the hash over the required exclusions still applies.
    let (asset, all) = mp4();
    let at_limit = format!("/{}", vec!["moov"; 16].join("/"));
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let list = exclusions(vec![bmff_exclusion(&at_limit, &[])]);
    let report = check(&bmff_hash_map(list, &hash), &asset);
    assert_eq!(code(&report), "assertion.bmffHash.match");
}

#[test]
fn a_missing_hash_is_a_mismatch_and_merkle_trees_are_not_checked() {
    let (asset, _) = mp4();
    let no_hash = map(&[("exclusions", CborValue::Array(bmff_required_exclusions()))]);
    assert_eq!(
        code(&check(&no_hash, &asset)),
        "assertion.bmffHash.mismatch"
    );
    let merkle = map(&[
        ("exclusions", CborValue::Array(bmff_required_exclusions())),
        (
            "merkle",
            CborValue::Array(vec![map(&[("count", CborValue::Unsigned(1))])]),
        ),
    ]);
    assert!(code(&check(&merkle, &asset)).starts_with("not checked"));
}

#[test]
fn truncated_and_corrupted_bmff_assets_never_panic() {
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let map = bmff_hash_map(exclusions(vec![bmff_exclusion("/moov/udta", &[])]), &hash);
    for len in 0..asset.len() {
        let report = check(&map, &asset[..len]);
        assert!(!report.status.is_valid(), "prefix {len}");
    }
    let mut rng = Rng::new(0xB3FF);
    for _ in 0..2_000 {
        let mut mutated = asset.clone();
        let at = rng.below(mutated.len());
        mutated[at] = rng.next_u64() as u8;
        let _ = check(&map, &mutated);
    }
}
