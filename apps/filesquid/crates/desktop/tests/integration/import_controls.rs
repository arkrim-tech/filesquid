//! Task DE-1.9: the job routes and the import job controls on the desktop (DE-S-020, DE-S-019,
//! API-S-037) through the `filesquid://` scheme with the per-launch token. An import of three
//! files runs on the native worker, held by the rig's gate on the first file after its commit;
//! the controls arrive while that file is in hand. A pause suspends the job after that file
//! with its checkpoint and the launch-time replay leaves it paused; a resume hands it to the
//! import runner, which finishes the same manifest without reading the finished file again; a
//! cancel ends it cancelled with no further file read; a resume that cleared the pause before
//! the checkpoint keeps the run going. Each control is journaled once, by the API; the event
//! bus the desktop event channel forwards carries every `job.update` of the job, in order,
//! with none dropped; and no answer, event or journal entry names a source path, file or
//! folder.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/import_rig.rs"]
#[allow(dead_code)]
mod import_rig;

use filesquid_api::events::topic::JOB_UPDATE;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::ApiEvent;
use import_rig::{fresh_dir, ImportRig, LIBRARY};
use journal::block_on;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast::Receiver;

/// Longest wait for a job to reach a state.
const JOB_WAIT: Duration = Duration::from_secs(30);

/// The names no answer, event or journal entry may carry.
const NAMES: [&str; 4] = ["Trip", "Alpha day", "Bravo night", "Charlie noon"];

/// A folder `<dir>/Trip` with three files, listed in this order (filesystem I/O).
fn trip_folder(dir: &Path) -> PathBuf {
    let folder = dir.join("Trip");
    std::fs::create_dir_all(&folder).expect("folder");
    for (name, bytes) in [
        ("Alpha day.jpg", b"alpha".as_slice()),
        ("Bravo night.jpg", b"bravo".as_slice()),
        ("Charlie noon.jpg", b"charlie".as_slice()),
    ] {
        std::fs::write(folder.join(name), bytes).expect("file");
    }
    folder
}

/// The `job.update` events of one job, collected as they arrive.
struct Updates {
    receiver: Receiver<ApiEvent>,
    seen: Vec<Value>,
}

impl Updates {
    /// Collect from `receiver`.
    fn new(receiver: Receiver<ApiEvent>) -> Self {
        Self {
            receiver,
            seen: Vec::new(),
        }
    }

    /// Take every event waiting, failing the test when the bus dropped any.
    fn drain(&mut self) {
        for _ in 0..filesquid_api::events::EVENT_BUFFER * 4 {
            match self.receiver.try_recv() {
                Ok(event) if event.topic == JOB_UPDATE => self.seen.push(event.data),
                Ok(_) => {}
                Err(TryRecvError::Lagged(missed)) => panic!("{missed} events were dropped"),
                Err(TryRecvError::Empty | TryRecvError::Closed) => return,
            }
        }
    }

    /// The (state, control, done) of every update of `job`, in order.
    fn of(&mut self, job: &str) -> Vec<(String, Option<String>, u64)> {
        self.drain();
        self.seen
            .iter()
            .filter(|view| view["id"] == job)
            .map(|view| {
                (
                    view["state"].as_str().expect("state").to_string(),
                    view["control"].as_str().map(str::to_string),
                    view["done"].as_u64().expect("done"),
                )
            })
            .collect()
    }
}

/// One expected update. Pure.
fn update(state: &str, control: Option<&str>, done: u64) -> (String, Option<String>, u64) {
    (state.to_string(), control.map(str::to_string), done)
}

/// Start the import of the picked folder under `operation`, held on its first file; answer
/// the job ID once the worker holds that file.
fn start_held(rig: &ImportRig, operation: &str) -> String {
    rig.gate.arm();
    let (status, answer) = block_on(rig.import(LIBRARY, operation, "folder"));
    assert_eq!(status, 200, "{answer}");
    assert!(rig.gate.wait_held(), "the worker holds the first file");
    answer["job_id"].as_str().expect("job").to_string()
}

/// POST the control `control` of `job`, asserting it is answered.
fn control(rig: &ImportRig, job: &str, control: &str) -> Value {
    let (status, answer) = block_on(rig.post(&format!("/api/v1/jobs/{job}/{control}")));
    assert_eq!(status, 200, "{control}: {answer}");
    answer
}

/// The job view of `job` once `done` holds for it (at most [`JOB_WAIT`]), draining `updates`
/// meanwhile.
fn wait_for(
    rig: &ImportRig,
    job: &str,
    updates: &mut Updates,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + JOB_WAIT;
    loop {
        let (status, view) = block_on(rig.get(&format!("/api/v1/jobs/{job}")));
        assert_eq!(status, 200, "{view}");
        updates.drain();
        if done(&view) || Instant::now() > deadline {
            return view;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The journal entries of `operation` for the job `job`.
fn entries(rig: &ImportRig, operation: &str, job: &str) -> Vec<JournalEntry> {
    rig.journal()
        .into_iter()
        .filter(|e| e.operation == operation && e.target_ids.iter().any(|t| t == job))
        .collect()
}

/// The imported files, and the files quarantined, of the journal.
fn file_outcomes(rig: &ImportRig) -> (usize, usize) {
    let files = rig.imported();
    let failed = files
        .iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .count();
    (files.len() - failed, failed)
}

/// Assert that `text` names no source file or folder.
fn assert_unnamed(text: &str) {
    for name in NAMES {
        assert!(!text.contains(name), "{name} in {text}");
    }
}

#[test]
fn a_paused_import_stops_after_the_file_in_hand_and_resumes_without_reading_it_again() {
    let dir = fresh_dir("pause");
    let folder = trip_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder.clone()])));
    let mut updates = Updates::new(rig.subscribe());
    let job = start_held(&rig, "op-pause");
    let (status, listed) = block_on(rig.get("/api/v1/jobs?kind=import"));
    assert_eq!(status, 200, "the job routes answer, not 503: {listed}");
    assert_eq!(listed["jobs"][0]["id"], job.as_str());
    assert_eq!(listed["jobs"][0]["state"], "running");
    let paused = control(&rig, &job, "pause");
    assert_eq!(paused["job"]["control"], "pause", "{paused}");
    assert_eq!(
        paused["job"]["state"], "running",
        "asked at the next checkpoint"
    );
    rig.gate.release();
    let view = wait_for(&rig, &job, &mut updates, |v| v["paused"] == true);
    assert_eq!(view["state"], "pending", "{view}");
    assert_eq!(view["done"], 1, "checkpoint after the file in hand");
    assert_eq!(view["total"], 3);
    assert_eq!(file_outcomes(&rig), (1, 0), "only the file in hand");
    std::fs::remove_file(folder.join("Alpha day.jpg")).expect("finished file removed");
    let resumed = control(&rig, &job, "resume");
    assert_eq!(resumed["scheduled"], true, "handed to the import runner");
    let view = wait_for(&rig, &job, &mut updates, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    assert_eq!(view["done"], 3);
    rig.stop_worker();
    assert_eq!(
        file_outcomes(&rig),
        (3, 0),
        "the same manifest, the finished file not read again"
    );
    assert_eq!(rig.file_objects(), 3);
    assert_eq!(
        updates.of(&job),
        [
            update("pending", None, 0),
            update("running", None, 0),
            update("running", Some("pause"), 0),
            update("running", Some("pause"), 1),
            update("pending", Some("pause"), 1),
            update("pending", None, 1),
            update("running", None, 1),
            update("running", None, 2),
            update("running", None, 3),
            update("completed", None, 3),
        ],
        "every stored change, in order"
    );
    assert_eq!(entries(&rig, "job.paused", &job).len(), 1);
    assert_eq!(entries(&rig, "job.resumed", &job).len(), 1);
    assert_eq!(entries(&rig, "job.suspended", &job).len(), 1);
    let refused = entries(&rig, "job.refused", &job);
    assert!(refused.is_empty(), "{refused:?}");
    assert_unnamed(&format!("{:?}", rig.journal()));
    assert_unnamed(&format!("{:?}", updates.seen));
}

#[test]
fn a_cancelled_import_ends_after_the_file_in_hand_with_no_further_file_read() {
    let dir = fresh_dir("cancel-job");
    let folder = trip_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let mut updates = Updates::new(rig.subscribe());
    let job = start_held(&rig, "op-cancel-job");
    let asked = control(&rig, &job, "cancel");
    assert_eq!(asked["job"]["control"], "cancel", "{asked}");
    rig.gate.release();
    let view = wait_for(&rig, &job, &mut updates, |v| v["state"] == "cancelled");
    assert_eq!(view["state"], "cancelled", "{view}");
    assert_eq!(view["done"], 1);
    rig.stop_worker();
    assert_eq!(file_outcomes(&rig), (1, 0), "no further file read");
    assert_eq!(rig.file_objects(), 1);
    assert_eq!(
        updates.of(&job),
        [
            update("pending", None, 0),
            update("running", None, 0),
            update("running", Some("cancel"), 0),
            update("running", Some("cancel"), 1),
            update("cancelled", None, 1),
        ]
    );
    assert_eq!(entries(&rig, "job.cancel_requested", &job).len(), 1);
    assert_eq!(entries(&rig, "job.cancelled", &job).len(), 1);
    let (status, refused) = block_on(rig.post(&format!("/api/v1/jobs/{job}/resume")));
    assert_eq!(status, 409, "a finished job refuses a resume: {refused}");
    assert_unnamed(&format!("{:?}", rig.journal()));
}

#[test]
fn a_resume_that_clears_the_pause_before_the_checkpoint_keeps_the_run_going() {
    let dir = fresh_dir("resume-early");
    let folder = trip_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let mut updates = Updates::new(rig.subscribe());
    let job = start_held(&rig, "op-resume-early");
    control(&rig, &job, "pause");
    let resumed = control(&rig, &job, "resume");
    assert_eq!(resumed["job"]["state"], "running", "{resumed}");
    assert_eq!(
        resumed["scheduled"], false,
        "still running, not handed over"
    );
    rig.gate.release();
    let view = wait_for(&rig, &job, &mut updates, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    assert_eq!(view["attempts"], 1, "one run");
    rig.stop_worker();
    assert_eq!(file_outcomes(&rig), (3, 0));
    assert_eq!(
        entries(&rig, "job.suspended", &job).len(),
        0,
        "never suspended"
    );
    assert_eq!(entries(&rig, "job.paused", &job).len(), 1);
    assert_eq!(entries(&rig, "job.resumed", &job).len(), 1);
}

#[test]
fn a_paused_import_is_not_started_by_the_next_launch_until_it_is_resumed() {
    let dir = fresh_dir("pause-relaunch");
    let folder = trip_folder(&dir);
    let mut rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let mut updates = Updates::new(rig.subscribe());
    let job = start_held(&rig, "op-pause-relaunch");
    control(&rig, &job, "pause");
    rig.gate.release();
    wait_for(&rig, &job, &mut updates, |v| v["paused"] == true);
    block_on(rig.relaunch());
    let mut updates = Updates::new(rig.subscribe());
    std::thread::sleep(Duration::from_millis(1_500));
    let (status, view) = block_on(rig.get(&format!("/api/v1/jobs/{job}")));
    assert_eq!(status, 200, "{view}");
    assert_eq!(view["paused"], true, "the replay left it paused: {view}");
    assert_eq!(view["attempts"], 1, "never started again");
    assert!(
        entries(&rig, "job.refused", &job).is_empty(),
        "not even tried"
    );
    assert!(entries(&rig, "import.job", &job).is_empty());
    let resumed = control(&rig, &job, "resume");
    assert_eq!(resumed["scheduled"], true, "{resumed}");
    let view = wait_for(&rig, &job, &mut updates, |v| v["state"] == "completed");
    assert_eq!(view["state"], "completed", "{view}");
    rig.stop_worker();
    assert_eq!(file_outcomes(&rig), (3, 0));
    assert_eq!(rig.file_objects(), 3);
}
