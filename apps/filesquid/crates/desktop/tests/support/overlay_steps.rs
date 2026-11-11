//! Steps of `desktop_distribution_overlay_F-058.feature` (Task DE-F3.1; DE-S-007, ADR 035
//! Decision 3): the shipped overlay files merged over `tauri.conf.json` the way the Tauri CLI
//! merges its `--config` values, and the capability policy run over the result. They read
//! configuration files only; building the disk image is a macOS step.

use super::overlay::{self, list_at, read_crate_json, text_at};
use super::DesktopWorld;
use crate::capability_policy::Violation;
use cucumber::{given, then, when};
use serde_json::{json, Value};

/// The overlays of one scenario, in merge order, and the configuration they give.
#[derive(Debug, Default)]
pub struct OverlayScenario {
    overlays: Vec<Value>,
    effective: Value,
}

/// The shipped overlay file a scenario names.
fn shipped(name: &str) -> Value {
    match name {
        "filesquid" => read_crate_json(overlay::FILESQUID),
        "macos-dmg" => read_crate_json(overlay::MACOS_DMG),
        other => panic!("no shipped overlay named {other}"),
    }
}

/// Keep `overlays` as the scenario's configuration.
fn configure(world: &mut DesktopWorld, overlays: Vec<Value>) {
    let effective = overlay::read_effective(&overlays);
    world.overlay = Some(OverlayScenario {
        overlays,
        effective,
    });
}

/// The scenario's effective configuration.
fn effective(world: &DesktopWorld) -> &Value {
    &world.overlay.as_ref().expect("a configuration").effective
}

#[given(expr = "the desktop build configuration with the {string} overlay")]
fn with_one(world: &mut DesktopWorld, name: String) {
    configure(world, vec![shipped(&name)]);
}

#[given(expr = "the desktop build configuration with the {string} and {string} overlays")]
fn with_two(world: &mut DesktopWorld, first: String, second: String) {
    configure(world, vec![shipped(&first), shipped(&second)]);
}

#[given(
    expr = "the desktop build configuration with another distribution's overlay and the {string} overlay"
)]
fn with_another_distribution(world: &mut DesktopWorld, name: String) {
    let other = json!({
        "productName": "ImageSquid",
        "identifier": "app.imagesquid.desktop",
        "build": { "frontendDist": "../../../imagesquid/dist" },
        "app": { "windows": [{ "label": "main", "title": "ImageSquid" }] }
    });
    configure(world, vec![other, shipped(&name)]);
}

#[given(
    expr = "the desktop build configuration with the {string} overlay and one that enables the capability {string}"
)]
fn with_a_widening_overlay(world: &mut DesktopWorld, name: String, capability: String) {
    let wide = json!({ "app": { "security": { "capabilities": ["default", capability] } } });
    configure(world, vec![shipped(&name), wide]);
}

#[when(expr = "the capability policy checks that configuration for {string}")]
fn check_policy(world: &mut DesktopWorld, platform: String) {
    let overlays = &world.overlay.as_ref().expect("a configuration").overlays;
    world.violations = overlay::read_violations(&platform, overlays);
}

#[then(expr = "the product name should be {string}")]
fn product_name(world: &mut DesktopWorld, name: String) {
    assert_eq!(
        text_at(effective(world), "/productName"),
        Some(name.as_str())
    );
}

#[then(expr = "the application identifier should be {string}")]
fn identifier(world: &mut DesktopWorld, id: String) {
    assert_eq!(text_at(effective(world), "/identifier"), Some(id.as_str()));
}

#[then(expr = "the main window title should be {string}")]
fn window_title(world: &mut DesktopWorld, title: String) {
    let conf = effective(world);
    assert_eq!(text_at(conf, "/app/windows/0/title"), Some(title.as_str()));
}

#[then(expr = "the UI directory should be {string}")]
fn ui_directory(world: &mut DesktopWorld, dir: String) {
    let conf = effective(world);
    assert_eq!(text_at(conf, "/build/frontendDist"), Some(dir.as_str()));
}

#[then("every listed icon should exist, with a macOS icon among them")]
fn icons_exist(world: &mut DesktopWorld) {
    let icons = list_at(effective(world), "/bundle/icon");
    assert!(
        icons.iter().any(|icon| icon.ends_with(".icns")),
        "{icons:?}"
    );
    for icon in &icons {
        assert!(overlay::crate_root().join(icon).is_file(), "{icon}");
    }
}

#[then("the build should produce installers")]
fn installers_on(world: &mut DesktopWorld) {
    assert_eq!(
        effective(world).pointer("/bundle/active"),
        Some(&json!(true))
    );
}

#[then("the build should produce no installers")]
fn installers_off(world: &mut DesktopWorld) {
    assert_eq!(
        effective(world).pointer("/bundle/active"),
        Some(&json!(false))
    );
}

#[then(expr = "the only installer should be {string}")]
fn only_installer(world: &mut DesktopWorld, target: String) {
    assert_eq!(list_at(effective(world), "/bundle/targets"), vec![target]);
}

#[then("the app should be signed ad hoc, without a Developer ID")]
fn signed_ad_hoc(world: &mut DesktopWorld) {
    let conf = effective(world);
    assert_eq!(text_at(conf, "/bundle/macOS/signingIdentity"), Some("-"));
    assert_eq!(conf.pointer("/bundle/macOS/providerShortName"), None);
}

#[then("no updater should be configured")]
fn no_updater(world: &mut DesktopWorld) {
    let conf = effective(world);
    assert_eq!(conf.pointer("/bundle/createUpdaterArtifacts"), None);
    assert_eq!(conf.pointer("/plugins/updater"), None);
}

#[then("the configuration should pass the capability policy")]
fn passes_policy(world: &mut DesktopWorld) {
    assert_eq!(world.violations, Vec::new());
}

#[then(expr = "the policy should refuse the enabled capability list {string}")]
fn refused_capability_list(world: &mut DesktopWorld, list: String) {
    let names = list.split(", ").map(String::from).collect();
    assert_eq!(world.violations, vec![Violation::CapabilityList(names)]);
}
