//! Version-inventory repair for heads without one (IO-1.4; OCFL-S-020,
//! OCFL-S-018, OCFL-S-019, OCFL-S-002; PRD F-042, NFR-001; ADR 016).
//!
//! Every scenario runs on the in-memory backend and on a real directory
//! through `FsBackend`. Objects without a head version inventory (W010) are
//! the pinned `1.1/warn-objects/W010_no_version_inventory` fixture (sha512)
//! and sha256 objects committed by the engine writer whose head version
//! inventory and sidecar were then removed. Crashes are injected by the
//! fault backend, which never completes the write of a chosen key; stale
//! holders are made by a hook that hands the lease to another writer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{
    CommitError, RepairError, ReservationError, VersionDraft, VersionReservations,
};

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/fixtures.rs"]
mod fixtures;
#[path = "../support/write_rig.rs"]
mod rig;

use rig::{key, ready, sha256, v, Rig, OBJECT_ID, ROOT};

/// The identifier of the pinned W010 fixture object.
const FIXTURE_ID: &str = "ark:123/abc";

/// Scratch directory for one filesystem backend, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh `FsBackend` on its own scratch directory.
fn fs_backend() -> (FsBackend, Scratch) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-repair-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// Run `$scenario` on a memory backend and on a filesystem backend.
macro_rules! on_both {
    ($scenario:ident) => {{
        $scenario(&Rig::new(MemoryBackend::new()));
        let (backend, _scratch) = fs_backend();
        $scenario(&Rig::new(backend));
    }};
}

/// Copy every file of the fixture directory `dir` under `prefix` of the
/// rig's storage root, at most 64 levels deep.
fn copy_tree<B: StorageBackend>(rig: &Rig<B>, dir: &Path, prefix: &str, depth: usize) {
    assert!(depth < 64, "fixture tree too deep");
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("list fixture")
        .map(|e| e.expect("entry"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}/{name}");
        if entry.path().is_dir() {
            copy_tree(rig, &entry.path(), &rel, depth + 1);
        } else {
            let bytes = std::fs::read(entry.path()).expect("read fixture");
            ready(rig.storage.put(&key(&rel), &bytes)).expect("copy fixture");
        }
    }
}

/// A copy of the pinned W010 fixture at the rig's object root.
fn w010_fixture<B: StorageBackend>(rig: &Rig<B>) {
    let dir = fixtures::tree().join("warn-objects/W010_no_version_inventory");
    copy_tree(rig, &dir, ROOT, 0);
    rig.clear_log();
}

/// A sha256 object committed at v2 by the engine writer whose head (v2)
/// version inventory and sidecar were then removed.
fn sha256_without_head_inventory<B: StorageBackend>(rig: &Rig<B>) {
    rig.commit_v1();
    let mut draft = rig.draft("v2", &[("c.txt", b"charlie")]);
    draft.keep("a.txt", sha256(b"alpha"));
    rig.commit("writer-a", Some("v1"), &draft).expect("v2");
    builder::delete(&rig.storage, ROOT, "v2/inventory.json");
    builder::delete(&rig.storage, ROOT, "v2/inventory.json.sha256");
    rig.expire_leases();
    rig.clear_log();
}

/// A v3 draft on the sha256 object.
fn v3_draft<B: StorageBackend>(rig: &Rig<B>) -> VersionDraft {
    let mut draft = rig.draft("v3", &[("d.txt", b"delta")]);
    draft.keep("a.txt", sha256(b"alpha"));
    draft.keep("c.txt", sha256(b"charlie"));
    draft
}

/// The storage writes logged so far (lease writes excluded).
fn storage_writes<B: StorageBackend>(rig: &Rig<B>) -> Vec<String> {
    rig.writes()
        .into_iter()
        .filter(|w| w.starts_with("storage "))
        .collect()
}

/// Assert the repair was refused and wrote nothing to the storage root.
fn assert_refused_without_writing<B: StorageBackend>(
    rig: &Rig<B>,
    before: &BTreeMap<String, String>,
    result: Result<filesquid_ocfl::write::RepairReceipt, RepairError>,
) -> RepairError {
    let err = result.expect_err("refused");
    assert_eq!(storage_writes(rig), Vec::<String>::new(), "{err:?}");
    assert_eq!(&rig.snapshot(""), before, "nothing changed");
    err
}

/// The logical state of the committed head as (path, digest) pairs.
fn logical_state<B: StorageBackend>(rig: &Rig<B>) -> Vec<(String, String)> {
    let object = rig.open().expect("open");
    let state = object.state(object.head()).expect("state");
    state
        .files()
        .map(|f| (f.path.clone(), f.digest.clone()))
        .collect()
}

/// Every key of `after` but `added` has the same tag in `before`, and
/// `added` are exactly the new keys.
fn assert_only_added(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    added: &[&str],
) {
    let mut kept = after.clone();
    for raw in added {
        assert!(kept.remove(*raw).is_some(), "{raw} was added");
    }
    assert_eq!(&kept, before, "every other file is unchanged");
}

// ---- AC1: the W010 fixture and a sha256 object are repaired ----------------

fn fixture_is_repaired(rig: &Rig<impl StorageBackend>) {
    w010_fixture(rig);
    assert!(rig.validate().warning_codes().contains("W010"));
    let mut draft = VersionDraft::new(FIXTURE_ID, rig.now());
    let staged = key("stage/fixture/b.txt");
    ready(rig.staging.put(&staged, b"bravo")).expect("stage");
    draft.stage("b.txt", staged);
    let refused = rig
        .commit("writer-a", Some("v1"), &draft)
        .expect_err("W010");
    assert!(
        matches!(refused, CommitError::MissingVersionInventory { .. }),
        "{refused:?}"
    );
    let before = rig.snapshot("");
    rig.clear_log();
    let receipt = rig.repair("writer-a").expect("repaired");
    assert!(!receipt.took_over, "the refused commit released its lease");
    assert_eq!(receipt.object_id, FIXTURE_ID);
    assert_eq!(receipt.version, v("v1"));
    assert_eq!(receipt.digest_algorithm, DigestAlgorithm::Sha512);
    let root = rig.bytes("obj/inventory.json");
    assert_eq!(
        receipt.inventory_digest,
        DigestAlgorithm::Sha512.digest_hex(&root)
    );
    assert_eq!(
        receipt.written,
        ["v1/inventory.json", "v1/inventory.json.sha512"]
    );
    assert_eq!(rig.bytes("obj/v1/inventory.json"), root);
    assert_eq!(
        rig.bytes("obj/v1/inventory.json.sha512"),
        rig.bytes("obj/inventory.json.sha512")
    );
    assert_only_added(
        &before,
        &rig.snapshot(""),
        &["obj/v1/inventory.json", "obj/v1/inventory.json.sha512"],
    );
    let report = rig.validate();
    assert!(report.errors().is_empty(), "{:?}", report.errors());
    assert!(!report.warning_codes().contains("W010"));
    rig.expire_leases();
    rig.clear_log();
    let after = rig
        .commit("writer-a", Some("v1"), &draft)
        .expect_err("sha512");
    assert!(
        matches!(after, CommitError::UnsupportedAlgorithm { .. }),
        "{after:?}"
    );
    assert_eq!(storage_writes(rig), Vec::<String>::new());
}

#[test]
fn the_w010_fixture_is_repaired_and_stays_closed_to_commits_as_sha512() {
    on_both!(fixture_is_repaired);
}

fn sha256_object_is_repaired(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    let refused = rig
        .commit("writer-a", Some("v2"), &v3_draft(rig))
        .expect_err("W010");
    assert!(
        matches!(refused, CommitError::MissingVersionInventory { .. }),
        "{refused:?}"
    );
    let state = logical_state(rig);
    let before = rig.snapshot("");
    rig.clear_log();
    let receipt = rig.repair("writer-a").expect("repaired");
    assert_eq!(receipt.object_id, OBJECT_ID);
    assert_eq!(receipt.version, v("v2"));
    assert_eq!(
        receipt.written,
        ["v2/inventory.json", "v2/inventory.json.sha256"]
    );
    let writes = storage_writes(rig);
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert!(writes[0].ends_with("obj/v2/inventory.json"), "{writes:?}");
    assert!(
        writes[1].ends_with("obj/v2/inventory.json.sha256"),
        "the sidecar is written last: {writes:?}"
    );
    assert_eq!(
        rig.bytes("obj/v2/inventory.json"),
        rig.bytes("obj/inventory.json")
    );
    assert_eq!(
        rig.bytes("obj/v2/inventory.json.sha256"),
        rig.bytes("obj/inventory.json.sha256")
    );
    assert_only_added(
        &before,
        &rig.snapshot(""),
        &["obj/v2/inventory.json", "obj/v2/inventory.json.sha256"],
    );
    assert_eq!(logical_state(rig), state, "the logical state is unchanged");
    assert_eq!(rig.open().expect("open").head(), &v("v2"), "no new version");
    let report = rig.validate();
    assert!(report.errors().is_empty(), "{:?}", report.errors());
    assert!(!report.warning_codes().contains("W010"));
    let committed = rig
        .commit("writer-a", Some("v2"), &v3_draft(rig))
        .expect("v3 on the repaired object");
    assert_eq!(committed.version, v("v3"));
    rig.assert_valid();
}

#[test]
fn a_repaired_sha256_object_becomes_editable() {
    on_both!(sha256_object_is_repaired);
}

// ---- AC2: refusals without writing ------------------------------------------

fn complete_head_is_refused(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    rig.expire_leases();
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(
        matches!(err, RepairError::NothingToRepair { .. }),
        "{err:?}"
    );
}

#[test]
fn a_head_that_already_has_its_version_inventory_is_refused() {
    on_both!(complete_head_is_refused);
}

fn validator_error_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    ready(rig.storage.put(&key("obj/v2/content/c.txt"), b"tampered")).expect("damage");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    let RepairError::Invalid { errors } = &err else {
        panic!("{err:?}");
    };
    assert!(errors.iter().any(|f| f.code == "E092"), "{errors:?}");
}

#[test]
fn an_object_with_a_validator_error_is_refused() {
    on_both!(validator_error_is_refused);
}

fn root_sidecar_mismatch_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    let wrong = format!("{}  inventory.json\n", sha256(b"other"));
    ready(
        rig.storage
            .put(&key("obj/inventory.json.sha256"), wrong.as_bytes()),
    )
    .expect("plant");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(
        matches!(err, RepairError::RootSidecarMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn an_object_whose_root_sidecar_does_not_match_is_refused() {
    on_both!(root_sidecar_mismatch_is_refused);
}

fn held_reservation_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    ready(rig.leases.reserve(OBJECT_ID, &v("v3"), "writer-b")).expect("held");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(
        matches!(err, RepairError::Reservation(ReservationError::Held { .. })),
        "{err:?}"
    );
}

#[test]
fn a_reservation_held_by_another_writer_refuses_the_repair() {
    on_both!(held_reservation_is_refused);
}

fn lost_reservation_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    let leases = rig.leases.clone();
    let clock = rig.clock.clone();
    rig.storage.after_write(
        "obj/v2/inventory.json",
        Box::new(move || {
            clock.advance(rig::LEASES.ttl_ms);
            ready(leases.reserve(OBJECT_ID, &v("v3"), "writer-b")).expect("reassigned");
        }),
    );
    let state = logical_state(rig);
    let err = rig.repair("writer-a").expect_err("lost");
    assert!(
        matches!(err, RepairError::Reservation(ReservationError::Lost { .. })),
        "{err:?}"
    );
    assert!(
        !rig.stored("obj/v2/inventory.json.sha256"),
        "no write after the reservation was lost"
    );
    assert_eq!(logical_state(rig), state);
}

#[test]
fn a_lost_reservation_stops_the_repair_before_its_next_write() {
    on_both!(lost_reservation_is_refused);
}

fn unlisted_file_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    ready(rig.storage.put(&key("obj/v2/content/stray.txt"), b"stray")).expect("plant");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(matches!(err, RepairError::NotRepairable { .. }), "{err:?}");
}

#[test]
fn a_head_with_an_unlisted_file_is_refused() {
    on_both!(unlisted_file_is_refused);
}

// ---- AC3: crash between the version inventory and its sidecar ---------------

/// Crash the repair right at the version sidecar, so only the version
/// inventory is left, and let the crashed writer's lease lapse.
fn crash_before_sidecar<B: StorageBackend>(rig: &Rig<B>) {
    rig.storage.crash_at("obj/v2/inventory.json.sha256");
    assert!(rig.try_repair("writer-a").is_err(), "the writer crashed");
    assert!(rig.stored("obj/v2/inventory.json"));
    assert!(!rig.stored("obj/v2/inventory.json.sha256"));
    rig.expire_leases();
    rig.clear_log();
}

fn interrupted_repair_completes(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    let state = logical_state(rig);
    crash_before_sidecar(rig);
    assert_eq!(logical_state(rig), state, "the logical state is unchanged");
    let codes: Vec<&str> = rig.validate().errors().iter().map(|f| f.code).collect();
    assert_eq!(codes, ["E058"], "only the missing version sidecar");
    let refused = rig
        .commit("writer-a", Some("v2"), &v3_draft(rig))
        .expect_err("half-repaired head");
    assert!(
        matches!(refused, CommitError::MissingVersionInventory { .. }),
        "{refused:?}"
    );
    let before = rig.snapshot("");
    rig.clear_log();
    let receipt = rig.repair("writer-b").expect("resumed");
    assert_eq!(receipt.written, ["v2/inventory.json.sha256"]);
    assert_only_added(
        &before,
        &rig.snapshot(""),
        &["obj/v2/inventory.json.sha256"],
    );
    rig.assert_valid();
    assert_eq!(logical_state(rig), state);
    rig.commit("writer-b", Some("v2"), &v3_draft(rig))
        .expect("v3");
}

#[test]
fn an_interrupted_repair_is_completed_by_running_it_again() {
    on_both!(interrupted_repair_completes);
}

fn partial_with_differing_inventory_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    let mut other = rig.bytes("obj/inventory.json");
    other.extend_from_slice(b"\n");
    ready(rig.storage.put(&key("obj/v2/inventory.json"), &other)).expect("plant");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(matches!(err, RepairError::NotRepairable { .. }), "{err:?}");
}

#[test]
fn a_head_version_inventory_that_differs_from_the_root_is_refused() {
    on_both!(partial_with_differing_inventory_is_refused);
}

fn partial_with_unlisted_file_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    crash_before_sidecar(rig);
    ready(rig.storage.put(&key("obj/v2/notes.txt"), b"note")).expect("plant");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(matches!(err, RepairError::NotRepairable { .. }), "{err:?}");
}

#[test]
fn an_interrupted_repair_with_another_unlisted_file_is_refused() {
    on_both!(partial_with_unlisted_file_is_refused);
}

fn partial_with_damaged_content_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    crash_before_sidecar(rig);
    ready(rig.storage.put(&key("obj/v1/content/a.txt"), b"tampered")).expect("damage");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    let RepairError::Invalid { errors } = &err else {
        panic!("{err:?}");
    };
    assert!(errors.iter().any(|f| f.code == "E092"), "{errors:?}");
}

#[test]
fn an_interrupted_repair_on_damaged_content_is_refused() {
    on_both!(partial_with_damaged_content_is_refused);
}

fn partial_with_root_sidecar_mismatch_is_refused(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    crash_before_sidecar(rig);
    let wrong = format!("{}  inventory.json\n", sha256(b"other"));
    ready(
        rig.storage
            .put(&key("obj/inventory.json.sha256"), wrong.as_bytes()),
    )
    .expect("plant");
    rig.clear_log();
    let before = rig.snapshot("");
    let err = assert_refused_without_writing(rig, &before, rig.repair("writer-a"));
    assert!(
        matches!(err, RepairError::RootSidecarMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn an_interrupted_repair_whose_root_sidecar_does_not_match_is_refused() {
    on_both!(partial_with_root_sidecar_mismatch_is_refused);
}

// ---- OCFL-S-019, OCFL-S-020: inspection never repairs -----------------------

fn inspection_never_repairs(rig: &Rig<impl StorageBackend>) {
    w010_fixture(rig);
    let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
    let state = object.state(object.head()).expect("state");
    assert_eq!(state.files().count(), 1);
    assert!(rig.validate().warning_codes().contains("W010"));
    assert_eq!(rig.writes(), Vec::<String>::new());
    assert!(!rig.stored("obj/v1/inventory.json"));
}

#[test]
fn opening_reading_and_validating_never_repair() {
    on_both!(inspection_never_repairs);
}

fn repair_reports_a_takeover(rig: &Rig<impl StorageBackend>) {
    sha256_without_head_inventory(rig);
    ready(rig.leases.reserve(OBJECT_ID, &v("v2"), "stalled")).expect("a stalled writer holds v2");
    rig.expire_leases();
    let receipt = rig.repair("writer-a").expect("repaired");
    assert!(
        receipt.took_over,
        "the stalled writer's reservation was taken over"
    );
}

#[test]
fn a_repair_reports_whether_it_took_over_a_lapsed_reservation() {
    on_both!(repair_reports_a_takeover);
}
