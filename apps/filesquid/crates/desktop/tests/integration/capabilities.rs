//! Integration tests for the desktop capability policy (Task DE-0.1; DE-S-001, DE-S-002,
//! DE-S-006; ADR 002 minimum IPC policy): the shipped capability files, Tauri configuration,
//! crate manifest and sources pass the deny-by-default policy, and every way of widening the
//! webview's access is refused. The same checks run in `build.rs`, so a widened capability
//! fails the build.

use filesquid_desktop_lib::capability_policy::{
    check_app_security, check_capability, check_command_placement, check_linked_plugins,
    check_plugin_dependencies, Violation, APP_COMMANDS, APP_WINDOWS, ENABLED_CAPABILITIES,
    FORBIDDEN_PLUGINS, GRANTED_PERMISSIONS,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The desktop crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read and parse one JSON file of the crate. Performs filesystem I/O.
fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).expect("readable JSON file");
    serde_json::from_str(&text).expect("valid JSON")
}

/// Every regular file under `dir`, at most eight levels down, sorted. Performs filesystem I/O.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), 0usize)];
    while let Some((current, depth)) = pending.pop() {
        assert!(depth <= 8, "{} is nested too deeply", current.display());
        for entry in std::fs::read_dir(&current).expect("readable directory") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                pending.push((path, depth + 1));
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Every capability file under `capabilities/`, subdirectories included. Performs
/// filesystem I/O.
fn read_capability_files() -> Vec<(PathBuf, Value)> {
    files_under(&crate_root().join("capabilities"))
        .into_iter()
        .map(|p| (p.clone(), read_json(&p)))
        .collect()
}

/// A capability for the main window granting `permissions`.
fn main_window_capability(permissions: Value) -> Value {
    json!({ "identifier": "default", "windows": ["main"], "permissions": permissions })
}

#[test]
fn the_policy_grants_event_listening_on_the_main_window_only() {
    assert_eq!(APP_WINDOWS, ["main"]);
    assert_eq!(
        GRANTED_PERMISSIONS,
        ["core:event:allow-listen", "core:event:allow-unlisten"]
    );
    assert_eq!(ENABLED_CAPABILITIES, ["default"]);
}

#[test]
fn the_policy_defines_no_app_command() {
    assert!(APP_COMMANDS.is_empty());
}

#[test]
fn file_shell_sql_and_dialog_plugins_are_forbidden() {
    for plugin in ["fs", "shell", "sql", "dialog", "http", "process", "opener"] {
        assert!(
            FORBIDDEN_PLUGINS.contains(&plugin),
            "{plugin} is not forbidden"
        );
    }
}

#[test]
fn every_shipped_capability_file_passes_the_policy() {
    let files = read_capability_files();
    assert!(!files.is_empty(), "no capability file found");
    for (path, doc) in files {
        assert_eq!(check_capability(&doc), Vec::new(), "{}", path.display());
    }
}

#[test]
fn the_shipped_capability_grants_exactly_the_policy() {
    let doc = read_json(&crate_root().join("capabilities/default.json"));
    assert_eq!(doc["windows"], json!(["main"]));
    assert_eq!(doc["permissions"], json!(GRANTED_PERMISSIONS));
}

#[test]
fn the_shipped_tauri_configuration_passes_the_policy() {
    let conf = read_json(&crate_root().join("tauri.conf.json"));
    assert_eq!(check_app_security(&conf), Vec::new());
}

#[test]
fn the_shipped_manifest_has_no_forbidden_tauri_plugin() {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml")).expect("manifest");
    assert_eq!(check_plugin_dependencies(&manifest), Vec::new());
}

#[test]
fn the_shipped_sources_define_no_unlisted_tauri_command() {
    let src = crate_root().join("src");
    for path in files_under(&src) {
        let relative = path
            .strip_prefix(&src)
            .expect("under src")
            .to_string_lossy();
        let source = std::fs::read_to_string(&path).expect("source file");
        assert_eq!(
            check_command_placement(&relative, &source),
            Vec::new(),
            "{relative}"
        );
    }
}

#[test]
fn core_plugins_and_other_variables_are_not_linked_plugins() {
    let keys = [
        "DEP_TAURI_EVENT__CORE_PLUGIN___PERMISSION_FILES_PATH",
        "DEP_TAURI_PATH__CORE_PLUGIN___PERMISSION_FILES_PATH",
        "CARGO_PKG_NAME",
        "DEP_TAURI_DEV",
    ];
    assert_eq!(check_linked_plugins(keys, false), Vec::new());
}

#[test]
fn a_linked_file_system_or_sql_plugin_is_refused_however_it_is_declared() {
    let keys = [
        "DEP_TAURI_PLUGIN_SQL_PERMISSION_FILES_PATH",
        "DEP_TAURI_PLUGIN_FS_PERMISSION_FILES_PATH",
    ];
    assert_eq!(
        check_linked_plugins(keys, true),
        vec![
            Violation::LinkedPlugin("tauri-plugin-fs".to_string()),
            Violation::LinkedPlugin("tauri-plugin-sql".to_string())
        ]
    );
}

#[test]
fn the_test_driver_plugin_is_linked_only_in_test_builds() {
    let keys = ["DEP_TAURI_PLUGIN_WDIO_WEBDRIVER_PERMISSION_FILES_PATH"];
    assert_eq!(check_linked_plugins(keys, true), Vec::new());
    assert_eq!(
        check_linked_plugins(keys, false),
        vec![Violation::LinkedPlugin(
            "tauri-plugin-wdio-webdriver".to_string()
        )]
    );
}

#[test]
fn a_plugin_declared_as_a_dependency_table_is_refused() {
    let manifest = "[dependencies.tauri-plugin-fs]\nversion = \"2\"\n";
    assert_eq!(
        check_plugin_dependencies(manifest),
        vec![Violation::PluginDependency("tauri-plugin-fs".to_string())]
    );
}

#[test]
fn a_malformed_permission_entry_is_refused() {
    let doc = main_window_capability(json!([42]));
    assert!(matches!(
        check_capability(&doc).as_slice(),
        [Violation::Malformed(_)]
    ));
}

#[test]
fn a_window_list_that_is_not_a_list_of_strings_is_refused() {
    for windows in [json!("*"), json!([7])] {
        let doc = json!({ "identifier": "default", "windows": windows, "permissions": [] });
        assert!(
            matches!(check_capability(&doc).as_slice(), [Violation::Malformed(_)]),
            "{windows}"
        );
    }
}

#[test]
fn an_app_acl_manifest_makes_tauri_check_every_custom_command() {
    let acl = std::fs::read_to_string(crate_root().join("permissions/app-acl.toml"))
        .expect("permissions/app-acl.toml declares the app ACL manifest");
    assert!(acl.contains("[[permission]]"));
    assert!(
        acl.contains("allow = []"),
        "the app permission must allow nothing"
    );
}

#[test]
fn a_file_system_permission_is_refused_as_a_forbidden_plugin() {
    let doc = main_window_capability(json!(["core:event:allow-listen", "fs:default"]));
    assert_eq!(
        check_capability(&doc),
        vec![Violation::ForbiddenPlugin("fs:default".to_string())]
    );
}

#[test]
fn sql_shell_and_dialog_permissions_are_refused() {
    for id in ["sql:default", "shell:allow-execute", "dialog:allow-open"] {
        let doc = main_window_capability(json!([id]));
        assert_eq!(
            check_capability(&doc),
            vec![Violation::ForbiddenPlugin(id.to_string())],
            "{id}"
        );
    }
}

#[test]
fn a_scoped_file_system_permission_object_is_refused() {
    let doc = main_window_capability(json!([
        { "identifier": "fs:allow-read-text-file", "allow": [{ "path": "$HOME/**" }] }
    ]));
    assert_eq!(
        check_capability(&doc),
        vec![Violation::ForbiddenPlugin(
            "fs:allow-read-text-file".to_string()
        )]
    );
}

#[test]
fn a_permission_that_returns_filesystem_paths_is_refused() {
    let doc = main_window_capability(json!(["core:path:default"]));
    assert_eq!(
        check_capability(&doc),
        vec![Violation::PermissionNotGranted(
            "core:path:default".to_string()
        )]
    );
}

#[test]
fn emitting_events_from_the_webview_is_not_granted() {
    let doc = main_window_capability(json!(["core:event:default"]));
    assert_eq!(
        check_capability(&doc),
        vec![Violation::PermissionNotGranted(
            "core:event:default".to_string()
        )]
    );
}

#[test]
fn a_capability_for_every_window_is_refused() {
    let doc = json!({ "identifier": "default", "windows": ["*"], "permissions": [] });
    assert_eq!(
        check_capability(&doc),
        vec![Violation::UnknownWindow("*".to_string())]
    );
}

#[test]
fn a_capability_for_webview_labels_is_refused() {
    let doc = json!({
        "identifier": "default", "windows": ["main"], "webviews": ["preview"], "permissions": []
    });
    assert_eq!(check_capability(&doc), vec![Violation::WebviewTargets]);
}

#[test]
fn a_capability_for_remote_pages_is_refused() {
    let doc = json!({
        "identifier": "default",
        "windows": ["main"],
        "remote": { "urls": ["https://*.example.com"] },
        "permissions": ["core:event:allow-listen"]
    });
    assert_eq!(check_capability(&doc), vec![Violation::RemoteContext]);
}

#[test]
fn a_capability_without_a_permission_list_is_malformed() {
    let doc = json!({ "identifier": "default", "windows": ["main"] });
    assert!(matches!(
        check_capability(&doc).as_slice(),
        [Violation::Malformed(_)]
    ));
}

#[test]
fn an_unlisted_capability_identifier_is_refused() {
    let doc = json!({ "identifier": "extra", "windows": ["main"], "permissions": [] });
    assert_eq!(
        check_capability(&doc),
        vec![Violation::CapabilityNotEnabled("extra".to_string())]
    );
}

#[test]
fn a_configuration_that_enables_another_capability_is_refused() {
    let conf = json!({ "app": { "withGlobalTauri": false,
        "security": { "capabilities": ["default", "extra"] } } });
    assert_eq!(
        check_app_security(&conf),
        vec![Violation::CapabilityList(vec![
            "default".to_string(),
            "extra".to_string()
        ])]
    );
}

#[test]
fn an_inline_capability_in_the_configuration_is_refused() {
    let conf = json!({ "app": { "withGlobalTauri": false, "security": { "capabilities": [
        { "identifier": "inline", "windows": ["*"], "permissions": ["fs:default"] }
    ] } } });
    assert_eq!(check_app_security(&conf), vec![Violation::InlineCapability]);
}

#[test]
fn a_configuration_with_no_capability_list_is_refused() {
    let conf = json!({ "app": { "withGlobalTauri": false, "security": {} } });
    assert_eq!(
        check_app_security(&conf),
        vec![Violation::CapabilityList(Vec::new())]
    );
}

#[test]
fn the_global_tauri_object_is_refused() {
    let conf = json!({ "app": { "withGlobalTauri": true,
        "security": { "capabilities": ["default"] } } });
    assert_eq!(check_app_security(&conf), vec![Violation::GlobalTauri]);
}

#[test]
fn the_asset_protocol_is_refused_in_the_configuration() {
    let conf = json!({ "app": { "withGlobalTauri": false, "security": {
        "capabilities": ["default"],
        "assetProtocol": { "enable": true, "scope": ["$HOME/**"] } } } });
    assert_eq!(check_app_security(&conf), vec![Violation::AssetProtocol]);
}

#[test]
fn a_file_system_plugin_dependency_is_refused() {
    let manifest = "[dependencies]\ntauri = { version = \"2\", features = [] }\n\
                    tauri-plugin-fs = \"2\"\ntauri-plugin-sql = { version = \"2\" }\n";
    assert_eq!(
        check_plugin_dependencies(manifest),
        vec![
            Violation::PluginDependency("tauri-plugin-fs".to_string()),
            Violation::PluginDependency("tauri-plugin-sql".to_string())
        ]
    );
}

#[test]
fn the_test_driver_plugin_must_stay_optional() {
    let manifest = "[dependencies]\ntauri-plugin-wdio-webdriver = \"1.5.0\"\n";
    assert_eq!(
        check_plugin_dependencies(manifest),
        vec![Violation::PluginDependency(
            "tauri-plugin-wdio-webdriver".to_string()
        )]
    );
}

#[test]
fn the_asset_protocol_feature_is_refused_in_the_manifest() {
    let manifest = "[dependencies]\ntauri = { version = \"2\", features = [\"protocol-asset\"] }\n";
    assert_eq!(
        check_plugin_dependencies(manifest),
        vec![Violation::AssetProtocol]
    );
}

#[test]
fn a_tauri_command_in_lib_rs_is_refused() {
    let source = "#[tauri::command]\nfn health_ping() -> String { String::new() }\n";
    assert_eq!(
        check_command_placement("lib.rs", source),
        vec![Violation::CommandInLib("health_ping".to_string())]
    );
}

#[test]
fn an_unlisted_tauri_command_in_a_submodule_is_refused() {
    let source = "    #[tauri::command(rename_all = \"snake_case\")]\n    pub async fn read_any_file(path: String) {}\n";
    assert_eq!(
        check_command_placement("files.rs", source),
        vec![Violation::UnlistedCommand("read_any_file".to_string())]
    );
}

#[test]
fn a_documented_mention_of_the_attribute_is_not_a_command() {
    let source = "/// Never put `#[tauri::command]` here.\n// #[tauri::command]\nfn ok() {}\n";
    assert_eq!(check_command_placement("lib.rs", source), Vec::new());
}

#[test]
fn every_violation_names_its_fix() {
    let violations = [
        Violation::Malformed("x".into()),
        Violation::UnknownWindow("*".into()),
        Violation::WebviewTargets,
        Violation::RemoteContext,
        Violation::PermissionNotGranted("core:path:default".into()),
        Violation::ForbiddenPlugin("fs:default".into()),
        Violation::CapabilityNotEnabled("extra".into()),
        Violation::CapabilityList(vec!["extra".into()]),
        Violation::InlineCapability,
        Violation::GlobalTauri,
        Violation::AssetProtocol,
        Violation::PluginDependency("tauri-plugin-fs".into()),
        Violation::LinkedPlugin("tauri-plugin-fs".into()),
        Violation::CommandInLib("x".into()),
        Violation::UnlistedCommand("x".into()),
    ];
    for violation in violations {
        assert!(violation.to_string().len() > 20, "{violation:?}");
    }
}
