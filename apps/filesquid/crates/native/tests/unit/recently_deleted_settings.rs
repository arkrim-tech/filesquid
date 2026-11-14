//! Tasks API-F2.15 and API-F2.30 (API-S-019, DE-S-014; ADR 041 Decisions 10 and 11): the
//! installation's Recently deleted settings in the desktop's operational store. They default
//! to 30 days, `keep_version_history` and an expiry run every 60 minutes, survive a reopen,
//! refuse an out-of-range period or interval and are journaled with every change, without
//! any value. A store written before the expiry interval existed gains it at 60 minutes.

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::library::{ExpiryMode, InstallationSettings, RetentionSettings};
use filesquid_native::SqliteRetentionSettings;
use std::sync::Arc;

/// A fresh operational store path.
fn store_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("f215-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir.join("filesquid.sqlite")
}

#[test]
fn settings_default_survive_a_reopen_and_are_journaled() {
    let path = store_path("default");
    let journal = Arc::new(MemoryJournal::new("desktop"));
    let store = SqliteRetentionSettings::open(&path, journal.clone()).expect("open");
    assert_eq!(store.read().expect("read"), InstallationSettings::default());
    let changed = InstallationSettings {
        default_retention_days: 90,
        expiry: ExpiryMode::DeleteAllVersionHistory,
        expiry_interval_minutes: 15,
    };
    let entry =
        JournalEntry::new("settings.recently_deleted.set", Outcome::Succeeded).actor("owner");
    store.store(&changed, entry).expect("store");
    drop(store);
    let reopened = SqliteRetentionSettings::open(&path, journal.clone()).expect("reopen");
    assert_eq!(reopened.read().expect("read"), changed);
    let entries = journal
        .query(&JournalQuery::default())
        .expect("journal")
        .entries;
    let set: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == "settings.recently_deleted.set")
        .collect();
    assert_eq!(set.len(), 1);
    assert!(set[0].target_ids.is_empty(), "no value is journaled");
    assert_eq!(set[0].actor, "owner");
    for days in [0, 3_651] {
        let bad = InstallationSettings {
            default_retention_days: days,
            ..changed
        };
        let refused = reopened.store(&bad, JournalEntry::new("x", Outcome::Succeeded));
        assert!(refused.is_err(), "the store refuses {days} days too");
        assert_eq!(reopened.read().expect("read"), changed);
    }
}

/// The `settings.recently_deleted.set` entries of `journal`.
fn set_entries(journal: &MemoryJournal) -> Vec<JournalEntry> {
    let entries = journal
        .query(&JournalQuery::default())
        .expect("journal")
        .entries;
    entries
        .into_iter()
        .filter(|e| e.operation == "settings.recently_deleted.set")
        .collect()
}

/// Task API-F2.30: the expiry interval is 60 minutes by default; 5 and 1,440 are stored and
/// survive a reopen; 4 and 1,441 are refused with nothing stored and nothing journaled.
#[test]
fn the_expiry_interval_is_stored_within_its_bounds_and_survives_a_reopen() {
    let path = store_path("interval");
    let journal = Arc::new(MemoryJournal::new("desktop"));
    let store = SqliteRetentionSettings::open(&path, journal.clone()).expect("open");
    assert_eq!(store.read().expect("read").expiry_interval_minutes, 60);
    let set = || JournalEntry::new("settings.recently_deleted.set", Outcome::Succeeded);
    for (n, minutes) in [5, 1_440].into_iter().enumerate() {
        let changed = InstallationSettings {
            expiry_interval_minutes: minutes,
            ..InstallationSettings::default()
        };
        store.store(&changed, set().actor("owner")).expect("store");
        let reopened = SqliteRetentionSettings::open(&path, journal.clone()).expect("reopen");
        assert_eq!(reopened.read().expect("read"), changed);
        let entries = set_entries(&journal);
        assert_eq!(entries.len(), n + 1, "journaled once");
        assert!(entries[n].target_ids.is_empty(), "no value is journaled");
    }
    for minutes in [0, 4, 1_441] {
        let bad = InstallationSettings {
            expiry_interval_minutes: minutes,
            ..InstallationSettings::default()
        };
        assert!(store.store(&bad, set()).is_err(), "{minutes} is refused");
        assert_eq!(store.read().expect("read").expiry_interval_minutes, 1_440);
        assert_eq!(set_entries(&journal).len(), 2, "a refusal journals nothing");
    }
}

/// The settings table as Task API-F2.15 created it, before the expiry interval existed.
const SCHEMA_BEFORE_THE_INTERVAL: &str = "
CREATE TABLE recently_deleted_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    default_retention_days INTEGER NOT NULL CHECK (default_retention_days BETWEEN 1 AND 3650),
    expiry TEXT NOT NULL CHECK (expiry IN ('keep_version_history', 'delete_all_version_history'))
);
INSERT INTO recently_deleted_settings VALUES (1, 45, 'delete_all_version_history');
";

/// Task API-F2.30: an operational store written before the expiry interval existed keeps its
/// settings and gains the interval at 60 minutes, which can then be changed; opening it
/// again changes nothing.
#[test]
fn a_store_from_before_the_expiry_interval_gains_it_at_60_minutes() {
    let path = store_path("migrate");
    let old = rusqlite::Connection::open(&path).expect("old store");
    old.execute_batch(SCHEMA_BEFORE_THE_INTERVAL)
        .expect("old schema");
    drop(old);
    let journal = Arc::new(MemoryJournal::new("desktop"));
    let before = InstallationSettings {
        default_retention_days: 45,
        expiry: ExpiryMode::DeleteAllVersionHistory,
        expiry_interval_minutes: 60,
    };
    let store = SqliteRetentionSettings::open(&path, journal.clone()).expect("open");
    assert_eq!(store.read().expect("read"), before);
    let changed = InstallationSettings {
        expiry_interval_minutes: 30,
        ..before
    };
    let entry = JournalEntry::new("settings.recently_deleted.set", Outcome::Succeeded);
    store.store(&changed, entry).expect("store");
    drop(store);
    let reopened = SqliteRetentionSettings::open(&path, journal).expect("reopen");
    assert_eq!(reopened.read().expect("read"), changed);
}
