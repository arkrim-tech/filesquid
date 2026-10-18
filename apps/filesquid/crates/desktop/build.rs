//! Tauri build script.
//!
//! The desktop host defines no `#[tauri::command]`s: the UI reaches the FileSquid API through
//! the `filesquid://` custom URI scheme. The only capability (`capabilities/default.json`)
//! grants the main window event listen/unlisten.
//!
//! The app ACL manifest is declared twice over: `tauri_build::AppManifest` is given
//! `capability_policy::APP_COMMANDS` (empty today), and `permissions/app-acl.toml` declares one
//! app permission that allows nothing. Tauri builds an app ACL manifest only when the app has
//! at least one permission (`has_app_manifest` in `tauri-build`), and with one it checks every
//! custom command against the capabilities and refuses it unless a capability grants it;
//! without one it runs the app's own commands for local pages unchecked. If a custom command
//! is ever added, list it in `APP_COMMANDS` (which generates its `allow-`/`deny-`
//! permissions), define it in a submodule, and grant it in a capability.
//!
//! Before `tauri_build`, this script runs `src/capability_inputs.rs` over the capability files,
//! the effective Tauri configuration (`tauri.conf.json`, the platform file and the CLI's
//! `TAURI_CONFIG` merged), `Cargo.toml`, the linked Tauri plugins, the app ACL file and the
//! sources (DE-S-001, DE-S-002), and fails the build on any violation, so a widened capability
//! never compiles.
//!
//! It also creates a stub `frontendDist` so `generate_context!` works before the first Vite
//! build of `apps/filesquid/ui`.

#[allow(dead_code)]
#[path = "src/capability_policy.rs"]
mod capability_policy;

#[allow(dead_code)]
#[path = "src/capability_inputs.rs"]
mod capability_inputs;

use capability_inputs::PolicyContext;
use std::fs;
use std::path::Path;

/// Inputs outside the Tauri configuration whose change must re-run this script.
const WATCHED: &[&str] = &["capabilities", "permissions", "Cargo.toml", "src"];

/// The variable through which the crate's tests learn which Tauri plugins this build links.
const PLUGIN_KEYS_ENV: &str = "FILESQUID_TAURI_PLUGIN_KEYS";

fn main() {
    let keys = plugin_keys();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let cli_config = std::env::var("TAURI_CONFIG").ok();
    let ctx = PolicyContext {
        root: Path::new("."),
        platform: capability_inputs::tauri_platform(&target_os),
        cli_config: cli_config.as_deref(),
        plugin_keys: &keys,
        test_build: std::env::var_os("CARGO_FEATURE_TEST_WEBDRIVER").is_some(),
    };
    announce_inputs(&ctx, &keys);
    enforce_policy(&ctx);
    write_frontend_stub();
    let manifest = tauri_build::AppManifest::new().commands(capability_policy::APP_COMMANDS);
    if let Err(error) =
        tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
    {
        panic!("tauri-build failed: {error:#}");
    }
}

/// Tell Cargo which inputs re-run this script, and hand the linked-plugin keys to the
/// crate's tests.
fn announce_inputs(ctx: &PolicyContext, keys: &[String]) {
    for path in WATCHED {
        println!("cargo:rerun-if-changed={path}");
    }
    for path in capability_inputs::config_paths(ctx) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
    println!("cargo:rustc-env={PLUGIN_KEYS_ENV}={}", keys.join(","));
}

/// The names of the `DEP_<CRATE>_PERMISSION_FILES_PATH` variables Tauri plugins set for this
/// build script, sorted. Reads the environment.
fn plugin_keys() -> Vec<String> {
    let mut keys: Vec<String> = std::env::vars_os()
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .filter(|key| key.starts_with("DEP_") && key.ends_with("_PERMISSION_FILES_PATH"))
        .collect();
    keys.sort();
    keys
}

/// Check every policy input and stop the build on any violation. Performs filesystem I/O.
fn enforce_policy(ctx: &PolicyContext) {
    let violations = match capability_inputs::read_policy_violations(ctx) {
        Ok(violations) => violations,
        Err(message) => panic!("capability policy inputs unreadable: {message}"),
    };
    if !violations.is_empty() {
        let lines: Vec<String> = violations.iter().map(|v| format!("  - {v}")).collect();
        panic!(
            "the desktop capability policy is violated (src/capability_policy.rs):\n{}",
            lines.join("\n")
        );
    }
}

/// Write `../../ui/dist/index.html` when the UI has not been built yet. Performs filesystem
/// I/O.
fn write_frontend_stub() {
    let dist = Path::new("../../ui/dist");
    if !dist.join("index.html").exists() {
        fs::create_dir_all(dist).expect("ui dist dir");
        fs::write(
            dist.join("index.html"),
            "<!doctype html><title>FileSquid</title>\n",
        )
        .expect("ui dist stub");
    }
}
