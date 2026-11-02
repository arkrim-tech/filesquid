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

/// The operation ID and object reference of PROV-O linkage (DE-0.10,
/// API-S-028; ADR 040 Decision 6) are kept by both stores.
#[test]
fn both_stores_keep_the_operation_id_and_object_reference() {
    let (stores, dir) = stores("linkage");
    let object_ref = "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b@v3";
    for store in &stores {
        let entry = JournalEntry::new("ocfl.commit", Outcome::Succeeded)
            .operation_id("op-1")
            .object_ref(object_ref);
        store.append(&[entry]).expect("append");
        let stored = &store.query(&JournalQuery::default()).unwrap().entries[0];
        assert_eq!(stored.operation_id.as_deref(), Some("op-1"));
        assert_eq!(stored.object_ref.as_deref(), Some(object_ref));
        let leaky = JournalEntry::new("ocfl.commit", Outcome::Succeeded)
            .operation_id("Grandma's birthday")
            .object_ref("/Users/ana/a.jpg@v1");
        store.append(&[leaky]).expect("append");
        let page = store.query(&JournalQuery::default()).unwrap();
        let stored = page
            .entries
            .iter()
            .find(|e| e.operation_id.as_deref() != Some("op-1"))
            .expect("leaky entry");
        assert_eq!(stored.operation_id.as_deref(), Some(REDACTED));
        assert_eq!(stored.object_ref.as_deref(), Some(REDACTED));
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// A journal created before the linkage columns existed is migrated
/// forward on open: old rows read without them, new rows keep them.
#[test]
fn an_older_journal_gains_the_linkage_columns_on_open() {
    let dir = temp_dir("migrate");
    let path = dir.join("filesquid.sqlite");
    {
        let conn = rusqlite::Connection::open(&path).expect("old database");
        conn.execute_batch(
            "CREATE TABLE activity_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT, time_ms INTEGER NOT NULL,
                host TEXT NOT NULL, process_id TEXT, actor TEXT NOT NULL,
                operation TEXT NOT NULL, outcome TEXT NOT NULL, level TEXT NOT NULL,
                target_ids TEXT NOT NULL, duration_ms INTEGER, error_code TEXT);
             INSERT INTO activity_journal (time_ms, host, actor, operation, outcome, level, target_ids)
             VALUES (1000, 'host-a', 'system', 'host.started', 'info', 'info', '[]');",
        )
        .expect("old schema");
    }
    for _ in 0..2 {
        let store = SqliteJournal::open(&path, "host-a").expect("migrated open");
        let entries = store.query(&JournalQuery::default()).unwrap().entries;
        let old = entries
            .iter()
            .find(|e| e.operation == "host.started")
            .unwrap();
        assert_eq!(old.operation_id, None);
        assert_eq!(old.object_ref, None);
    }
    let store = SqliteJournal::open(&path, "host-a").expect("open");
    store
        .append(&[JournalEntry::new("ocfl.commit", Outcome::Succeeded).operation_id("op-9")])
        .expect("append");
    let page = store.query(&JournalQuery::default()).unwrap();
    assert!(page
        .entries
        .iter()
        .any(|e| e.operation_id.as_deref() == Some("op-9")));
    drop(store);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Journal entry IDs of a commit are assigned before it (API-S-028; ADR 040
/// Decision 6): both stores hand out a block of IDs no other entry takes,
/// store entries under those IDs, and store a replayed entry only once.
#[test]
fn both_stores_assign_ids_before_the_entries_are_written() {
    let (stores, dir) = stores("allocate");
    for store in &stores {
        store
            .append(&[JournalEntry::new("host.started", Outcome::Info)])
            .expect("append");
        let reserved = store.allocate_ids(2).expect("allocate");
        assert_eq!(reserved.len(), 2);
        assert!(reserved[0] < reserved[1]);
        let later = store
            .append(&[JournalEntry::new("process.progress", Outcome::Info)])
            .expect("append");
        assert!(later[0] > reserved[1], "allocated IDs are never reused");
        let mut commit = JournalEntry::new("ocfl.commit", Outcome::Succeeded).operation_id("op-1");
        commit.id = Some(reserved[0]);
        assert_eq!(
            store.append_new(&[commit.clone()]).unwrap(),
            [(reserved[0], true)]
        );
        assert_eq!(
            store.append_new(&[commit]).unwrap(),
            [(reserved[0], false)],
            "a replay keeps its ID and says it stored nothing"
        );
        let entries = store.query(&JournalQuery::default()).unwrap().entries;
        let commits: Vec<_> = entries
            .iter()
            .filter(|e| e.operation == "ocfl.commit")
            .collect();
        assert_eq!(commits.len(), 1, "stored once");
        assert_eq!(commits[0].id, Some(reserved[0]));
        assert!(store.allocate_ids(0).unwrap().is_empty());
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// An entry carrying `client_event_id` (API-S-030).
fn client_event(id: &str) -> JournalEntry {
    let mut entry = JournalEntry::new("ui.console.warn", Outcome::Info).level(Level::Warn);
    entry.client_event_id = Some(id.to_string());
    entry
}

/// Each client event ID is stored once by both stores (API-S-030): appending it again,
/// in the same call or a later one, stores nothing and answers the first entry's id.
#[test]
fn both_stores_store_each_client_event_id_once() {
    let (stores, dir) = stores("client-events");
    let id = "00000001-0000-4000-8000-000000000000";
    for store in &stores {
        let first = store
            .append_new(&[client_event(id), client_event(id)])
            .expect("append");
        assert!(first[0].1, "stored now");
        assert_eq!(first[1], (first[0].0, false), "the same id in one call");
        let again = store.append_new(&[client_event(id)]).expect("append");
        assert_eq!(again, [(first[0].0, false)]);
        assert_eq!(store.count(&JournalQuery::default()).unwrap(), 1);
        let stored = store.entry(first[0].0).unwrap().expect("entry");
        assert_eq!(stored.client_event_id.as_deref(), Some(id));
        store
            .append(&[JournalEntry::new("host.started", Outcome::Info)])
            .expect("entries without a client event id are never de-duplicated");
        store
            .append(&[JournalEntry::new("host.started", Outcome::Info)])
            .expect("append");
        assert_eq!(store.count(&JournalQuery::default()).unwrap(), 3);
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// The SQLite store's unique index keeps de-duplicating after the database is reopened.
#[test]
fn client_event_de_duplication_survives_a_restart() {
    let dir = temp_dir("client-events-restart");
    let path = dir.join("filesquid.sqlite");
    let id = "00000002-0000-4000-8000-000000000000";
    let first = {
        let store = SqliteJournal::open(&path, "host-a").expect("open");
        store.append_new(&[client_event(id)]).expect("append")[0]
    };
    let store = SqliteJournal::open(&path, "host-a").expect("reopen");
    assert_eq!(
        store.append_new(&[client_event(id)]).expect("append"),
        [(first.0, false)]
    );
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 1);
    drop(store);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// A client event ID that is not an opaque token is dropped by both stores, so two such
/// entries are both kept instead of colliding on one redacted key.
#[test]
fn a_client_event_id_that_is_not_opaque_is_dropped_and_its_entry_kept() {
    let (stores, dir) = stores("client-event-redaction");
    for store in &stores {
        store
            .append(&[
                client_event("Grandma's birthday"),
                client_event("Ben's party"),
            ])
            .expect("append");
        let entries = store.query(&JournalQuery::default()).unwrap().entries;
        assert_eq!(entries.len(), 2, "neither entry is lost");
        assert!(entries.iter().all(|e| e.client_event_id.is_none()));
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// An entry with a preset id whose client event ID another row holds answers the
/// holder's id, not its own.
#[test]
fn a_preset_id_repeating_a_held_client_event_id_answers_the_holder() {
    let (stores, dir) = stores("client-event-preset");
    let id = "00000004-0000-4000-8000-000000000000";
    for store in &stores {
        let held = store.append_new(&[client_event(id)]).expect("append")[0].0;
        let mut replay = client_event(id);
        replay.id = Some(store.allocate_ids(1).expect("allocate")[0]);
        assert_eq!(
            store.append_new(&[replay]).expect("append"),
            [(held, false)]
        );
        assert_eq!(store.count(&JournalQuery::default()).unwrap(), 1);
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// The library, object and operation ID filters of API-S-031 select the same entries in
/// both stores, and `entry` reads one entry by id.
#[test]
fn both_stores_filter_by_library_object_and_operation_id() {
    let (stores, dir) = stores("query-filters");
    let object = "urn:uuid:5f0c3e44-2a7b-4c1d-9e8f-0123456789ab";
    for store in &stores {
        store
            .append(&[
                JournalEntry::new("ocfl.commit", Outcome::Succeeded)
                    .operation_id("op-1")
                    .object_ref(format!("{object}@v1"))
                    .target("lib-a"),
                JournalEntry::new("ocfl.validation.finding", Outcome::Info)
                    .target("lib-a")
                    .target(object),
                JournalEntry::new("ocfl.commit", Outcome::Succeeded)
                    .operation_id("op-2")
                    .object_ref(format!("{object}0@v1"))
                    .target("lib-b"),
            ])
            .expect("append");
        let ids = |query: JournalQuery| -> Vec<i64> {
            store
                .query(&query)
                .unwrap()
                .entries
                .iter()
                .filter_map(|e| e.id)
                .collect()
        };
        let all = ids(JournalQuery::default());
        let library = JournalQuery {
            library_id: Some("lib-a".into()),
            ..JournalQuery::default()
        };
        assert_eq!(ids(library), all[..2]);
        let by_object = JournalQuery {
            object_id: Some(object.into()),
            ..JournalQuery::default()
        };
        assert_eq!(
            ids(by_object),
            all[..2],
            "a longer object id is another object"
        );
        let by_operation = JournalQuery {
            operation_id: Some("op-2".into()),
            ..JournalQuery::default()
        };
        assert_eq!(ids(by_operation.clone()), all[2..]);
        assert_eq!(store.count(&by_operation).unwrap(), 1);
        assert_eq!(
            store.entry(all[1]).unwrap().expect("entry").operation,
            "ocfl.validation.finding"
        );
        assert!(store.entry(all[2] + 100).unwrap().is_none());
    }
    drop(stores);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// A journal created before client event IDs existed gains the column and its unique
/// index on open.
#[test]
fn an_older_journal_gains_the_client_event_id_column_on_open() {
    let dir = temp_dir("migrate-client-events");
    let path = dir.join("filesquid.sqlite");
    {
        let conn = rusqlite::Connection::open(&path).expect("old database");
        conn.execute_batch(
            "CREATE TABLE activity_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT, time_ms INTEGER NOT NULL,
                host TEXT NOT NULL, process_id TEXT, actor TEXT NOT NULL,
                operation TEXT NOT NULL, outcome TEXT NOT NULL, level TEXT NOT NULL,
                target_ids TEXT NOT NULL, duration_ms INTEGER, error_code TEXT,
                operation_id TEXT, object_ref TEXT);",
        )
        .expect("old schema");
    }
    let store = SqliteJournal::open(&path, "host-a").expect("migrated open");
    let id = "00000003-0000-4000-8000-000000000000";
    store.append(&[client_event(id)]).expect("append");
    assert!(!store.append_new(&[client_event(id)]).expect("append")[0].1);
    drop(store);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// DE-0.11: a refilled store reserves ids through the highest id its sealed journal read, so
/// a new entry never takes an id still to be restored; a lower reservation changes nothing.
#[test]
fn reserving_through_an_id_makes_new_ids_greater() {
    let dir = temp_dir("reserve-through");
    let store = SqliteJournal::open(&dir.join("filesquid.sqlite"), "desktop").expect("store");
    store.reserve_through(100).expect("reserve");
    store.reserve_through(5).expect("a lower reservation");
    let ids = store
        .append(&[entry("host.started", Outcome::Succeeded, 1)])
        .expect("append");
    assert_eq!(ids, vec![101]);
    drop(store);
    std::fs::remove_dir_all(dir).expect("cleanup");
}
