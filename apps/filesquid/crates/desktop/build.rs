//! Tauri build script.
//!
//! The desktop host defines no `#[tauri::command]`s: the UI reaches the FileSquid API through
//! the `filesquid://` custom URI scheme. The only capability (`capabilities/default.json`)
//! grants the main window event listen/unlisten.
//!
//! `permissions/app-acl.toml` declares one app permission that allows nothing. Its only job is
//! to make Tauri build an app ACL manifest, so every custom command is checked against the
//! capabilities and refused unless one grants it (without an app manifest Tauri skips that
//! check for the app's own pages). If a custom command is ever added, list it in
//! `capability_policy::APP_COMMANDS`, define it in a submodule, and grant its permission in a
//! capability.
//!
//! Before `tauri_build::build`, this script checks the capability files, `tauri.conf.json`,
//! `Cargo.toml`, the Tauri plugins linked into the build and the sources against
//! `src/capability_policy.rs` (DE-S-001, DE-S-002) and fails the build on any violation, so a
//! widened capability never compiles.
//!
//! It also creates a stub `frontendDist` so `generate_context!` works before the first Vite
//! build of `apps/filesquid/ui`.

#[allow(dead_code)]
#[path = "src/capability_policy.rs"]
mod capability_policy;

use capability_policy::Violation;
use std::fs;
use std::path::{Path, PathBuf};

/// Files and directories whose change must re-run this script.
const WATCHED: &[&str] = &[
    "capabilities",
    "permissions",
    "tauri.conf.json",
    "Cargo.toml",
    "src",
];

fn main() {
    for path in WATCHED {
        println!("cargo:rerun-if-changed={path}");
    }
    let violations = match check_policy_inputs() {
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
    write_frontend_stub();
    tauri_build::build();
}

/// Deepest directory level searched under `capabilities/` and `src/`.
const MAX_DEPTH: usize = 8;

/// Read every policy input and return the violations. Performs filesystem I/O and reads the
/// build script's environment.
fn check_policy_inputs() -> Result<Vec<Violation>, String> {
    let mut found = Vec::new();
    // Tauri loads every file under capabilities/, in subdirectories too; a file that is not
    // JSON fails here as unreadable, so the check fails closed.
    for path in files_under(Path::new("capabilities"))? {
        found.extend(capability_policy::check_capability(&read_json(&path)?));
    }
    let conf = read_json(Path::new("tauri.conf.json"))?;
    found.extend(capability_policy::check_app_security(&conf));
    found.extend(capability_policy::check_plugin_dependencies(&read_text(
        Path::new("Cargo.toml"),
    )?));
    let keys: Vec<String> = std::env::vars_os()
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .collect();
    let test_build = std::env::var_os("CARGO_FEATURE_TEST_WEBDRIVER").is_some();
    found.extend(capability_policy::check_linked_plugins(
        keys.iter().map(String::as_str),
        test_build,
    ));
    found.extend(check_sources()?);
    Ok(found)
}

/// Check every `.rs` file under `src/` for `#[tauri::command]` placement, naming each by its
/// path relative to `src/` (`lib.rs` is the crate root). Performs filesystem I/O.
fn check_sources() -> Result<Vec<Violation>, String> {
    let src = Path::new("src");
    let mut found = Vec::new();
    for path in files_under(src)? {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let relative = path.strip_prefix(src).unwrap_or(&path).to_string_lossy();
        found.extend(capability_policy::check_command_placement(
            &relative.replace('\\', "/"),
            &read_text(&path)?,
        ));
    }
    Ok(found)
}

/// Every regular file under `dir`, at most [`MAX_DEPTH`] levels down, sorted. Fails when the
/// tree is deeper. Performs filesystem I/O.
fn files_under(dir: &Path) -> Result<Vec<PathBuf>, String> {
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

/// Read and parse a JSON file. Performs filesystem I/O.
fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    serde_json::from_str(&read_text(path)?).map_err(|e| format!("{}: {e}", path.display()))
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
