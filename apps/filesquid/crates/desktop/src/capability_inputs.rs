//! The I/O half of the desktop capability policy: reads every policy input of the crate and
//! runs the pure checks of `capability_policy` on them (DE-S-001, DE-S-002).
//!
//! This file is not a module of the library. `build.rs` includes it (with
//! `capability_policy`) to fail the build on a violation, and the tests include it to prove
//! the shipped inputs pass, so the build and the tests read exactly the same inputs. The
//! including crate must provide `crate::capability_policy`.

use crate::capability_policy::{self as policy, Violation};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Deepest directory level searched under `capabilities/` and `src/`.
const MAX_DEPTH: usize = 8;

/// The Tauri configuration formats the policy cannot read. Tauri would load these, so their
/// presence fails the check instead of being skipped.
const UNCHECKED_CONFIG_FILES: &[&str] = &["tauri.conf.json5", "Tauri.toml"];

/// What the policy checks besides the crate's files.
pub struct PolicyContext<'a> {
    /// The desktop crate root (where `tauri.conf.json` is).
    pub root: &'a Path,
    /// The target platform as Tauri names it in `tauri.<platform>.conf.json` (`linux`,
    /// `macos`, `windows`, `android`, `ios`).
    pub platform: &'a str,
    /// The JSON the Tauri CLI passes as `TAURI_CONFIG` (`--config`), if any.
    pub cli_config: Option<&'a str>,
    /// The build script's environment variable names (see
    /// `capability_policy::check_linked_plugins`).
    pub plugin_keys: &'a [String],
    /// Whether this is a test-instrumented build (feature `test-webdriver`).
    pub test_build: bool,
}

/// The Tauri platform name for a Cargo `target_os` value (`CARGO_CFG_TARGET_OS` or
/// `std::env::consts::OS`).
pub fn tauri_platform(target_os: &str) -> &str {
    match target_os {
        "macos" | "ios" | "android" | "windows" => target_os,
        _ => "linux",
    }
}

/// Read every policy input under `ctx.root` and return all violations. Performs filesystem
/// I/O.
///
/// # Errors
/// An input could not be read or parsed (a capability file that is not JSON included), so
/// the policy cannot be checked.
pub fn read_policy_violations(ctx: &PolicyContext) -> Result<Vec<Violation>, String> {
    let mut found = Vec::new();
    for path in files_under(&ctx.root.join("capabilities"))? {
        found.extend(policy::check_capability(&read_json(&path)?));
    }
    let (conf, unchecked) = read_effective_config(ctx)?;
    found.extend(unchecked);
    found.extend(policy::check_app_security(&conf));
    found.extend(policy::check_plugin_dependencies(&read_text(
        &ctx.root.join("Cargo.toml"),
    )?));
    found.extend(policy::check_linked_plugins(
        ctx.plugin_keys.iter().map(String::as_str),
        ctx.test_build,
    ));
    let acl = read_optional_text(&ctx.root.join("permissions/app-acl.toml"))?;
    found.extend(policy::check_app_acl(acl.as_deref()));
    found.extend(read_source_violations(&ctx.root.join("src"))?);
    Ok(found)
}

/// The configuration files whose change must re-run the policy check: the base file and the
/// platform file when it exists. Performs filesystem I/O (existence checks).
pub fn config_paths(ctx: &PolicyContext) -> Vec<PathBuf> {
    let platform = ctx.root.join(format!("tauri.{}.conf.json", ctx.platform));
    let mut paths = vec![ctx.root.join("tauri.conf.json")];
    if platform.exists() {
        paths.push(platform);
    }
    paths
}

/// Read the effective Tauri configuration the way Tauri builds it: `tauri.conf.json`, then
/// `tauri.<platform>.conf.json` when present, then the CLI's `TAURI_CONFIG`, each merged with
/// `capability_policy::merge_config`. Also reports JSON5 and TOML configuration files, which
/// the policy cannot read. Performs filesystem I/O.
///
/// # Errors
/// The base file is missing, or a file or the CLI JSON is not valid JSON.
pub fn read_effective_config(ctx: &PolicyContext) -> Result<(Value, Vec<Violation>), String> {
    let mut conf = read_json(&ctx.root.join("tauri.conf.json"))?;
    let platform = ctx.root.join(format!("tauri.{}.conf.json", ctx.platform));
    if platform.exists() {
        policy::merge_config(&mut conf, &read_json(&platform)?);
    }
    if let Some(cli) = ctx.cli_config {
        let overlay = serde_json::from_str(cli).map_err(|e| format!("TAURI_CONFIG: {e}"))?;
        policy::merge_config(&mut conf, &overlay);
    }
    Ok((conf, unchecked_config_files(ctx)))
}

/// The JSON5 and TOML configuration files present in the crate root, base and platform.
/// Performs filesystem I/O (existence checks).
fn unchecked_config_files(ctx: &PolicyContext) -> Vec<Violation> {
    let platform_json5 = format!("tauri.{}.conf.json5", ctx.platform);
    let platform_toml = format!("Tauri.{}.toml", ctx.platform);
    let names = UNCHECKED_CONFIG_FILES
        .iter()
        .copied()
        .chain([platform_json5.as_str(), platform_toml.as_str()]);
    names
        .filter(|name| ctx.root.join(name).exists())
        .map(|name| Violation::UncheckedConfig(name.to_string()))
        .collect()
}

/// Check every `.rs` file under `src` for Tauri command placement, naming each by its path
/// relative to `src` (`lib.rs` is the crate root). Performs filesystem I/O.
fn read_source_violations(src: &Path) -> Result<Vec<Violation>, String> {
    let mut found = Vec::new();
    for path in files_under(src)? {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let relative = path.strip_prefix(src).unwrap_or(&path).to_string_lossy();
        found.extend(policy::check_command_placement(
            &relative.replace('\\', "/"),
            &read_text(&path)?,
        ));
    }
    Ok(found)
}

/// Every regular file under `dir`, at most [`MAX_DEPTH`] levels down, sorted. Performs
/// filesystem I/O.
///
/// # Errors
/// A directory cannot be read, or the tree is deeper than [`MAX_DEPTH`].
pub fn files_under(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), 0usize)];
    while let Some((current, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(format!("{} is nested too deeply", current.display()));
        }
        let entries = fs::read_dir(&current).map_err(|e| format!("{}: {e}", current.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("{}: {e}", current.display()))?
                .path();
            if path.is_dir() {
                pending.push((path, depth + 1));
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Read a file as text. Performs filesystem I/O.
fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Read a file as text, or `None` when it does not exist. Performs filesystem I/O.
fn read_optional_text(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    read_text(path).map(Some)
}

/// Read and parse a JSON file. Performs filesystem I/O.
fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(&read_text(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
