//! C2PA-S-002 (hard bindings): BMFF-hash validation (C2PA 2.4 sections
//! 15.12.2, 18.6 and A.5.6) on a small MP4 whose expected hash input
//! (`offset || data` per root box) is computed independently in the test.

#[path = "../support/assets.rs"]
#[allow(dead_code)]
mod assets;
#[path = "../support/bmff_store.rs"]
mod bmff_store;
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
        &ParseLimits::default(),
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
    // Move the excluded `free` box in front of `moov`: every hashed byte
    // stays the same and in the same order, and only `moov`'s offset
    // changes. The offset is part of the input, so the old hash fails.
    let (asset, all) = mp4();
    let piece = |i: usize| asset[all[i].start as usize..all[i].end as usize].to_vec();
    let moved = [
        piece(FTYP),
        piece(UUID),
        piece(FREE),
        piece(MOOV),
        piece(MDAT),
    ]
    .concat();
    let free_len = all[FREE].end - all[FREE].start;
    let moved_moov = all[MOOV].start + free_len..all[MOOV].end + free_len;
    let list = exclusions(vec![bmff_exclusion("/free", &[])]);
    let old = bmff_expected_hash(&asset, &[all[MOOV].clone(), all[MDAT].clone()], &[]);
    let new = bmff_expected_hash(&moved, &[moved_moov, all[MDAT].clone()], &[]);
    let report = check(&bmff_hash_map(list.clone(), &new), &moved);
    assert_eq!(code(&report), "assertion.bmffHash.match", "control");
    let report = check(&bmff_hash_map(list, &old), &moved);
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
fn a_missing_hash_is_malformed_and_merkle_trees_are_not_checked() {
    let (asset, _) = mp4();
    let no_hash = map(&[("exclusions", CborValue::Array(bmff_required_exclusions()))]);
    assert_eq!(
        code(&check(&no_hash, &asset)),
        "assertion.bmffHash.malformed"
    );
    let text_hash = map(&[
        ("exclusions", CborValue::Array(bmff_required_exclusions())),
        ("hash", text("not bytes")),
    ]);
    assert_eq!(
        code(&check(&text_hash, &asset)),
        "assertion.bmffHash.malformed"
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

#[test]
fn the_test_store_locator_skips_c2pa_boxes_of_other_purposes() {
    // A `merkle` C2PA box before the `manifest` box is passed over.
    let (asset, all) = mp4();
    let mut merkle = C2PA_BMFF_UUID.to_vec();
    merkle.extend_from_slice(&[0, 0, 0, 0]);
    merkle.extend_from_slice(b"merkle\0");
    merkle.extend_from_slice(&[1; 8]);
    let store = binding_store(
        "c2pa.hash.bmff.v3",
        &bmff_hash_map(bmff_required_exclusions(), &[0; 32]),
    );
    let mut manifest = C2PA_BMFF_UUID.to_vec();
    manifest.extend_from_slice(&[0, 0, 0, 0]);
    manifest.extend_from_slice(b"manifest\0");
    manifest.extend_from_slice(&[0; 8]);
    manifest.extend_from_slice(&store);
    let ftyp = &asset[all[FTYP].start as usize..all[FTYP].end as usize];
    let built = [
        ftyp.to_vec(),
        bmff_box(b"uuid", &merkle),
        bmff_box(b"uuid", &manifest),
    ]
    .concat();
    let found = bmff_store::mp4_manifest_store(&built).expect("located");
    assert_eq!(found, Some(store));
}

#[test]
fn the_asset_box_limit_bounds_the_bmff_walk() {
    // The required exclusions are all root-level, so the walk visits the
    // five root boxes: five pass, four are reported as not checked.
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let map = bmff_hash_map(bmff_required_exclusions(), &hash);
    assert_eq!(
        code(&check_with_box_limit(&map, &asset, 5)),
        "assertion.bmffHash.match"
    );
    assert_eq!(
        code(&check_with_box_limit(&map, &asset, 4)),
        "not checked: the asset has more boxes than ParseLimits::max_asset_boxes"
    );
}

/// Validate the BMFF-hash `value` against `asset` with `max_asset_boxes`.
fn check_with_box_limit(value: &CborValue, asset: &[u8], max_asset_boxes: usize) -> BindingReport {
    let store = ManifestStore::from_bytes(
        binding_store("c2pa.hash.bmff.v3", value),
        &ParseLimits::default(),
    )
    .expect("store");
    let limits = ParseLimits {
        max_asset_boxes,
        ..ParseLimits::default()
    };
    block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(asset.to_vec()),
        None,
        &limits,
        &RustCryptoBackend,
    ))
    .expect("asset readable")
}

#[test]
fn nested_boxes_count_toward_the_box_limit() {
    // Five root boxes plus `moov`'s two children are walked for
    // `/moov/udta`: seven pass, six are reported as not checked, never as
    // an exclusion that matched nothing.
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[all[UDTA].clone()]);
    let map = bmff_hash_map(exclusions(vec![bmff_exclusion("/moov/udta", &[])]), &hash);
    assert_eq!(
        code(&check_with_box_limit(&map, &asset, 7)),
        "assertion.bmffHash.match"
    );
    assert_eq!(
        code(&check_with_box_limit(&map, &asset, 6)),
        "not checked: the asset has more boxes than ParseLimits::max_asset_boxes"
    );
}

#[test]
fn a_broken_box_list_under_an_xpath_is_a_mismatch_not_no_match() {
    // `mdat` holds 0x05 bytes, which do not parse as boxes. Descending into
    // it is an error, not an exclusion that silently matched nothing.
    let (asset, all) = mp4();
    let hash = expected(&asset, &all, &[MOOV, FREE, MDAT], &[]);
    let map = bmff_hash_map(exclusions(vec![bmff_exclusion("/mdat/abcd", &[])]), &hash);
    let report = check(&map, &asset);
    assert_eq!(
        report.status,
        BindingStatus::Invalid {
            code: "assertion.bmffHash.mismatch",
            reason: "the asset is not a valid BMFF box structure",
        }
    );
    // A path that parses but finds no such box is a genuine no-match.
    let absent = bmff_hash_map(exclusions(vec![bmff_exclusion("/moov/abcd", &[])]), &hash);
    assert_eq!(code(&check(&absent, &asset)), "assertion.bmffHash.match");
}

#[test]
fn the_test_store_locator_handles_large_sizes_and_its_box_limit() {
    let (asset, all) = mp4();
    let piece = |i: usize| asset[all[i].start as usize..all[i].end as usize].to_vec();
    // `free` written with a 64-bit size (`size` 1, then the largesize).
    let free = &piece(FREE)[8..];
    let mut large_free = 1u32.to_be_bytes().to_vec();
    large_free.extend_from_slice(b"free");
    large_free.extend_from_slice(&(16 + free.len() as u64).to_be_bytes());
    large_free.extend_from_slice(free);
    let store = sample_store();
    let mut manifest = C2PA_BMFF_UUID.to_vec();
    manifest.extend_from_slice(&[0, 0, 0, 0]);
    manifest.extend_from_slice(b"manifest\0");
    manifest.extend_from_slice(&[0; 8]);
    manifest.extend_from_slice(&store);
    let with_large = [piece(FTYP), large_free, bmff_box(b"uuid", &manifest)].concat();
    let found = bmff_store::mp4_manifest_store(&with_large).expect("located");
    assert_eq!(found, Some(store), "the manifest after a large-size box");
    // Three root boxes without a manifest: a limit of three walks to the
    // end, two stops early.
    let no_manifest = [piece(FTYP), piece(MOOV), piece(MDAT)].concat();
    assert_eq!(
        bmff_store::mp4_manifest_store_within(&no_manifest, 3),
        Ok(None)
    );
    assert!(bmff_store::mp4_manifest_store_within(&no_manifest, 2).is_err());
}
