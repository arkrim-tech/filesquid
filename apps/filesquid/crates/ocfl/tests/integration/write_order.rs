//! Local write order and unpublished `vN` (IO-0.6; OCFL-S-018, OCFL-S-004,
//! OCFL-S-016; PRD F-042, NFR-001; ADR 007, ADR 012, ADR 016 "Write
//! (forward delta)" and "Published head (readers)").
//!
//! Every scenario runs on the in-memory backend and on a real directory
//! through `FsBackend`. Crashes are injected by a backend that never
//! completes the write of a chosen key, so the writer is dropped mid-write
//! with its reservation still held. Stale holders are made by a hook that
//! lets the lease lapse and hands it to another writer during a write.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{CommitError, ReservationError, VersionDraft, VersionReservations};
use filesquid_ocfl::OcflError;

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/write_rig.rs"]
mod rig;

use rig::{key, ready, sha256, v, Rig, OBJECT_ID, ROOT};

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
        .join(format!("ocfl-write-{}-{n}", std::process::id()));
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

/// The v2 draft most scenarios use: keep a.txt, rename b.txt to c.txt
/// (same bytes), add d.txt and its duplicate e.txt.
fn v2_draft<B: StorageBackend>(rig: &Rig<B>) -> VersionDraft {
    let mut draft = rig.draft(
        "v2",
        &[
            ("c.txt", b"bravo"),
            ("d.txt", b"delta"),
            ("e.txt", b"delta"),
        ],
    );
    draft.keep("a.txt", sha256(b"alpha"));
    draft
}

/// A draft that adds `d.txt` and `e.txt` with different bytes.
fn two_new_files<B: StorageBackend>(rig: &Rig<B>, tag: &str) -> VersionDraft {
    let mut draft = rig.draft(tag, &[("d.txt", b"delta"), ("e.txt", b"echo")]);
    draft.keep("a.txt", sha256(b"alpha"));
    draft.keep("b.txt", sha256(b"bravo"));
    draft
}

/// The committed head, read through the engine.
fn head<B: StorageBackend>(rig: &Rig<B>) -> String {
    rig.open().expect("open").head().to_string()
}

// ---- C8, C18: a new object and its next version ------------------------

fn a_new_object_commits_as_v1(rig: &Rig<impl StorageBackend>) {
    let receipt = rig.commit_v1();
    assert_eq!(receipt.object_id, OBJECT_ID);
    assert_eq!(receipt.version, v("v1"));
    assert_eq!(receipt.written, ["v1/content/a.txt", "v1/content/b.txt"]);
    let object = rig.open().expect("open");
    assert_eq!(object.head(), &v("v1"));
    assert_eq!(object.inventory().digest_algorithm.name(), "sha256");
    assert!(receipt.created.ends_with('Z'), "{}", receipt.created);
    rig.assert_valid();
}

#[test]
fn a_new_object_commits_as_v1_and_validates() {
    on_both!(a_new_object_commits_as_v1);
}

fn only_new_bytes_are_written(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let receipt = rig
        .commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect("v2");
    assert_eq!(receipt.version, v("v2"));
    assert_eq!(receipt.written, ["v2/content/d.txt"]);
    let listed: Vec<String> = rig.snapshot("obj/v2/content/").into_keys().collect();
    assert_eq!(listed, ["obj/v2/content/d.txt"]);
    let state = rig.open().expect("open").state(&v("v2")).expect("v2 state");
    let content = |p: &str| state.get(p).expect(p).content_path.clone();
    assert_eq!(content("a.txt"), "v1/content/a.txt");
    assert_eq!(content("c.txt"), "v1/content/b.txt");
    assert_eq!(content("e.txt"), "v2/content/d.txt");
    assert!(state.get("b.txt").is_none(), "b.txt was renamed");
    rig.assert_valid();
}

#[test]
fn the_next_version_writes_only_new_bytes() {
    on_both!(only_new_bytes_are_written);
}

fn earlier_versions_stay_unchanged(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let before = rig.snapshot("obj/v1/");
    let v1_block = rig.open().expect("open").inventory().versions[&v("v1")].clone();
    rig.commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect("v2");
    assert_eq!(rig.snapshot("obj/v1/"), before);
    let inventory = rig.open().expect("open").inventory().clone();
    assert_eq!(inventory.versions[&v("v1")], v1_block);
    assert_eq!(inventory.head, v("v2"));
    assert_eq!(inventory.versions.len(), 2);
}

#[test]
fn earlier_versions_and_inventories_are_never_changed() {
    on_both!(earlier_versions_stay_unchanged);
}

// ---- C1, C9: reservation first, root inventory and sidecar last ---------

fn writes_follow_the_order(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    rig.clear_log();
    rig.commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect("v2");
    let lease = rig.leases.lease_key(OBJECT_ID).expect("lease key");
    let writes = rig.writes();
    assert_eq!(writes.first(), Some(&format!("leases put_if {lease}")));
    assert_eq!(writes.last(), Some(&format!("leases put_if {lease}")));
    let storage: Vec<&str> = writes
        .iter()
        .filter_map(|w| w.strip_prefix("storage "))
        .collect();
    assert_eq!(
        storage,
        [
            "complete obj/v2/content/d.txt",
            "complete obj/v2/content/e.txt",
            "put_if obj/v2/inventory.json",
            "put_if obj/v2/inventory.json.sha256",
            "put_if obj/inventory.json",
            "put_if obj/inventory.json.sha256",
        ]
    );
}

#[test]
fn writes_follow_the_publication_order() {
    on_both!(writes_follow_the_order);
}

fn v1_reserves_before_its_declaration(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let writes = rig.writes();
    let lease = rig.leases.lease_key(OBJECT_ID).expect("lease key");
    assert_eq!(writes.first(), Some(&format!("leases put_if {lease}")));
    assert_eq!(writes[1], "storage put_if obj/0=ocfl_object_1.1");
    let tail: Vec<&String> = writes.iter().rev().skip(1).take(2).collect();
    assert_eq!(tail[0], "storage put_if obj/inventory.json.sha256");
    assert_eq!(tail[1], "storage put_if obj/inventory.json");
}

#[test]
fn a_new_object_is_reserved_before_any_object_root_byte() {
    on_both!(v1_reserves_before_its_declaration);
}

// ---- C2, C5: exclusive reservation, moved head --------------------------

fn held_reservation_refuses(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let other = ready(rig.leases.reserve(OBJECT_ID, &v("v2"), "writer-b")).expect("held");
    rig.clear_log();
    let err = rig
        .commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect_err("held");
    assert!(
        matches!(err, CommitError::Reservation(ReservationError::Held { .. })),
        "{err:?}"
    );
    assert!(rig.writes().iter().all(|w| !w.starts_with("storage")));
    assert_eq!(ready(rig.leases.confirm(&other)), Ok(()));
}

#[test]
fn a_reservation_held_by_another_writer_refuses_the_commit() {
    on_both!(held_reservation_refuses);
}

fn moved_head_refuses(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    rig.commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect("v2");
    rig.clear_log();
    let err = rig
        .commit("writer-b", Some("v1"), &two_new_files(rig, "late"))
        .expect_err("moved");
    match err {
        CommitError::HeadMoved { expected, found } => {
            assert_eq!(expected.as_deref(), Some("v1"));
            assert_eq!(found.as_deref(), Some("v2"));
        }
        other => panic!("{other:?}"),
    }
    assert!(rig.writes().iter().all(|w| !w.starts_with("storage")));
    assert_eq!(head(rig), "v2");
    let other = ready(rig.leases.reserve(OBJECT_ID, &v("v3"), "writer-c"));
    assert!(other.is_ok(), "the refused writer released its reservation");
}

#[test]
fn a_moved_head_is_refused_without_publishing() {
    on_both!(moved_head_refuses);
}

fn existing_object_needs_expected_head(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let err = rig
        .commit("writer-b", None, &rig.draft("again", &[("x.txt", b"x")]))
        .expect_err("exists");
    assert!(matches!(err, CommitError::HeadMoved { .. }), "{err:?}");
    assert_eq!(head(rig), "v1");
}

#[test]
fn creating_an_object_that_exists_is_refused() {
    on_both!(existing_object_needs_expected_head);
}

// ---- C11, C12, C13: crash injection --------------------------------------

/// Crash at `crash_key` while committing v2, check that readers see v1 and
/// that every key in `left` survived the crash, then that a second writer
/// recovers once the lease lapsed.
fn crash_then_recover<B: StorageBackend>(
    rig: &Rig<B>,
    crash_key: &str,
    unpublished: bool,
    left: &[&str],
) {
    rig.commit_v1();
    rig.storage.crash_at(crash_key);
    let crashed = rig.try_commit("writer-a", Some("v1"), &two_new_files(rig, "v2"));
    assert!(crashed.is_err(), "the writer crashed at {crash_key}");
    assert!(rig
        .writes()
        .iter()
        .any(|w| w.ends_with(&format!("crash {crash_key}"))));
    let object = rig.open().expect("open after the crash");
    assert_eq!(object.head(), &v("v1"), "v2 is unpublished");
    let pending = object.unpublished_head().map(ToString::to_string);
    assert_eq!(pending.as_deref(), unpublished.then_some("v2"));
    let pinned = object.state(&v("v2")).expect_err("not committed");
    assert!(
        matches!(pinned, ReadError::VersionNotFound { .. }),
        "{pinned:?}"
    );
    for raw in left {
        assert!(rig.stored(raw), "{raw} was left by the crashed writer");
    }
    recover_v2(rig);
}

/// The crashed writer's lease blocks a retry until it lapses; the retry then
/// publishes v2 exactly once and the object validates.
fn recover_v2<B: StorageBackend>(rig: &Rig<B>) {
    let draft = two_new_files(rig, "retry");
    let held = rig
        .commit("writer-b", Some("v1"), &draft)
        .expect_err("held");
    assert!(
        matches!(
            held,
            CommitError::Reservation(ReservationError::Held { .. })
        ),
        "{held:?}"
    );
    rig.expire_leases();
    let receipt = rig
        .commit("writer-b", Some("v1"), &draft)
        .expect("recovered");
    assert_eq!(receipt.version, v("v2"));
    assert!(receipt.took_over, "the retry took over the crashed lease");
    let object = rig.open().expect("open");
    let versions: Vec<String> = object
        .history()
        .iter()
        .map(|h| h.version.to_string())
        .collect();
    assert_eq!(versions, ["v1", "v2"]);
    assert_eq!(object.unpublished_head(), None);
    rig.assert_valid();
}

fn crash_during_content(rig: &Rig<impl StorageBackend>) {
    crash_then_recover(
        rig,
        "obj/v2/content/e.txt",
        false,
        &["obj/v2/content/d.txt"],
    );
}

#[test]
fn a_crash_while_writing_content_leaves_vn_unpublished() {
    on_both!(crash_during_content);
}

fn crash_before_version_inventory(rig: &Rig<impl StorageBackend>) {
    let left = ["obj/v2/content/d.txt", "obj/v2/content/e.txt"];
    crash_then_recover(rig, "obj/v2/inventory.json", false, &left);
}

#[test]
fn a_crash_between_content_and_inventories_leaves_vn_unpublished() {
    on_both!(crash_before_version_inventory);
}

fn crash_before_root_inventory(rig: &Rig<impl StorageBackend>) {
    let left = ["obj/v2/inventory.json.sha256"];
    crash_then_recover(rig, "obj/inventory.json", false, &left);
}

#[test]
fn a_crash_before_the_root_inventory_leaves_vn_unpublished() {
    on_both!(crash_before_root_inventory);
}

fn crash_before_root_sidecar(rig: &Rig<impl StorageBackend>) {
    crash_then_recover(
        rig,
        "obj/inventory.json.sha256",
        true,
        &["obj/v2/inventory.json"],
    );
}

#[test]
fn a_crash_before_the_root_sidecar_reads_as_unpublished() {
    on_both!(crash_before_root_sidecar);
}

fn crash_before_v1_sidecar(rig: &Rig<impl StorageBackend>) {
    rig.storage.crash_at("obj/inventory.json.sha256");
    let draft = rig.draft("v1", &[("a.txt", b"alpha")]);
    assert!(rig.try_commit("writer-a", None, &draft).is_err(), "crashed");
    let err = rig.open().expect_err("no committed version");
    assert_eq!(err.code(), Some("E058"), "{err:?}");
    rig.expire_leases();
    let receipt = rig.commit("writer-b", None, &draft).expect("recovered");
    assert_eq!(receipt.version, v("v1"));
    rig.assert_valid();
}

#[test]
fn a_crash_before_the_first_root_sidecar_publishes_nothing() {
    on_both!(crash_before_v1_sidecar);
}

fn uncommitted_first_keeps_its_id(rig: &Rig<impl StorageBackend>) {
    rig.storage.crash_at("obj/inventory.json.sha256");
    let draft = rig.draft("v1", &[("a.txt", b"alpha")]);
    assert!(rig.try_commit("writer-a", None, &draft).is_err(), "crashed");
    rig.expire_leases();
    let mut other = VersionDraft::new("urn:example:other", rig.now());
    other.stage("a.txt", key("stage/v1/a.txt"));
    let err = rig
        .commit("writer-b", None, &other)
        .expect_err("another id");
    assert!(matches!(err, CommitError::InvalidDraft { .. }), "{err:?}");
    rig.commit("writer-b", None, &draft)
        .expect("same id recovers");
    rig.assert_valid();
}

#[test]
fn an_uncommitted_first_version_cannot_be_taken_over_by_another_object() {
    on_both!(uncommitted_first_keeps_its_id);
}

fn head_without_version_inventory_refused(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    builder::delete(&rig.storage, ROOT, "v1/inventory.json");
    builder::delete(&rig.storage, ROOT, "v1/inventory.json.sha256");
    rig.clear_log();
    let err = rig
        .commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect_err("no v1 inventory");
    assert!(
        matches!(err, CommitError::MissingVersionInventory { .. }),
        "{err:?}"
    );
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_head_without_its_version_inventory_is_refused() {
    on_both!(head_without_version_inventory_refused);
}

fn head_version_inventory_mismatch_refused(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let root = rig.bytes("obj/inventory.json");
    let mut differs = root.clone();
    differs.extend_from_slice(b"\n");
    let wrong_sidecar = format!("{}  inventory.json\n", sha256(b"other"));
    let damages: [(&str, &[u8]); 2] = [
        ("obj/v1/inventory.json", &differs),
        ("obj/v1/inventory.json.sha256", wrong_sidecar.as_bytes()),
    ];
    for (raw, bytes) in damages {
        let original = rig.bytes(raw);
        ready(rig.storage.put(&key(raw), bytes)).expect("damage");
        rig.clear_log();
        let err = rig
            .commit("writer-a", Some("v1"), &v2_draft(rig))
            .expect_err("mismatching head version inventory");
        assert!(
            matches!(&err, CommitError::VersionInventoryMismatch { key } if key == raw),
            "{err:?}"
        );
        assert_eq!(rig.writes(), Vec::<String>::new());
        ready(rig.storage.put(&key(raw), &original)).expect("restore");
    }
}

#[test]
fn a_head_version_inventory_that_does_not_match_is_refused() {
    on_both!(head_version_inventory_mismatch_refused);
}

fn crash_leaves_v2_flagged(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    rig.storage.crash_at("obj/inventory.json");
    let crashed = rig.try_commit("writer-a", Some("v1"), &two_new_files(rig, "v2"));
    assert!(crashed.is_err());
    let report = rig.validate();
    let codes: Vec<&str> = report.errors().iter().map(|f| f.code).collect();
    assert!(
        codes.contains(&"E046"),
        "unpublished v2 is reported: {codes:?}"
    );
}

#[test]
fn an_unpublished_vn_is_reported_by_the_validator() {
    on_both!(crash_leaves_v2_flagged);
}

fn leftovers_are_reconciled(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    for (raw, bytes) in [
        ("obj/v2/content/d.txt", b"not delta" as &[u8]),
        ("obj/v2/content/stray.txt", b"stray"),
        ("obj/v2/inventory.json", b"{}"),
    ] {
        ready(rig.storage.put(&key(raw), bytes)).expect("plant");
    }
    rig.commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect("v2");
    assert!(!rig.stored("obj/v2/content/stray.txt"));
    assert_eq!(rig.bytes("obj/v2/content/d.txt"), b"delta");
    rig.assert_valid();
}

#[test]
fn leftovers_of_an_unpublished_vn_are_reconciled_under_the_reservation() {
    on_both!(leftovers_are_reconciled);
}

// ---- C3, C4: stale holders ------------------------------------------------

/// Let the lease lapse and hand it to `writer-b` right after `trigger` is
/// written, while `writer-a` is still committing.
fn reassign_after<B: StorageBackend>(rig: &Rig<B>, trigger: &str) {
    let leases = rig.leases.clone();
    let clock = rig.clock.clone();
    rig.storage.after_write(
        trigger,
        Box::new(move || {
            clock.advance(rig::LEASES.ttl_ms);
            ready(leases.reserve(OBJECT_ID, &v("v2"), "writer-b")).expect("reassigned");
        }),
    );
}

fn stale_holder_writes_no_content(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let root_before = rig.bytes("obj/inventory.json");
    reassign_after(rig, "obj/v2/content/d.txt");
    let err = rig
        .commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect_err("stale");
    assert!(
        matches!(err, CommitError::Reservation(ReservationError::Lost { .. })),
        "{err:?}"
    );
    assert!(
        !rig.stored("obj/v2/content/e.txt"),
        "no write after reassignment"
    );
    assert!(!rig.stored("obj/v2/inventory.json"));
    assert_eq!(rig.bytes("obj/inventory.json"), root_before);
    rig.expire_leases();
    let mut draft = rig.draft("v2b", &[("d.txt", b"delta two")]);
    draft.keep("a.txt", sha256(b"alpha"));
    rig.commit("writer-b", Some("v1"), &draft)
        .expect("new holder");
    assert_eq!(rig.bytes("obj/v2/content/d.txt"), b"delta two");
    rig.assert_valid();
}

#[test]
fn a_stale_lease_holder_cannot_write_content_after_reassignment() {
    on_both!(stale_holder_writes_no_content);
}

fn stale_holder_cannot_publish(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let root_before = rig.bytes("obj/inventory.json");
    reassign_after(rig, "obj/v2/inventory.json.sha256");
    rig.clear_log();
    let err = rig
        .commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect_err("stale");
    assert!(
        matches!(err, CommitError::Reservation(ReservationError::Lost { .. })),
        "{err:?}"
    );
    assert_eq!(rig.bytes("obj/inventory.json"), root_before);
    assert_eq!(head(rig), "v1");
    let root_writes: Vec<String> = rig
        .writes()
        .into_iter()
        .filter(|w| w.contains(" obj/inventory.json"))
        .collect();
    assert_eq!(
        root_writes,
        Vec::<String>::new(),
        "the head was not touched"
    );
}

#[test]
fn a_stale_lease_holder_cannot_publish_the_head_after_reassignment() {
    on_both!(stale_holder_cannot_publish);
}

// ---- C10: assembly outside the object root -------------------------------

fn changed_staging_is_refused(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    let lease = rig.leases.lease_key(OBJECT_ID).expect("lease key");
    let staging = rig.staging.clone();
    rig.leases.backend().after_write(
        lease.as_str(),
        Box::new(move || {
            let staged = key("stage/v2/d.txt");
            ready(staging.put(&staged, b"tampered")).expect("tamper");
        }),
    );
    let err = rig
        .commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect_err("changed");
    assert!(matches!(err, CommitError::StagingChanged { .. }), "{err:?}");
    assert!(!rig.stored("obj/v2/content/d.txt"));
    assert_eq!(head(rig), "v1");
    let next = ready(rig.leases.reserve(OBJECT_ID, &v("v2"), "writer-b"));
    assert!(next.is_ok(), "the failed writer released its reservation");
}

#[test]
fn staged_bytes_that_change_after_assembly_are_refused() {
    on_both!(changed_staging_is_refused);
}

// ---- C15, C17, C8: refusals before any write ------------------------------

fn bad_drafts_reserve_nothing(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    rig.clear_log();
    let mut offset = VersionDraft::new(OBJECT_ID, "2026-10-18T12:00:00+02:00");
    offset.keep("a.txt", sha256(b"alpha"));
    let mut unknown = VersionDraft::new(OBJECT_ID, rig.now());
    unknown.keep("a.txt", sha256(b"never stored"));
    let mut wrong_id = VersionDraft::new("urn:example:other", rig.now());
    wrong_id.keep("a.txt", sha256(b"alpha"));
    let mut escaping = VersionDraft::new(OBJECT_ID, rig.now());
    escaping.keep("../a.txt", sha256(b"alpha"));
    let staged = key("stage/bad/ab.txt");
    ready(rig.staging.put(&staged, b"backslash")).expect("stage");
    let mut backslash = VersionDraft::new(OBJECT_ID, rig.now());
    backslash.stage("a\\b.txt", staged);
    for draft in [offset, unknown, wrong_id, escaping, backslash] {
        let err = rig.commit("writer-a", Some("v1"), &draft).expect_err("bad");
        assert!(matches!(err, CommitError::InvalidDraft { .. }), "{err:?}");
    }
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn invalid_drafts_are_refused_before_any_reservation() {
    on_both!(bad_drafts_reserve_nothing);
}

fn sha512_objects_are_refused(rig: &Rig<impl StorageBackend>) {
    let mut spec = builder::ObjectSpec::new(OBJECT_ID, vec![vec![("a.txt", b"alpha" as &[u8])]]);
    spec.algorithm = filesquid_ocfl::digest::DigestAlgorithm::Sha512;
    builder::build(&rig.storage, ROOT, &spec);
    let draft = rig.draft("v2", &[("b.txt", b"bravo")]);
    let err = rig
        .commit("writer-a", Some("v1"), &draft)
        .expect_err("sha512");
    assert!(
        matches!(err, CommitError::UnsupportedAlgorithm { .. }),
        "{err:?}"
    );
}

#[test]
fn an_object_that_does_not_use_sha256_is_refused() {
    on_both!(sha512_objects_are_refused);
}

fn git_in_object_root_refuses(rig: &Rig<impl StorageBackend>) {
    rig.commit_v1();
    ready(rig.storage.put(&key("obj/.git/config"), b"[core]")).expect("plant");
    let err = rig
        .commit("writer-a", Some("v1"), &v2_draft(rig))
        .expect_err("git");
    assert!(
        matches!(
            err,
            CommitError::Guard(OcflError::GitDirectoryForbidden { .. })
        ),
        "{err:?}"
    );
    assert!(!rig.stored("obj/v2/content/d.txt"));
}

#[test]
fn a_git_directory_in_the_object_root_refuses_the_commit() {
    on_both!(git_in_object_root_refuses);
}

// ---- Takeover indicator (OCFL-S-018; consumed by API-F2.7) ---------------

fn takeover_is_reported_only_after_a_lapsed_lease(rig: &Rig<impl StorageBackend>) {
    let first = rig.commit_v1();
    assert!(!first.took_over, "a first reservation takes nothing over");
    let second = rig
        .commit("writer-a", Some("v1"), &two_new_files(rig, "v2"))
        .expect("v2");
    assert!(!second.took_over, "a released lease is not taken over");
    let held = ready(rig.leases.reserve(OBJECT_ID, &v("v3"), "stalled")).expect("hold v3");
    assert!(!held.took_over());
    rig.expire_leases();
    let third = rig
        .commit("writer-b", Some("v2"), &two_new_files(rig, "v3"))
        .expect("v3");
    assert!(third.took_over, "the stalled writer's lease was taken over");
}

#[test]
fn a_commit_reports_whether_it_took_over_a_lapsed_reservation() {
    on_both!(takeover_is_reported_only_after_a_lapsed_lease);
}
