//! Routing of `fsquid` arguments to module command runners (Task CLI-0.1).

use std::fs;
use std::path::PathBuf;

use filesquid_cli::{ambiguous_commands, resolve, run, usage, Route, MODULES, QUALIFIED_ONLY};

/// Build an owned argument vector from string literals.
fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn unique_command_runs_at_top_level() {
    let argv = args(&["get", "dcterms:title"]);
    match resolve(&argv) {
        Route::Run { module, args } => {
            assert_eq!(module.name, "metadata");
            assert_eq!(args, &argv[..]);
        }
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn module_group_strips_the_module_name() {
    let argv = args(&["metadata", "get", "dcterms:title"]);
    match resolve(&argv) {
        Route::Run { module, args } => {
            assert_eq!(module.name, "metadata");
            assert_eq!(args, &argv[1..]);
        }
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn bare_module_name_shows_module_help() {
    let argv = args(&["metadata"]);
    match resolve(&argv) {
        Route::Run { module, args } => {
            assert_eq!(module.name, "metadata");
            assert!(args.is_empty());
        }
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn validate_without_module_needs_the_module() {
    let argv = args(&["validate", "dcterms:title"]);
    match resolve(&argv) {
        Route::NeedsModule { command, modules } => {
            assert_eq!(command, "validate");
            assert!(modules.contains(&"metadata"));
        }
        other => panic!("expected NeedsModule, got {other:?}"),
    }
    let err = run(&argv).expect_err("validate needs a module");
    assert!(err.contains("fsquid metadata validate"), "{err}");
}

#[test]
fn validate_with_module_runs() {
    let code = run(&args(&["metadata", "validate", "dcterms:title"])).expect("ok");
    assert_eq!(code, 0);
    let code = run(&args(&["metadata", "validate", "dcterms:name"])).expect("runs");
    assert_eq!(code, 1);
}

#[test]
fn unknown_command_is_an_error() {
    let argv = args(&["mods"]);
    assert!(matches!(resolve(&argv), Route::Unknown(ref c) if c == "mods"));
    let err = run(&argv).expect_err("unknown");
    assert!(err.contains("unknown command: mods"), "{err}");
}

#[test]
fn help_forms_resolve_to_help() {
    for argv in [args(&[]), args(&["help"]), args(&["-h"]), args(&["--help"])] {
        assert!(matches!(resolve(&argv), Route::Help), "{argv:?}");
    }
    assert_eq!(run(&args(&["help"])).expect("help"), 0);
}

#[test]
fn usage_names_fsquid_and_both_forms() {
    let text = usage();
    assert!(text.contains("fsquid <command> [args]"));
    assert!(text.contains("fsquid <module> <command> [args]"));
    assert!(text.contains("fsquid metadata validate"));
    assert!(!text.contains("filesquid-metadata <command>"));
}

#[test]
fn shared_command_names_are_qualifier_only() {
    for command in ambiguous_commands() {
        assert!(
            QUALIFIED_ONLY.contains(&command),
            "`{command}` is defined by two modules; add it to QUALIFIED_ONLY or rename it"
        );
    }
}

#[test]
fn module_names_are_not_command_names() {
    for module in MODULES {
        for other in MODULES {
            assert!(
                !other.commands.contains(&module.name),
                "module name `{}` is also a command",
                module.name
            );
        }
    }
}

#[test]
fn fsquid_is_the_only_cli_binary_in_the_workspace() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let hosts = ["filesquid-desktop", "filesquid-server"];
    for entry in fs::read_dir(&crates).expect("crates dir") {
        let manifest = entry.expect("entry").path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        for name in bin_names(&text) {
            assert!(
                name == "fsquid" || hosts.contains(&name.as_str()),
                "{} declares CLI binary `{name}`; every CLI MUST be `fsquid`",
                manifest.display()
            );
        }
    }
}

/// Return the `name` of every `[[bin]]` table in a Cargo manifest.
fn bin_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_bin = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_bin = line == "[[bin]]";
            continue;
        }
        if let Some(value) = line.strip_prefix("name = ").filter(|_| in_bin) {
            names.push(value.trim_matches('"').to_string());
        }
    }
    names
}
