//! Step definitions for `copy_guard_warning_OCFL-S-025.feature` (IO-1.6).
//! Each scenario makes one copy the copy guard refuses (a file copy, a
//! checkout or a staging write) on the write rig of
//! `write_order_OCFL-S-018`, with a [`WarningCapture`] as the subscriber,
//! and checks the fields of the warning it records.

use cucumber::{then, when};
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea};
use filesquid_ocfl::read::{Object, ReadError};

use crate::capture::{Fields, WarningCapture};
use crate::change::model;
use crate::rig::{key, ready, v, OBJECT_ID, ROOT};
use crate::write_steps::rig;
use crate::OcflWorld;

/// The message of the copy guard's warning.
const REFUSED: &str = "refused a copy into an OCFL root";

/// What the copy-warning scenarios carry between steps.
#[derive(Debug, Default)]
pub struct CopyWarningState {
    /// Whether the copy was refused as a copy into an OCFL root.
    refused: Option<bool>,
    /// The copy-guard warnings recorded during the copy.
    warnings: Vec<Fields>,
}

/// Whether `error` is the copy guard's refusal.
fn is_guard_refusal(error: &ReadError) -> bool {
    matches!(error, ReadError::CopyIntoOcflRoot { .. })
}

/// Whether `error` is the copy guard's refusal, from the change model.
fn is_change_refusal(error: &ChangeError) -> bool {
    matches!(error, ChangeError::Read(read) if is_guard_refusal(read))
}

/// The one recorded warning.
fn warning(world: &OcflWorld) -> &Fields {
    world
        .copy_warning
        .warnings
        .first()
        .expect("a copy-guard warning")
}

#[when(expr = "I copy {string} of {string} into {string} while warnings are recorded")]
fn copy_recorded(world: &mut OcflWorld, path: String, version: String, dest: String) {
    let capture = WarningCapture::default();
    let rig = rig(world);
    let result = capture.run(|| {
        let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
        ready(object.copy_file(&v(&version), &path, &rig.storage, &key(&dest), None))
    });
    world.copy_warning.refused = Some(result.as_ref().is_err_and(is_guard_refusal));
    world.copy_warning.warnings = capture.with_message(REFUSED);
}

#[when(expr = "I check out {string} under {string} while warnings are recorded")]
fn checkout_recorded(world: &mut OcflWorld, version: String, prefix: String) {
    let capture = WarningCapture::default();
    let rig = rig(world);
    let m = model(rig, "writer-a");
    let result = capture.run(|| ready(m.checkout(&v(&version), &rig.storage, &prefix)));
    world.copy_warning.refused = Some(result.as_ref().is_err_and(is_change_refusal));
    world.copy_warning.warnings = capture.with_message(REFUSED);
}

#[when(expr = "I stage {string} in a staging area under {string} while warnings are recorded")]
fn stage_recorded(world: &mut OcflWorld, path: String, prefix: String) {
    let capture = WarningCapture::default();
    let rig = rig(world);
    let area = StagingArea::new(&rig.storage, &prefix).expect("prefix");
    let m = ChangeModel::new(&rig.storage, &*rig.leases, "w", ROOT, OBJECT_ID, area);
    let result = capture.run(|| ready(m.stage_bytes(&path, b"delta")));
    world.copy_warning.refused = Some(result.as_ref().is_err_and(is_change_refusal));
    world.copy_warning.warnings = capture.with_message(REFUSED);
}

#[then("the copy should be refused and one copy-guard warning recorded")]
fn refused_with_one_warning(world: &mut OcflWorld) {
    assert_eq!(world.copy_warning.refused, Some(true), "refused");
    assert_eq!(world.copy_warning.warnings.len(), 1, "one warning");
}

#[then(expr = "the warning should name the destination {string} and the root {string}")]
fn names_destination_and_root(world: &mut OcflWorld, destination: String, root: String) {
    let fields = warning(world);
    assert_eq!(fields.get("destination"), Some(&destination), "{fields:?}");
    assert_eq!(fields.get("root"), Some(&root), "{fields:?}");
}

#[then(expr = "the warning should name the root {string}")]
fn names_root(world: &mut OcflWorld, root: String) {
    let fields = warning(world);
    assert_eq!(fields.get("root"), Some(&root), "{fields:?}");
    assert!(fields.contains_key("destination"), "{fields:?}");
}

#[then(expr = "the warning should name the object {string} at version {string}")]
fn names_object_and_version(world: &mut OcflWorld, object: String, version: String) {
    let fields = warning(world);
    assert_eq!(fields.get("object"), Some(&object), "{fields:?}");
    assert_eq!(fields.get("version"), Some(&version), "{fields:?}");
}

#[then(expr = "the warning should name the object {string} without a version")]
fn names_object_only(world: &mut OcflWorld, object: String) {
    let fields = warning(world);
    assert_eq!(fields.get("object"), Some(&object), "{fields:?}");
    assert!(!fields.contains_key("version"), "{fields:?}");
}
