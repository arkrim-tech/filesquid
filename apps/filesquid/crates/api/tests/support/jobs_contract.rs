//! The job routes in the contract test (Task API-F1.6; API-S-037): a job service over the
//! owner state's monitor, with one known pending job per job path, and samples of the job
//! schemas.

use filesquid_api::jobs::{
    Job, JobControl, JobControlAnswer, JobPage, JobState, JobStore, JobView, Jobs, MemoryJobStore,
};
use filesquid_api::monitor::Monitor;
use filesquid_api::{ApiEvent, ApiState};
use serde_json::Value;
use std::sync::Arc;

/// The job each job path names: one per control, so cancelling one cannot refuse another.
pub fn concrete_job(path: &str) -> String {
    let id = ["pause", "resume", "cancel"]
        .into_iter()
        .find(|control| path.ends_with(&format!("/{control}")))
        .map_or("sample-job".to_string(), |control| {
            format!("sample-job-{control}")
        });
    path.replace("{job}", &id)
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
    }
}

/// Install a job service holding the jobs [`concrete_job`] names.
pub fn install(state: &ApiState, monitor: &Monitor) {
    let store = Arc::new(MemoryJobStore::new());
    for suffix in ["", "-pause", "-resume", "-cancel"] {
        store
            .insert(&pending(&format!("sample-job{suffix}")))
            .expect("job");
    }
    let jobs = Jobs::new(
        store,
        monitor.journal(),
        monitor.registry().clone(),
        "launch",
    )
    .with_events(state.events.clone());
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
    let views = [
        JobView::of(&pending("job")),
        JobView::of(&paused),
        JobView::of(&failed),
    ];
    let states = [
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
        JobState::Cancelled,
    ];
    vec![
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
