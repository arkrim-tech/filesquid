//! Task CL-1.1: create and open a local library on an owner-chosen OCFL storage root, copy
//! a completed snapshot, rebuild from files, and never edit the source in place from
//! inspection or copy (F-001, F-042, SRS-F-001, SRS-F-042, API-S-005, API-S-006).

#[path = "../support/ocfl_fixture.rs"]
mod fixture;

use filesquid_api::journal::Outcome;
use filesquid_api::local_library::LocalLibraryError;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend, MAX_PUT_BYTES};
use fixture::{key, library, ready, CountingBackend, Rig};

/// Paths or storage keys that must never appear in answers or the journal.
const PATH_MARKERS: [&str; 4] = ["/", "inventory.json", "obj-a", "content"];

/// Assert `text` carries no path, key or object root.
fn assert_no_paths(text: &str) {
    for marker in PATH_MARKERS {
        assert!(!text.contains(marker), "{marker} leaked in {text}");
    }
}

#[test]
fn create_writes_only_the_storage_root_declaration() {
    let rig = Rig::new();
    let summary = ready(rig.service.create_root("owner", &library(), &rig.root)).expect("create");
    assert_eq!(summary.spec_version, "1.1");
    assert_eq!(summary.objects, 0);
    let keys: Vec<String> = rig.root.snapshot().into_iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["0=ocfl_1.1"]);
    assert_eq!(rig.bytes("0=ocfl_1.1"), b"ocfl_1.1\n");
    let journaled = rig.journaled("library.root.create");
    assert_eq!(journaled.len(), 1);
    assert_eq!(journaled[0].outcome, Outcome::Succeeded);
    assert_eq!(journaled[0].target_ids, [library().as_str()]);
}

#[test]
fn create_refuses_a_folder_that_is_not_empty() {
    let rig = Rig::new();
    ready(rig.root.put(&key("holiday.jpg"), b"jpeg")).expect("put");
    let writes = rig.root.writes();
    let err = ready(rig.service.create_root("owner", &library(), &rig.root)).expect_err("refused");
    assert_eq!(err.error_code(), "root_not_empty");
    assert_eq!(rig.root.writes(), writes, "nothing written");
    let journaled = rig.journaled("library.root.create");
    assert_eq!(journaled.len(), 1);
    assert_eq!(journaled[0].outcome, Outcome::Failed);
    assert_eq!(journaled[0].error_code.as_deref(), Some("root_not_empty"));
}

#[test]
fn create_twice_is_refused() {
    let rig = Rig::with_root();
    let err = ready(rig.service.create_root("owner", &library(), &rig.root)).expect_err("again");
    assert_eq!(err.error_code(), "root_not_empty");
}

#[test]
fn open_reports_the_objects_and_writes_nothing() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.w010_object("obj-b", "urn:uuid:b");
    let writes = rig.root.writes();
    let summary = ready(rig.service.open_root("owner", &library(), &rig.root)).expect("open");
    assert_eq!(summary.objects, 2);
    assert_eq!(rig.root.writes(), writes, "open never writes");
    assert!(!rig.stored("obj-b/v2/inventory.json"), "open never repairs");
}

#[test]
fn open_refuses_a_folder_that_is_not_a_storage_root() {
    let rig = Rig::new();
    ready(rig.root.put(&key("notes.txt"), b"hello")).expect("put");
    let err = ready(rig.service.open_root("owner", &library(), &rig.root)).expect_err("refused");
    assert_eq!(err.error_code(), "not_a_storage_root");
    assert_no_paths(err.message());
    let journaled = rig.journaled("library.root.open");
    assert_eq!(journaled.len(), 1);
    assert_eq!(
        journaled[0].error_code.as_deref(),
        Some("not_a_storage_root")
    );
}

#[test]
fn open_refuses_git_metadata_in_the_root() {
    let rig = Rig::with_root();
    ready(rig.root.put(&key(".git/HEAD"), b"ref: main")).expect("put");
    let err = ready(rig.service.open_root("owner", &library(), &rig.root)).expect_err("refused");
    assert_eq!(err.error_code(), "git_in_root");
}

#[test]
fn copy_produces_a_checked_snapshot_and_never_writes_the_source() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.w010_object("obj-b", "urn:uuid:b");
    ready(rig.root.put(&key("obj-a/v2/stray.tmp"), b"junk")).expect("stray");
    let before = rig.root.snapshot();
    let writes = rig.root.writes();
    let dest = CountingBackend::new();
    let report = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect("copy");
    assert_eq!(rig.root.writes(), writes, "copy never writes the source");
    assert_eq!(rig.root.snapshot(), before);
    assert_eq!(report.objects, 2);
    assert!(report.invalid_objects.is_empty(), "{report:?}");
    let copied: Vec<String> = dest.snapshot().into_iter().map(|(k, _)| k).collect();
    assert!(copied.contains(&"0=ocfl_1.1".to_string()));
    assert!(copied.contains(&"obj-a/v2/inventory.json".to_string()));
    assert!(
        !copied.contains(&"obj-a/v2/stray.tmp".to_string()),
        "unlisted files stay"
    );
    assert!(
        !copied.contains(&"obj-b/v2/inventory.json".to_string()),
        "copy never repairs"
    );
    assert_eq!(report.files, copied.len() as u64);
    let summary = ready(rig.service.open_root("owner", &library(), &dest)).expect("copy opens");
    assert_eq!(summary.objects, 2);
    let journaled = rig.journaled("library.snapshot.copy");
    assert_eq!(journaled.len(), 1);
    assert_eq!(journaled[0].outcome, Outcome::Succeeded);
}

#[test]
fn copy_streams_a_file_larger_than_one_put() {
    let rig = Rig::with_root();
    let big = vec![7u8; (MAX_PUT_BYTES + 1024) as usize];
    let draft = rig.draft_whole("urn:uuid:big", "big.bin", &big);
    rig.commit("obj-big", None, &draft).expect("commit");
    let dest = CountingBackend::new();
    let report = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect("copy");
    assert!(report.invalid_objects.is_empty(), "{report:?}");
    assert!(report.bytes > MAX_PUT_BYTES);
    let copied = ready(dest.head(&key("obj-big/v1/content/big.bin"))).expect("copied");
    assert_eq!(copied.size, big.len() as u64);
}

#[test]
fn copy_refuses_a_destination_that_is_not_empty() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    let dest = MemoryBackend::new();
    ready(dest.put(&key("0=ocfl_1.1"), b"ocfl_1.1\n")).expect("put");
    let err = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect_err("refused");
    assert_eq!(err.error_code(), "destination_not_empty");
    let page = ready(dest.list("", None, 10)).expect("list");
    assert_eq!(page.keys.len(), 1, "the destination is untouched");
    let journaled = rig.journaled("library.snapshot.copy");
    assert_eq!(journaled[0].outcome, Outcome::Failed);
}

#[test]
fn copy_refuses_a_snapshot_with_an_unpublished_head() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.remove("obj-a/inventory.json.sha256");
    let sidecar = rig.bytes("obj-a/v1/inventory.json.sha256");
    ready(rig.root.put(&key("obj-a/inventory.json.sha256"), &sidecar)).expect("stale sidecar");
    let dest = MemoryBackend::new();
    let err = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect_err("refused");
    assert_eq!(err.error_code(), "snapshot_incomplete");
    assert!(err.retryable());
}

#[test]
fn copy_reports_an_external_edit_found_in_the_copy() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    ready(rig.root.put(&key("obj-a/v1/content/a.txt"), b"edited")).expect("tamper");
    let dest = MemoryBackend::new();
    let report = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect("copy");
    assert_eq!(report.invalid_objects.len(), 1);
    assert_eq!(report.invalid_objects[0].object_id, "urn:uuid:a");
    assert!(!report.invalid_objects[0].error_codes.is_empty());
}

#[test]
fn rebuild_reconstructs_from_files_and_repairs_nothing() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.w010_object("obj-b", "urn:uuid:b");
    ready(
        rig.root
            .put(&key("obj-c/0=ocfl_object_1.1"), b"ocfl_object_1.1\n"),
    )
    .expect("broken");
    let writes = rig.root.writes();
    let report = ready(rig.service.rebuild("owner", &library(), &rig.root)).expect("rebuild");
    assert_eq!(rig.root.writes(), writes, "rebuild never writes");
    assert_eq!(report.objects.len(), 2);
    let a = &report.objects[0];
    assert_eq!(
        (a.object_id.as_str(), a.head.as_str(), a.files),
        ("urn:uuid:a", "v2", 2)
    );
    assert_eq!(a.digest_algorithm, "sha256");
    assert!(a.error_codes.is_empty() && !a.missing_version_inventory);
    let b = &report.objects[1];
    assert!(
        b.missing_version_inventory,
        "W010 is reported, not repaired"
    );
    assert_eq!(report.unreadable.len(), 1);
    assert_eq!(report.unreadable[0].error_code, "E063");
    assert_no_paths(&format!("{report:?}").replace("urn:uuid:", ""));
    let journaled = rig.journaled("library.rebuild");
    assert_eq!(journaled.len(), 1);
    assert_eq!(journaled[0].outcome, Outcome::Succeeded);
}

#[test]
fn rebuild_reports_an_external_edit_instead_of_adopting_it() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    ready(rig.root.put(&key("obj-a/v2/content/b.txt"), b"tampered")).expect("tamper");
    let report = ready(rig.service.rebuild("owner", &library(), &rig.root)).expect("rebuild");
    assert!(!report.objects[0].error_codes.is_empty(), "{report:?}");
}

#[test]
fn errors_carry_stable_codes_and_api_errors() {
    let err = LocalLibraryError::refused("root_not_empty", "msg");
    assert_eq!(err.error_code(), "root_not_empty");
    assert!(!err.retryable());
    let api: filesquid_api::ApiError = LocalLibraryError::Forbidden.into();
    assert_eq!(api.code(), filesquid_api::ErrorCode::Forbidden);
    assert!(LocalLibraryError::Unavailable.retryable());
}
