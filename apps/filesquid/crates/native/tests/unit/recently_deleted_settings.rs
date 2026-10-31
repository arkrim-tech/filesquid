//! Task API-F2.15 (API-S-019; ADR 041 Decisions 10 and 11): the installation's Recently
//! deleted settings in the desktop's operational store. They default to 30 days and
//! `keep_version_history`, survive a reopen, refuse an out-of-range period and are journaled
//! with every change, without any value.

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
    let bad = InstallationSettings {
        default_retention_days: 0,
        ..changed
    };
    let refused = reopened.store(&bad, JournalEntry::new("x", Outcome::Succeeded));
    assert!(
        refused.is_err(),
        "the store refuses an out-of-range period too"
    );
    assert_eq!(reopened.read().expect("read"), changed);
}
