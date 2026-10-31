//! Task DE-1.8: the review of an import and its per-file progress on the desktop (API-S-036
//! **Review**, API-S-037 **Per-file progress**, DE-S-019) through the `filesquid://` scheme
//! with the per-launch token. After the scripted dialog the host answers a plan and stages
//! nothing; only a confirmed plan whose source is unchanged starts a job; a discarded plan
//! starts nothing; files the library already holds are counted by digest and skipped; a
//! file's failure is kept with its code and a retry imports that file alone, without reading
//! a finished file again; a job waiting for a retry is not started by the next launch; and no
//! answer names a path.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/import_rig.rs"]
#[allow(dead_code)]
mod import_rig;

use import_rig::{fresh_dir, holiday_folder, ImportRig, LIBRARY};
use journal::block_on;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Longest wait for a job to reach a state.
const JOB_WAIT: Duration = Duration::from_secs(30);

/// The staging limit every plan states: 10 GiB.
const STAGING_LIMIT: u64 = 10 * 1024 * 1024 * 1024;

/// The import jobs the job routes list.
fn import_jobs(rig: &ImportRig) -> Vec<Value> {
    let (status, page) = block_on(rig.get("/api/v1/jobs?kind=import"));
    assert_eq!(status, 200, "{page}");
    page["jobs"].as_array().expect("jobs").clone()
}

/// The job view of `job` once `done` holds for it (at most [`JOB_WAIT`]).
fn wait_for(rig: &ImportRig, job: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + JOB_WAIT;
    loop {
        let (status, view) = block_on(rig.get(&format!("/api/v1/jobs/{job}")));
        assert_eq!(status, 200, "{view}");
        if done(&view) || Instant::now() > deadline {
            return view;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The files of job `job` as the items route answers them.
fn job_items(rig: &ImportRig, job: &str) -> Vec<Value> {
    let (status, page) = block_on(rig.get(&format!("/api/v1/jobs/{job}/items")));
    assert_eq!(status, 200, "{page}");
    page["items"].as_array().expect("items").clone()
}

/// The plan ID of a `planned` answer.
fn plan_id(answer: &Value) -> String {
    assert_eq!(answer["import"], "planned", "{answer}");
    answer["plan_id"].as_str().expect("plan id").to_string()
}

#[test]
fn a_picked_folder_is_planned_with_nothing_staged_and_started_only_when_confirmed() {
    let dir = fresh_dir("review");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.plan(LIBRARY, "op-review", "folder"));
    assert_eq!(status, 200, "{answer}");
    let plan = plan_id(&answer);
    let facts = &answer["plan"];
    assert_eq!(facts["files_found"], 2, "{facts}");
    assert_eq!(facts["files_new"], 2);
    assert_eq!(facts["files_present"], 0);
    assert_eq!(facts["bytes_to_copy"], 9, "jpeg and notes");
    assert_eq!(facts["names_numbered"], 0);
    assert_eq!(facts["staging_limit_bytes"], STAGING_LIMIT);
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(facts["free_bytes_after"].is_u64(), "{facts}");
    } else {
        assert!(facts["free_bytes_after"].is_null(), "{facts}");
    }
    assert!(
        import_jobs(&rig).is_empty(),
        "no job before the confirmation"
    );
    assert_eq!(rig.file_objects(), 0, "nothing staged or committed");
    let (status, items) = block_on(rig.get(&format!("/api/v1/imports/{plan}/items")));
    assert_eq!(status, 200, "{items}");
    let names: Vec<&str> = items["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["Beach day.jpg", "Notes.txt"], "base names only");
    let (status, again) = block_on(rig.plan(LIBRARY, "op-review", "folder"));
    assert_eq!(
        (status, plan_id(&again)),
        (200, plan.clone()),
        "the same plan"
    );
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    let (status, started) = block_on(rig.start(&plan));
    assert_eq!(status, 200, "{started}");
    assert_eq!(started["import"], "started");
    let job = started["job_id"].as_str().expect("job").to_string();
    let (_, repeated) = block_on(rig.start(&plan));
    assert_eq!(
        repeated["job_id"],
        job.as_str(),
        "a repeated start answers the job"
    );
    assert_eq!(rig.wait_for_files(2), 2);
    let view = wait_for(&rig, &job, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    rig.stop_worker();
    assert_eq!(rig.file_objects(), 2);
    assert_eq!(import_jobs(&rig).len(), 1, "one job");
}

#[test]
fn a_discarded_plan_starts_nothing_and_a_changed_source_is_refused() {
    let dir = fresh_dir("discard");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder.clone()])));
    let (_, answer) = block_on(rig.plan(LIBRARY, "op-discard", "folder"));
    let discarded = plan_id(&answer);
    let (status, gone) = block_on(rig.discard(&discarded));
    assert_eq!(status, 200, "{gone}");
    let (status, refused) = block_on(rig.start(&discarded));
    assert_eq!(status, 404, "plan_not_found: {refused}");
    let (_, answer) = block_on(rig.plan(LIBRARY, "op-changed", "folder"));
    let changed = plan_id(&answer);
    std::fs::write(folder.join("Notes.txt"), b"notes, longer now").expect("changed");
    let (status, refused) = block_on(rig.start(&changed));
    assert_eq!(status, 409, "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|m| m.contains("source_changed")),
        "{refused}"
    );
    rig.stop_worker();
    assert!(import_jobs(&rig).is_empty(), "no job");
    assert_eq!(rig.file_objects(), 0);
}

#[test]
fn files_the_library_already_holds_are_counted_by_digest_and_skipped() {
    let dir = fresh_dir("present");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, _) = block_on(rig.import(LIBRARY, "op-first", "folder"));
    assert_eq!(status, 200);
    assert_eq!(rig.wait_for_files(2), 2);
    let (_, answer) = block_on(rig.plan(LIBRARY, "op-second", "folder"));
    let facts = &answer["plan"];
    assert_eq!(facts["files_present"], 2, "{facts}");
    assert_eq!(facts["files_new"], 0);
    assert_eq!(facts["bytes_to_copy"], 0);
    let (_, started) = block_on(rig.start(&plan_id(&answer)));
    let job = started["job_id"].as_str().expect("job").to_string();
    let view = wait_for(&rig, &job, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    rig.stop_worker();
    let items = job_items(&rig, &job);
    let states: Vec<&str> = items
        .iter()
        .map(|item| item["state"].as_str().expect("state"))
        .collect();
    assert_eq!(states, ["skipped", "skipped"]);
    assert_eq!(rig.file_objects(), 2, "nothing copied twice");
}

#[test]
fn a_failed_file_keeps_its_code_and_is_retried_alone_after_a_restart() {
    let dir = fresh_dir("retry");
    let folder = holiday_folder(&dir);
    let mut rig = block_on(ImportRig::launch(dir, Some(vec![folder.clone()])));
    rig.gate.arm();
    let (_, answer) = block_on(rig.plan(LIBRARY, "op-retry", "folder"));
    let (_, started) = block_on(rig.start(&plan_id(&answer)));
    let job = started["job_id"].as_str().expect("job").to_string();
    assert!(rig.gate.wait_held(), "the first file is in hand");
    std::fs::remove_file(folder.join("Notes.txt")).expect("second file gone");
    rig.gate.release();
    let view = wait_for(&rig, &job, |v| v["done"] == 2);
    assert_eq!(view["state"], "pending", "waits for a retry: {view}");
    let items = job_items(&rig, &job);
    assert_eq!(items[0]["state"], "copied");
    assert_eq!(items[1]["state"], "failed");
    assert_eq!(items[1]["error_code"], "source_missing");
    block_on(rig.relaunch());
    std::thread::sleep(Duration::from_millis(1_000));
    let (_, view) = block_on(rig.get(&format!("/api/v1/jobs/{job}")));
    assert_eq!(view["attempts"], 1, "the launch did not start it: {view}");
    std::fs::write(folder.join("Notes.txt"), b"notes").expect("second file back");
    std::fs::remove_file(folder.join("Beach day.jpg")).expect("first file never read again");
    let item = items[1]["item_id"].as_str().expect("item");
    let path = format!("/api/v1/jobs/{job}/items/{item}/retry");
    let (status, retried) = block_on(rig.post(&path));
    assert_eq!(status, 200, "{retried}");
    assert_eq!(retried["scheduled"], true, "{retried}");
    let view = wait_for(&rig, &job, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    rig.stop_worker();
    let items = job_items(&rig, &job);
    assert_eq!(items[1]["state"], "copied", "{items:?}");
    assert_eq!(rig.file_objects(), 2);
    let text = format!("{:?}", rig.journal());
    for name in ["Notes", "Beach", "Holidays"] {
        assert!(!text.contains(name), "no source name journaled: {name}");
    }
}
