//! `wasm32-unknown-unknown` build check for `filesquid-prov` (PROV-S-001).
//!
//! Runs `cargo check` for the library into its own target subdirectory, so it
//! never waits on the build lock held by the surrounding `cargo test`. A
//! missing Rust target fails the check; it is skipped only when the caller
//! sets `FILESQUID_SKIP_WASM32=1`.

use std::path::PathBuf;
use std::process::Command;

/// The Rust target the browser host uses.
pub const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Environment variable that, set to `1`, skips the check explicitly.
pub const SKIP_ENV: &str = "FILESQUID_SKIP_WASM32";

/// Return the skip reason when the caller opted out with [`SKIP_ENV`]`=1`.
pub fn opted_out() -> Option<String> {
    (std::env::var(SKIP_ENV).as_deref() == Ok("1"))
        .then(|| format!("{SKIP_ENV}=1 is set; the {WASM_TARGET} build was not checked"))
}

/// Return whether the Rust standard library for [`WASM_TARGET`] is installed.
fn target_installed() -> Result<bool, String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let out = Command::new(&rustc)
        .args(["--print", "target-libdir", "--target", WASM_TARGET])
        .output()
        .map_err(|err| format!("could not run `{rustc}`: {err}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{rustc} --print target-libdir` failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(PathBuf::from(dir).is_dir())
}

/// Workspace manifest two levels above this crate (`apps/filesquid/Cargo.toml`).
fn workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")
}

/// Target directory for the check: `$CARGO_TARGET_DIR` (or the workspace
/// `target/`) plus a subdirectory owned by this check.
fn check_target_dir() -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_manifest().with_file_name("target"));
    base.join("filesquid-prov-wasm32-check")
}

/// Check the library for [`WASM_TARGET`] with its default features.
///
/// A missing target or a failed check is `Err`, so the gate cannot pass
/// without the check running; only [`SKIP_ENV`] skips it, and `main` then
/// filters the `@wasm32` scenarios out and says so.
pub fn check_for_wasm() -> Result<(), String> {
    if !target_installed()? {
        return Err(format!(
            "Rust target {WASM_TARGET} is not installed: run `rustup target add {WASM_TARGET}` \
             (or set {SKIP_ENV}=1 to skip this check explicitly)"
        ));
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["check", "--manifest-path"])
        .arg(workspace_manifest())
        .args(["-p", "filesquid-prov", "--lib", "--locked"])
        .args(["--target", WASM_TARGET])
        .arg("--target-dir")
        .arg(check_target_dir())
        .output()
        .map_err(|err| format!("run cargo: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).into_owned())
}
