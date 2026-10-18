//! Test support for the desktop capability policy: runs the same input reader as `build.rs`
//! (`src/capability_inputs.rs`) over the shipped crate, with the linked-plugin keys `build.rs`
//! recorded for this build. The including test crate must provide `crate::capability_inputs`
//! and `crate::capability_policy`.

use crate::capability_inputs::{read_policy_violations, tauri_platform, PolicyContext};
use crate::capability_policy::Violation;
use std::path::Path;

/// The `DEP_<CRATE>_PERMISSION_FILES_PATH` keys `build.rs` saw for this build.
pub fn linked_plugin_keys() -> Vec<String> {
    env!("FILESQUID_TAURI_PLUGIN_KEYS")
        .split(',')
        .filter(|key| !key.is_empty())
        .map(String::from)
        .collect()
}

/// Every policy violation of the shipped crate, checked as `build.rs` checks it for the
/// platform the tests run on, without a CLI overlay. Performs filesystem I/O.
pub fn read_shipped_violations() -> Vec<Violation> {
    let keys = linked_plugin_keys();
    let ctx = PolicyContext {
        root: Path::new(env!("CARGO_MANIFEST_DIR")),
        platform: tauri_platform(std::env::consts::OS),
        cli_config: None,
        plugin_keys: &keys,
        test_build: cfg!(feature = "test-webdriver"),
    };
    read_policy_violations(&ctx).expect("readable policy inputs")
}
