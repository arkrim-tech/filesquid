//! Independent community OCFL validators as test oracles (OCFL-S-010 (b);
//! ADR 016 "Community OCFL programs", ADR 023). They check objects the
//! engine wrote; they are never the writer and never a crate dependency.
//!
//! Two validators are recognised:
//!
//! - `ocfl-validate.py` (or `ocfl-validate`) from `ocfl-py`
//!   (`pip install ocfl-py`), run as `ocfl-validate.py <object root>`;
//! - `rocfl` (`cargo install rocfl`), run as
//!   `rocfl -r <object root> validate -p .`.
//!
//! Both exit 0 for a valid object (warnings allowed) and non-zero otherwise.
//! `FILESQUID_OCFL_COMMUNITY_VALIDATOR` names one validator program to use
//! instead of searching `PATH`. When none is found the community check is
//! skipped with a message, unless `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR`
//! is `1`, which makes a missing validator a test failure.

#![allow(dead_code)]

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The variable that names one validator program to use.
pub const VALIDATOR_VAR: &str = "FILESQUID_OCFL_COMMUNITY_VALIDATOR";

/// The variable that makes a missing validator a failure when it is `1`.
pub const REQUIRE_VAR: &str = "FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR";

/// Program names searched on `PATH`, in order.
const PROGRAMS: [&str; 3] = ["ocfl-validate.py", "ocfl-validate", "rocfl"];

/// Most `PATH` entries searched.
const MAX_PATH_ENTRIES: usize = 256;

/// Which community program a validator is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `ocfl-validate` from `ocfl-py`.
    OcflPy,
    /// `rocfl validate`.
    Rocfl,
}

/// One community validator program.
#[derive(Debug, Clone)]
pub struct Validator {
    /// Which program it is.
    pub kind: Kind,
    /// Where it is.
    pub program: PathBuf,
}

/// A validator's verdict on one object.
#[derive(Debug, Clone)]
pub struct Verdict {
    /// Whether the program reported the object valid (exit status 0).
    pub valid: bool,
    /// What it printed on stdout and stderr.
    pub output: String,
}

impl Validator {
    /// The validator a program path names, by its file name.
    fn from_program(program: PathBuf) -> Self {
        let name = program
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = if name.contains("rocfl") {
            Kind::Rocfl
        } else {
            Kind::OcflPy
        };
        Self { kind, program }
    }

    /// A short name for reports, such as `ocfl-py` or `rocfl`.
    pub fn name(&self) -> &'static str {
        match self.kind {
            Kind::OcflPy => "ocfl-py",
            Kind::Rocfl => "rocfl",
        }
    }

    /// Validate the object whose root directory is `object_root`. Runs the
    /// program as a separate process; it reads the object and writes
    /// nothing.
    pub fn run_validation(&self, object_root: &Path) -> Verdict {
        let mut command = Command::new(&self.program);
        match self.kind {
            Kind::OcflPy => command.arg(object_root),
            Kind::Rocfl => command
                .arg("-r")
                .arg(object_root)
                .args(["validate", "-p", "."]),
        };
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("could not run {}: {err}", self.program.display()));
        let text = String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr);
        Verdict {
            valid: output.status.success(),
            output: text,
        }
    }
}

/// Every community validator available: the program in
/// [`VALIDATOR_VAR`] when set, otherwise each of `ocfl-py` and `rocfl`
/// found on `PATH` (one of each at most).
pub fn find_validators() -> Vec<Validator> {
    if let Some(program) = env::var_os(VALIDATOR_VAR) {
        return vec![Validator::from_program(PathBuf::from(program))];
    }
    let mut found: Vec<Validator> = Vec::new();
    for name in PROGRAMS {
        let Some(program) = on_path(name) else {
            continue;
        };
        let validator = Validator::from_program(program);
        if found.iter().all(|v| v.kind != validator.kind) {
            found.push(validator);
        }
    }
    found
}

/// The first executable called `name` in a `PATH` directory.
fn on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .take(MAX_PATH_ENTRIES)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Whether a missing validator fails the test ([`REQUIRE_VAR`] is `1`).
pub fn required() -> bool {
    env::var(REQUIRE_VAR).is_ok_and(|value| value == "1")
}

/// The validators to run. With none available: panic when one is
/// [`required`], otherwise print why the community check is skipped and
/// return an empty list.
pub fn validators_or_skip() -> Vec<Validator> {
    let found = find_validators();
    if !found.is_empty() {
        return found;
    }
    let how = "install ocfl-py (`pip install ocfl-py`, provides ocfl-validate.py) or \
               rocfl (`cargo install rocfl`), or name one in FILESQUID_OCFL_COMMUNITY_VALIDATOR";
    assert!(
        !required(),
        "{REQUIRE_VAR}=1 but no community OCFL validator was found: {how}"
    );
    println!("SKIPPED community OCFL validation (OCFL-S-010 (b)): no validator found; {how}");
    found
}
