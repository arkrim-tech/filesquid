//! Task CLI-0.2: the `fsquid` binary opens `<data dir>/filesquid.sqlite` before any command
//! runs (API-S-031; ADR 036 Decision 8). `FILESQUID_DATA_DIR` selects the journal, a command
//! runs while another connection holds the same journal open in WAL mode, and a journal that
//! cannot be opened refuses the command, changes nothing and prints the `journal_unavailable`
//! stdout line.

use filesquid_api::journal::{JournalQuery, JournalStore, Outcome};
use filesquid_native::{SqliteJournal, JOURNAL_FILE};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OBJECT_ID: &str = "urn:uuid:7d2a1b0c-3e4f-4a5b-8c6d-9e0f1a2b3c4d";

/// A fresh, empty directory for one test (removed on drop).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cli-0.2-bin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run the `fsquid` binary with `args` and `FILESQUID_DATA_DIR` set to `data_dir`.
fn fsquid(data_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fsquid"))
        .args(args)
        .env("FILESQUID_DATA_DIR", data_dir)
        .output()
        .expect("run fsquid")
}

fn set_title(data_dir: &Path, object: &Path) -> Output {
    let object = object.to_str().expect("utf-8 path");
    fsquid(
        data_dir,
        &[
            "set",
            "dcterms:title",
            "Holiday",
            "--id",
            OBJECT_ID,
            "--object",
            object,
        ],
    )
}

fn operations(journal: &SqliteJournal) -> Vec<(String, Outcome)> {
    let page = journal.query(&JournalQuery::default()).expect("query");
    page.entries
        .into_iter()
        .map(|e| (e.operation, e.outcome))
        .collect()
}

#[test]
fn filesquid_data_dir_selects_the_journal() {
    let scratch = Scratch::new("data-dir");
    let data_dir = scratch.path("data");
    let out = set_title(&data_dir, &scratch.path("object"));
    assert!(out.status.success(), "{out:?}");
    let journal = SqliteJournal::open(&data_dir.join(JOURNAL_FILE), "test").expect("journal");
    assert_eq!(
        operations(&journal),
        [("cli.metadata.set".to_string(), Outcome::Succeeded)]
    );
}

#[test]
fn a_command_runs_while_another_connection_holds_the_journal_in_wal_mode() {
    let scratch = Scratch::new("wal");
    let data_dir = scratch.path("data");
    std::fs::create_dir_all(&data_dir).expect("data dir");
    let held = SqliteJournal::open(&data_dir.join(JOURNAL_FILE), "filesquid-desktop")
        .expect("held journal");
    let out = set_title(&data_dir, &scratch.path("object"));
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        operations(&held),
        [("cli.metadata.set".to_string(), Outcome::Succeeded)],
        "the held connection sees the CLI's entry"
    );
}

#[test]
fn an_unopenable_journal_refuses_the_command_and_changes_nothing() {
    let scratch = Scratch::new("unavailable");
    let not_a_dir = scratch.path("data");
    std::fs::write(&not_a_dir, b"a file, not a directory").expect("file");
    let object = scratch.path("object");
    let out = set_title(&not_a_dir, &object);
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert!(!object.exists(), "nothing was written");
    let stdout = String::from_utf8(out.stdout).expect("utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "one refusal line: {stdout}");
    let refusal: serde_json::Value = serde_json::from_str(lines[0]).expect("json line");
    assert_eq!(refusal["journal_unavailable"], true);
    assert_eq!(refusal["operation"], "cli.metadata.set");
    assert_eq!(refusal["outcome"], "failed");
    assert!(!stdout.contains("Holiday") && !stdout.contains(&*scratch.0.to_string_lossy()));
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(stderr.contains("FILESQUID_DATA_DIR"), "{stderr}");
}
