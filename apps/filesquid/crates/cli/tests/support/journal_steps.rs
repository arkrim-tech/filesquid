//! Step definitions for `journaled_fsquid_commands_F-061.feature` (Task CLI-0.2; API-S-031,
//! ADR 036 Decision 8): the `fsquid` binary run against a scratch data directory, its journal
//! read back through `filesquid-native`'s SQLite journal store.

use crate::CliWorld;
use cucumber::{given, then, when};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore};
use filesquid_native::{SqliteJournal, JOURNAL_FILE};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// The identifier of the objects the scenarios create.
const OBJECT_ID: &str = "urn:uuid:0c1d2e3f-4a5b-4c6d-8e7f-90a1b2c3d4e5";

/// The title the scenarios set; no journal entry may hold it.
const TITLE: &str = "Holiday";

/// Scenario counter, so each scenario gets its own directory.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Per-scenario state of the journal steps.
#[derive(Default)]
pub struct JournalWorld {
    root: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    held: Option<SqliteJournal>,
    output: Option<Output>,
}

impl std::fmt::Debug for JournalWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalWorld").finish_non_exhaustive()
    }
}

impl Drop for JournalWorld {
    fn drop(&mut self) {
        self.held = None;
        if let Some(root) = &self.root {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

impl JournalWorld {
    fn root(&self) -> &Path {
        self.root.as_deref().expect("a scratch directory")
    }

    fn data_dir(&self) -> &Path {
        self.data_dir.as_deref().expect("a data directory")
    }

    fn object(&self) -> PathBuf {
        self.root().join("holiday-album")
    }

    /// Run the `fsquid` binary with `args` against the scenario's data directory.
    fn fsquid(&mut self, args: &[&str]) {
        let output = Command::new(env!("CARGO_BIN_EXE_fsquid"))
            .args(args)
            .env("FILESQUID_DATA_DIR", self.data_dir())
            .output()
            .expect("run fsquid");
        self.output = Some(output);
    }

    fn set_title(&mut self) {
        let object = self.object().to_string_lossy().into_owned();
        self.fsquid(&[
            "set",
            "dcterms:title",
            TITLE,
            "--id",
            OBJECT_ID,
            "--object",
            &object,
        ]);
    }

    /// Every entry of the scenario's journal.
    fn entries(&self) -> Vec<JournalEntry> {
        let journal =
            SqliteJournal::open(&self.data_dir().join(JOURNAL_FILE), "test").expect("journal");
        journal
            .query(&JournalQuery::default())
            .expect("query")
            .entries
    }

    fn stdout(&self) -> String {
        let output = self.output.as_ref().expect("fsquid ran");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

#[given("a fresh FileSquid data directory")]
fn fresh_data_dir(w: &mut CliWorld) {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("cli-0.2-bdd-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch");
    w.journal.data_dir = Some(root.join("data"));
    w.journal.root = Some(root);
}

#[given("a data directory that cannot hold a journal")]
fn unusable_data_dir(w: &mut CliWorld) {
    let path = w.journal.root().join("not-a-directory");
    std::fs::write(&path, b"a file").expect("file");
    w.journal.data_dir = Some(path);
}

#[given("another connection holds the journal open")]
fn held_open(w: &mut CliWorld) {
    let dir = w.journal.data_dir().to_path_buf();
    std::fs::create_dir_all(&dir).expect("data dir");
    let journal = SqliteJournal::open(&dir.join(JOURNAL_FILE), "filesquid-desktop").expect("held");
    w.journal.held = Some(journal);
}

#[when("I set a title on a new object with fsquid")]
fn set_title(w: &mut CliWorld) {
    w.journal.set_title();
}

#[when("I set the same title again with fsquid")]
fn set_title_again(w: &mut CliWorld) {
    w.journal.set_title();
}

#[when("I read the title with fsquid")]
fn read_title(w: &mut CliWorld) {
    let object = w.journal.object().to_string_lossy().into_owned();
    w.journal
        .fsquid(&["get", "dcterms:title", "--object", &object]);
}

#[when(expr = "I run the fsquid binary with {string}")]
fn run_binary(w: &mut CliWorld, line: String) {
    let args: Vec<&str> = line.split_whitespace().collect();
    w.journal.fsquid(&args);
}

#[then(expr = "the journal should hold one {string} entry that {string}")]
fn one_entry(w: &mut CliWorld, operation: String, outcome: String) {
    let entries = w.journal.entries();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].operation, operation);
    assert_eq!(entries[0].outcome.as_str(), outcome);
}

#[then(expr = "the journal should hold one {string} entry that {string} with {string}")]
fn one_failed_entry(w: &mut CliWorld, operation: String, outcome: String, code: String) {
    one_entry(w, operation, outcome);
    let entries = w.journal.entries();
    assert_eq!(entries[0].error_code.as_deref(), Some(code.as_str()));
}

#[then(expr = "the journal should hold {int} entry")]
fn entry_count(w: &mut CliWorld, count: usize) {
    assert_eq!(w.journal.entries().len(), count);
}

#[then("that entry should name the object and its version but no path or value")]
fn opaque_only(w: &mut CliWorld) {
    let entries = w.journal.entries();
    assert_eq!(
        entries[0].object_ref.as_deref(),
        Some(&*format!("{OBJECT_ID}@v1"))
    );
    let text = serde_json::to_string(&entries[0]).expect("json");
    let root = w.journal.root().to_string_lossy().into_owned();
    for leak in [TITLE, "dcterms:title", "holiday-album", root.as_str()] {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
}

#[then(expr = "that connection should see the {string} entry")]
fn held_sees(w: &mut CliWorld, operation: String) {
    let held = w.journal.held.as_ref().expect("a held connection");
    let entries = held.query(&JournalQuery::default()).expect("query").entries;
    assert!(
        entries.iter().any(|e| e.operation == operation),
        "{entries:?}"
    );
}

#[then(expr = "fsquid should refuse with one journal_unavailable line naming {string}")]
fn refused(w: &mut CliWorld, operation: String) {
    let output = w.journal.output.as_ref().expect("fsquid ran");
    assert!(!output.status.success());
    let stdout = w.journal.stdout();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "{stdout}");
    let line: serde_json::Value = serde_json::from_str(lines[0]).expect("json");
    assert_eq!(line["journal_unavailable"], true);
    assert_eq!(line["operation"], operation);
}

#[then("no object should have been written")]
fn nothing_written(w: &mut CliWorld) {
    assert!(!w.journal.object().exists());
}
