//! Task CLI-0.2: every `fsquid` command that changes state or fails journals one redacted
//! entry (API-S-031; ADR 036 Decision 8). These tests run the commands in-process against an
//! injected journal store: a commit leaves one `cli.<module>.<command>` entry with opaque IDs
//! only, a failure leaves one failed entry, a command line that names no module command
//! leaves one failed `cli.route` entry, a no-op leaves none, and a journal write that fails
//! after a commit exits non-zero with the `journal_write_failed` stdout line.

use filesquid_api::journal::{
    JournalEntry, JournalError, JournalPage, JournalQuery, JournalStore, MemoryJournal, Outcome,
    RetentionPolicy,
};
use filesquid_cli::journal::{run_journaled, EXIT_JOURNAL_WRITE_FAILED, JOURNAL_HOST};
use std::path::PathBuf;

/// The object identifier the tests create.
const OBJECT_ID: &str = "urn:uuid:5b0c9a9e-6f0e-4d39-9a7e-3f5f4c1d2e10";

/// A statement value no journal entry may hold.
const VALUE: &str = "Grandma's birthday";

/// A fresh, empty directory for one test (removed by [`Scratch`]'s drop).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cli-0.2-unit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn object(&self) -> String {
        self.0.join("holiday-album").to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn entries(store: &MemoryJournal) -> Vec<JournalEntry> {
    store
        .query(&JournalQuery::default())
        .expect("query")
        .entries
}

/// Run `fsquid` with `line`, the object directory appended, against `store`.
fn run_on(store: &dyn JournalStore, object: &str, line: &[&str]) -> (i32, String) {
    let mut argv = args(line);
    argv.extend(args(&["--object", object]));
    let mut out = Vec::new();
    let code = run_journaled(&argv, store, &mut out);
    (code, String::from_utf8(out).expect("utf-8"))
}

fn set_title(store: &dyn JournalStore, object: &str) -> (i32, String) {
    run_on(
        store,
        object,
        &["set", "dcterms:title", VALUE, "--id", OBJECT_ID],
    )
}

#[test]
fn a_command_that_commits_leaves_one_redacted_entry() {
    let scratch = Scratch::new("commit");
    let store = MemoryJournal::new(JOURNAL_HOST);
    let (code, _) = set_title(&store, &scratch.object());
    assert_eq!(code, 0);
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 1, "{journaled:?}");
    let entry = &journaled[0];
    assert_eq!(entry.operation, "cli.metadata.set");
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(
        entry.object_ref.as_deref(),
        Some(&*format!("{OBJECT_ID}@v1"))
    );
    let text = serde_json::to_string(entry).expect("json");
    for leak in [
        VALUE,
        "dcterms:title",
        scratch.0.to_string_lossy().as_ref(),
        "holiday-album",
    ] {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
}

#[test]
fn a_failed_command_leaves_one_failed_entry() {
    let scratch = Scratch::new("failed");
    let store = MemoryJournal::new(JOURNAL_HOST);
    set_title(&store, &scratch.object());
    let (code, _) = run_on(&store, &scratch.object(), &["get", "dcterms:creator"]);
    assert_ne!(code, 0);
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 2, "{journaled:?}");
    let failed = &journaled[1];
    assert_eq!(failed.operation, "cli.metadata.get");
    assert_eq!(failed.outcome, Outcome::Failed);
    assert_eq!(failed.error_code.as_deref(), Some("command_failed"));
}

#[test]
fn a_refused_command_leaves_one_failed_entry_with_its_error_code() {
    let scratch = Scratch::new("refused");
    let store = MemoryJournal::new(JOURNAL_HOST);
    let (code, _) = run_on(&store, &scratch.object(), &["set", "dcterms:title"]);
    assert_ne!(code, 0);
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 1, "{journaled:?}");
    assert_eq!(journaled[0].operation, "cli.metadata.set");
    assert_eq!(journaled[0].error_code.as_deref(), Some("command_error"));
}

#[test]
fn validate_without_its_module_leaves_one_failed_route_entry() {
    let store = MemoryJournal::new(JOURNAL_HOST);
    let mut out = Vec::new();
    let code = run_journaled(&args(&["validate", "dcterms:title"]), &store, &mut out);
    assert_ne!(code, 0);
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 1, "{journaled:?}");
    assert_eq!(journaled[0].operation, "cli.route");
    assert_eq!(journaled[0].outcome, Outcome::Failed);
    assert_eq!(journaled[0].error_code.as_deref(), Some("module_required"));
}

#[test]
fn an_unknown_command_leaves_one_failed_route_entry() {
    let store = MemoryJournal::new(JOURNAL_HOST);
    for line in [&["mods"][..], &["metadata", "mods"][..]] {
        let mut out = Vec::new();
        assert_ne!(run_journaled(&args(line), &store, &mut out), 0);
    }
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 2, "{journaled:?}");
    for entry in journaled {
        assert_eq!(entry.operation, "cli.route");
        assert_eq!(entry.error_code.as_deref(), Some("unknown_command"));
    }
}

#[test]
fn a_command_that_changes_nothing_leaves_no_entry() {
    let scratch = Scratch::new("noop");
    let store = MemoryJournal::new(JOURNAL_HOST);
    set_title(&store, &scratch.object());
    let (again, _) = set_title(&store, &scratch.object());
    let (read, _) = run_on(&store, &scratch.object(), &["get", "dcterms:title"]);
    let mut out = Vec::new();
    let help = run_journaled(&args(&["help"]), &store, &mut out);
    assert_eq!((again, read, help), (0, 0, 0));
    assert_eq!(
        entries(&store).len(),
        1,
        "only the first commit is journaled"
    );
}

/// A journal store whose writes all fail.
struct Refusing;

impl JournalStore for Refusing {
    fn host(&self) -> &str {
        JOURNAL_HOST
    }

    fn append(&self, _entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        Err(JournalError("disk full".into()))
    }

    fn query(&self, _query: &JournalQuery) -> Result<JournalPage, JournalError> {
        Ok(JournalPage::from_overfetch(Vec::new(), 1))
    }

    fn count(&self, _query: &JournalQuery) -> Result<u64, JournalError> {
        Ok(0)
    }

    fn apply_retention(&self, _policy: RetentionPolicy, _now: i64) -> Result<u64, JournalError> {
        Ok(0)
    }

    fn ping(&self) -> Result<(), JournalError> {
        Ok(())
    }
}

#[test]
fn a_journal_write_that_fails_after_a_commit_exits_non_zero_with_the_entry() {
    let scratch = Scratch::new("write-failed");
    let (code, out) = set_title(&Refusing, &scratch.object());
    assert_eq!(code, EXIT_JOURNAL_WRITE_FAILED);
    let line = out
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v["journal_write_failed"] == true)
        .unwrap_or_else(|| panic!("no journal_write_failed line in {out}"));
    assert_eq!(line["operation"], "cli.metadata.set");
    assert_eq!(line["outcome"], "succeeded");
    assert!(!out.contains(VALUE), "{out}");
    assert!(
        scratch.0.join("holiday-album/inventory.json").exists(),
        "the change itself was committed"
    );
}

#[test]
fn a_successful_export_writes_a_file_and_is_journaled() {
    let scratch = Scratch::new("export");
    let store = MemoryJournal::new(JOURNAL_HOST);
    set_title(&store, &scratch.object());
    let sidecar = scratch.0.join("holiday.csv").to_string_lossy().into_owned();
    let (code, _) = run_on(&store, &scratch.object(), &["export", "csv", &sidecar]);
    assert_eq!(code, 0);
    assert!(scratch.0.join("holiday.csv").exists());
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 2, "{journaled:?}");
    assert_eq!(journaled[1].operation, "cli.metadata.export");
    assert_eq!(journaled[1].outcome, Outcome::Succeeded);
    assert!(!serde_json::to_string(&journaled[1])
        .expect("json")
        .contains("holiday"));
}

#[test]
fn a_registry_that_is_the_object_is_named_once() {
    let scratch = Scratch::new("registry");
    let store = MemoryJournal::new(JOURNAL_HOST);
    let object = scratch.object();
    set_title(&store, &object);
    let (code, _) = run_on(
        &store,
        &object,
        &[
            "set",
            "dcterms:title",
            "Another title",
            "--registry",
            &object,
        ],
    );
    assert_eq!(code, 0);
    let journaled = entries(&store);
    assert_eq!(journaled.len(), 2, "{journaled:?}");
    assert_eq!(
        journaled[1].object_ref.as_deref(),
        Some(&*format!("{OBJECT_ID}@v2"))
    );
    assert!(journaled[1].target_ids.is_empty(), "{journaled:?}");
}

#[test]
fn an_inventory_that_cannot_be_read_counts_as_unobserved() {
    use filesquid_cli::journal::object_heads;
    let scratch = Scratch::new("unreadable");
    let object = scratch.0.join("holiday-album");
    std::fs::create_dir_all(&object).expect("dir");
    std::fs::write(object.join("inventory.json"), b"{ not json").expect("inventory");
    let empty = scratch.0.join("empty");
    let heads = object_heads(&[object, empty]);
    assert!(heads.heads.is_empty());
    assert_eq!(
        heads.unobserved, 1,
        "a missing inventory is no object, not unobserved"
    );
}

#[test]
fn a_failed_command_whose_entry_cannot_be_written_keeps_its_own_code() {
    let scratch = Scratch::new("failed-write");
    set_title(&MemoryJournal::new(JOURNAL_HOST), &scratch.object());
    let (code, out) = run_on(&Refusing, &scratch.object(), &["get", "dcterms:creator"]);
    assert_eq!(code, 1, "nothing changed, so the command's own code stays");
    assert!(out.contains("\"journal_write_failed\":true"), "{out}");
}
