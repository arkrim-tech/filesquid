//! Unit tests for the library routes' host side (Task DE-1.1): the folders the owner chose
//! ([`HostLocations`]) and the persistent repair ledger ([`SqliteRepairLedger`]).

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, Outcome};
use filesquid_api::local_library::{
    LibraryLocations, RecordedRepair, RepairLedger, RepairRecord, MAX_LOCATION_ID_BYTES,
};
use filesquid_native::import::plans::SqliteImportPlans;
use filesquid_native::{
    HostLocations, SqliteFindings, SqliteJobs, SqliteJournal, SqlitePreferences,
    SqliteRepairLedger, JOURNAL_FILE,
};
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "filesquid-native-library-host-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }

    /// A new folder `name` inside this directory.
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.0.join(name);
        std::fs::create_dir_all(&folder).expect("folder");
        folder
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove the test directory: {err}");
        }
    }
}

#[test]
fn a_registered_folder_gets_an_opaque_id_without_its_path() {
    let dir = TempDir::new("opaque");
    let locations = HostLocations::new(&dir.folder("data"));
    let folder = dir.folder("Family archive");
    let id = locations.register(&folder).expect("registered");
    assert!(id.starts_with("loc-"), "{id}");
    assert!(id.len() <= MAX_LOCATION_ID_BYTES);
    assert!(
        !id.contains("Family"),
        "the ID never carries the folder name"
    );
    assert!(locations.location(&id).is_some());
    assert!(locations.location("loc-unknown").is_none());
}

#[test]
fn one_folder_chosen_twice_keeps_its_id_and_a_second_host_id_shares_its_leases() {
    let dir = TempDir::new("shared");
    let locations = HostLocations::new(&dir.folder("data"));
    let folder = dir.folder("library");
    let first = locations.register(&folder).expect("first");
    let again = locations.register(&folder).expect("again");
    assert_eq!(first, again, "a repeated choice answers the existing ID");
    locations
        .register_as("loc-e2e-same", &folder)
        .expect("host id");
    let a = locations.location(&first).expect("a");
    let b = locations.location("loc-e2e-same").expect("b");
    assert!(Arc::ptr_eq(&a.reservations, &b.reservations));
    assert!(Arc::ptr_eq(&a.backend, &b.backend));
    let leases = dir.0.join("data").join("locations");
    let entries = std::fs::read_dir(&leases).expect("locations dir").count();
    assert_eq!(entries, 1, "one lease folder for the folder");
    assert!(
        locations.overlaps(&first, "loc-e2e-same"),
        "the same folder overlaps"
    );
}

#[test]
fn nested_and_unknown_locations_overlap_and_siblings_do_not() {
    let dir = TempDir::new("overlap");
    let locations = HostLocations::new(&dir.folder("data"));
    let outer = locations.register(&dir.folder("outer")).expect("outer");
    let inner = locations
        .register(&dir.folder("outer/inner"))
        .expect("inner");
    let sibling = locations.register(&dir.folder("outer2")).expect("sibling");
    assert!(locations.overlaps(&outer, &inner));
    assert!(locations.overlaps(&inner, &outer));
    assert!(
        !locations.overlaps(&outer, &sibling),
        "a name prefix is not a folder prefix"
    );
    assert!(locations.overlaps(&outer, "loc-unknown"));
}

#[test]
fn a_file_or_a_missing_folder_is_refused_without_its_path() {
    let dir = TempDir::new("refused");
    let locations = HostLocations::new(&dir.folder("data"));
    let file = dir.0.join("secret-name.txt");
    std::fs::write(&file, b"x").expect("file");
    for target in [file, dir.0.join("missing-secret")] {
        let err = locations.register(&target).expect_err("refused");
        assert!(!err.contains("secret"), "{err}");
    }
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_refused_as_a_location() {
    let dir = TempDir::new("link");
    let locations = HostLocations::new(&dir.folder("data"));
    let real = dir.folder("real");
    let link = dir.0.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("link");
    assert!(locations.register(&link).is_err());
}

/// A redacted repair journal entry for `object`.
fn repair_entry(object: &str, outcome: Outcome) -> JournalEntry {
    JournalEntry::new("ocfl.repair.version_inventory", outcome).target(object)
}

/// The receipt of a repaired `object`.
fn receipt(object: &str) -> RecordedRepair {
    RecordedRepair {
        object_id: object.into(),
        version: "v2".into(),
        digest_algorithm: "sha512".into(),
        inventory_digest: "ab12".into(),
        written: vec![
            "v2/inventory.json".into(),
            "v2/inventory.json.sha512".into(),
        ],
    }
}

/// How many journal rows `path` holds for the repair operation.
fn repair_rows(path: &std::path::Path) -> usize {
    let journal = SqliteJournal::open(path, "desktop").expect("journal");
    let query = JournalQuery {
        operation: Some("ocfl.repair.version_inventory".into()),
        ..JournalQuery::default()
    };
    journal.query(&query).expect("query").entries.len()
}

#[test]
fn the_ledger_journals_consents_and_receipts_and_keeps_them_across_a_restart() {
    let dir = TempDir::new("ledger");
    let path = dir.0.join(JOURNAL_FILE);
    {
        let ledger = SqliteRepairLedger::open(&path, "desktop").expect("ledger");
        ledger
            .record_consent(repair_entry("urn:uuid:a", Outcome::Started))
            .expect("consent");
        let repaired = RepairRecord {
            journal: repair_entry("urn:uuid:a", Outcome::Succeeded),
            receipt: Some(receipt("urn:uuid:a")),
        };
        ledger.record_outcome(repaired).expect("outcome");
        let refused = RepairRecord {
            journal: repair_entry("urn:uuid:b", Outcome::Failed).error_code("nothing_to_repair"),
            receipt: None,
        };
        ledger.record_outcome(refused).expect("refusal");
    }
    let reopened = SqliteRepairLedger::open(&path, "desktop").expect("reopened");
    assert_eq!(reopened.consent_count().expect("consents"), 1);
    assert_eq!(
        reopened.receipts().expect("receipts"),
        vec![receipt("urn:uuid:a")]
    );
    assert_eq!(
        repair_rows(&path),
        3,
        "consent and both outcomes are journaled"
    );
}

#[test]
fn a_host_chosen_id_must_be_well_formed_and_unique() {
    let dir = TempDir::new("chosen-id");
    let locations = HostLocations::new(&dir.folder("data"));
    let folder = dir.folder("library");
    locations
        .register_as("loc-e2e-create", &folder)
        .expect("registered");
    assert!(locations.location("loc-e2e-create").is_some());
    assert!(
        locations.register_as("loc-e2e-create", &folder).is_err(),
        "taken"
    );
    for bad in [
        "e2e",
        "loc-",
        "loc-a/b",
        "loc-a b",
        &format!("loc-{}", "a".repeat(200)),
    ] {
        assert!(locations.register_as(bad, &folder).is_err(), "{bad}");
    }
}

#[test]
fn a_folder_that_overlaps_the_data_directory_is_refused_and_prepares_nothing() {
    let dir = TempDir::new("data-overlap");
    let data = dir.folder("data");
    let locations = HostLocations::new(&data);
    for folder in [data.clone(), dir.folder("data/files"), dir.0.clone()] {
        let err = locations.register(&folder).expect_err("refused");
        assert!(err.contains("data directory"), "{err}");
    }
    assert!(!data.join("locations").exists(), "nothing was prepared");
}

#[test]
fn a_refused_id_prepares_no_storage() {
    let dir = TempDir::new("refused-id");
    let data = dir.folder("data");
    let locations = HostLocations::new(&data);
    locations
        .register_as("loc-taken", &dir.folder("first"))
        .expect("first");
    let before = std::fs::read_dir(data.join("locations"))
        .expect("dir")
        .count();
    assert!(locations
        .register_as("loc-taken", &dir.folder("second"))
        .is_err());
    let after = std::fs::read_dir(data.join("locations"))
        .expect("dir")
        .count();
    assert_eq!(before, after, "the refused folder got no lease folder");
}

#[test]
fn concurrent_choices_of_one_folder_get_one_id() {
    let dir = TempDir::new("concurrent");
    let locations = Arc::new(HostLocations::new(&dir.folder("data")));
    let folder = dir.folder("library");
    let ids: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| locations.register(&folder).expect("registered")))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("join"))
            .collect()
    });
    assert!(
        ids.iter().all(|id| *id == ids[0]),
        "one ID for one folder: {ids:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_stores_refuse_a_symbolic_link_in_place_of_the_operational_store() {
    let dir = TempDir::new("store-link");
    let real = dir.0.join("elsewhere.sqlite");
    std::fs::write(&real, b"").expect("file");
    let link = dir.0.join(JOURNAL_FILE);
    std::os::unix::fs::symlink(&real, &link).expect("link");
    assert!(SqliteRepairLedger::open(&link, "desktop").is_err());
    assert!(SqlitePreferences::open(&link).is_err());
    assert!(SqliteJobs::open(&link).is_err());
    assert!(SqliteFindings::open(&link).is_err());
    assert!(SqliteImportPlans::open(&link).is_err());
    assert_eq!(
        std::fs::metadata(&real).expect("target").len(),
        0,
        "never written"
    );
}

/// A folder above the operational store may be a symbolic link (macOS's `/var` is one for
/// `/private/var`, a home folder may be one): every store opens through it, and the file is
/// written in the folder the link names.
#[cfg(unix)]
#[test]
fn the_stores_open_through_a_linked_folder_above_the_operational_store() {
    let dir = TempDir::new("folder-link");
    let real = dir.folder("real");
    let link = dir.0.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("link");
    let store = link.join(JOURNAL_FILE);
    SqliteRepairLedger::open(&store, "desktop").expect("repair ledger");
    SqlitePreferences::open(&store).expect("preferences");
    SqliteJobs::open(&store).expect("jobs");
    SqliteFindings::open(&store).expect("findings");
    SqliteImportPlans::open(&store).expect("import plans");
    assert!(real.join(JOURNAL_FILE).is_file());
}

// ---- Library facts (Task API-F2.18; API-S-038) ----

#[test]
fn the_registry_keeps_each_librarys_name_and_last_opened_time_without_its_folder() {
    use filesquid_api::local_library::{LibraryRegistry, UNTITLED_LIBRARY};
    use filesquid_core::LibraryId;
    let dir = TempDir::new("registry");
    let locations = HostLocations::new(&dir.folder("data"));
    let location = locations
        .register(&dir.folder("Family archive"))
        .expect("registered");
    let library = LibraryId::new("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b").expect("id");
    assert_eq!(
        locations.remember(&library, &location, None, 1_000),
        UNTITLED_LIBRARY
    );
    assert_eq!(
        locations.remember(&library, &location, Some("Holidays"), 2_000),
        "Holidays"
    );
    assert_eq!(locations.rename(&library, "Holidays"), Some(false));
    assert_eq!(locations.rename(&library, "Holidays 1990s"), Some(true));
    let entry = locations.entry(&library).expect("remembered");
    assert_eq!(entry.name, "Holidays 1990s");
    assert_eq!(entry.last_opened_ms, 2_000);
    assert_eq!(locations.entries().len(), 1);
    let other = LibraryId::new("7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80").expect("id");
    assert_eq!(
        locations.rename(&other, "x"),
        None,
        "a library never opened here"
    );
}

#[test]
fn free_space_is_read_on_linux_and_macos_and_unreadable_for_an_unknown_location() {
    use filesquid_api::local_library::{FreeSpace, LibraryRegistry};
    let dir = TempDir::new("free");
    let locations = HostLocations::new(&dir.folder("data"));
    let location = locations
        .register(&dir.folder("library"))
        .expect("registered");
    let space = locations.free_space(&location);
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(
            matches!(space, FreeSpace::Bytes(bytes) if bytes > 0),
            "{space:?}"
        );
    } else {
        assert_eq!(space, FreeSpace::VolumeUnreadable, "not read by this build");
    }
    assert_eq!(
        locations.free_space("loc-unknown"),
        FreeSpace::VolumeUnreadable
    );
}
