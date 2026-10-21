//! Steps of `desktop_imports_F-002.feature` (Task DE-1.2; SRS-F-002, API-S-036, DE-S-019,
//! DE-S-006): the desktop's import route over a folder-bound library, driven through the
//! `filesquid://` scheme with the per-launch token and a scripted dialog
//! ([`ImportRig`](super::import_rig::ImportRig)), in a scratch directory that is removed when
//! the scenario ends.

use super::import_rig::{fresh_dir, holiday_folder, ImportRig, LIBRARY, UNOPENED_LIBRARY};
use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::imports::IMPORT_START_OPERATION;
use serde_json::Value;
use std::path::PathBuf;

/// What one import scenario set up and saw.
#[derive(Debug, Default)]
pub struct ImportScenario {
    dir: Option<PathBuf>,
    rig: Option<ImportRig>,
    answers: Vec<(u16, Value)>,
}

/// The scenario's import state.
fn scenario(world: &mut DesktopWorld) -> &mut ImportScenario {
    world.imports.get_or_insert_with(ImportScenario::default)
}

/// The scenario's rig, launched with the dialog's pick.
fn rig(world: &mut DesktopWorld) -> &mut ImportRig {
    scenario(world)
        .rig
        .as_mut()
        .expect("the dialog's pick is set")
}

/// The last answer.
fn last_answer(world: &mut DesktopWorld) -> (u16, Value) {
    scenario(world).answers.last().cloned().expect("an answer")
}

#[given("the desktop host has started its import route over an open library")]
fn started(world: &mut DesktopWorld) {
    scenario(world).dir = Some(fresh_dir("feature"));
}

#[given(expr = "the owner will pick a folder of {int} files")]
async fn will_pick_folder(world: &mut DesktopWorld, count: usize) {
    assert_eq!(count, 2, "the holiday folder holds two files");
    let dir = scenario(world).dir.clone().expect("dir");
    let folder = holiday_folder(&dir);
    scenario(world).rig = Some(ImportRig::launch(dir, Some(vec![folder])).await);
}

#[given("the owner will close the dialog without picking anything")]
async fn will_cancel(world: &mut DesktopWorld) {
    let dir = scenario(world).dir.clone().expect("dir");
    scenario(world).rig = Some(ImportRig::launch(dir, None).await);
}

#[given("the import worker has stopped for the app to quit")]
fn worker_stopped(world: &mut DesktopWorld) {
    rig(world).stop_worker();
}

/// POST an import and keep the answer.
async fn ask(world: &mut DesktopWorld, library: &str, operation: &str, source: &str) {
    let answer = rig(world).import(library, operation, source).await;
    scenario(world).answers.push(answer);
}

#[when(expr = "the app asks to import a folder under the operation {string}")]
async fn import_folder(world: &mut DesktopWorld, operation: String) {
    ask(world, LIBRARY, &operation, "folder").await;
}

#[when(expr = "the app asks to import a folder under the operation {string} again")]
async fn import_folder_again(world: &mut DesktopWorld, operation: String) {
    ask(world, LIBRARY, &operation, "folder").await;
}

#[when(expr = "the app asks to import files under the operation {string}")]
async fn import_files(world: &mut DesktopWorld, operation: String) {
    ask(world, LIBRARY, &operation, "files").await;
}

#[when("the app asks to import into a library that is not open")]
async fn import_unopened(world: &mut DesktopWorld) {
    ask(world, UNOPENED_LIBRARY, "op-unopened", "folder").await;
}

#[then("the answer should name a started import job")]
fn answer_started(world: &mut DesktopWorld) {
    let (status, answer) = last_answer(world);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["import"], "started");
    assert_eq!(answer["outcome"]["kind"], "pending");
    assert!(answer["job_id"].is_string(), "{answer}");
}

#[then("the answer should say the import was cancelled")]
fn answer_cancelled(world: &mut DesktopWorld) {
    let (status, answer) = last_answer(world);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["import"], "cancelled");
    assert_eq!(answer["outcome"]["kind"], "committed");
    assert!(answer.get("job_id").is_none(), "{answer}");
}

#[then("both answers should name the same import job")]
fn same_job(world: &mut DesktopWorld) {
    let answers = &scenario(world).answers;
    assert_eq!(answers.len(), 2);
    assert!(answers[0].1["job_id"].is_string());
    assert_eq!(answers[0].1["job_id"], answers[1].1["job_id"]);
}

#[then("the dialog should not have been shown")]
fn never_shown(world: &mut DesktopWorld) {
    assert_eq!(rig(world).dialog.shown(), 0);
}

#[then("the dialog should have been shown once")]
fn shown_once(world: &mut DesktopWorld) {
    assert_eq!(rig(world).dialog.shown(), 1);
}

#[then(expr = "{int} files should be imported, each as its own object")]
fn files_imported(world: &mut DesktopWorld, count: usize) {
    let rig = rig(world);
    assert_eq!(rig.wait_for_files(count), count);
    rig.stop_worker();
    assert_eq!(rig.imported().len(), count, "each file imported once");
    assert_eq!(rig.file_objects(), count);
}

#[then("no file should be imported")]
fn none_imported(world: &mut DesktopWorld) {
    let rig = rig(world);
    rig.stop_worker();
    assert!(rig.imported().is_empty());
    assert_eq!(rig.file_objects(), 0);
}

#[then("no journal entry should name a picked file or folder")]
fn no_names(world: &mut DesktopWorld) {
    let rig = rig(world);
    let text = format!("{:?}", rig.journal());
    for name in ["Holidays", "Beach day", "Notes.txt"] {
        assert!(!text.contains(name), "{name} journaled");
    }
    assert!(!text.contains(rig.dir.to_string_lossy().as_ref()));
}

#[then("the import should be refused as not found")]
fn refused_not_found(world: &mut DesktopWorld) {
    let (status, answer) = last_answer(world);
    assert_eq!(status, 404, "{answer}");
    assert_eq!(answer["code"], "not_found");
}

#[then("the import should be refused as unavailable")]
fn refused_unavailable(world: &mut DesktopWorld) {
    let (status, answer) = last_answer(world);
    assert_eq!(status, 503, "{answer}");
    assert_eq!(answer["code"], "unavailable");
}

#[then("the journal should record one import request without any path")]
fn one_request(world: &mut DesktopWorld) {
    let rig = rig(world);
    rig.stop_worker();
    let starts: Vec<_> = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == IMPORT_START_OPERATION)
        .collect();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].error_code.as_deref(), Some("library_not_found"));
    assert!(!format!("{starts:?}").contains(rig.dir.to_string_lossy().as_ref()));
}

#[then(expr = "after the app restarts {int} files should be imported, each as its own object")]
async fn after_restart(world: &mut DesktopWorld, count: usize) {
    assert_eq!(
        rig(world).file_objects(),
        0,
        "nothing imported before the restart"
    );
    rig(world).relaunch().await;
    files_imported(world, count);
}
