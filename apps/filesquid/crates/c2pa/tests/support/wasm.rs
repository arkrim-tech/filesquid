//! `wasm32-unknown-unknown` build check for `filesquid-c2pa` (C2PA-S-009).
//!
//! Runs `cargo build` for the library into its own target subdirectory, so it
//! never waits on the build lock held by the surrounding `cargo test`. A
//! missing Rust target fails the check; it is skipped only when the caller
//! sets `FILESQUID_SKIP_WASM32=1`.
//! A test crate that includes this module also includes `support/deps.rs` as
//! `mod deps`.

use std::path::PathBuf;
use std::process::Command;

use crate::deps;

/// The Rust target the browser host uses.
pub const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Outcome of one build check.
#[derive(Debug, PartialEq, Eq)]
pub enum WasmBuild {
    /// The library compiled for the target.
    Built,
    /// The check could not run; the string says why.
    Skipped(String),
}

/// Return whether the Rust standard library for [`WASM_TARGET`] is installed.
///
/// A `rustc` that cannot be started or that exits with an error is `Err`
/// with the real cause, not reported as a missing target.
fn target_installed() -> Result<bool, String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let out = Command::new(&rustc)
        .args(["--print", "target-libdir", "--target", WASM_TARGET])
        .output()
        .map_err(|err| format!("could not run `{rustc}`: {err}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{rustc} --print target-libdir --target {WASM_TARGET}` failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(PathBuf::from(dir).is_dir())
}

/// Target directory for the check: `$CARGO_TARGET_DIR` (or the workspace
/// `target/`) plus a subdirectory owned by this check.
fn check_target_dir() -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| deps::workspace_manifest().with_file_name("target"));
    base.join("filesquid-c2pa-wasm32-check")
}

/// Environment variable that, set to `1`, skips the check explicitly.
pub const SKIP_ENV: &str = "FILESQUID_SKIP_WASM32";

/// Return the skip reason when the caller opted out with [`SKIP_ENV`]`=1`.
pub fn opted_out() -> Option<String> {
    (std::env::var(SKIP_ENV).as_deref() == Ok("1"))
        .then(|| format!("{SKIP_ENV}=1 is set; the {WASM_TARGET} build was not checked"))
}

/// Build the library for [`WASM_TARGET`], with or without default features.
///
/// Returns [`WasmBuild::Skipped`] only when the caller opted out with
/// [`SKIP_ENV`]. A missing target or a failed build is `Err`, so the gate
/// cannot pass without the check running.
pub fn build_for_wasm(default_features: bool) -> Result<WasmBuild, String> {
    if let Some(reason) = opted_out() {
        return Ok(WasmBuild::Skipped(reason));
    }
    if !target_installed()? {
        return Err(format!(
            "Rust target {WASM_TARGET} is not installed: run `rustup target add {WASM_TARGET}` \
             (or set {SKIP_ENV}=1 to skip this check explicitly)"
        ));
    }
    let mut command = Command::new(deps::cargo_binary());
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(deps::workspace_manifest())
        .args([
            "-p",
            deps::SELF_NAME,
            "--lib",
            "--locked",
            "--target",
            WASM_TARGET,
        ])
        .arg("--target-dir")
        .arg(check_target_dir());
    if !default_features {
        command.arg("--no-default-features");
    }
    let output = command
        .output()
        .map_err(|err| format!("run cargo: {err}"))?;
    if output.status.success() {
        return Ok(WasmBuild::Built);
    }
    Err(String::from_utf8_lossy(&output.stderr).into_owned())
}
