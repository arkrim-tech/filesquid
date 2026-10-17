//! C2PA-S-009: the library builds for `wasm32-unknown-unknown`, both with the
//! default RustCrypto backend and with default features off. This runs in the
//! local pre-commit gate through `cargo test`. A missing target fails the
//! test; it is skipped only when `FILESQUID_SKIP_WASM32=1` is set.

#[path = "../support/deps.rs"]
#[allow(dead_code)]
mod deps;
#[path = "../support/wasm.rs"]
mod wasm;

use wasm::{build_for_wasm, WasmBuild};

/// Run one build; fail on a missing target or a compile error.
fn assert_builds(default_features: bool) {
    match build_for_wasm(default_features) {
        Ok(WasmBuild::Built) => {}
        Ok(WasmBuild::Skipped(reason)) => eprintln!("SKIPPED wasm32 build: {reason}"),
        Err(output) => {
            panic!("wasm32 build check failed (default_features={default_features}):\n{output}")
        }
    }
}

#[test]
fn builds_for_wasm32_with_default_features_off() {
    assert_builds(false);
}

#[test]
fn builds_for_wasm32_with_the_rust_crypto_backend() {
    assert_builds(true);
}
