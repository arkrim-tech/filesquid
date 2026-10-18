//! C2PA-S-002 (hard bindings): data-hash and general box-hash validation
//! on JPEG and PNG assets (C2PA 2.4 sections 15.12.1, 15.12.3, 18.5,
//! 18.7), the choice of manifest and assertion (15.12, 9.1), and the hash
//! algorithm (13.1, 15.4.1).

#[path = "../support/assets.rs"]
#[allow(dead_code)]
mod assets;
#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use assets::*;
use filesquid_c2pa::binding::{validate_hard_binding, BindingKind, BindingReport, BindingStatus};
use filesquid_c2pa::cbor::CborValue;
use filesquid_c2pa::crypto::RustCryptoBackend;
use filesquid_c2pa::io::MemorySource;
use filesquid_c2pa::manifest::{ManifestStore, ParseLimits};
use filesquid_c2pa::read::{read_asset, AssetManifest};
use fixtures::*;
use futures::executor::block_on;

/// Read `asset`, then validate its embedded store's hard binding.
fn check(asset: &[u8]) -> BindingReport {
    let read = block_on(read_asset(
        &mut MemorySource::new(asset.to_vec()),
        &ParseLimits::default(),
    ));
    let Ok(AssetManifest::Embedded { store, layout }) = read else {
        panic!("embedded store expected: {read:?}");
    };
    block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(asset.to_vec()),
        Some(&layout),
        &ParseLimits::default(),
        &RustCryptoBackend,
    ))
    .expect("asset readable")
}

/// Validate a sidecar `store` against `asset` (no embedded store).
fn check_sidecar(store: Vec<u8>, asset: &[u8]) -> BindingReport {
    let store = ManifestStore::from_bytes(store, &ParseLimits::default()).expect("store");
    block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(asset.to_vec()),
        None,
        &ParseLimits::default(),
        &RustCryptoBackend,
    ))
    .expect("asset readable")
}

/// The status code, or the not-checked reason prefixed with `not checked:`.
fn code(report: &BindingReport) -> String {
    match &report.status {
        BindingStatus::Valid { code } | BindingStatus::Invalid { code, .. } => code.to_string(),
        BindingStatus::NotChecked { reason } => format!("not checked: {reason}"),
    }
}

/// A data-hash asset with custom exclusions.
fn data_hash_with(
    kind: Kind,
    exclusions: impl Fn(&std::ops::Range<u64>) -> Vec<(u64, u64)>,
) -> Vec<u8> {
    data_hash_asset_with(kind, exclusions, |map| binding_store("c2pa.hash.data", map))
}

/// Flip one byte of the piece called `name` in `asset` built for `kind`.
fn tamper(kind: Kind, asset: &mut [u8], name: &str) {
    let read = block_on(read_asset(
        &mut MemorySource::new(asset.to_vec()),
        &ParseLimits::default(),
    ));
    let Ok(AssetManifest::Embedded { layout, .. }) = read else {
        panic!("{kind:?}: embedded store expected");
    };
    let target = layout.boxes.iter().find(|b| b.name == name).expect("box");
    let at = (target.range.end - 1) as usize;
    asset[at] ^= 0x55;
}

#[test]
fn a_valid_data_hash_matches() {
    for kind in [Kind::Jpeg, Kind::Png] {
        let report = check(&data_hash_asset(kind));
        assert_eq!(code(&report), "assertion.dataHash.match", "{kind:?}");
        assert_eq!(report.kind, Some(BindingKind::DataHash));
        assert_eq!(report.manifest.as_deref(), Some(ACTIVE_LABEL));
        assert_eq!(report.assertion.as_deref(), Some("c2pa.hash.data"));
        assert!(report.informational.is_empty());
        assert!(report.status.is_valid());
    }
}

#[test]
fn a_changed_byte_outside_the_exclusion_is_a_data_hash_mismatch() {
    for (kind, name) in [(Kind::Jpeg, "DQT"), (Kind::Png, "IDAT")] {
        let mut asset = data_hash_asset(kind);
        tamper(kind, &mut asset, name);
        assert_eq!(
            code(&check(&asset)),
            "assertion.dataHash.mismatch",
            "{kind:?}"
        );
    }
}

#[test]
fn overlapping_or_unordered_exclusions_are_malformed() {
    let overlap = data_hash_with(Kind::Jpeg, |c| {
        vec![(c.start, c.end - c.start), (c.end - 1, 2)]
    });
    assert_eq!(code(&check(&overlap)), "assertion.dataHash.malformed");
    let unordered = data_hash_with(Kind::Jpeg, |c| vec![(c.start, c.end - c.start), (2, 2)]);
    assert_eq!(code(&check(&unordered)), "assertion.dataHash.malformed");
}

#[test]
fn an_exclusion_past_the_end_of_the_asset_is_a_mismatch() {
    let past = data_hash_with(Kind::Png, |c| {
        vec![(c.start, c.end - c.start), (c.end + 10_000, 1)]
    });
    assert_eq!(code(&check(&past)), "assertion.dataHash.mismatch");
}

#[test]
fn the_store_exclusion_must_cover_only_the_store() {
    for kind in [Kind::Jpeg, Kind::Png] {
        let wider = data_hash_with(kind, |c| vec![(c.start, c.end - c.start + 2)]);
        let report = check(&wider);
        assert_eq!(
            code(&report),
            "assertion.dataHash.mismatch",
            "{kind:?} wider"
        );
        let earlier = data_hash_with(kind, |c| vec![(c.start - 1, c.end - c.start + 1)]);
        assert_eq!(
            code(&check(&earlier)),
            "assertion.dataHash.mismatch",
            "{kind:?} earlier"
        );
        let shorter = data_hash_with(kind, |c| vec![(c.start, c.end - c.start - 1)]);
        assert_eq!(
            code(&check(&shorter)),
            "assertion.dataHash.mismatch",
            "{kind:?} shorter"
        );
    }
}

#[test]
fn a_store_that_is_not_excluded_cannot_match() {
    let none = data_hash_with(Kind::Jpeg, |_| Vec::new());
    assert_eq!(code(&check(&none)), "assertion.dataHash.mismatch");
}

#[test]
fn other_exclusions_are_reported_as_informational() {
    // Four bytes inside the APP0 payload (asset metadata), then the store.
    let extra = data_hash_with(Kind::Jpeg, |c| vec![(8, 4), (c.start, c.end - c.start)]);
    let report = check(&extra);
    assert_eq!(code(&report), "assertion.dataHash.match");
    assert_eq!(
        report.informational,
        ["assertion.dataHash.additionalExclusionsPresent"]
    );
}

#[test]
fn malformed_exclusion_values_are_malformed() {
    let negative = |c: &std::ops::Range<u64>| {
        map(&[
            (
                "exclusions",
                CborValue::Array(vec![map(&[
                    ("start", CborValue::Negative(0)),
                    ("length", CborValue::Unsigned(c.end - c.start)),
                ])]),
            ),
            ("hash", bytes(&[0; 32])),
        ])
    };
    let store = binding_store("c2pa.hash.data", &negative(&(0..10)));
    let (asset, _) = assemble(&pieces(Kind::Png, &store));
    assert_eq!(code(&check(&asset)), "assertion.dataHash.malformed");
}

#[test]
fn a_missing_hash_field_is_a_mismatch() {
    let no_hash = map(&[("exclusions", CborValue::Array(vec![]))]);
    let (asset, _) = assemble(&pieces(
        Kind::Jpeg,
        &binding_store("c2pa.hash.data", &no_hash),
    ));
    assert_eq!(code(&check(&asset)), "assertion.dataHash.mismatch");
}

/// A store whose claim has no `alg` and whose only assertion is `label`.
fn store_without_claim_alg(label: &str, value: &CborValue) -> Vec<u8> {
    let CborValue::Map(pairs) = claim_v2(&[label]) else {
        panic!("map");
    };
    let claim = CborValue::Map(
        pairs
            .into_iter()
            .filter(|(k, _)| k.as_text() != Some("alg"))
            .collect(),
    );
    store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[cbor_assertion(label, value)]),
            claim_box("c2pa.claim.v2", &encode(&claim)),
            signature_box(),
        ],
    )])
}

#[test]
fn the_hash_algorithm_must_be_allowed_and_known() {
    let md5 = map(&[("alg", text("md5")), ("hash", bytes(&[0; 16]))]);
    let (asset, _) = assemble(&pieces(Kind::Png, &binding_store("c2pa.hash.data", &md5)));
    assert_eq!(code(&check(&asset)), "algorithm.unsupported");
    let nowhere = map(&[
        ("exclusions", CborValue::Array(vec![])),
        ("hash", bytes(&[0; 32])),
    ]);
    let (asset, _) = assemble(&pieces(
        Kind::Png,
        &store_without_claim_alg("c2pa.hash.data", &nowhere),
    ));
    assert_eq!(code(&check(&asset)), "algorithm.unsupported");
}

#[test]
fn the_claim_alg_applies_when_the_assertion_has_none() {
    let asset = data_hash_asset_with(
        Kind::Jpeg,
        |c| vec![(c.start, c.end - c.start)],
        |m| {
            let CborValue::Map(pairs) = m else {
                panic!("map")
            };
            let no_alg = CborValue::Map(
                pairs
                    .iter()
                    .filter(|(k, _)| k.as_text() != Some("alg"))
                    .cloned()
                    .collect(),
            );
            binding_store("c2pa.hash.data", &no_alg)
        },
    );
    assert_eq!(code(&check(&asset)), "assertion.dataHash.match");
}

#[test]
fn a_sidecar_store_binds_a_whole_asset() {
    let asset = b"any asset bytes, hashed in full".to_vec();
    let store = binding_store("c2pa.hash.data", &data_hash_map(&[], &sha256(&asset)));
    assert_eq!(
        code(&check_sidecar(store.clone(), &asset)),
        "assertion.dataHash.match"
    );
    let mut changed = asset.clone();
    changed[0] ^= 1;
    assert_eq!(
        code(&check_sidecar(store, &changed)),
        "assertion.dataHash.mismatch"
    );
}

#[test]
fn hard_binding_presence_rules() {
    let asset = b"asset".to_vec();
    let none = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[cbor_assertion("c2pa.actions.v2", &actions_value())]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["c2pa.actions.v2"]))),
            signature_box(),
        ],
    )]);
    assert_eq!(
        code(&check_sidecar(none, &asset)),
        "claim.hardBindings.missing"
    );
    let hash = data_hash_map(&[], &sha256(&asset));
    let two = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[
                cbor_assertion("c2pa.hash.data", &hash),
                cbor_assertion("c2pa.hash.data__1", &hash),
            ]),
            claim_box(
                "c2pa.claim.v2",
                &encode(&claim_v2(&["c2pa.hash.data", "c2pa.hash.data__1"])),
            ),
            signature_box(),
        ],
    )]);
    assert_eq!(
        code(&check_sidecar(two, &asset)),
        "assertion.multipleHardBindings"
    );
    let legacy_bmff = binding_store("c2pa.hash.bmff", &map(&[("hash", bytes(&[0; 32]))]));
    assert_eq!(
        code(&check_sidecar(legacy_bmff, &asset)),
        "claim.hardBindings.missing",
        "c2pa.hash.bmff (v1) is ignored"
    );
    let bad_cbor = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[superbox(
                uuid(b"cbor"),
                "c2pa.hash.data",
                &[boxed(b"cbor", &[0xFF])],
            )]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["c2pa.hash.data"]))),
            signature_box(),
        ],
    )]);
    assert_eq!(
        code(&check_sidecar(bad_cbor, &asset)),
        "assertion.cbor.invalid"
    );
}

#[test]
fn bindings_the_engine_cannot_check_are_reported_as_not_checked() {
    let asset = b"asset".to_vec();
    let collection = binding_store(
        "c2pa.hash.collection.data",
        &map(&[("uris", CborValue::Array(vec![]))]),
    );
    assert!(code(&check_sidecar(collection, &asset)).starts_with("not checked"));
    let compressed = store(&[manifest(
        b"c2cm",
        ACTIVE_LABEL,
        &[boxed(b"brob", &[1, 2, 3])],
    )]);
    assert!(code(&check_sidecar(compressed, &asset)).starts_with("not checked"));
    let unusable = store(&[manifest(b"c2ma", ACTIVE_LABEL, &[signature_box()])]);
    assert!(code(&check_sidecar(unusable, &asset)).starts_with("not checked"));
}

#[test]
fn a_mismatch_with_a_multi_asset_hash_present_is_not_checked() {
    let asset = b"asset".to_vec();
    let multi = store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&[
                cbor_assertion("c2pa.hash.data", &data_hash_map(&[], &[0; 32])),
                cbor_assertion(
                    "c2pa.hash.multi-asset",
                    &map(&[("parts", CborValue::Array(vec![]))]),
                ),
            ]),
            claim_box(
                "c2pa.claim.v2",
                &encode(&claim_v2(&["c2pa.hash.data", "c2pa.hash.multi-asset"])),
            ),
            signature_box(),
        ],
    )]);
    let report = check_sidecar(multi, &asset);
    assert!(code(&report).starts_with("not checked"), "{report:?}");
}

#[test]
fn an_update_manifest_uses_its_parent_binding_and_the_current_store_length() {
    let std_label = "urn:c2pa:aaaaaaaa-1111-4222-8333-444444444444";
    let update_label = "urn:c2pa:bbbbbbbb-1111-4222-8333-444444444444";
    for kind in [Kind::Jpeg, Kind::Png] {
        // Hash the asset when the store held only the standard manifest.
        let original = data_hash_asset_with(
            kind,
            |c| vec![(c.start, c.end - c.start)],
            |m| store(&[binding_manifest(std_label, "c2pa.hash.data", m)]),
        );
        let read = block_on(read_asset(
            &mut MemorySource::new(original.clone()),
            &ParseLimits::default(),
        ));
        let Ok(AssetManifest::Embedded { store: first, .. }) = read else {
            panic!("store")
        };
        // Append an update manifest: the store grows, the hash stays.
        let first_bytes = first.bytes().to_vec();
        let manifests: Vec<u8> =
            first_bytes[first.manifests()[0].range.start..first.manifests()[0].range.end].to_vec();
        let grown = store(&[manifests, update_manifest(update_label, std_label)]);
        let (asset, _) = assemble(&pieces(kind, &grown));
        let report = check(&asset);
        assert_eq!(code(&report), "assertion.dataHash.match", "{kind:?}");
        assert_eq!(report.manifest.as_deref(), Some(std_label));
    }
}

#[test]
fn an_update_manifest_without_a_parent_has_no_hard_binding() {
    let asset = b"asset".to_vec();
    let orphan = store(&[update_manifest(ACTIVE_LABEL, "urn:c2pa:absent")]);
    assert_eq!(
        code(&check_sidecar(orphan, &asset)),
        "claim.hardBindings.missing"
    );
    let loop_label = "urn:c2pa:cccccccc-1111-4222-8333-444444444444";
    let cycle = store(&[
        update_manifest(loop_label, ACTIVE_LABEL),
        update_manifest(ACTIVE_LABEL, loop_label),
    ]);
    assert_eq!(
        code(&check_sidecar(cycle, &asset)),
        "claim.hardBindings.missing"
    );
}

#[test]
fn a_valid_box_hash_matches() {
    for kind in [Kind::Jpeg, Kind::Png] {
        let report = check(&box_hash_asset(kind));
        assert_eq!(code(&report), "assertion.boxesHash.match", "{kind:?}");
        assert_eq!(report.kind, Some(BindingKind::BoxHash));
        assert!(report.informational.is_empty());
    }
}

#[test]
fn a_changed_box_is_a_box_hash_mismatch() {
    for (kind, name) in [(Kind::Jpeg, "DQT"), (Kind::Png, "IDAT")] {
        let mut asset = box_hash_asset(kind);
        tamper(kind, &mut asset, name);
        assert_eq!(
            code(&check(&asset)),
            "assertion.boxesHash.mismatch",
            "{kind:?}"
        );
    }
}

#[test]
fn boxes_out_of_order_are_a_mismatch_and_unlisted_boxes_are_unknown() {
    let swapped = box_hash_asset_with(Kind::Jpeg, |p| {
        let mut entries = default_box_entries(p);
        entries.swap(3, 4);
        entries
    });
    assert_eq!(code(&check(&swapped)), "assertion.boxesHash.mismatch");
    let missing = box_hash_asset_with(Kind::Jpeg, |p| {
        let mut entries = default_box_entries(p);
        entries.remove(1);
        entries
    });
    assert_eq!(code(&check(&missing)), "assertion.boxesHash.unknownBox");
    let absent = box_hash_asset_with(Kind::Png, |p| {
        let mut entries = default_box_entries(p);
        entries.push(box_entry(&["tEXt"], &[0; 32], &[]));
        entries
    });
    assert_eq!(code(&check(&absent)), "assertion.boxesHash.mismatch");
}

#[test]
fn one_entry_can_hash_a_run_of_boxes() {
    let asset = box_hash_asset_with(Kind::Jpeg, |p| {
        let mut entries = default_box_entries(p);
        let run = [p[3].1.clone(), p[4].1.clone()].concat();
        entries.splice(3..5, [box_entry(&["DQT", "SOS"], &sha256(&run), &[])]);
        entries
    });
    assert_eq!(code(&check(&asset)), "assertion.boxesHash.match");
}

/// One box exclusion range.
fn box_exclusion(start: u64, length: u64, index: Option<u64>) -> CborValue {
    let mut fields = vec![
        ("start", CborValue::Unsigned(start)),
        ("length", CborValue::Unsigned(length)),
    ];
    if let Some(index) = index {
        fields.push(("boxIndex", CborValue::Unsigned(index)));
    }
    map(&fields)
}

#[test]
fn box_exclusions_are_applied_and_reported() {
    let asset = box_hash_asset_with(Kind::Jpeg, |p| {
        let mut entries = default_box_entries(p);
        let app0 = &p[1].1;
        let kept = [&app0[..4], &app0[9..]].concat();
        entries[1] = box_entry(
            &["APP0"],
            &sha256(&kept),
            &[(
                "exclusions",
                CborValue::Array(vec![box_exclusion(4, 5, None)]),
            )],
        );
        entries
    });
    let report = check(&asset);
    assert_eq!(code(&report), "assertion.boxesHash.match");
    assert_eq!(
        report.informational,
        ["assertion.boxesHash.additionalExclusionsPresent"]
    );
}

#[test]
fn bad_box_exclusions_are_malformed_or_mismatched() {
    let with = |exclusions: Vec<CborValue>| {
        box_hash_asset_with(Kind::Jpeg, move |p| {
            let mut entries = default_box_entries(p);
            entries[1] = box_entry(
                &["APP0"],
                &[0; 32],
                &[("exclusions", CborValue::Array(exclusions.clone()))],
            );
            entries
        })
    };
    let bad_index = with(vec![box_exclusion(0, 1, Some(1))]);
    assert_eq!(code(&check(&bad_index)), "assertion.boxesHash.malformed");
    let overlap = with(vec![box_exclusion(2, 4, None), box_exclusion(5, 2, None)]);
    assert_eq!(code(&check(&overlap)), "assertion.boxesHash.malformed");
    let past = with(vec![box_exclusion(10, 100, None)]);
    assert_eq!(code(&check(&past)), "assertion.boxesHash.mismatch");
}

#[test]
fn an_excluded_box_is_not_compared() {
    let asset = box_hash_asset_with(Kind::Png, |p| {
        let mut entries = default_box_entries(p);
        entries[1] = box_entry(&["IHDR"], &[0], &[("excluded", CborValue::Bool(true))]);
        entries
    });
    assert_eq!(code(&check(&asset)), "assertion.boxesHash.match");
}

#[test]
fn malformed_box_maps_are_malformed() {
    let shapes = [
        map(&[("alg", text("sha256"))]),
        map(&[("boxes", CborValue::Array(vec![])), ("alg", text("sha256"))]),
        box_hash_map(vec![map(&[("hash", bytes(&[0; 32]))])]),
        box_hash_map(vec![box_entry(&[], &[0; 32], &[])]),
    ];
    for shape in shapes {
        let (asset, _) = assemble(&pieces(
            Kind::Png,
            &binding_store("c2pa.hash.boxes", &shape),
        ));
        assert_eq!(
            code(&check(&asset)),
            "assertion.boxesHash.malformed",
            "{shape:?}"
        );
    }
}

#[test]
fn bytes_after_the_last_box_are_covered_by_c2pa_after() {
    let trailer = b"multi-part trailer".to_vec();
    let mut asset = box_hash_asset_with(Kind::Png, |p| {
        let mut entries = default_box_entries(p);
        entries.push(box_entry(
            &["c2pa.after"],
            &sha256(b"multi-part trailer"),
            &[],
        ));
        entries
    });
    asset.extend_from_slice(&trailer);
    assert_eq!(code(&check(&asset)), "assertion.boxesHash.match");
    let last = asset.len() - 1;
    asset[last] ^= 1;
    assert_eq!(code(&check(&asset)), "assertion.boxesHash.mismatch");
}

/// A standard manifest whose assertion store holds `assertions` and whose
/// claim references only `referenced`.
fn manifest_referencing(label: &str, assertions: &[Vec<u8>], referenced: &[&str]) -> Vec<u8> {
    manifest(
        b"c2ma",
        label,
        &[
            assertion_store(assertions),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(referenced))),
            signature_box(),
        ],
    )
}

#[test]
fn a_hard_binding_the_claim_does_not_reference_is_ignored() {
    let asset = b"asset".to_vec();
    let good = data_hash_map(&[], &sha256(&asset));
    let unreferenced = store(&[manifest_referencing(
        ACTIVE_LABEL,
        &[
            cbor_assertion("c2pa.actions.v2", &actions_value()),
            cbor_assertion("c2pa.hash.data", &good),
        ],
        &["c2pa.actions.v2"],
    )]);
    assert_eq!(
        code(&check_sidecar(unreferenced, &asset)),
        "claim.hardBindings.missing"
    );
    // An unreferenced second binding neither counts as a second binding
    // nor replaces the referenced one.
    let bad = data_hash_map(&[], &[0; 32]);
    let extra = store(&[manifest_referencing(
        ACTIVE_LABEL,
        &[
            cbor_assertion("c2pa.hash.data", &good),
            cbor_assertion("c2pa.hash.data__1", &bad),
        ],
        &["c2pa.hash.data"],
    )]);
    let report = check_sidecar(extra, &asset);
    assert_eq!(code(&report), "assertion.dataHash.match");
    assert_eq!(report.assertion.as_deref(), Some("c2pa.hash.data"));
}

#[test]
fn a_parent_ingredient_the_claim_does_not_reference_is_ignored() {
    let asset = b"asset".to_vec();
    let std_label = "urn:c2pa:aaaaaaaa-1111-4222-8333-444444444444";
    let parent = manifest_referencing(
        std_label,
        &[cbor_assertion(
            "c2pa.hash.data",
            &data_hash_map(&[], &sha256(&asset)),
        )],
        &["c2pa.hash.data"],
    );
    let update = manifest(
        b"c2um",
        ACTIVE_LABEL,
        &[
            assertion_store(&[
                cbor_assertion("c2pa.actions.v2", &actions_value()),
                cbor_assertion("c2pa.ingredient.v3", &parent_ingredient(std_label)),
            ]),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["c2pa.actions.v2"]))),
            signature_box(),
        ],
    );
    let report = check_sidecar(store(&[parent, update]), &asset);
    assert_eq!(code(&report), "claim.hardBindings.missing");
}

#[test]
fn an_unusable_update_manifest_is_not_checked() {
    let asset = b"asset".to_vec();
    let std_label = "urn:c2pa:aaaaaaaa-1111-4222-8333-444444444444";
    let parent = manifest_referencing(
        std_label,
        &[cbor_assertion(
            "c2pa.hash.data",
            &data_hash_map(&[], &sha256(&asset)),
        )],
        &["c2pa.hash.data"],
    );
    let broken_update = manifest(b"c2um", ACTIVE_LABEL, &[signature_box()]);
    let report = check_sidecar(store(&[parent, broken_update]), &asset);
    assert_eq!(
        code(&report),
        "not checked: an update manifest on the way to the hard binding is unusable"
    );
}
