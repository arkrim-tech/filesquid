//! Cucumber harness for the `fsquid` CLI Gherkin under `tests/features/`: routing (F-050) and
//! journaled commands (F-061, Task CLI-0.2, steps in `tests/support/journal_steps.rs`).
//!
//! Each run clears `tests/allure-results/` and writes
//! `tests/allure-results/cucumber-junit.xml` with one `<testcase>` per scenario.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use cucumber::{then, when, writer, StatsWriter as _, World, WriterExt as _};
use filesquid_cli::{resolve, run, Route};

#[path = "support/journal_steps.rs"]
mod journal_steps;

/// World state for F-050 CLI scenarios.
#[derive(Debug, Default, World)]
struct CliWorld {
    /// Arguments of the last `fsquid` invocation.
    argv: Vec<String>,
    /// Outcome of the last `fsquid` invocation.
    outcome: Option<Result<i32, String>>,
    /// `(package, binary)` pairs Cargo builds in the workspace (hosts excluded).
    binaries: Vec<(String, String)>,
    /// The journaled `fsquid` runs of the F-061 scenarios.
    journal: journal_steps::JournalWorld,
}

#[when(expr = "I run fsquid with {string}")]
fn run_fsquid(world: &mut CliWorld, line: String) {
    world.argv = line.split_whitespace().map(str::to_string).collect();
    world.outcome = Some(run(&world.argv));
}

#[then("the metadata module should handle it")]
fn metadata_handles(world: &mut CliWorld) {
    match resolve(&world.argv) {
        Route::Run { module, .. } => assert_eq!(module.name, "metadata"),
        other => panic!("expected the metadata module, got {other:?}"),
    }
}

#[then(expr = "fsquid should refuse and suggest {string}")]
fn refuses_with_hint(world: &mut CliWorld, hint: String) {
    let err = world.outcome.clone().expect("ran").expect_err("refused");
    assert!(err.contains(&hint), "{err}");
}

#[then("the command should succeed")]
fn succeeds(world: &mut CliWorld) {
    assert_eq!(world.outcome.clone().expect("ran"), Ok(0));
}

#[then("the routed help should list extract, get, set, export, import, and validate")]
fn routed_help_lists_commands(world: &mut CliWorld) {
    let Route::Run { module, args } = resolve(&world.argv) else {
        panic!("expected a module route for {:?}", world.argv);
    };
    assert_eq!(args, ["help".to_string()]);
    for command in ["extract", "get", "set", "export", "import", "validate"] {
        let line = format!("\n  {command} ");
        assert!(module.usage.contains(&line), "{command} missing");
    }
}

#[then("fsquid should report an unknown command")]
fn unknown(world: &mut CliWorld) {
    let err = world.outcome.clone().expect("ran").expect_err("unknown");
    assert!(err.contains("unknown command"), "{err}");
}

#[when("I list the CLI binaries the workspace builds")]
fn list_binaries(world: &mut CliWorld) {
    let hosts = ["filesquid-desktop", "filesquid-server"];
    world.binaries = bin_targets(&read_cargo_metadata())
        .into_iter()
        .filter(|(package, _)| !hosts.contains(&package.as_str()))
        .collect();
}

#[then("the only CLI binary should be fsquid")]
fn only_fsquid(world: &mut CliWorld) {
    let expected = vec![("filesquid-cli".to_string(), "fsquid".to_string())];
    assert_eq!(world.binaries, expected);
}

/// Run `cargo metadata --no-deps` for this workspace and parse its JSON.
///
/// Cargo reports every target it would build, including binaries it
/// auto-discovers from `src/main.rs` and `src/bin/*.rs`, so no manifest
/// parsing is needed here.
fn read_cargo_metadata() -> serde_json::Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata JSON")
}

/// Return the `(package, binary)` name of every `bin` target in `metadata`. Pure.
///
/// # Arguments
/// * `metadata` - the parsed output of `cargo metadata --format-version 1`.
fn bin_targets(metadata: &serde_json::Value) -> Vec<(String, String)> {
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let mut bins: Vec<(String, String)> = packages
        .iter()
        .flat_map(|package| {
            let name = package["name"].as_str().unwrap_or_default().to_string();
            let targets = package["targets"].as_array().cloned().unwrap_or_default();
            targets
                .into_iter()
                .filter(|t| {
                    t["kind"]
                        .as_array()
                        .is_some_and(|k| k.contains(&"bin".into()))
                })
                .map(move |t| {
                    (
                        name.clone(),
                        t["name"].as_str().unwrap_or_default().to_string(),
                    )
                })
        })
        .collect();
    bins.sort();
    bins
}

/// Remove `dir` if it exists; any other error fails the run.
fn remove_dir_if_present(dir: &Path) {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => panic!("cannot clear {}: {err}", dir.display()),
    }
}

#[tokio::main]
async fn main() {
    let results_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    remove_dir_if_present(&results_dir);
    fs::create_dir_all(&results_dir).expect("allure-results");
    let features = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features");
    let junit = fs::File::create(results_dir.join("cucumber-junit.xml")).expect("junit file");
    let writer = CliWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<CliWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .run(features)
        .await;
    if writer.execution_has_failed() {
        std::process::exit(1);
    }
}
