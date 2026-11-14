//! Test support for the Tauri configuration overlays (Task DE-F3.1; DE-S-007): reads the
//! overlay files of the crate and merges them over `tauri.conf.json` in the order the Tauri CLI
//! merges its `--config` values, with the same merge the build script uses
//! (`capability_policy::merge_config`). The including test crate must provide
//! `crate::capability_inputs` and `crate::capability_policy`.

use crate::capability_inputs::{read_policy_violations, PolicyContext};
use crate::capability_policy::{merge_config, Violation};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The brand overlay of the default distribution.
pub const FILESQUID: &str = "tauri.filesquid.json";

/// The bundling overlay of the macOS early-access DMG, shared by every distribution.
pub const MACOS_DMG: &str = "tauri.bundle.macos-dmg.json";

/// The desktop crate root.
pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read and parse one JSON file of the crate, named relative to its root. Performs filesystem
/// I/O.
///
/// # Panics
/// The file is missing or is not JSON.
pub fn read_crate_json(name: &str) -> Value {
    let path = crate_root().join(name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// Merge `overlays` in order into one JSON value, as the CLI does before it hands the result
/// to the build as `TAURI_CONFIG`. Pure.
pub fn merge_overlays(overlays: &[Value]) -> Value {
    let mut merged = Value::Object(serde_json::Map::new());
    for overlay in overlays {
        merge_config(&mut merged, overlay);
    }
    merged
}

/// The configuration the build sees: `tauri.conf.json` with `overlays` merged in order.
/// Performs filesystem I/O.
pub fn read_effective(overlays: &[Value]) -> Value {
    let mut conf = read_crate_json("tauri.conf.json");
    merge_config(&mut conf, &merge_overlays(overlays));
    conf
}

/// The policy violations of the shipped crate, for `platform`, with `overlays` passed the way
/// the CLI passes them. Performs filesystem I/O.
pub fn read_violations(platform: &str, overlays: &[Value]) -> Vec<Violation> {
    let cli = merge_overlays(overlays).to_string();
    let keys = crate::policy_support::linked_plugin_keys();
    let ctx = PolicyContext {
        root: Path::new(env!("CARGO_MANIFEST_DIR")),
        platform,
        cli_config: Some(&cli),
        plugin_keys: &keys,
        test_build: cfg!(feature = "test-webdriver"),
    };
    read_policy_violations(&ctx).expect("readable policy inputs")
}

/// The string at `pointer` (RFC 6901) in `conf`, if it is one. Pure.
pub fn text_at<'a>(conf: &'a Value, pointer: &str) -> Option<&'a str> {
    conf.pointer(pointer).and_then(Value::as_str)
}

/// The strings of the array at `pointer` in `conf`; empty when it is not an array. Pure.
pub fn list_at(conf: &Value, pointer: &str) -> Vec<String> {
    conf.pointer(pointer)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
