//! Step definitions for `version_inventory_repair_OCFL-S-020.feature`
//! (IO-1.4). Every scenario runs the engine's version-inventory repair on
//! an in-memory storage root behind the fault-injecting backend
//! (`crate::fault`, `crate::rig`).

use std::collections::BTreeMap;
use std::fmt;

use cucumber::{given, then, when};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::write::{
    CommitError, RepairError, RepairReceipt, ReservationError, VersionDraft, VersionReservations,
};

use crate::rig::{key, ready, sha256, v, Rig, OBJECT_ID, ROOT};
use crate::OcflWorld;

/// The identifier of the pinned W010 fixture object.
const FIXTURE_ID: &str = "ark:123/abc";

/// What the repair scenarios carry between steps.
#[derive(Default)]
pub struct RepairState {
    rig: Option<Rig<MemoryBackend>>,
    result: Option<Result<RepairReceipt, RepairError>>,
    before: BTreeMap<String, String>,
    state: Vec<(String, String)>,
}

impl fmt::Debug for RepairState {
    /// Show the outcome; the rig holds closures that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepairState")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

/// The scenario's rig.
fn rig(world: &OcflWorld) -> &Rig<MemoryBackend> {
    world
        .repair
        .rig
        .as_ref()
        .expect("Given an object to repair")
}

/// The logical state of the committed head as (path, digest) pairs.
fn logical_state(rig: &Rig<MemoryBackend>) -> Vec<(String, String)> {
    let object = rig.open().expect("open");
    let state = object.state(object.head()).expect("state");
    state
        .files()
        .map(|f| (f.path.clone(), f.digest.clone()))
        .collect()
}

/// Copy every file of the fixture directory `dir` under `prefix`, at most
/// 64 levels deep.
fn copy_tree(rig: &Rig<MemoryBackend>, dir: &std::path::Path, prefix: &str, depth: usize) {
    assert!(depth < 64, "fixture tree too deep");
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("list fixture")
        .map(|e| e.expect("entry"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let rel = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if entry.path().is_dir() {
            copy_tree(rig, &entry.path(), &rel, depth + 1);
        } else {
            let bytes = std::fs::read(entry.path()).expect("read fixture");
            ready(rig.storage.put(&key(&rel), &bytes)).expect("copy fixture");
        }
    }
}

/// A draft adding `b.txt` to the fixture object.
fn fixture_draft(rig: &Rig<MemoryBackend>) -> VersionDraft {
    let mut draft = VersionDraft::new(FIXTURE_ID, rig.now());
    let staged = key("stage/fixture/b.txt");
    ready(rig.staging.put(&staged, b"bravo")).expect("stage");
    draft.stage("b.txt", staged);
    draft
}

/// A v3 draft on the sha256 object.
fn v3_draft(rig: &Rig<MemoryBackend>) -> VersionDraft {
    let mut draft = rig.draft("v3", &[("d.txt", b"delta")]);
    draft.keep("a.txt", sha256(b"alpha"));
    draft.keep("c.txt", sha256(b"charlie"));
    draft
}

/// Remember the object as it is now, then forget the write log.
fn remember(world: &mut OcflWorld) {
    let rig = rig(world);
    let before = rig.snapshot("");
    let state = logical_state(rig);
    rig.clear_log();
    world.repair.before = before;
    world.repair.state = state;
}

#[given("a copy of the pinned W010 fixture object")]
fn w010_fixture(world: &mut OcflWorld) {
    let rig = Rig::new(MemoryBackend::new());
    let dir = crate::fixtures::tree().join("warn-objects/W010_no_version_inventory");
    copy_tree(&rig, &dir, ROOT, 0);
    world.repair.rig = Some(rig);
    remember(world);
}

#[given("a commit on it is refused for the missing version inventory")]
fn fixture_commit_refused(world: &mut OcflWorld) {
    let rig = rig(world);
    let err = rig
        .commit("writer-a", Some("v1"), &fixture_draft(rig))
        .expect_err("W010");
    assert!(
        matches!(err, CommitError::MissingVersionInventory { .. }),
        "{err:?}"
    );
    remember(world);
}

#[given(expr = "a sha256 object committed at {string} whose head version inventory was removed")]
fn sha256_without_head_inventory(world: &mut OcflWorld, _head: String) {
    let rig = Rig::new(MemoryBackend::new());
    rig.commit_v1();
    let mut draft = rig.draft("v2", &[("c.txt", b"charlie")]);
    draft.keep("a.txt", sha256(b"alpha"));
    rig.commit("writer-a", Some("v1"), &draft).expect("v2");
    for rel in ["v2/inventory.json", "v2/inventory.json.sha256"] {
        ready(rig.storage.delete(&key(&format!("{ROOT}/{rel}")))).expect("remove");
    }
    rig.expire_leases();
    world.repair.rig = Some(rig);
    remember(world);
}

#[given(expr = "the object was then changed by {}")]
fn damaged(world: &mut OcflWorld, damage: String) {
    let rig = rig(world);
    let wrong_sidecar = format!("{}  inventory.json\n", sha256(b"other"));
    let mut other_inventory = rig.bytes("obj/inventory.json");
    other_inventory.extend_from_slice(b"\n");
    match damage.as_str() {
        "restoring the head version inventory" => {
            put(
                rig,
                "obj/v2/inventory.json",
                &rig.bytes("obj/inventory.json"),
            );
            put(
                rig,
                "obj/v2/inventory.json.sha256",
                &rig.bytes("obj/inventory.json.sha256"),
            );
        }
        "a content file whose digest no longer matches" => {
            put(rig, "obj/v1/content/a.txt", b"tampered")
        }
        "a root sidecar that does not match" => {
            put(rig, "obj/inventory.json.sha256", wrong_sidecar.as_bytes())
        }
        "an unlisted file in the head version directory" => {
            put(rig, "obj/v2/content/stray.txt", b"stray")
        }
        "a head version inventory that differs" => {
            put(rig, "obj/v2/inventory.json", &other_inventory)
        }
        "a reservation held by another writer" => {
            ready(rig.leases.reserve(OBJECT_ID, &v("v3"), "writer-b")).expect("held");
        }
        other => panic!("unknown damage {other:?}"),
    }
    rig.clear_log();
    world.repair.before = rig.snapshot("");
}

/// Store `bytes` at `raw`.
fn put(rig: &Rig<MemoryBackend>, raw: &str, bytes: &[u8]) {
    ready(rig.storage.put(&key(raw), bytes)).expect("put");
}

#[given(expr = "the reservation passes to another writer after {string} is written")]
fn passes_after(world: &mut OcflWorld, trigger: String) {
    let rig = rig(world);
    let leases = rig.leases.clone();
    let clock = rig.clock.clone();
    rig.storage.after_write(
        &trigger,
        Box::new(move || {
            clock.advance(crate::rig::LEASES.ttl_ms);
            ready(leases.reserve(OBJECT_ID, &v("v3"), "writer-b")).expect("reassigned");
        }),
    );
}

#[given(expr = "a repair crashed before writing {string}")]
fn crashed_repair(world: &mut OcflWorld, crash_key: String) {
    let rig = rig(world);
    rig.storage.crash_at(&crash_key);
    assert!(rig.try_repair("writer-a").is_err(), "the repair crashed");
    assert!(!rig.stored(&crash_key));
    rig.expire_leases();
    rig.clear_log();
    world.repair.before = rig.snapshot("");
}

#[when("the version-inventory repair runs")]
#[when("the version-inventory repair runs again")]
fn repair_runs(world: &mut OcflWorld) {
    let result = rig(world).repair("writer-b");
    world.repair.result = Some(result);
}

#[when("the object is opened, read and validated")]
fn inspect(world: &mut OcflWorld) {
    let rig = rig(world);
    let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
    object.state(object.head()).expect("state");
    ready(validate_object(&rig.storage, ROOT)).expect("validate");
}

/// The repair's receipt.
fn receipt(world: &OcflWorld) -> &RepairReceipt {
    match world.repair.result.as_ref().expect("a repair ran") {
        Ok(receipt) => receipt,
        Err(err) => panic!("refused: {err:?}"),
    }
}

#[then(expr = "the repair should have written {string} and {string}")]
fn wrote_two(world: &mut OcflWorld, first: String, second: String) {
    assert_eq!(receipt(world).written, [first, second]);
}

#[then(expr = "the repair should have written only {string}")]
fn wrote_one(world: &mut OcflWorld, only: String) {
    assert_eq!(receipt(world).written, [only]);
}

#[then("the repaired head files should be byte-identical to the root inventory and root sidecar")]
fn byte_identical(world: &mut OcflWorld) {
    let rig = rig(world);
    let receipt = receipt(world);
    let root = rig.bytes("obj/inventory.json");
    assert_eq!(rig.bytes(&format!("obj/{}", receipt.written[0])), root);
    let sidecar = format!("obj/inventory.json.{}", receipt.digest_algorithm.name());
    assert_eq!(
        rig.bytes(&format!("obj/{}", receipt.written[1])),
        rig.bytes(&sidecar)
    );
    assert_eq!(receipt.digest_algorithm, DigestAlgorithm::Sha512);
    assert_eq!(
        receipt.inventory_digest,
        DigestAlgorithm::Sha512.digest_hex(&root)
    );
}

#[then("no other file of the object should have changed")]
fn others_unchanged(world: &mut OcflWorld) {
    let mut after = rig(world).snapshot("");
    for rel in &receipt(world).written {
        assert!(after.remove(&format!("obj/{rel}")).is_some(), "{rel} added");
    }
    assert_eq!(after, world.repair.before);
}

#[then("the repaired object should validate with zero errors and without W010")]
fn validates(world: &mut OcflWorld) {
    let report = rig(world).validate();
    assert!(report.errors().is_empty(), "{:?}", report.errors());
    assert!(!report.warning_codes().contains("W010"));
}

#[then("a commit on the repaired object should be refused as not sha256 without writing")]
fn sha512_refused(world: &mut OcflWorld) {
    let rig = rig(world);
    rig.expire_leases();
    rig.clear_log();
    let err = rig
        .commit("writer-a", Some("v1"), &fixture_draft(rig))
        .expect_err("sha512");
    assert!(
        matches!(err, CommitError::UnsupportedAlgorithm { .. }),
        "{err:?}"
    );
    let storage: Vec<String> = rig
        .writes()
        .into_iter()
        .filter(|w| w.starts_with("storage "))
        .collect();
    assert_eq!(storage, Vec::<String>::new());
}

#[then("the logical state should be unchanged by the repair")]
fn state_unchanged(world: &mut OcflWorld) {
    assert_eq!(logical_state(rig(world)), world.repair.state);
}

#[then(expr = "a commit of {string} on the repaired object should succeed")]
fn commit_succeeds(world: &mut OcflWorld, version: String) {
    let rig = rig(world);
    let receipt = rig
        .commit("writer-b", Some("v2"), &v3_draft(rig))
        .expect("commit");
    assert_eq!(receipt.version, v(&version));
    rig.assert_valid();
}

#[then(
    expr = "a commit of {string} on the repaired object should be refused for the missing head version sidecar"
)]
fn commit_refused(world: &mut OcflWorld, _version: String) {
    let rig = rig(world);
    let err = rig
        .commit("writer-b", Some("v2"), &v3_draft(rig))
        .expect_err("half-repaired head");
    assert!(
        matches!(err, CommitError::MissingVersionInventory { .. }),
        "{err:?}"
    );
    rig.expire_leases();
}

#[then(expr = "the repair should be refused as {}")]
fn refused_as(world: &mut OcflWorld, refusal: String) {
    let err = match world.repair.result.as_ref().expect("a repair ran") {
        Ok(receipt) => panic!("repaired {receipt:?}"),
        Err(err) => err,
    };
    let matched = match refusal.as_str() {
        "nothing to repair" => matches!(err, RepairError::NothingToRepair { .. }),
        "invalid" => matches!(err, RepairError::Invalid { .. }),
        "a root sidecar mismatch" => matches!(err, RepairError::RootSidecarMismatch { .. }),
        "not repairable" => matches!(err, RepairError::NotRepairable { .. }),
        "a held reservation" => {
            matches!(err, RepairError::Reservation(ReservationError::Held { .. }))
        }
        "a lost reservation" => {
            matches!(err, RepairError::Reservation(ReservationError::Lost { .. }))
        }
        other => panic!("unknown refusal {other:?}"),
    };
    assert!(matched, "expected {refusal}, got {err:?}");
}

#[then("the repair should have written nothing")]
fn wrote_nothing(world: &mut OcflWorld) {
    let rig = rig(world);
    let storage: Vec<String> = rig
        .writes()
        .into_iter()
        .filter(|w| w.starts_with("storage "))
        .collect();
    assert_eq!(storage, Vec::<String>::new());
    assert_eq!(rig.snapshot(""), world.repair.before);
}

#[then(expr = "{string} should not be in the repaired object")]
fn not_in_object(world: &mut OcflWorld, raw: String) {
    assert!(!rig(world).stored(&raw), "{raw} was written");
}

#[then("nothing should have been written to the storage root")]
fn nothing_written(world: &mut OcflWorld) {
    let rig = rig(world);
    assert_eq!(rig.writes(), Vec::<String>::new());
    assert_eq!(rig.snapshot(""), world.repair.before);
}
