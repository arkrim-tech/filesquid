//! C2PA-S-007 / C2PA-S-008: the resolved runtime dependency graph of
//! `filesquid-c2pa` holds no other FileSquid crate, distribution, host,
//! SQLite, media, network or official `c2pa` crate. The rule table is also
//! tested on synthetic names, so a forbidden dependency is proven to fail.

#[path = "../support/deps.rs"]
#[allow(dead_code)]
mod deps;

use deps::{
    forbidden_reason, package_names, resolved_runtime_packages, sample_name, violations, RULES,
};

#[test]
fn every_rule_rejects_a_matching_name() {
    for rule in RULES {
        let name = sample_name(rule);
        assert_eq!(
            forbidden_reason(&name),
            Some(rule.reason),
            "{name} must be rejected by rule {}",
            rule.pattern
        );
    }
}

#[test]
fn async_runtimes_and_network_clients_are_rules() {
    for name in [
        "isahc",
        "surf",
        "curl",
        "attohttpc",
        "async-std",
        "mio",
        "smol",
        "async-executor",
        "async-io",
    ] {
        assert!(RULES.iter().any(|rule| rule.pattern == name), "{name} rule");
    }
    assert_eq!(forbidden_reason("smol_str"), None, "smol is an exact rule");
}

#[test]
fn rules_reject_every_named_crate() {
    for name in [
        "filesquid-ocfl",
        "filesquid-core",
        "filesquid-metadata",
        "filesquid-api",
        "imagesquid-ui",
        "arkrim-theme",
        "axum",
        "tauri",
        "tauri-build",
        "rusqlite",
        "sqlite",
        "libsqlite3-sys",
        "sqlx",
        "sqlx-core",
        "libvips",
        "ffmpeg-sys",
        "ffmpeg-next",
        "c2pa",
        "c2pa-crypto",
        "c2patool",
        "reqwest",
        "hyper",
        "hyper-util",
        "ureq",
        "tokio",
    ] {
        assert!(forbidden_reason(name).is_some(), "{name} must be forbidden");
    }
}

#[test]
fn rules_allow_this_crate_and_primitive_crates() {
    for name in [
        "filesquid-c2pa",
        "sha2",
        "p256",
        "ed25519-dalek",
        "thiserror",
        "c2pa_like",
    ] {
        assert_eq!(forbidden_reason(name), None, "{name} must be allowed");
    }
}

#[test]
fn package_names_parse_cargo_tree_lines() {
    let tree = "filesquid-c2pa v0.1.0 (/w/crates/c2pa)\nsha2 v0.11.1\nsha2 v0.11.1\n\n";
    assert_eq!(package_names(tree), ["filesquid-c2pa", "sha2"]);
}

#[test]
fn a_forbidden_dependency_would_fail_the_check() {
    let found = violations(["sha2", "filesquid-ocfl", "c2pa"]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with("filesquid-ocfl: C2PA-S-008"));
    assert!(found[1].starts_with("c2pa: C2PA-S-007"));
}

#[test]
fn default_runtime_graph_is_isolated() {
    let names = resolved_runtime_packages(&[]).expect("cargo tree");
    assert!(names.iter().any(|n| n == "filesquid-c2pa"), "{names:?}");
    let found = violations(names.iter().map(String::as_str));
    assert!(
        found.is_empty(),
        "forbidden runtime dependencies: {found:?}"
    );
}

#[test]
fn all_features_runtime_graph_is_isolated() {
    let names = resolved_runtime_packages(&["--all-features"]).expect("cargo tree");
    let found = violations(names.iter().map(String::as_str));
    assert!(
        found.is_empty(),
        "forbidden runtime dependencies: {found:?}"
    );
}

#[test]
fn no_default_features_graph_has_no_crypto_crate() {
    let names = resolved_runtime_packages(&["--no-default-features"]).expect("cargo tree");
    for crypto in ["sha2", "p256", "p384", "p521", "ed25519-dalek"] {
        assert!(
            !names.iter().any(|n| n == crypto),
            "{crypto} must only come with the rust-crypto feature"
        );
    }
}

#[test]
fn manifest_is_not_published_yet() {
    let manifest = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("Cargo.toml");
    assert!(manifest.contains("\npublish = false\n"));
    assert!(
        !manifest.contains("path = \"../../.."),
        "no path outside apps/filesquid"
    );
}
