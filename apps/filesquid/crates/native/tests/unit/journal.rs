//! Unit tests for the activity journal stores (SRS-F-061 K1, K2, K13): the same contract
//! against the API's in-memory store and this crate's SQLite store. Moved here from
//! `filesquid-api` with the SQLite store (Task CL-0.1).

use filesquid_api::journal::{
    is_opaque, JournalEntry, JournalQuery, JournalStore, Level, MemoryJournal, Outcome,
    RetentionPolicy, MAX_OPAQUE_LEN, REDACTED,
};
use filesquid_native::SqliteJournal;
use log_names::is_log_file_name;
use std::path::PathBuf;

#[path = "../support/log_names.rs"]
#[allow(dead_code)]
mod log_names;

#[test]
fn log_file_names_match_the_original_suffix_rule() {
    let ext = log_names::LOG_EXTENSION;
    for name in [
        format!("host.{ext}"),
        format!(".{ext}"),
        format!("a.b.{ext}"),
    ] {
        assert!(is_log_file_name(&name), "{name}");
    }
    for name in [
        "filesquid.sqlite",
        "filesquid.sqlite-wal",
        "log",
        "catalog",
        "x.logs",
    ] {
        assert!(!is_log_file_name(name), "{name}");
    }
}

/// Fresh temporary directory for one test (removed by the caller).
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-journal-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn entry(op: &str, outcome: Outcome, at_ms: i64) -> JournalEntry {
    JournalEntry::new(op, outcome)
        .actor("user:ana")
        .target("obj-1")
        .at_ms(at_ms)
}

/// Run the same contract against both store implementations.
fn stores(name: &str) -> (Vec<Box<dyn JournalStore>>, PathBuf) {
    let dir = temp_dir(name);
    let sqlite = SqliteJournal::open(&dir.join("filesquid.sqlite"), "host-a").expect("open sqlite");
    (
        vec![Box::new(MemoryJournal::new("host-a")), Box::new(sqlite)],
        dir,
    )
}

#[test]
fn append_then_query_returns_entries_in_order_with_ids() {
    let (stores, dir) = stores("order");
    for store in stores {
        store
            .append(&[
                entry("import", Outcome::Succeeded, 1_000),
                entry("ocfl.commit", Outcome::Succeeded, 2_000),
            ])
            .unwrap();
        let page = store.query(&JournalQuery::default()).unwrap();
        let ops: Vec<_> = page.entries.iter().map(|e| e.operation.as_str()).collect();
        assert_eq!(ops, ["import", "ocfl.commit"]);
        assert!(page
            .entries
            .iter()
            .all(|e| e.id.is_some() && e.host == "host-a"));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn filters_and_cursor_paging() {
    let (stores, dir) = stores("filters");
    for store in stores {
        let mut batch = Vec::new();
        for i in 0..5 {
            batch.push(entry("ocfl.commit", Outcome::Failed, 1_000 + i));
            batch.push(entry("import", Outcome::Succeeded, 1_000 + i));
        }
        store.append(&batch).unwrap();
        let q = JournalQuery {
            operation: Some("ocfl.commit".into()),
            outcome: Some(Outcome::Failed),
            limit: 2,
            ..Default::default()
        };
        let first = store.query(&q).unwrap();
        assert_eq!(first.entries.len(), 2);
        let next = first.next_cursor.expect("more pages");
        let second = store
            .query(&JournalQuery {
                after_id: Some(next),
                ..q.clone()
            })
            .unwrap();
        assert_eq!(second.entries.len(), 2);
        assert!(second.entries[0].id > first.entries[1].id);
        assert_eq!(
            store
                .count(&JournalQuery {
                    operation: Some("ocfl.commit".into()),
                    ..Default::default()
                })
                .unwrap(),
            5
        );
        let window = JournalQuery {
            from_ms: Some(1_002),
            to_ms: Some(1_003),
            ..Default::default()
        };
        assert_eq!(store.count(&window).unwrap(), 4);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn entries_serialize_rfc3339_utc_time_and_lowercase_enums() {
    let e = entry("import", Outcome::Failed, 1_700_000_000_123)
        .level(Level::Error)
        .error_code("EIO");
    let json = serde_json::to_value(&e).unwrap();
    assert_eq!(json["time"], "2023-11-14T22:13:20.123Z");
    assert_eq!(json["outcome"], "failed");
    assert_eq!(json["level"], "error");
    assert_eq!(json["error_code"], "EIO");
}

#[test]
fn sqlite_journal_is_append_only() {
    let dir = temp_dir("append-only");
    let path = dir.join("filesquid.sqlite");
    let store = SqliteJournal::open(&path, "host-a").unwrap();
    store
        .append(&[entry("import", Outcome::Succeeded, 1)])
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert!(conn
        .execute("UPDATE activity_journal SET operation = 'x'", [])
        .is_err());
    assert!(conn.execute("DELETE FROM activity_journal", []).is_err());
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn domain_operations_are_journaled_in_the_callers_transaction() {
    let dir = temp_dir("tx");
    let path = dir.join("filesquid.sqlite");
    let store = SqliteJournal::open(&path, "host-a").unwrap();
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    {
        let tx = conn.transaction().unwrap();
        SqliteJournal::record_in_transaction(
            &tx,
            "host-a",
            &entry("rename", Outcome::Succeeded, 5),
        )
        .unwrap();
        // rolled back: neither the change nor its journal entry exists
    }
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 0);
    let tx = conn.transaction().unwrap();
    SqliteJournal::record_in_transaction(&tx, "host-a", &entry("rename", Outcome::Succeeded, 6))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn retention_keeps_everything_by_default_and_journals_its_own_deletions() {
    let (stores, dir) = stores("retention");
    for store in stores {
        store
            .append(&[
                entry("import", Outcome::Succeeded, 0),
                entry("import", Outcome::Succeeded, 10 * 86_400_000),
            ])
            .unwrap();
        assert_eq!(
            store
                .apply_retention(RetentionPolicy::KeepAll, 40 * 86_400_000)
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .apply_retention(RetentionPolicy::MaxAgeDays(35), 40 * 86_400_000)
                .unwrap(),
            1
        );
        let ops: Vec<_> = store
            .query(&JournalQuery::default())
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.operation)
            .collect();
        assert_eq!(ops, ["import", "journal.retention"]);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_creates_the_database_and_writes_no_log_files() {
    let dir = temp_dir("files");
    let store = SqliteJournal::open(&dir.join("filesquid.sqlite"), "host-a").unwrap();
    store
        .append(&[entry("import", Outcome::Succeeded, 1)])
        .unwrap();
    store.ping().unwrap();
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().any(|n| n == "filesquid.sqlite"));
    assert!(
        names.iter().all(|n| !is_log_file_name(n)),
        "no log files: {names:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn descending_queries_page_newest_first() {
    let (stores, dir) = stores("desc");
    for store in stores {
        let batch: Vec<_> = (0..5)
            .map(|i| entry(&format!("op{i}"), Outcome::Succeeded, 1_000 + i))
            .collect();
        store.append(&batch).unwrap();
        let q = JournalQuery {
            descending: true,
            limit: 2,
            ..Default::default()
        };
        let first = store.query(&q).unwrap();
        let ops: Vec<_> = first.entries.iter().map(|e| e.operation.clone()).collect();
        assert_eq!(ops, ["op4", "op3"]);
        let second = store
            .query(&JournalQuery {
                after_id: first.next_cursor,
                ..q.clone()
            })
            .unwrap();
        let ops: Vec<_> = second.entries.iter().map(|e| e.operation.clone()).collect();
        assert_eq!(ops, ["op2", "op1"]);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn identifiers_that_are_not_opaque_are_redacted_by_both_stores() {
    let (stores, dir) = stores("redact");
    for store in &stores {
        let leaky = JournalEntry::new("import", Outcome::Succeeded)
            .actor("Ana Smith")
            .target("obj-1")
            .target("/Users/ana/Pictures/Grandma's birthday.jpg")
            .error_code("caption: sunset at the beach");
        store.append(&[leaky]).expect("append");
        let stored = &store.query(&JournalQuery::default()).unwrap().entries[0];
        assert_eq!(stored.actor, REDACTED);
        assert_eq!(stored.target_ids, ["obj-1", REDACTED]);
        assert_eq!(stored.error_code.as_deref(), Some(REDACTED));
        assert_eq!(stored.operation, "import");
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn opaque_identifiers_are_kept() {
    for id in [
        "user:ana",
        "obj-1",
        "process.started",
        "4f1c2a9e-0b7d-4c1e-9a3f-2d6b8e7c5a10",
    ] {
        assert!(is_opaque(id), "{id}");
    }
    for text in [
        "",
        "two words",
        "a/b",
        "Grandma's",
        &"x".repeat(MAX_OPAQUE_LEN + 1),
    ] {
        assert!(!is_opaque(text), "{text}");
    }
}

#[cfg(unix)]
#[test]
fn the_sqlite_journal_and_its_wal_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("mode");
    let path = dir.join("filesquid.sqlite");
    let store = SqliteJournal::open(&path, "host-a").expect("open");
    store
        .append(&[JournalEntry::new("import", Outcome::Succeeded)])
        .expect("append");
    for file in ["filesquid.sqlite", "filesquid.sqlite-wal"] {
        let mode = std::fs::metadata(dir.join(file))
            .expect(file)
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{file}");
    }
    drop(store);
    std::fs::remove_dir_all(dir).expect("cleanup");
}
