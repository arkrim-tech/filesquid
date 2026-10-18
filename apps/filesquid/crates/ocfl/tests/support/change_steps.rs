//! Step definitions for `change_model_OCFL-S-014.feature` (IO-0.10). Every
//! scenario runs the change model on an in-memory storage root behind the
//! fault-injecting backend, with staging on the rig's own staging backend.

use std::collections::BTreeMap;
use std::fmt;

use cucumber::{given, then, when};
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea};
use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{CommitError, CommitReceipt};

use crate::change::{commit_files, labels, meta, model, state_of, status};
use crate::rig::{key, ready, sha256, v, Rig, OBJECT_ID, ROOT};
use crate::OcflWorld;

/// What the change-model scenarios carry between steps.
#[derive(Default)]
pub struct ChangeState {
    rig: Option<Rig<MemoryBackend>>,
    result: Option<Result<CommitReceipt, ChangeError>>,
    before: BTreeMap<String, String>,
    stage_error: Option<ChangeError>,
}

impl fmt::Debug for ChangeState {
    /// Show the outcome; the rig holds closures that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangeState")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

/// The scenario's rig.
fn rig(world: &OcflWorld) -> &Rig<MemoryBackend> {
    world.change.rig.as_ref().expect("Given an object")
}

/// The last commit's receipt.
fn receipt(world: &OcflWorld) -> &CommitReceipt {
    match world.change.result.as_ref().expect("a commit ran") {
        Ok(receipt) => receipt,
        Err(err) => panic!("the commit failed: {err:?}"),
    }
}

/// The last commit's error.
fn error(world: &OcflWorld) -> &ChangeError {
    match world.change.result.as_ref().expect("a commit ran") {
        Ok(receipt) => panic!("the commit succeeded: {receipt:?}"),
        Err(err) => err,
    }
}

#[given(
    expr = "an object committed at {string} with {string}, {string} and {string} through the change model"
)]
fn object_at_v1(world: &mut OcflWorld, version: String, a: String, b: String, c: String) {
    let rig = Rig::new(MemoryBackend::new());
    let files: [(&str, &[u8]); 3] = [(&a, b"alpha"), (&b, b"bravo"), (&c, b"charlie")];
    let m = model(&rig, "writer-a");
    let receipt = commit_files(&m, &rig, &files, "v1").expect("commit v1");
    assert_eq!(receipt.version, v(&version));
    world.change.rig = Some(rig);
}

#[given(expr = "writer {string} committed {string} deleting {string}")]
fn committed_delete(world: &mut OcflWorld, holder: String, version: String, path: String) {
    let rig = rig(world);
    let m = model(rig, &holder);
    ready(m.stage_remove(&path)).expect("stage delete");
    let receipt = ready(m.commit(meta(rig, "delete"))).expect("commit");
    assert_eq!(receipt.version, v(&version));
}

#[given(expr = "writer {string} committed {string} with new bytes for {string}")]
fn committed_bytes(world: &mut OcflWorld, holder: String, version: String, path: String) {
    let rig = rig(world);
    let m = model(rig, &holder);
    let files: [(&str, &[u8]); 1] = [(&path, holder.as_bytes())];
    let receipt = commit_files(&m, rig, &files, "change").expect("commit");
    assert_eq!(receipt.version, v(&version));
}

#[given(expr = "writer {string} staged new bytes for {string}")]
fn staged_bytes(world: &mut OcflWorld, holder: String, path: String) {
    let rig = rig(world);
    let bytes = format!("{holder} {path}");
    ready(model(rig, &holder).stage_bytes(&path, bytes.as_bytes())).expect("stage");
}

#[given(expr = "the head version inventory of {string} was removed")]
fn remove_head_inventory(world: &mut OcflWorld, version: String) {
    let rig = rig(world);
    for name in ["inventory.json", "inventory.json.sha256"] {
        let raw = format!("{ROOT}/{version}/{name}");
        ready(rig.storage.delete(&key(&raw))).expect("remove");
    }
}

#[when(
    expr = "writer {string} stages new bytes for {string}, renames {string} to {string} and deletes {string}"
)]
fn stage_mixed(world: &mut OcflWorld, holder: String, a: String, b: String, to: String, c: String) {
    let rig = rig(world);
    let m = model(rig, &holder);
    ready(m.stage_bytes(&a, b"new alpha")).expect("change");
    ready(m.stage_rename(&b, &to)).expect("rename");
    ready(m.stage_remove(&c)).expect("delete");
}

#[when(expr = "writer {string} stages the original bytes of {string}")]
fn stage_original(world: &mut OcflWorld, holder: String, path: String) {
    let rig = rig(world);
    ready(model(rig, &holder).stage_bytes(&path, b"charlie")).expect("stage");
}

#[when(expr = "writer {string} commits the staged changes")]
fn commit_staged(world: &mut OcflWorld, holder: String) {
    let before = rig(world).snapshot(ROOT);
    let rig = rig(world);
    let result = ready(model(rig, &holder).commit(meta(rig, "staged")));
    world.change.before = before;
    world.change.result = Some(result);
}

#[when(expr = "writer {string} reverts to {string}")]
fn revert(world: &mut OcflWorld, holder: String, version: String) {
    let before = rig(world).snapshot(ROOT);
    let rig = rig(world);
    let result = ready(model(rig, &holder).revert(&v(&version), meta(rig, "revert")));
    world.change.before = before;
    world.change.result = Some(result);
}

#[when("a writer stages a file in a staging area inside the object root")]
fn stage_inside(world: &mut OcflWorld) {
    let rig = rig(world);
    let inside = StagingArea::new(&rig.storage, "obj/staging").expect("prefix");
    let m = ChangeModel::new(&rig.storage, &*rig.leases, "w", ROOT, OBJECT_ID, inside);
    world.change.stage_error = ready(m.stage_bytes("d.txt", b"d")).err();
}

#[then(expr = "the status should list {string}, {string} and {string}")]
fn status_three(world: &mut OcflWorld, a: String, b: String, c: String) {
    let changes = status(&model(rig(world), "writer-a")).changes;
    assert_eq!(labels(&changes), [a, b, c]);
}

#[then(expr = "the status should list {string}")]
fn status_one(world: &mut OcflWorld, a: String) {
    let changes = status(&model(rig(world), "writer-a")).changes;
    assert_eq!(labels(&changes), [a]);
}

#[then(expr = "the object head should be {string} through the change model")]
fn head_is(world: &mut OcflWorld, version: String) {
    assert_eq!(rig(world).open().expect("open").head(), &v(&version));
}

#[then(expr = "the commit should have written only {string}")]
fn written_only(world: &mut OcflWorld, path: String) {
    assert_eq!(receipt(world).written, [path]);
}

#[then("the commit should have written nothing")]
fn written_nothing(world: &mut OcflWorld) {
    assert!(receipt(world).written.is_empty());
}

#[then("the change-model object should validate with zero errors")]
fn valid(world: &mut OcflWorld) {
    rig(world).assert_valid();
}

#[then(expr = "{string} at {string} should point at {string}")]
fn points_at(world: &mut OcflWorld, path: String, version: String, content: String) {
    let object = rig(world).open().expect("open");
    let state = object.state(&v(&version)).expect("state");
    assert_eq!(state.get(&path).expect("path").content_path, content);
}

#[then(expr = "the state of {string} should equal the state of {string}")]
fn states_equal(world: &mut OcflWorld, a: String, b: String) {
    assert_eq!(state_of(rig(world), &a), state_of(rig(world), &b));
}

#[then(expr = "no file of {string} or {string} should have changed")]
fn history_kept(world: &mut OcflWorld, a: String, b: String) {
    let after = rig(world).snapshot(ROOT);
    for (k, tag) in &world.change.before {
        let earlier = [&a, &b]
            .iter()
            .any(|ver| k.starts_with(&format!("{ROOT}/{ver}/")));
        if earlier {
            assert_eq!(after.get(k), Some(tag), "{k} changed");
        }
    }
}

#[then(expr = "{string} should hold the changes of both writers")]
fn both_writers(world: &mut OcflWorld, version: String) {
    let state = state_of(rig(world), &version);
    assert_eq!(state["a.txt"], sha256(b"writer-a a.txt"));
    assert_eq!(state["b.txt"], sha256(b"writer-b"));
}

#[then(expr = "the commit should be refused as a conflict on {string}")]
fn conflict_on(world: &mut OcflWorld, path: String) {
    let ChangeError::Conflict { paths, .. } = error(world) else {
        panic!("expected a conflict, got {:?}", error(world));
    };
    assert_eq!(paths, &[path]);
    assert_eq!(rig(world).snapshot(ROOT), world.change.before);
}

#[then("the commit should be refused as nothing to commit")]
fn nothing_to_commit(world: &mut OcflWorld) {
    assert!(matches!(error(world), ChangeError::NothingToCommit));
}

#[then("the commit should be refused for the missing version inventory")]
fn missing_inventory(world: &mut OcflWorld) {
    let err = error(world);
    assert!(
        matches!(
            err,
            ChangeError::Commit(CommitError::MissingVersionInventory { .. })
        ),
        "{err:?}"
    );
    assert_eq!(rig(world).snapshot(ROOT), world.change.before);
}

#[then(expr = "the head version inventory of {string} should still be missing")]
fn still_missing(world: &mut OcflWorld, version: String) {
    assert!(!rig(world).stored(&format!("{ROOT}/{version}/inventory.json")));
}

#[then("staging should be refused as inside an OCFL root")]
fn staging_refused(world: &mut OcflWorld) {
    let err = world
        .change
        .stage_error
        .as_ref()
        .expect("staging was refused");
    assert!(
        matches!(err, ChangeError::Read(ReadError::CopyIntoOcflRoot { .. })),
        "{err:?}"
    );
    assert!(rig(world).snapshot("obj/staging").is_empty());
}
