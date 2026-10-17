//! `wasm32-unknown-unknown` build check for `filesquid-c2pa` (C2PA-S-009).
//!
//! Runs `cargo build` for the library into its own target subdirectory, so it
//! never waits on the build lock held by the surrounding `cargo test`. When the
//! Rust target is not installed the check is skipped with a logged reason.
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
fn target_installed() -> bool {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc)
        .args(["--print", "target-libdir", "--target", WASM_TARGET])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            PathBuf::from(dir).is_dir()
        }
        _ => false,
    }
}

/// Target directory for the check: `$CARGO_TARGET_DIR` (or the workspace
/// `target/`) plus a subdirectory owned by this check.
fn check_target_dir() -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| deps::workspace_manifest().with_file_name("target"));
    base.join("filesquid-c2pa-wasm32-check")
}

/// Build the library for [`WASM_TARGET`], with or without default features.
///
/// Returns `Err` with the compiler output when the build fails.
pub fn build_for_wasm(default_features: bool) -> Result<WasmBuild, String> {
    if !target_installed() {
        return Ok(WasmBuild::Skipped(format!(
            "Rust target {WASM_TARGET} is not installed; run `rustup target add {WASM_TARGET}`"
        )));
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
