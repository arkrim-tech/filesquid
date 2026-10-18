//! The desktop host's deny-by-default capability policy (DE-S-001, DE-S-002, DE-S-006;
//! [ADR 002](../../../../../resources/architecture/002-tauri2-desktop-shell.md) minimum IPC
//! policy; [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//!
//! The webview reaches every FileSquid feature through the `filesquid://` scheme, where the
//! API authorises each call. Its only Tauri IPC is listening for the API's events, so the
//! policy grants exactly event listen and unlisten to the main window. Filesystem and SQLite
//! access stay in the Rust host (`filesquid-native`, scoped to the app data directory) and are
//! never granted to the webview as Tauri permissions: no fs, SQL, shell, dialog, HTTP, process
//! or opener plugin, no `core:path` (which returns absolute paths), no asset protocol, no remote
//! pages, no custom app command.
//!
//! Every check here is pure: it takes parsed JSON, file text or environment variable names
//! and returns the [`Violation`]s it finds. `build.rs` includes this file and fails the build
//! when the capability files, `tauri.conf.json`, `Cargo.toml`, the linked Tauri plugins or the
//! sources break the policy; the
//! integration tests run the same checks. Widening the webview's access therefore means
//! changing this file in a reviewed change, never only a capability file.

use serde_json::Value;
use std::fmt;

/// The windows a capability may name. Only the main app window exists.
pub const APP_WINDOWS: &[&str] = &["main"];

/// The capabilities `tauri.conf.json` enables, by identifier.
pub const ENABLED_CAPABILITIES: &[&str] = &["default"];

/// Every Tauri permission the policy grants: the main window may listen for and stop
/// listening for the API's events (`filesquid-event`), and nothing else.
pub const GRANTED_PERMISSIONS: &[&str] = &["core:event:allow-listen", "core:event:allow-unlisten"];

/// Plugins whose permissions would give the webview file, SQL, program, dialog, network or
/// URL-opening access outside the API. Their permissions are refused by name.
pub const FORBIDDEN_PLUGINS: &[&str] =
    &["fs", "shell", "sql", "dialog", "http", "process", "opener"];

/// Custom app commands (`#[tauri::command]`) the host defines. Empty: the UI uses the API
/// through the scheme. A command added here also needs a capability that grants it.
pub const APP_COMMANDS: &[&str] = &[];

/// Tauri plugin crates the manifest may depend on. Each MUST be an optional dependency that
/// only a test-instrumented build enables.
pub const TEST_ONLY_PLUGIN_CRATES: &[&str] = &["tauri-plugin-wdio-webdriver"];

/// One way a capability file, the Tauri configuration, the manifest or a source file breaks
/// the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// The document does not have the expected shape; the text says what is missing.
    Malformed(String),
    /// A capability names a window that is not in [`APP_WINDOWS`] (a wildcard included).
    UnknownWindow(String),
    /// A capability targets webview labels instead of windows.
    WebviewTargets,
    /// A capability grants permissions to remote pages.
    RemoteContext,
    /// A capability grants a permission that is not in [`GRANTED_PERMISSIONS`].
    PermissionNotGranted(String),
    /// A capability grants a permission of a plugin in [`FORBIDDEN_PLUGINS`].
    ForbiddenPlugin(String),
    /// A capability file has an identifier that is not in [`ENABLED_CAPABILITIES`].
    CapabilityNotEnabled(String),
    /// `app.security.capabilities` is not exactly [`ENABLED_CAPABILITIES`].
    CapabilityList(Vec<String>),
    /// `app.security.capabilities` defines a capability inline.
    InlineCapability,
    /// `app.withGlobalTauri` is enabled.
    GlobalTauri,
    /// The asset protocol is enabled (config or the `protocol-asset` Cargo feature).
    AssetProtocol,
    /// The manifest depends on a Tauri plugin outside [`TEST_ONLY_PLUGIN_CRATES`], or on a
    /// test-only plugin as a non-optional dependency.
    PluginDependency(String),
    /// A Tauri plugin crate other than a core plugin is linked into the build (a test-only
    /// plugin is allowed in a test-instrumented build).
    LinkedPlugin(String),
    /// A `#[tauri::command]` is defined in `lib.rs`, next to the handler.
    CommandInLib(String),
    /// A `#[tauri::command]` is defined that is not in [`APP_COMMANDS`].
    UnlistedCommand(String),
    /// `tauri.conf.json` (or an overlay) creates a window whose label is not in
    /// [`APP_WINDOWS`].
    ConfiguredWindow(String),
    /// The app ACL manifest (`permissions/app-acl.toml`) is missing or allows a command; the
    /// text says which.
    AppAcl(String),
    /// A Tauri configuration file in a format the policy cannot read (JSON5 or TOML) exists,
    /// so the effective configuration cannot be checked.
    UncheckedConfig(String),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed policy input: {what}"),
            Self::UnknownWindow(w) => write!(f, "capability names window {w:?}; only {APP_WINDOWS:?} may be named"),
            Self::WebviewTargets => f.write_str("capability targets webview labels; name app windows only"),
            Self::RemoteContext => f.write_str("capability grants IPC to remote pages; remove its `remote` block"),
            Self::PermissionNotGranted(p) => write!(f, "permission {p:?} is not granted by the policy; the API serves this through the filesquid:// scheme"),
            Self::ForbiddenPlugin(p) => write!(f, "permission {p:?} belongs to a forbidden plugin; file, SQL and program access stay in the Rust host"),
            Self::CapabilityNotEnabled(id) => write!(f, "capability {id:?} is not enabled; only {ENABLED_CAPABILITIES:?} may exist"),
            Self::CapabilityList(list) => write!(f, "app.security.capabilities is {list:?}; it must be {ENABLED_CAPABILITIES:?}"),
            Self::InlineCapability => f.write_str("app.security.capabilities defines a capability inline; use capabilities/*.json"),
            Self::GlobalTauri => f.write_str("app.withGlobalTauri is enabled; set it to false"),
            Self::AssetProtocol => f.write_str("the asset protocol is enabled; file bytes go through the API content route"),
            Self::PluginDependency(c) => write!(f, "manifest depends on {c:?}; only optional test-only plugins {TEST_ONLY_PLUGIN_CRATES:?} are allowed"),
            Self::LinkedPlugin(c) => write!(f, "the build links the Tauri plugin {c:?}; only core plugins (and {TEST_ONLY_PLUGIN_CRATES:?} in test builds) may be linked"),
            Self::CommandInLib(c) => write!(f, "Tauri command {c:?} is defined in lib.rs; define commands in a submodule"),
            Self::UnlistedCommand(c) => write!(f, "Tauri command {c:?} is not in APP_COMMANDS; list it and grant it in a capability"),
            Self::ConfiguredWindow(w) => write!(f, "the Tauri configuration creates window {w:?}; only {APP_WINDOWS:?} may exist"),
            Self::AppAcl(what) => write!(f, "permissions/app-acl.toml {what}; it must declare one permission with `commands.allow = []`"),
            Self::UncheckedConfig(file) => write!(f, "{file} cannot be checked by the capability policy; use JSON (tauri.<platform>.conf.json)"),
        }
    }
}

/// Check one capability document (`capabilities/*.json`) against the policy.
pub fn check_capability(doc: &Value) -> Vec<Violation> {
    let mut found = check_capability_targets(doc);
    match doc.get("permissions").and_then(Value::as_array) {
        Some(permissions) => found.extend(permissions.iter().filter_map(check_permission)),
        None => found.push(Violation::Malformed(
            "capability has no permissions list".into(),
        )),
    }
    found
}

/// Check a capability's identifier, windows, webviews and remote context.
fn check_capability_targets(doc: &Value) -> Vec<Violation> {
    let mut found = Vec::new();
    let identifier = doc
        .get("identifier")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !ENABLED_CAPABILITIES.contains(&identifier) {
        found.push(Violation::CapabilityNotEnabled(identifier.to_string()));
    }
    match string_list(doc.get("windows"), "windows") {
        Ok(windows) => found.extend(
            windows
                .into_iter()
                .filter(|w| !APP_WINDOWS.contains(&w.as_str()))
                .map(Violation::UnknownWindow),
        ),
        Err(violation) => found.push(violation),
    }
    match string_list(doc.get("webviews"), "webviews") {
        Ok(webviews) if webviews.is_empty() => {}
        Ok(_) => found.push(Violation::WebviewTargets),
        Err(violation) => found.push(violation),
    }
    if doc.get("remote").is_some() {
        found.push(Violation::RemoteContext);
    }
    found
}

/// Check one permission entry: a permission identifier, or an object with an `identifier`
/// (a scoped permission). Any other entry is malformed.
fn check_permission(entry: &Value) -> Option<Violation> {
    let Some(identifier) = entry
        .as_str()
        .or_else(|| entry.get("identifier").and_then(Value::as_str))
    else {
        return Some(Violation::Malformed(format!(
            "permission entry {entry} is neither an identifier nor an object with one"
        )));
    };
    let plugin = identifier.split(':').next().unwrap_or_default();
    if FORBIDDEN_PLUGINS.contains(&plugin) {
        return Some(Violation::ForbiddenPlugin(identifier.to_string()));
    }
    let granted = entry.is_string() && GRANTED_PERMISSIONS.contains(&identifier);
    (!granted).then(|| Violation::PermissionNotGranted(identifier.to_string()))
}

/// Check the `app` block of the effective Tauri configuration (`tauri.conf.json` with its
/// overlays merged, see [`merge_config`]): the windows it creates, the enabled capability
/// list, the global Tauri object and the asset protocol.
pub fn check_app_security(conf: &Value) -> Vec<Violation> {
    let app = conf.get("app").unwrap_or(&Value::Null);
    let security = app.get("security").unwrap_or(&Value::Null);
    let mut found = check_configured_windows(app);
    let list = security.get("capabilities").and_then(Value::as_array);
    let entries = list.map(Vec::as_slice).unwrap_or_default();
    if entries.iter().any(|entry| !entry.is_string()) {
        found.push(Violation::InlineCapability);
    } else {
        let names: Vec<String> = entries
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
        if names != ENABLED_CAPABILITIES {
            found.push(Violation::CapabilityList(names));
        }
    }
    if app.get("withGlobalTauri").and_then(Value::as_bool) != Some(false) {
        found.push(Violation::GlobalTauri);
    }
    let asset = security.get("assetProtocol").and_then(|a| a.get("enable"));
    if asset.and_then(Value::as_bool) == Some(true) {
        found.push(Violation::AssetProtocol);
    }
    found
}

/// Check the windows `app.windows` creates: each label (Tauri's default label is `main`)
/// must be in [`APP_WINDOWS`].
fn check_configured_windows(app: &Value) -> Vec<Violation> {
    let Some(windows) = app.get("windows") else {
        return Vec::new();
    };
    let Some(windows) = windows.as_array() else {
        return vec![Violation::Malformed("`app.windows` is not a list".into())];
    };
    windows
        .iter()
        .map(|w| w.get("label").and_then(Value::as_str).unwrap_or("main"))
        .filter(|label| !APP_WINDOWS.contains(label))
        .map(|label| Violation::ConfiguredWindow(label.to_string()))
        .collect()
}

/// Merge a Tauri configuration overlay into `base` the way Tauri does (JSON Merge Patch,
/// RFC 7396): objects merge key by key, `null` removes a key, anything else replaces.
/// Tauri applies the platform file (`tauri.<platform>.conf.json`) and then the CLI's
/// `--config` (passed to the build as `TAURI_CONFIG`), so the policy checks the result.
pub fn merge_config(base: &mut Value, overlay: &Value) {
    let Some(patch) = overlay.as_object() else {
        *base = overlay.clone();
        return;
    };
    if !base.is_object() {
        *base = Value::Object(serde_json::Map::new());
    }
    let Some(target) = base.as_object_mut() else {
        return;
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge_config(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// Check the app ACL manifest text (`permissions/app-acl.toml`, `None` when it is missing):
/// it must declare at least one permission, so Tauri builds an app ACL manifest and checks
/// every custom command, and every `commands.allow` in it must be empty. Line-based TOML
/// reading, matching the file's dotted-key style; any other `allow` key is refused.
pub fn check_app_acl(text: Option<&str>) -> Vec<Violation> {
    let Some(text) = text else {
        return vec![Violation::AppAcl("is missing".into())];
    };
    let code: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .collect();
    let mut found = Vec::new();
    if !code.contains(&"[[permission]]") {
        found.push(Violation::AppAcl("declares no permission".into()));
    }
    let allows: Vec<&&str> = code.iter().filter(|l| l.contains("allow")).collect();
    if allows.is_empty() {
        found.push(Violation::AppAcl("has no `commands.allow = []`".into()));
    }
    found.extend(
        allows
            .into_iter()
            .filter(|l| **l != "commands.allow = []")
            .map(|l| Violation::AppAcl(format!("has `{l}`"))),
    );
    found
}

/// Check the crate manifest text for Tauri plugin dependencies and the asset protocol
/// feature. Line-based: a test-only plugin must be declared on one line with
/// `optional = true`, as in this crate; a plugin in a `[dependencies.<name>]` table is always
/// refused. [`check_linked_plugins`] checks the plugins actually linked, whatever the syntax.
pub fn check_plugin_dependencies(manifest: &str) -> Vec<Violation> {
    let mut found = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.contains("protocol-asset") {
            found.push(Violation::AssetProtocol);
        }
        if let Some(name) = plugin_table_name(line).or_else(|| renamed_plugin_package(line)) {
            found.push(Violation::PluginDependency(name));
            continue;
        }
        let name = line.split(['=', ' ', '.']).next().unwrap_or_default();
        if !name.starts_with("tauri-plugin-") {
            continue;
        }
        let test_only = TEST_ONLY_PLUGIN_CRATES.contains(&name);
        if !test_only || !line.contains("optional = true") {
            found.push(Violation::PluginDependency(name.to_string()));
        }
    }
    found
}

/// The plugin crate a table header such as `[dependencies.tauri-plugin-fs]` declares.
fn plugin_table_name(line: &str) -> Option<String> {
    if !line.starts_with('[') {
        return None;
    }
    let start = line.find("tauri-plugin-")?;
    let name: String = line[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    Some(name)
}

/// The plugin crate a renamed dependency such as `fs = { package = "tauri-plugin-fs" }`
/// declares.
fn renamed_plugin_package(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("package")?;
    let start = rest.find("tauri-plugin-")?;
    let name: String = rest[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    Some(name)
}

/// Check the Tauri plugins linked into the build. `env_keys` are the build script's
/// environment variable names: every Tauri plugin publishes its permission files to dependent
/// build scripts as `DEP_<CRATE>_PERMISSION_FILES_PATH` (core plugins carry the
/// `__CORE_PLUGIN__` token), which is how `tauri-build` finds them. Every linked plugin that is
/// not a core plugin is refused, except a [`TEST_ONLY_PLUGIN_CRATES`] plugin in a
/// test-instrumented build (`test_build`).
pub fn check_linked_plugins<'a>(
    env_keys: impl IntoIterator<Item = &'a str>,
    test_build: bool,
) -> Vec<Violation> {
    let mut found: Vec<Violation> = env_keys
        .into_iter()
        .filter_map(linked_plugin_crate)
        .filter(|name| !(test_build && TEST_ONLY_PLUGIN_CRATES.contains(&name.as_str())))
        .map(Violation::LinkedPlugin)
        .collect();
    found.sort_by_key(ToString::to_string);
    found
}

/// The plugin crate (`tauri-plugin-…`) an environment key announces, or `None` for any other
/// key and for core plugins.
fn linked_plugin_crate(key: &str) -> Option<String> {
    let crate_var = key
        .strip_prefix("DEP_")?
        .strip_suffix("_PERMISSION_FILES_PATH")?;
    if crate_var.ends_with("__CORE_PLUGIN__") {
        return None;
    }
    Some(crate_var.to_lowercase().replace('_', "-"))
}

/// Check one source file for Tauri commands: none may be in `lib.rs`, and each elsewhere
/// must be listed in [`APP_COMMANDS`]. `file_name` is the path relative to `src/` (`lib.rs`
/// is the crate root). A command is marked by any code line that names `tauri::command`
/// (`#[tauri::command]`, `#[cfg_attr(…, tauri::command)]`, a multi-line attribute) or, in a
/// file that imports it, by a `#[command…]` attribute; comment lines do not count, and the
/// import itself is not a command. The command's name is the next function declared.
pub fn check_command_placement(file_name: &str, source: &str) -> Vec<Violation> {
    let imported = source
        .lines()
        .map(str::trim)
        .any(|l| l.starts_with("use ") && l.contains(COMMAND_PATH));
    let mut found = Vec::new();
    let mut lines = source.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if !is_command_marker(line, imported) {
            continue;
        }
        let name = lines.find_map(command_name).unwrap_or_default();
        if file_name == "lib.rs" {
            found.push(Violation::CommandInLib(name));
        } else if !APP_COMMANDS.contains(&name.as_str()) {
            found.push(Violation::UnlistedCommand(name));
        }
    }
    found
}

/// The attribute path of a Tauri command, spelled with `concat!` so this file's own code
/// never names it and so never marks itself.
const COMMAND_PATH: &str = concat!("tauri", "::", "command");

/// Whether the trimmed `line` marks a Tauri command; `imported` says whether the file
/// imports `tauri::command`, which makes a bare `#[command]` attribute one.
fn is_command_marker(line: &str, imported: bool) -> bool {
    let comment = line.starts_with("//") || line.starts_with("/*") || line.starts_with('*');
    if comment || line.starts_with("use ") {
        return false;
    }
    line.contains(COMMAND_PATH) || (imported && line.starts_with("#[command"))
}

/// The function name declared on `line`, if it declares one (`fn`, `pub fn`, `async fn`, …).
fn command_name(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The strings of the optional JSON array `field`: an absent field is an empty list, and
/// anything but an array of strings is malformed.
fn string_list(value: Option<&Value>, field: &str) -> Result<Vec<String>, Violation> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let malformed = || Violation::Malformed(format!("`{field}` is not a list of strings"));
    let items = value.as_array().ok_or_else(malformed)?;
    items
        .iter()
        .map(|item| item.as_str().map(String::from).ok_or_else(malformed))
        .collect()
}
