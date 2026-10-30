//! The job routes in the contract test (Task API-F1.6; API-S-037): a job service over the
//! owner state's monitor, with one known pending job per job path, and samples of the job
//! schemas.

use filesquid_api::jobs::{
    Job, JobControl, JobControlAnswer, JobDestination, JobItem, JobItemPage, JobItemState, JobPage,
    JobState, JobStore, JobView, Jobs, MemoryJobItems, MemoryJobStore,
};
use filesquid_api::monitor::Monitor;
use filesquid_api::{ApiEvent, ApiState};
use serde_json::Value;
use std::sync::Arc;

/// The job each job path names: one per control and retry, so cancelling one cannot refuse
/// another; the item of the one-file retry is [`FAILED_ITEM`].
pub fn concrete_job(path: &str) -> String {
    let id = ["pause", "resume", "cancel", "retry", "retry-failed"]
        .into_iter()
        .find(|control| path.ends_with(&format!("/{control}")))
        .map_or("sample-job".to_string(), |control| {
            format!("sample-job-{control}")
        });
    path.replace("{job}", &id).replace("{item}", FAILED_ITEM)
}

/// The failed file of every sample upload job.
pub const FAILED_ITEM: &str = "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f13";

/// The destination of every sample upload job.
fn destination() -> JobDestination {
    JobDestination {
        collection_id: "2c0d1e2f-3a4b-4c5d-8e6f-7a8b9c0d1e2f".into(),
        folder: Some("Site photos".into()),
    }
}

/// The files of every sample upload job: one copied, one copying and one failed.
fn files() -> Vec<JobItem> {
    let file = |id: &str, name: &str, state, done, code: Option<&str>| JobItem {
        item_id: id.into(),
        name: name.into(),
        folder: Some("Site photos".into()),
        state,
        bytes_done: done,
        bytes_total: 6_400_000,
        error_code: code.map(str::to_string),
    };
    vec![
        file(
            "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11",
            "IMG_4101.JPG",
            JobItemState::Copied,
            6_400_000,
            None,
        ),
        file(
            "0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f12",
            "IMG_4102.CR3",
            JobItemState::Copying,
            3_968_000,
            None,
        ),
        file(
            FAILED_ITEM,
            "IMG_4099.JPG",
            JobItemState::Failed,
            0,
            Some("source_changed"),
        ),
    ]
}

/// A pending job `id`. Pure.
fn pending(id: &str) -> Job {
    Job {
        id: id.into(),
        kind: "import".into(),
        operation_id: format!("op-{id}"),
        source: "import:library:digest".into(),
        state: JobState::Pending,
        checkpoint: None,
        done: 0,
        total: Some(3),
        attempts: 0,
        fence: 0,
        holder: None,
        lease_expires_ms: None,
        created_ms: 1_760_000_000_000,
        updated_ms: 1_760_000_000_000,
        error_code: None,
        control: None,
        result: None,
    }
}

/// Install a job service holding the jobs [`concrete_job`] names.
pub fn install(state: &ApiState, monitor: &Monitor) {
    let store = Arc::new(MemoryJobStore::new());
    let items = Arc::new(MemoryJobItems::new());
    for suffix in [
        "",
        "-pause",
        "-resume",
        "-cancel",
        "-retry",
        "-retry-failed",
    ] {
        let id = format!("sample-job{suffix}");
        store.insert(&pending(&id)).expect("job");
        items.put(&id, Some(destination()), files());
    }
    let jobs = Jobs::new(
        store,
        monitor.journal(),
        monitor.registry().clone(),
        "launch",
    )
    .with_events(state.events.clone())
    .with_items(items);
    assert!(state.install_jobs(Arc::new(jobs)));
}

/// The `job.update` event a job service with an event bus publishes at a submission.
pub async fn job_update_event() -> Option<ApiEvent> {
    let bus = filesquid_api::EventBus::new();
    let mut sub = bus.subscription().expect("slot");
    let journal = Arc::new(filesquid_api::journal::MemoryJournal::new("host"));
    let monitor = Monitor::start(
        journal,
        Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        filesquid_api::monitor::MonitorConfig::new("host"),
        filesquid_api::EventBus::new(),
    );
    let jobs = Jobs::new(
        Arc::new(MemoryJobStore::new()),
        monitor.journal(),
        monitor.registry().clone(),
        "launch",
    )
    .with_events(bus);
    let spec = filesquid_api::jobs::JobSpec {
        kind: "import".into(),
        operation_id: "op-1".into(),
        source: "import:library:digest".into(),
    };
    jobs.submit(spec).expect("submit");
    let event = sub.next().await;
    monitor.shutdown();
    event
}

/// Serialized samples of the job schemas.
pub fn samples() -> Vec<(&'static str, Vec<Value>)> {
    let mut paused = pending("paused");
    paused.control = Some(JobControl::Pause);
    let mut failed = pending("failed");
    failed.state = JobState::Failed;
    failed.error_code = Some("source_changed".into());
    let mut rebuilt = pending("rebuilt");
    rebuilt.kind = "library.rebuild".into();
    rebuilt.state = JobState::Completed;
    rebuilt.result = Some(
        r#"{"objects":[{"object_id":"urn:uuid:a","head":"v1","files":1,"digest_algorithm":"sha512","error_codes":[],"missing_version_inventory":false}],"unreadable":[{"error_code":"E034"}]}"#
            .into(),
    );
    let views = [
        JobView::of(&pending("job")).with_destination(Some(destination())),
        JobView::of(&paused),
        JobView::of(&failed),
        JobView::of(&rebuilt),
    ];
    let item_states = [
        JobItemState::Queued,
        JobItemState::Copying,
        JobItemState::Copied,
        JobItemState::Skipped,
        JobItemState::Failed,
    ];
    let states = [
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
        JobState::Cancelled,
    ];
    vec![
        (
            "JobDestination",
            vec![
                json(&destination()),
                json(&JobDestination {
                    folder: None,
                    ..destination()
                }),
            ],
        ),
        ("JobItemState", item_states.iter().map(json).collect()),
        ("JobItem", files().iter().map(json).collect()),
        (
            "JobItemPage",
            vec![
                json(&JobItemPage {
                    items: files(),
                    next_cursor: Some("3".into()),
                }),
                json(&JobItemPage {
                    items: Vec::new(),
                    next_cursor: None,
                }),
            ],
        ),
        ("JobView", views.iter().map(json).collect()),
        ("JobState", states.iter().map(json).collect()),
        (
            "JobControl",
            [JobControl::Pause, JobControl::Cancel]
                .iter()
                .map(json)
                .collect(),
        ),
        (
            "JobPage",
            vec![json(&JobPage {
                jobs: views.to_vec(),
                truncated: true,
            })],
        ),
        (
            "JobControlAnswer",
            vec![json(&JobControlAnswer {
                job: views[1].clone(),
                scheduled: false,
            })],
        ),
    ]
}

/// `value` as JSON.
fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("json")
}
