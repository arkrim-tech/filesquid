//! Task DE-1.2: the desktop's import route (API-S-036, DE-S-019) through the `filesquid://`
//! scheme with the per-launch token: the owner picks a folder in the host's dialog (scripted
//! here), one durable import job runs on the native worker and commits every file as its own
//! object with its record; the same operation ID answers the same job; a cancelled dialog makes
//! no job; a library that is not open is refused; a job the worker could not take runs at the
//! next launch; and no answer or journal entry names a path.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/import_rig.rs"]
#[allow(dead_code)]
mod import_rig;

use filesquid_api::imports::IMPORT_START_OPERATION;
use import_rig::{fresh_dir, holiday_folder, ImportRig, LIBRARY, UNOPENED_LIBRARY};
use journal::block_on;
use serde_json::json;

#[test]
fn a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job() {
    let dir = fresh_dir("folder");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-import-1", "folder"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["outcome"], "started");
    let job = answer["job_id"].as_str().expect("job").to_string();
    let (status, again) = block_on(rig.import(LIBRARY, "op-import-1", "folder"));
    assert_eq!(status, 200, "{again}");
    assert_eq!(
        again["job_id"],
        job.as_str(),
        "the same operation answers the same job"
    );
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    assert_eq!(rig.wait_for_files(2), 2, "each file imported");
    rig.stop_worker();
    assert_eq!(rig.imported().len(), 2, "each file imported once");
    let text = format!("{:?}", rig.journal());
    assert!(
        !text.contains("Beach day") && !text.contains("Holidays"),
        "no names"
    );
    assert_eq!(rig.file_objects(), 2, "one object per file");
}

#[test]
fn a_cancelled_dialog_or_an_unopened_library_makes_no_job() {
    let rig = block_on(ImportRig::launch(fresh_dir("cancel"), None));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-cancel", "files"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer,
        json!({ "operation_id": "op-cancel", "outcome": "cancelled" })
    );
    let (status, _) = block_on(rig.import(UNOPENED_LIBRARY, "op-other", "files"));
    assert_eq!(status, 404, "that library is not open");
    rig.stop_worker();
    let starts = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == IMPORT_START_OPERATION)
        .count();
    assert_eq!(starts, 2, "each request journaled once");
    assert_eq!(rig.file_objects(), 0, "nothing imported");
}

#[test]
fn a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch() {
    let dir = fresh_dir("relaunch");
    let folder = holiday_folder(&dir);
    let mut rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    rig.stop_worker();
    let (status, answer) = block_on(rig.import(LIBRARY, "op-later", "folder"));
    assert_eq!(status, 503, "{answer}");
    assert_eq!(rig.file_objects(), 0, "nothing imported yet");
    block_on(rig.relaunch());
    assert_eq!(rig.wait_for_files(2), 2, "the kept job ran at the launch");
    let (status, again) = block_on(rig.import(LIBRARY, "op-later", "folder"));
    assert_eq!(status, 200, "{again}");
    assert_eq!(again["outcome"], "started", "the operation names its job");
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    rig.stop_worker();
    assert_eq!(rig.file_objects(), 2);
}
