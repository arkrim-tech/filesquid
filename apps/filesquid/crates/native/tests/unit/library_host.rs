//! Unit tests for the library routes' host side (Task DE-1.1): the folders the owner chose
//! ([`HostLocations`]) and the persistent repair ledger ([`SqliteRepairLedger`]).

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, Outcome};
use filesquid_api::local_library::{
    LibraryLocations, RecordedRepair, RepairLedger, RepairRecord, MAX_LOCATION_ID_BYTES,
};
use filesquid_native::{HostLocations, SqliteJournal, SqliteRepairLedger, JOURNAL_FILE};
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
fn one_folder_registered_twice_shares_its_leases() {
    let dir = TempDir::new("shared");
    let locations = HostLocations::new(&dir.folder("data"));
    let folder = dir.folder("library");
    let first = locations.register(&folder).expect("first");
    let second = locations.register(&folder).expect("second");
    assert_ne!(first, second);
    let a = locations.location(&first).expect("a");
    let b = locations.location(&second).expect("b");
    assert!(Arc::ptr_eq(&a.reservations, &b.reservations));
    assert!(Arc::ptr_eq(&a.backend, &b.backend));
    let leases = dir.0.join("data").join("locations");
    let entries = std::fs::read_dir(&leases).expect("locations dir").count();
    assert_eq!(entries, 1, "both registrations use one lease folder");
    assert!(
        locations.overlaps(&first, &second),
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
