//! C2PA-S-009: the library builds for `wasm32-unknown-unknown`, both with the
//! default RustCrypto backend and with default features off. This runs in the
//! local pre-commit gate through `cargo test --workspace`; it is skipped, with
//! the reason printed, when the Rust target is not installed.

#[path = "../support/deps.rs"]
#[allow(dead_code)]
mod deps;
#[path = "../support/wasm.rs"]
mod wasm;

use wasm::{build_for_wasm, WasmBuild};

/// Run one build and report a skip on stderr so it shows in the test log.
fn assert_builds(default_features: bool) {
    match build_for_wasm(default_features) {
        Ok(WasmBuild::Built) => {}
        Ok(WasmBuild::Skipped(reason)) => eprintln!("SKIPPED wasm32 build: {reason}"),
        Err(output) => {
            panic!("wasm32 build failed (default_features={default_features}):\n{output}")
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
