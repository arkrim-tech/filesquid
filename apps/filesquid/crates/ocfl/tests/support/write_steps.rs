//! Step definitions for `write_order_OCFL-S-018.feature` (IO-0.6). Every
//! scenario writes through the engine writer into an in-memory storage root
//! behind the fault-injecting backend (`crate::fault`, `crate::rig`).

use std::fmt;

use cucumber::{given, then, when};
use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{
    CommitError, CommitReceipt, ReservationError, VersionDraft, VersionReservations,
};

use crate::rig::{ready, sha256, v, Rig, OBJECT_ID};
use crate::OcflWorld;

/// What the write scenarios carry between steps.
#[derive(Default)]
pub struct WriteState {
    rig: Option<Rig<MemoryBackend>>,
    result: Option<Result<CommitReceipt, CommitError>>,
    root_before: Vec<u8>,
    retry: Option<VersionDraft>,
}

impl fmt::Debug for WriteState {
    /// Show the outcome; the rig holds closures that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteState")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

/// The scenario's rig.
pub fn rig(world: &OcflWorld) -> &Rig<MemoryBackend> {
    world.write.rig.as_ref().expect("Given an OCFL object")
}

/// A draft on top of v1 that keeps a.txt and b.txt and adds `files`.
fn adding(rig: &Rig<MemoryBackend>, tag: &str, files: &[&str]) -> VersionDraft {
    let staged: Vec<(&str, &[u8])> = files.iter().map(|f| (*f, f.as_bytes())).collect();
    let mut draft = rig.draft(tag, &staged);
    draft.keep("a.txt", sha256(b"alpha"));
    draft.keep("b.txt", sha256(b"bravo"));
    draft
}

/// The commit's error.
fn refusal(world: &mut OcflWorld) -> CommitError {
    match world.write.result.take().expect("a commit ran") {
        Ok(receipt) => panic!("committed {receipt:?}"),
        Err(err) => err,
    }
}

#[given(expr = "an OCFL object {string} committed at {string} through the engine writer")]
fn object_committed(world: &mut OcflWorld, _root: String, _version: String) {
    let rig = Rig::new(MemoryBackend::new());
    rig.commit_v1();
    world.write.root_before = rig.bytes("obj/inventory.json");
    rig.clear_log();
    world.write.rig = Some(rig);
}

/// Commit v2 on top of v1 as `holder`, adding `files`.
fn commit_v2(world: &mut OcflWorld, holder: &str, files: &[&str]) {
    let rig = rig(world);
    let draft = adding(rig, "v2", files);
    world.write.result = Some(rig.commit(holder, Some("v1"), &draft));
}

#[when(expr = "writer {string} commits {string} adding {string}")]
fn commit_adding(world: &mut OcflWorld, holder: String, _version: String, file: String) {
    commit_v2(world, &holder, &[file.as_str()]);
}

#[when(expr = "writer {string} commits {string} adding {string} and {string}")]
fn commit_adding_two(world: &mut OcflWorld, holder: String, _v: String, a: String, b: String) {
    commit_v2(world, &holder, &[a.as_str(), b.as_str()]);
}

#[when(expr = "writer {string} commits {string} renaming {string} to {string}")]
fn commit_renaming(world: &mut OcflWorld, holder: String, _v: String, from: String, to: String) {
    let rig = rig(world);
    let mut draft = VersionDraft::new(OBJECT_ID, rig.now());
    draft.keep("a.txt", sha256(b"alpha"));
    assert_eq!(from, "b.txt", "the background object holds b.txt");
    draft.keep(to, sha256(b"bravo"));
    world.write.result = Some(rig.commit(&holder, Some("v1"), &draft));
}

#[then(expr = "the reservation should come before every {string} byte")]
fn reservation_first(world: &mut OcflWorld, version: String) {
    let rig = rig(world);
    let writes = rig.writes();
    let first = writes.iter().position(|w| w.starts_with("leases put_if"));
    let byte = writes
        .iter()
        .position(|w| w.contains(&format!(" obj/{version}/")));
    assert!(first.is_some() && byte.is_some(), "{writes:?}");
    assert!(first < byte, "{writes:?}");
}

#[then("the writes should end with the version inventory, the root inventory and the root sidecar")]
fn writes_end_in_order(world: &mut OcflWorld) {
    let storage: Vec<String> = rig(world)
        .writes()
        .into_iter()
        .filter(|w| w.starts_with("storage "))
        .collect();
    let tail = &storage[storage.len() - 4..];
    assert_eq!(
        tail,
        [
            "storage put_if obj/v2/inventory.json",
            "storage put_if obj/v2/inventory.json.sha256",
            "storage put_if obj/inventory.json",
            "storage put_if obj/inventory.json.sha256",
        ]
    );
}

#[then(expr = "the object head should be {string}")]
fn object_head(world: &mut OcflWorld, version: String) {
    let head = rig(world).open().expect("open").head().to_string();
    assert_eq!(head, version);
}

#[then("the object should validate with zero errors")]
fn validates(world: &mut OcflWorld) {
    rig(world).assert_valid();
}

#[then(expr = "no content should be written under {string}")]
fn no_content(world: &mut OcflWorld, version: String) {
    let receipt = world.write.result.as_ref().expect("a commit ran");
    assert_eq!(
        receipt.as_ref().expect("committed").written,
        Vec::<String>::new()
    );
    let listed = rig(world).snapshot(&format!("obj/{version}/content/"));
    assert!(listed.is_empty(), "{listed:?}");
}

#[then(expr = "{string} should resolve to the content path {string}")]
fn resolves_to(world: &mut OcflWorld, path: String, content: String) {
    let object = rig(world).open().expect("open");
    let state = object.state(object.head()).expect("state");
    assert_eq!(
        state.get(&path).expect("logical file").content_path,
        content
    );
}

#[given(expr = "writer {string} crashed while writing {string} during the commit of {string}")]
fn crashed(world: &mut OcflWorld, holder: String, crash_key: String, _version: String) {
    let rig = rig(world);
    rig.storage.crash_at(&crash_key);
    let draft = adding(rig, "v2", &["d.txt"]);
    let outcome = rig.try_commit(&holder, Some("v1"), &draft);
    assert!(outcome.is_err(), "the writer crashed");
    world.write.retry = Some(draft);
}

#[when("I open the written object")]
fn open_written(world: &mut OcflWorld) {
    rig(world).open().expect("the object still opens");
}

#[then(expr = "pinning {string} should be refused as an unknown version")]
fn pin_refused(world: &mut OcflWorld, version: String) {
    let object = rig(world).open().expect("open");
    let err = object.state(&v(&version)).expect_err("unpublished");
    assert!(matches!(err, ReadError::VersionNotFound { .. }), "{err:?}");
}

#[when(expr = "the crashed writer's lease lapses and writer {string} retries the commit")]
fn retry(world: &mut OcflWorld, holder: String) {
    let draft = world.write.retry.take().expect("a crashed commit");
    let rig = rig(world);
    rig.expire_leases();
    let receipt = rig.commit(&holder, Some("v1"), &draft);
    world.write.result = Some(receipt);
}

#[then(expr = "the history should list {string}")]
fn history_lists(world: &mut OcflWorld, versions: String) {
    let object = rig(world).open().expect("open");
    let names: Vec<String> = object
        .history()
        .iter()
        .map(|h| h.version.to_string())
        .collect();
    assert_eq!(names.join(", "), versions);
}

#[given(expr = "writer {string} holds the reservation for {string}")]
fn holds(world: &mut OcflWorld, holder: String, version: String) {
    let rig = rig(world);
    ready(rig.leases.reserve(OBJECT_ID, &v(&version), &holder)).expect("reserve");
    rig.clear_log();
}

#[then("the commit should be refused because the reservation is held")]
fn refused_held(world: &mut OcflWorld) {
    let err = refusal(world);
    assert!(
        matches!(err, CommitError::Reservation(ReservationError::Held { .. })),
        "{err:?}"
    );
}

#[then(expr = "no byte should be written under {string}")]
fn nothing_under(world: &mut OcflWorld, version: String) {
    let listed = rig(world).snapshot(&format!("obj/{version}/"));
    assert!(listed.is_empty(), "{listed:?}");
}

#[given(expr = "the lease passes to writer {string} after {string} is written")]
fn passes_after(world: &mut OcflWorld, holder: String, trigger: String) {
    let rig = rig(world);
    let leases = rig.leases.clone();
    let clock = rig.clock.clone();
    rig.storage.after_write(
        &trigger,
        Box::new(move || {
            clock.advance(crate::rig::LEASES.ttl_ms);
            ready(leases.reserve(OBJECT_ID, &v("v2"), &holder)).expect("reassigned");
        }),
    );
}

#[then("the commit should be refused because the reservation was lost")]
fn refused_lost(world: &mut OcflWorld) {
    let err = refusal(world);
    assert!(
        matches!(err, CommitError::Reservation(ReservationError::Lost { .. })),
        "{err:?}"
    );
}

#[then(expr = "{string} should not be stored")]
fn not_stored(world: &mut OcflWorld, raw: String) {
    assert!(!rig(world).stored(&raw), "{raw} was written");
}

#[then("the root inventory should be unchanged")]
fn root_unchanged(world: &mut OcflWorld) {
    let now = rig(world).bytes("obj/inventory.json");
    assert_eq!(now, world.write.root_before);
}

#[given(expr = "writer {string} has committed {string} adding {string}")]
fn has_committed(world: &mut OcflWorld, holder: String, _version: String, file: String) {
    let rig = rig(world);
    let draft = adding(rig, "first", &[file.as_str()]);
    rig.commit(&holder, Some("v1"), &draft).expect("commit");
}

#[when(expr = "writer {string} commits on top of {string} adding {string}")]
fn commits_on_top(world: &mut OcflWorld, holder: String, base: String, file: String) {
    let rig = rig(world);
    let draft = adding(rig, "late", &[file.as_str()]);
    world.write.result = Some(rig.commit(&holder, Some(&base), &draft));
}

#[then(expr = "the commit should be refused because the head moved to {string}")]
fn refused_moved(world: &mut OcflWorld, found: String) {
    match refusal(world) {
        CommitError::HeadMoved { found: actual, .. } => {
            assert_eq!(actual.as_deref(), Some(found.as_str()));
        }
        other => panic!("{other:?}"),
    }
}
