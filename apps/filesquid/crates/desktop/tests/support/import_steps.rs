//! Steps of `desktop_imports_F-002.feature` (Tasks DE-1.2, DE-1.7, DE-1.9; SRS-F-002,
//! API-S-036, DE-S-019, DE-S-018, API-S-011, DE-S-006, DE-S-020, API-S-037): the desktop's import route over a library in its
//! own folder, attached to its catalog index, driven through the `filesquid://` scheme with
//! the per-launch token and a scripted dialog ([`ImportRig`](super::import_rig::ImportRig)),
//! in a scratch directory that is removed when the scenario ends.

use super::import_rig::{fresh_dir, holiday_folder, ImportRig, LIBRARY, UNOPENED_LIBRARY};
use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::imports::IMPORT_START_OPERATION;
use filesquid_api::navigation::{FolderScope, NodeRef};
use filesquid_core::CollectionId;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    assert_eq!(
        starts[0].error_code.as_deref(),
        Some("collection_not_found"),
        "the destination is not a collection of that library"
    );
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

#[when("the app asks to import a folder without naming a destination collection")]
async fn import_nowhere(world: &mut DesktopWorld) {
    let only_folder = json!({ "folder": "trips" });
    let answer = rig(world)
        .import_into(LIBRARY, "op-nowhere", "folder", only_folder)
        .await;
    scenario(world).answers.push(answer);
}

#[when(expr = "the import has finished with {int} files")]
fn import_finished(world: &mut DesktopWorld, count: usize) {
    assert_eq!(rig(world).wait_for_files(count), count);
}

#[then(expr = "the import should be refused as an invalid request naming {string}")]
fn refused_invalid(world: &mut DesktopWorld, code: String) {
    let (status, answer) = last_answer(world);
    assert_eq!(status, 400, "{answer}");
    assert_eq!(answer["code"], "invalid_request");
    let message = answer["message"].as_str().unwrap_or_default();
    assert!(message.contains(&code), "{answer}");
}

/// The folder scope of the rig's destination collection.
fn destination(world: &mut DesktopWorld) -> FolderScope {
    let collection = CollectionId::new(rig(world).collection.clone()).expect("collection");
    FolderScope::Collection(collection)
}

/// The names shown by `key` items of `page`: a file's display name, else its name.
fn shown(page: &Value, key: &str) -> Vec<String> {
    page[key]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            let name = item.get("display_name").unwrap_or(&item["name"]);
            name.as_str().expect("name").to_string()
        })
        .collect()
}

#[then(expr = "the collection should list the folder {string} holding {string} and {string}")]
async fn folder_holds(world: &mut DesktopWorld, folder: String, first: String, second: String) {
    let node = NodeRef::folder(destination(world), folder)
        .to_handle()
        .expect("handle");
    let uri = format!("/api/v1/files/listing?node={node}");
    let (status, page) = rig(world).get(&uri).await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(shown(&page, "entries"), [first, second]);
}

#[then(expr = "the collection should list the folders {string} and {string} in {string}")]
async fn folders_in(world: &mut DesktopWorld, first: String, second: String, parent: String) {
    assert_eq!(rig(world).wait_for_files(4), 4, "both imports finished");
    let node = NodeRef::folder(destination(world), parent)
        .to_handle()
        .expect("handle");
    let (status, page) = rig(world)
        .get(&format!("/api/v1/navigation?node={node}"))
        .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(shown(&page, "nodes"), [first, second]);
}

/// Longest wait for an import job to reach a state.
const JOB_WAIT: Duration = Duration::from_secs(30);

/// The job the first started import answered.
fn job_id(world: &mut DesktopWorld) -> String {
    let answers = &scenario(world).answers;
    let job = answers
        .iter()
        .find_map(|(_, answer)| answer["job_id"].as_str());
    job.expect("a started job").to_string()
}

#[given("the import worker will be held on its first file")]
fn held(world: &mut DesktopWorld) {
    rig(world).gate.arm();
}

/// POST the control `control` of the scenario's job, asserting it is answered.
async fn control(world: &mut DesktopWorld, control: &str) -> Value {
    let job = job_id(world);
    let path = format!("/api/v1/jobs/{job}/{control}");
    let (status, answer) = rig(world).post(&path).await;
    assert_eq!(status, 200, "{control}: {answer}");
    answer
}

#[when(expr = "the owner {word} the import while its first file is in hand")]
async fn control_in_hand(world: &mut DesktopWorld, verb: String) {
    assert!(
        rig(world).gate.wait_held(),
        "the worker holds the first file"
    );
    let name = match verb.as_str() {
        "pauses" => "pause",
        "cancels" => "cancel",
        other => panic!("no control {other}"),
    };
    control(world, name).await;
    rig(world).gate.release();
}

#[when("the owner resumes the import job")]
async fn resume(world: &mut DesktopWorld) {
    let answer = control(world, "resume").await;
    assert_eq!(
        answer["scheduled"], true,
        "handed to the import runner: {answer}"
    );
}

#[then(expr = "the import job should be {word} with {int} of {int} files done")]
async fn job_reaches(world: &mut DesktopWorld, state: String, done: u64, total: u64) {
    let job = job_id(world);
    let reached = |view: &Value| match state.as_str() {
        "paused" => view["paused"] == true,
        other => view["state"] == other,
    };
    let deadline = Instant::now() + JOB_WAIT;
    let mut view = Value::Null;
    while Instant::now() < deadline {
        view = rig(world).get(&format!("/api/v1/jobs/{job}")).await.1;
        if reached(&view) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(reached(&view), "{state}: {view}");
    assert_eq!(view["done"], done, "{view}");
    assert_eq!(view["total"], total, "{view}");
}

#[then(expr = "the job control {string} should be journaled once")]
fn control_journaled(world: &mut DesktopWorld, operation: String) {
    let job = job_id(world);
    let entries = rig(world)
        .journal()
        .into_iter()
        .filter(|e| e.operation == operation && e.target_ids.contains(&job))
        .count();
    assert_eq!(entries, 1, "{operation}");
}

#[then("only the file in hand should be imported")]
fn only_in_hand(world: &mut DesktopWorld) {
    let rig = rig(world);
    rig.stop_worker();
    assert_eq!(rig.imported().len(), 1, "no further file read");
    assert_eq!(rig.file_objects(), 1);
}
