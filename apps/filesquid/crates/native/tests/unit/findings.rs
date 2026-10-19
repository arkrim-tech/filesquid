//! Unit tests for the OCFL validation findings stores (Task API-F2.7, API-S-022): the same
//! contract against the API's in-memory store and this crate's SQLite store, plus the SQLite
//! store keeping findings, cursors and runs across a reopen (a host restart).

use filesquid_api::ocfl_validation::{
    Finding, FindingsStore, MemoryFindingsStore, RunOutcome, RunRecord, RunTrigger, Severity,
};
use filesquid_core::LibraryId;
use filesquid_native::SqliteFindings;
use std::path::PathBuf;

/// A fresh temporary directory for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-findings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// The library every test uses.
fn library() -> LibraryId {
    LibraryId::new("lib-1").expect("id")
}

/// A finding of `object` with `code` at `location`.
fn found(object: &str, code: &str, severity: Severity, location: &str) -> Finding {
    Finding {
        library_id: library(),
        object_id: object.into(),
        version: Some("v1".into()),
        code: code.into(),
        severity,
        location: location.into(),
    }
}

/// The store contract every findings store keeps.
fn contract(store: &dyn FindingsStore) {
    let e023 = found("urn:b", "E023", Severity::Error, "obj-b/v1/content/x");
    let w004 = found("urn:b", "W004", Severity::Warning, "obj-b/inventory.json");
    let change = store
        .replace_findings(&library(), "urn:b", vec![e023.clone(), w004.clone()])
        .expect("replace");
    assert_eq!((change.opened.len(), change.closed.len()), (2, 0));
    let repeated = store
        .replace_findings(&library(), "urn:c", vec![e023.clone(), e023.clone()])
        .expect("replace");
    assert_eq!(repeated.opened.len(), 1, "a repeat is one finding");
    assert_eq!(
        repeated.opened[0].object_id, "urn:c",
        "stored under the call's object"
    );
    store
        .replace_findings(&library(), "urn:c", Vec::new())
        .expect("close");
    let again = store
        .replace_findings(&library(), "urn:b", vec![e023.clone(), w004.clone()])
        .expect("replace");
    assert!(again.is_empty(), "the same findings open and close nothing");
    store
        .replace_findings(
            &library(),
            "urn:a",
            vec![found("urn:a", "E092", Severity::Error, "obj-a/x")],
        )
        .expect("replace");
    let summary = store.error_summary().expect("summary");
    assert_eq!(summary.codes, ["E023", "E092"]);
    assert_eq!(summary.objects, 2);
    let open = store.open_findings(10).expect("open");
    let order: Vec<(&str, &str)> = open
        .iter()
        .map(|f| (f.object_id.as_str(), f.code.as_str()))
        .collect();
    assert_eq!(
        order,
        [("urn:a", "E092"), ("urn:b", "E023"), ("urn:b", "W004")]
    );
    assert_eq!(store.open_findings(1).expect("open").len(), 1);
    assert_eq!(
        store.objects_with_findings(&library()).expect("objects"),
        ["urn:a", "urn:b"]
    );
    let fixed = store
        .replace_findings(&library(), "urn:b", vec![w004])
        .expect("replace");
    assert_eq!(fixed.closed, [e023]);
    let deleted = store
        .replace_findings(&library(), "urn:a", Vec::new())
        .expect("close");
    assert_eq!(deleted.closed.len(), 1);
    assert_eq!(store.error_summary().expect("summary").objects, 0);
    assert_eq!(store.cursor(&library()).expect("cursor"), None);
    store.set_cursor(&library(), Some("urn:b")).expect("set");
    assert_eq!(
        store.cursor(&library()).expect("cursor").as_deref(),
        Some("urn:b")
    );
    store.set_cursor(&library(), None).expect("clear");
    assert_eq!(store.cursor(&library()).expect("cursor"), None);
    let record = RunRecord {
        library_id: library(),
        trigger: RunTrigger::Scheduled,
        started_ms: 1,
        ended_ms: 2,
        outcome: RunOutcome::Capped,
        checked: 3,
        not_checked: 4,
        error_code: None,
    };
    store.record_run(&record).expect("record");
    assert_eq!(store.last_run(&library()).expect("last"), Some(record));
}

/// The contract that keeps libraries apart: findings, objects, cursor and last run of one
/// library never show in, or change, another's.
fn isolation(store: &dyn FindingsStore) {
    let other = LibraryId::new("lib-2").expect("id");
    let mine = found("urn:b", "E023", Severity::Error, "obj-b/v1/content/x");
    let theirs = Finding {
        library_id: other.clone(),
        ..found("urn:b", "E092", Severity::Error, "obj-b/x")
    };
    store
        .replace_findings(&library(), "urn:b", vec![mine.clone()])
        .expect("mine");
    let opened = store
        .replace_findings(&other, "urn:b", vec![theirs.clone()])
        .expect("theirs");
    assert_eq!(
        opened.opened,
        std::slice::from_ref(&theirs),
        "the same object ID in another library"
    );
    assert!(
        opened.closed.is_empty(),
        "nothing of the first library closes"
    );
    assert_eq!(
        store.objects_with_findings(&library()).expect("mine"),
        ["urn:b"]
    );
    assert_eq!(
        store.objects_with_findings(&other).expect("theirs"),
        ["urn:b"]
    );
    assert_eq!(store.error_summary().expect("summary").objects, 2);
    store.set_cursor(&library(), Some("urn:b")).expect("cursor");
    assert_eq!(store.cursor(&other).expect("theirs"), None);
    let record = RunRecord {
        library_id: library(),
        trigger: RunTrigger::Requested,
        started_ms: 1,
        ended_ms: 2,
        outcome: RunOutcome::Completed,
        checked: 1,
        not_checked: 0,
        error_code: None,
    };
    store.record_run(&record).expect("record");
    assert_eq!(store.last_run(&other).expect("theirs"), None);
    let closed = store
        .replace_findings(&other, "urn:b", Vec::new())
        .expect("close theirs");
    assert_eq!(closed.closed, [theirs]);
    assert_eq!(
        store.objects_with_findings(&other).expect("theirs"),
        Vec::<String>::new()
    );
    let open = store.open_findings(10).expect("open");
    assert_eq!(open, [mine], "only the first library's finding is left");
    assert_eq!(store.last_run(&library()).expect("mine"), Some(record));
}

#[test]
fn the_memory_store_keeps_the_contract() {
    contract(&MemoryFindingsStore::new());
    isolation(&MemoryFindingsStore::new());
}

#[test]
fn the_sqlite_store_keeps_the_contract() {
    let dir = temp_dir("contract");
    contract(&SqliteFindings::open(&dir.join("filesquid.sqlite")).expect("open"));
    isolation(&SqliteFindings::open(&dir.join("isolation.sqlite")).expect("open"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_sqlite_store_keeps_findings_cursor_and_run_across_a_restart() {
    let dir = temp_dir("restart");
    let path = dir.join("filesquid.sqlite");
    let record = RunRecord {
        library_id: library(),
        trigger: RunTrigger::Requested,
        started_ms: 10,
        ended_ms: 20,
        outcome: RunOutcome::Failed,
        checked: 0,
        not_checked: 0,
        error_code: Some("unavailable".into()),
    };
    {
        let store = SqliteFindings::open(&path).expect("open");
        let e023 = found("urn:b", "E023", Severity::Error, "obj-b/v1/content/x");
        store
            .replace_findings(&library(), "urn:b", vec![e023])
            .expect("replace");
        store.set_cursor(&library(), Some("urn:b")).expect("cursor");
        store.record_run(&record).expect("run");
    }
    let reopened = SqliteFindings::open(&path).expect("reopen");
    assert_eq!(reopened.open_findings(10).expect("open").len(), 1);
    assert_eq!(
        reopened.cursor(&library()).expect("cursor").as_deref(),
        Some("urn:b")
    );
    assert_eq!(reopened.last_run(&library()).expect("run"), Some(record));
    let _ = std::fs::remove_dir_all(&dir);
}
