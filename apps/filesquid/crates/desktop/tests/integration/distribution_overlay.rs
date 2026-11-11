//! Integration tests for the Tauri configuration overlays (Task DE-F3.1; DE-S-007, ADR 035
//! Decision 3): the `filesquid` brand overlay names the product, the identifier, the icons and
//! the UI directory; the macOS DMG overlay turns bundling on for an unsigned, ad-hoc signed
//! `.dmg` and nothing else; the base configuration alone builds no installer; and the webview
//! capability policy (DE-S-001, DE-S-002) holds with the overlays merged.
//!
//! These tests read configuration files. What the Tauri bundler does with them on macOS is
//! proven by building the DMG there, which the task walkthrough lists as the owner's steps.

use serde_json::{json, Value};

/// The policy module the shared input reader expects at `crate::capability_policy`.
mod capability_policy {
    pub use filesquid_desktop_lib::capability_policy::*;
}

#[allow(dead_code)]
#[path = "../../src/capability_inputs.rs"]
mod capability_inputs;

#[allow(dead_code)]
#[path = "../support/policy.rs"]
mod policy_support;

#[allow(dead_code)]
#[path = "../support/overlay.rs"]
mod overlay;

use overlay::{list_at, read_crate_json, read_effective, read_violations, text_at};

/// The brand overlay of the default distribution.
fn filesquid() -> Value {
    read_crate_json(overlay::FILESQUID)
}

/// The macOS DMG bundling overlay.
fn macos_dmg() -> Value {
    read_crate_json(overlay::MACOS_DMG)
}

/// An overlay shaped like another distribution's (`apps/imagesquid/tauri.imagesquid.json`).
fn other_distribution() -> Value {
    json!({
        "productName": "ImageSquid",
        "identifier": "app.imagesquid.desktop",
        "build": { "frontendDist": "../../../imagesquid/dist" },
        "app": { "windows": [{ "label": "main", "title": "ImageSquid" }] }
    })
}

#[test]
fn the_filesquid_overlay_names_the_product_and_the_identifier() {
    let conf = filesquid();
    assert_eq!(text_at(&conf, "/productName"), Some("FileSquid"));
    assert_eq!(
        text_at(&conf, "/identifier"),
        Some("tech.filesquid.desktop")
    );
    assert_eq!(text_at(&conf, "/app/windows/0/title"), Some("FileSquid"));
    assert_eq!(text_at(&conf, "/app/windows/0/label"), Some("main"));
}

#[test]
fn the_filesquid_overlay_names_the_default_ui_directory() {
    let conf = filesquid();
    assert_eq!(text_at(&conf, "/build/frontendDist"), Some("../../ui/dist"));
    let before = conf.pointer("/build/beforeBuildCommand/cwd");
    assert_eq!(before.and_then(Value::as_str), Some("../../ui"));
}

#[test]
fn the_filesquid_overlay_lists_icons_that_exist_with_a_macos_icon_among_them() {
    let icons = list_at(&filesquid(), "/bundle/icon");
    assert!(
        icons.iter().any(|icon| icon.ends_with(".icns")),
        "{icons:?}"
    );
    assert!(icons.iter().any(|icon| icon.ends_with(".png")), "{icons:?}");
    for icon in &icons {
        let path = overlay::crate_root().join(icon);
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        assert!(size > 0, "{} is missing or empty", path.display());
    }
}

#[test]
fn the_macos_icon_is_an_apple_icon_image() {
    let bytes = std::fs::read(overlay::crate_root().join("icons/icon.icns")).expect("icon.icns");
    assert_eq!(&bytes[..4], b"icns");
}

#[test]
fn the_filesquid_overlay_turns_no_bundling_on() {
    assert_eq!(filesquid().pointer("/bundle/active"), None);
    assert_eq!(filesquid().pointer("/bundle/targets"), None);
}

#[test]
fn the_base_configuration_alone_builds_no_installer() {
    let base = read_crate_json("tauri.conf.json");
    assert_eq!(base.pointer("/bundle/active"), Some(&json!(false)));
    let branded = read_effective(&[filesquid()]);
    assert_eq!(branded.pointer("/bundle/active"), Some(&json!(false)));
}

#[test]
fn no_platform_file_merges_bundling_in_without_being_named() {
    for platform in ["macos", "linux", "windows"] {
        let name = format!("tauri.{platform}.conf.json");
        assert!(!overlay::crate_root().join(&name).exists(), "{name}");
    }
}

#[test]
fn the_dmg_overlay_builds_a_dmg_and_no_other_installer() {
    let conf = read_effective(&[filesquid(), macos_dmg()]);
    assert_eq!(conf.pointer("/bundle/active"), Some(&json!(true)));
    assert_eq!(conf.pointer("/bundle/targets"), Some(&json!(["dmg"])));
}

#[test]
fn the_dmg_overlay_signs_ad_hoc_and_never_with_a_developer_id() {
    let conf = macos_dmg();
    assert_eq!(text_at(&conf, "/bundle/macOS/signingIdentity"), Some("-"));
    assert_eq!(conf.pointer("/bundle/macOS/providerShortName"), None);
}

#[test]
fn the_dmg_overlay_leaves_the_hardened_runtime_off_for_the_run_time_loaded_libvips() {
    let conf = macos_dmg();
    assert_eq!(
        conf.pointer("/bundle/macOS/hardenedRuntime"),
        Some(&json!(false))
    );
}

#[test]
fn the_dmg_overlay_configures_no_updater() {
    let conf = read_effective(&[filesquid(), macos_dmg()]);
    assert_eq!(conf.pointer("/bundle/createUpdaterArtifacts"), None);
    assert_eq!(conf.pointer("/plugins/updater"), None);
}

#[test]
fn the_dmg_overlay_carries_no_brand_so_every_distribution_reuses_it() {
    let conf = macos_dmg();
    for pointer in [
        "/productName",
        "/identifier",
        "/build",
        "/app",
        "/bundle/icon",
    ] {
        assert_eq!(conf.pointer(pointer), None, "{pointer}");
    }
    let other = read_effective(&[other_distribution(), macos_dmg()]);
    assert_eq!(text_at(&other, "/productName"), Some("ImageSquid"));
    assert_eq!(other.pointer("/bundle/targets"), Some(&json!(["dmg"])));
}

#[test]
fn the_filesquid_overlay_changes_nothing_the_base_configuration_decides_about_security() {
    let base = read_crate_json("tauri.conf.json");
    let branded = read_effective(&[filesquid()]);
    assert_eq!(
        branded.pointer("/app/security"),
        base.pointer("/app/security")
    );
    assert_eq!(branded.pointer("/app/withGlobalTauri"), Some(&json!(false)));
}

#[test]
fn the_capability_policy_holds_with_the_overlays_merged() {
    for platform in ["macos", "linux", "windows"] {
        assert_eq!(read_violations(platform, &[filesquid()]), Vec::new());
        assert_eq!(
            read_violations(platform, &[filesquid(), macos_dmg()]),
            Vec::new()
        );
    }
}

#[test]
fn later_overlays_win_in_the_order_the_cli_merges_them() {
    let conf = read_effective(&[filesquid(), other_distribution()]);
    assert_eq!(text_at(&conf, "/productName"), Some("ImageSquid"));
    let conf = read_effective(&[other_distribution(), filesquid()]);
    assert_eq!(text_at(&conf, "/productName"), Some("FileSquid"));
}
