//! C2PA-S-001 performance: a manifest store close to the default box limit
//! and a large embedded thumbnail parse within a bounded time.
//!
//! The bound is generous (debug builds on a shared machine); the test exists
//! to catch quadratic behaviour, which would take minutes at this size.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::time::{Duration, Instant};

use filesquid_c2pa::manifest::{ManifestStore, ParseLimits};
use fixtures::*;

/// Assertions in the large store: each is three boxes (superbox,
/// description, content), so this stays just under 65 536 boxes in total.
const ASSERTIONS: usize = 21_800;

/// Time allowed for one parse.
const BUDGET: Duration = Duration::from_secs(20);

/// A store with [`ASSERTIONS`] small CBOR assertions and an 8 MiB thumbnail.
fn large_store() -> Vec<u8> {
    let mut assertions: Vec<Vec<u8>> = (0..ASSERTIONS)
        .map(|i| cbor_assertion(&format!("vendor.item__{i}"), &map(&[("i", text("v"))])))
        .collect();
    assertions.push(embedded_assertion(
        "c2pa.thumbnail.claim",
        "image/jpeg",
        None,
        &vec![0xA5; 8 * 1024 * 1024],
    ));
    store(&[manifest(
        b"c2ma",
        ACTIVE_LABEL,
        &[
            assertion_store(&assertions),
            claim_box("c2pa.claim.v2", &encode(&claim_v2(&["vendor.item__0"]))),
            signature_box(),
        ],
    )])
}

#[test]
fn a_store_near_the_box_limit_parses_within_the_budget() {
    let bytes = large_store();
    let started = Instant::now();
    let store = ManifestStore::from_bytes(bytes, &ParseLimits::default()).expect("store");
    let elapsed = started.elapsed();
    let manifest = store.active_manifest().content.as_ref().expect("manifest");
    assert_eq!(manifest.assertions.len(), ASSERTIONS + 1);
    eprintln!(
        "parsed {} assertions and an 8 MiB thumbnail in {elapsed:?}",
        ASSERTIONS
    );
    assert!(elapsed < BUDGET, "parse took {elapsed:?}");
}
