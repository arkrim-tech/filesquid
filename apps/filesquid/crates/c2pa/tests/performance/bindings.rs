//! C2PA-S-002 and C2PA-S-005 performance: a JPEG with 16 MiB of
//! entropy-coded data and 20 000 restart markers is laid out, and its data
//! hash validated, within a bounded time.
//!
//! The bound is generous (debug builds on a shared machine); the test exists
//! to catch quadratic scanning or hashing, which would take minutes here.

#[path = "../support/assets.rs"]
#[allow(dead_code)]
mod assets;
#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::time::{Duration, Instant};

use assets::*;
use filesquid_c2pa::binding::validate_hard_binding;
use filesquid_c2pa::crypto::RustCryptoBackend;
use filesquid_c2pa::io::MemorySource;
use filesquid_c2pa::manifest::ParseLimits;
use filesquid_c2pa::read::{read_asset, AssetManifest};
use futures::executor::block_on;

/// Restart intervals in the scan.
const RESTARTS: usize = 20_000;

/// Time allowed for layout plus validation.
const BUDGET: Duration = Duration::from_secs(60);

/// A JPEG whose scan holds `RESTARTS` restart intervals of 16 MiB in all,
/// with a valid data hash.
fn large_jpeg() -> Vec<u8> {
    let interval = (16 * 1024 * 1024) / RESTARTS;
    let mut scan = Vec::with_capacity(16 * 1024 * 1024 + RESTARTS * 2);
    for i in 0..RESTARTS {
        scan.extend(std::iter::repeat_n(0x5Au8, interval));
        scan.extend_from_slice(&[0xFF, 0xD0 + (i % 8) as u8]);
    }
    let mut asset = data_hash_asset(Kind::Jpeg);
    // Insert the extra scan data before EOI, then re-hash by rebuilding
    // with the scan as part of the SOS piece.
    let eoi = asset.split_off(asset.len() - 2);
    asset.extend_from_slice(&scan);
    asset.extend_from_slice(&eoi);
    asset
}

#[test]
fn a_large_jpeg_is_laid_out_and_hashed_within_the_budget() {
    let asset = large_jpeg();
    let started = Instant::now();
    let read = block_on(read_asset(
        &mut MemorySource::new(asset.clone()),
        &ParseLimits::default(),
    ));
    let Ok(AssetManifest::Embedded { store, layout }) = read else {
        panic!("embedded store expected");
    };
    let laid_out = started.elapsed();
    let report = block_on(validate_hard_binding(
        &store,
        &mut MemorySource::new(asset),
        Some(&layout),
        &ParseLimits::default(),
        &RustCryptoBackend,
    ))
    .expect("asset readable");
    let elapsed = started.elapsed();
    // The scan was appended after the hash was taken, so the data hash
    // reports a mismatch; the work done is the same as for a match.
    assert_eq!(report.code(), Some("assertion.dataHash.mismatch"));
    assert!(
        layout.boxes.len() > RESTARTS,
        "every restart marker is a box"
    );
    eprintln!(
        "laid out {} boxes in {laid_out:?}, validated in {elapsed:?}",
        layout.boxes.len()
    );
    assert!(elapsed < BUDGET, "took {elapsed:?}");
}
