//! Version-bound logical files in the change model (IO-1.5; OCFL-S-024,
//! OCFL-S-015, OCFL-S-016, OCFL-S-018; PRD F-067, F-051).
//!
//! A commit may carry a function that returns logical files bound to the
//! version it reserves. The function runs under the `vN` reservation, sees
//! the reserved name and `created` time, runs again with the new name after
//! a rebase, and a failure publishes nothing.

use std::collections::BTreeMap;
use std::sync::Mutex;

use filesquid_ocfl::change::{ChangeError, VersionMeta};
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::storage::MemoryBackend;
use filesquid_ocfl::write::{
    BoundContext, CommitError, ReservationError, VersionBoundFiles, VersionReservations,
    MAX_BOUND_FILES,
};

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/change_rig.rs"]
mod change;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/write_rig.rs"]
mod rig;

use change::{commit_files, meta, model, state_of};
use rig::{ready, sha256, v, Rig, OBJECT_ID};

/// The facts one call of a bound-file function saw.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    version: String,
    created: String,
    base: Option<String>,
    base_paths: Vec<String>,
    paths: Vec<String>,
}

/// A bound-file function that records each call and returns one file,
/// `record/<version>.txt`, naming the version and its `created` time.
struct Recorder {
    calls: Mutex<Vec<Seen>>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Seen> {
        self.calls.lock().unwrap().clone()
    }
}

impl VersionBoundFiles for Recorder {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        self.calls.lock().unwrap().push(Seen {
            version: context.version.to_string(),
            created: context.created.to_string(),
            base: context.base.map(ToString::to_string),
            base_paths: context.base_state.keys().cloned().collect(),
            paths: context.state.keys().cloned().collect(),
        });
        let body = format!("{} {}", context.version, context.created);
        let path = format!("record/{}.txt", context.version);
        Ok(BTreeMap::from([(path, body.into_bytes())]))
    }
}

/// Commit v1 with `a.txt` through the change model.
fn commit_v1(rig: &Rig<MemoryBackend>) {
    let m = model(rig, "writer-a");
    commit_files(&m, rig, &[("a.txt", b"alpha")], "v1").expect("v1");
}

#[test]
fn bound_files_carry_the_reserved_version_and_created_time() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("b.txt", b"bravo")).expect("stage");
    let recorder = Recorder::new();
    let meta = meta(&rig, "add b");
    let created = meta.created.clone();
    let receipt = ready(m.commit_with(meta, Some(&recorder))).expect("commit");
    assert_eq!(receipt.version, v("v2"));
    assert_eq!(receipt.created, created);
    let calls = recorder.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        Seen {
            version: "v2".into(),
            created: created.clone(),
            base: Some("v1".into()),
            base_paths: vec!["a.txt".into()],
            paths: vec!["a.txt".into(), "b.txt".into()],
        }
    );
    let state = state_of(&rig, "v2");
    let body = format!("v2 {created}");
    assert_eq!(state["record/v2.txt"], sha256(body.as_bytes()));
    assert_eq!(rig.bytes("obj/v2/content/record/v2.txt"), body.into_bytes());
    assert!(receipt
        .written
        .contains(&"v2/content/record/v2.txt".to_string()));
    rig.assert_valid();
}

#[test]
fn the_function_runs_under_the_version_reservation() {
    let rig = Rig::new(MemoryBackend::new());
    let probe = |context: &BoundContext<'_>| {
        let other = ready(rig.leases.reserve(OBJECT_ID, context.version, "probe"));
        match other {
            Err(ReservationError::Held { .. }) => Ok(BTreeMap::new()),
            other => Err(format!("the version was not reserved: {other:?}")),
        }
    };
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha")).expect("stage");
    ready(m.commit_with(meta(&rig, "v1"), Some(&probe))).expect("commit under reservation");
}

#[test]
fn a_rebased_commit_binds_its_files_to_the_new_version() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let a = model(&rig, "writer-a");
    ready(a.stage_bytes("b.txt", b"bravo")).expect("stage on v1");
    let b = model(&rig, "writer-b");
    commit_files(&b, &rig, &[("c.txt", b"charlie")], "v2 by b").expect("v2");
    let recorder = Recorder::new();
    let receipt = ready(a.commit_with(meta(&rig, "rebased"), Some(&recorder))).expect("commit");
    assert_eq!(receipt.version, v("v3"));
    let calls = recorder.calls();
    assert_eq!(calls.last().unwrap().version, "v3");
    assert_eq!(calls.last().unwrap().base.as_deref(), Some("v2"));
    let state = state_of(&rig, "v3");
    assert!(state.contains_key("record/v3.txt"));
    assert!(!state.contains_key("record/v2.txt"));
    rig.assert_valid();
}

#[test]
fn a_failing_function_publishes_nothing_and_keeps_the_staging_area() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("b.txt", b"bravo")).expect("stage");
    let failing = |_: &BoundContext<'_>| Err("no record".to_string());
    let err = ready(m.commit_with(meta(&rig, "fails"), Some(&failing))).expect_err("refused");
    assert!(
        matches!(&err, ChangeError::Commit(CommitError::BoundFiles { reason }) if reason == "no record"),
        "{err:?}"
    );
    assert_eq!(rig.open().expect("open").head(), &v("v1"));
    assert!(!rig.stored("obj/v2/inventory.json"));
    let receipt = ready(m.commit(meta(&rig, "retry"))).expect("the reservation was released");
    assert_eq!(receipt.version, v("v2"));
    assert!(state_of(&rig, "v2").contains_key("b.txt"));
    rig.assert_valid();
}

#[test]
fn a_bound_file_may_not_replace_a_staged_one() {
    let rig = Rig::new(MemoryBackend::new());
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha")).expect("stage");
    let clash = |_: &BoundContext<'_>| Ok(BTreeMap::from([("a.txt".to_string(), b"x".to_vec())]));
    let err = ready(m.commit_with(meta(&rig, "clash"), Some(&clash))).expect_err("refused");
    assert!(
        matches!(err, ChangeError::Commit(CommitError::BoundFiles { .. })),
        "{err:?}"
    );
    assert!(!rig.stored("obj/0=ocfl_object_1.1"));
}

#[test]
fn too_many_bound_files_are_refused() {
    let rig = Rig::new(MemoryBackend::new());
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha")).expect("stage");
    let many = |_: &BoundContext<'_>| {
        Ok((0..=MAX_BOUND_FILES)
            .map(|n| (format!("bound/{n}.txt"), vec![b'x']))
            .collect())
    };
    let err = ready(m.commit_with(meta(&rig, "many"), Some(&many))).expect_err("refused");
    assert!(
        matches!(err, ChangeError::Commit(CommitError::BoundFiles { .. })),
        "{err:?}"
    );
}

#[test]
fn bound_bytes_the_object_holds_are_not_written_again() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("b.txt", b"bravo")).expect("stage");
    let copy = |_: &BoundContext<'_>| {
        Ok(BTreeMap::from([(
            "copy-of-a.txt".to_string(),
            b"alpha".to_vec(),
        )]))
    };
    let receipt = ready(m.commit_with(meta(&rig, "dedup"), Some(&copy))).expect("commit");
    assert_eq!(receipt.written, vec!["v2/content/b.txt".to_string()]);
    let state = state_of(&rig, "v2");
    assert_eq!(state["copy-of-a.txt"], state["a.txt"]);
    rig.assert_valid();
}

#[test]
fn revert_and_reinstate_also_carry_bound_files() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    commit_files(&m, &rig, &[("a.txt", b"alpha two")], "v2").expect("v2");
    let recorder = Recorder::new();
    let reverted =
        ready(m.revert_with(&v("v1"), meta(&rig, "revert"), Some(&recorder))).expect("revert");
    assert_eq!(reverted.version, v("v3"));
    assert!(state_of(&rig, "v3").contains_key("record/v3.txt"));
    let reinstated = ready(m.reinstate_with(
        &v("v2"),
        &["a.txt"],
        meta(&rig, "reinstate"),
        Some(&recorder),
    ))
    .expect("reinstate");
    assert_eq!(reinstated.version, v("v4"));
    let state = state_of(&rig, "v4");
    assert!(state.contains_key("record/v4.txt"));
    assert!(state.contains_key("record/v3.txt"), "earlier records stay");
    rig.assert_valid();
}

#[test]
fn version_user_and_message_pass_through_unchanged() {
    let rig = Rig::new(MemoryBackend::new());
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha")).expect("stage");
    let user = User {
        name: "FileSquid installation".into(),
        address: Some("urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into()),
    };
    let message = r#"filesquid.operation.v1 {"operation_id":"op-1","kind":"import"}"#;
    let meta = VersionMeta::new(rig.now())
        .with_message(message)
        .with_user(user.clone());
    let recorder = Recorder::new();
    ready(m.commit_with(meta, Some(&recorder))).expect("commit");
    let object = rig.open().expect("open");
    let block = &object.inventory().versions[&v("v1")];
    assert_eq!(block.message.as_deref(), Some(message));
    assert_eq!(block.user.as_ref(), Some(&user));
}

#[test]
fn a_bound_only_commit_adds_a_version_holding_only_the_bound_files() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let recorder = Recorder::new();
    let receipt = ready(m.commit_bound_only(meta(&rig, "record only"), &recorder)).expect("commit");
    assert_eq!(receipt.version, v("v2"));
    let before = state_of(&rig, "v1");
    let mut after = state_of(&rig, "v2");
    assert!(after.remove("record/v2.txt").is_some());
    assert_eq!(after, before, "nothing but the bound file changed");
    rig.assert_valid();
}

#[test]
fn a_bound_only_commit_with_no_files_publishes_nothing() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let none = |_: &BoundContext<'_>| Ok(BTreeMap::new());
    let err = ready(m.commit_bound_only(meta(&rig, "nothing"), &none)).expect_err("no-op");
    assert!(
        matches!(err, ChangeError::Commit(CommitError::BoundFiles { .. })),
        "{err:?}"
    );
    assert_eq!(rig.open().expect("open").head(), &v("v1"));
}

#[test]
fn a_bound_only_commit_leaves_staged_changes_alone() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("b.txt", b"bravo")).expect("stage");
    let recorder = Recorder::new();
    ready(m.commit_bound_only(meta(&rig, "record only"), &recorder)).expect("commit");
    assert!(!state_of(&rig, "v2").contains_key("b.txt"));
    let receipt = ready(m.commit(meta(&rig, "staged"))).expect("staged change still commits");
    assert_eq!(receipt.version, v("v3"));
    assert!(state_of(&rig, "v3").contains_key("b.txt"));
}
