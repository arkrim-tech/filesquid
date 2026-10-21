//! Step definitions for `job_routes_NFR-003.feature` (Task API-F1.6; API-S-036): a desktop
//! owner's router with a job service over the in-memory store, controls sent through the job
//! routes, and a counting worker driven step by step (its runner, for a resumed job, runs the
//! rest of the job at once).

use crate::MonitorWorld;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::events::topic;
use filesquid_api::jobs::{
    Job, JobClass, JobControl, JobError, JobLimits, JobRunner, JobSpec, JobState, Jobs,
    MemoryJobStore, RunningJob,
};
use filesquid_api::journal::MemoryJournal;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiEvent, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// The units each counting worker did, in order.
type Done = Arc<Mutex<Vec<u64>>>;

/// The counting runner: a resumed job runs from its checkpoint to its end.
struct Counter {
    jobs: Jobs,
    done: Done,
}

impl JobRunner for Counter {
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        let mut running = self.jobs.start(&job.id, &job.source)?;
        let total = running.job().total.unwrap_or(0);
        let first = next_unit(&running);
        for unit in first..=total {
            work(&mut running, &self.done, unit, total)?;
        }
        running.complete()
    }
}

/// The unit a run starts at: the one after its checkpoint, or the first. Pure.
fn next_unit(running: &RunningJob) -> u64 {
    running
        .checkpoint_text()
        .and_then(|text| text.parse().ok())
        .unwrap_or(1)
}

/// Do `unit` of `total` and checkpoint it.
fn work(running: &mut RunningJob, done: &Done, unit: u64, total: u64) -> Result<(), JobError> {
    done.lock().unwrap().push(unit);
    running.checkpoint(&(unit + 1).to_string(), unit, Some(total))
}

/// Per-scenario state of the job steps.
#[derive(Default)]
pub struct JobWorld {
    state: Option<ApiState>,
    token: String,
    jobs: Option<Jobs>,
    store: Option<Arc<MemoryJobStore>>,
    monitor: Option<Monitor>,
    done: Done,
    job: Option<Job>,
    running: Option<RunningJob>,
    capped: Option<Jobs>,
    transfers: Vec<Job>,
    events: Option<tokio::sync::broadcast::Receiver<ApiEvent>>,
    refused: Option<JobError>,
}

impl std::fmt::Debug for JobWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobWorld").finish_non_exhaustive()
    }
}

impl Drop for JobWorld {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            monitor.shutdown();
        }
    }
}

impl JobWorld {
    fn jobs(&self) -> &Jobs {
        self.jobs.as_ref().expect("a job service")
    }

    fn job_id(&self) -> String {
        self.job.as_ref().expect("a job").id.clone()
    }

    fn stored(&self) -> Job {
        self.jobs().job(&self.job_id()).expect("job")
    }

    /// Send `POST /api/v1/jobs/{job}/{control}` with the owner's token.
    async fn control(&self, control: &str) -> (StatusCode, Value) {
        let uri = format!("/api/v1/jobs/{}/{control}", self.job_id());
        let request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", format!("Bearer {}", self.token))
            .body(Body::empty())
            .expect("request");
        let state = self.state.clone().expect("state");
        let response = router(state).oneshot(request).await.expect("answer");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn submit(&mut self, kind: &str, total: u64) -> Job {
        let spec = JobSpec {
            kind: kind.into(),
            operation_id: format!(
                "op-{}",
                self.transfers.len() + usize::from(self.job.is_some())
            ),
            source: format!("{kind}:source"),
        };
        let job = self.jobs().submit(spec).expect("submit");
        let job = self
            .store
            .as_ref()
            .expect("store")
            .update(&job.id, &mut |mut j| {
                j.total = Some(total);
                Ok(j)
            })
            .expect("total");
        job
    }
}

use filesquid_api::jobs::JobStore as _;

#[given("a host with a job service and a counting worker")]
fn host(w: &mut MonitorWorld) {
    let token = LaunchToken::generate().expect("token");
    let state = ApiState::new();
    assert!(state.install_authoriser(Authoriser::desktop_owner(&token)));
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("desktop")),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        state.events.clone(),
    );
    let store = Arc::new(MemoryJobStore::new());
    let jobs = Jobs::new(
        store.clone(),
        monitor.journal(),
        monitor.registry().clone(),
        "launch-a",
    )
    .with_events(state.events.clone());
    assert!(state.install_jobs(Arc::new(jobs.clone())));
    let world = &mut w.jobs;
    let counter = Counter {
        jobs: jobs.clone(),
        done: world.done.clone(),
    };
    assert!(state.install_job_runner("count", Arc::new(counter)));
    world.events = Some(state.events.subscribe());
    world.token = token.secret().to_string();
    world.state = Some(state);
    world.jobs = Some(jobs);
    world.store = Some(store);
    world.monitor = Some(monitor);
}

#[given(expr = "a counting job of {int} units that has done {int}")]
fn job_done_so_far(w: &mut MonitorWorld, total: u64, done: u64) {
    let world = &mut w.jobs;
    let job = world.submit("count", total);
    let mut running = world.jobs().start(&job.id, &job.source).expect("start");
    for unit in 1..=done {
        work(&mut running, &world.done, unit, total).expect("work");
    }
    world.job = Some(job);
    world.running = Some(running);
}

#[given(expr = "a counting job of {int} units that waits")]
fn waiting_job(w: &mut MonitorWorld, total: u64) {
    let job = w.jobs.submit("count", total);
    w.jobs.job = Some(job);
}

#[when(expr = "I {word} the job through the job routes")]
async fn control_job(w: &mut MonitorWorld, control: String) {
    let (status, answer) = w.jobs.control(&control).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
}

#[when("the worker reaches its next checkpoint")]
fn next_checkpoint(w: &mut MonitorWorld) {
    let world = &mut w.jobs;
    let mut running = world.running.take().expect("a running job");
    let total = running.job().total.unwrap_or(0);
    let unit = next_unit_after(&world.done);
    work(&mut running, &world.done, unit, total).expect("work");
    assert_eq!(running.requested(), Some(JobControl::Pause));
    running.suspend().expect("suspend");
}

/// The unit after the last one done. Pure.
fn next_unit_after(done: &Done) -> u64 {
    done.lock().unwrap().last().map_or(1, |last| last + 1)
}

#[then(expr = "the job should be paused with {int} units done")]
fn paused_with(w: &mut MonitorWorld, done: u64) {
    let job = w.jobs.stored();
    assert!(job.is_paused(), "{job:?}");
    assert_eq!(job.done, done);
}

#[then("the job should complete")]
fn completed(w: &mut MonitorWorld) {
    assert_eq!(w.jobs.stored().state, JobState::Completed);
}

#[then("the job should be cancelled")]
fn cancelled(w: &mut MonitorWorld) {
    assert_eq!(w.jobs.stored().state, JobState::Cancelled);
}

#[then("every unit should have been done exactly once")]
fn each_unit_once(w: &mut MonitorWorld) {
    let total = w.jobs.stored().total.unwrap_or(0);
    let done = w.jobs.done.lock().unwrap().clone();
    assert_eq!(done, (1..=total).collect::<Vec<_>>());
}

#[then("the worker should not be able to start it")]
fn cannot_start(w: &mut MonitorWorld) {
    let job = w.jobs.stored();
    let refused = w.jobs.jobs().start(&job.id, &job.source).expect_err("held");
    assert_eq!(refused, JobError::Conflict("job_paused"));
}

#[given(expr = "transfers are capped at {int}")]
fn capped(w: &mut MonitorWorld, cap: u32) {
    let jobs = w
        .jobs
        .jobs()
        .clone()
        .with_class("transfer", JobClass::Transfer)
        .with_limits(JobLimits {
            transfers: cap,
            ..JobLimits::default()
        });
    w.jobs.capped = Some(jobs);
}

#[given("two waiting transfer jobs")]
fn two_transfers(w: &mut MonitorWorld) {
    for _ in 0..2 {
        let job = w.jobs.submit("transfer", 1);
        w.jobs.transfers.push(job);
    }
}

#[when("the first transfer starts")]
fn first_transfer(w: &mut MonitorWorld) {
    let world = &mut w.jobs;
    let first = world.transfers[0].clone();
    let running = world
        .capped
        .as_ref()
        .expect("capped")
        .start(&first.id, &first.source)
        .expect("first");
    world.running = Some(running);
    let second = world.transfers[1].clone();
    world.refused = world
        .capped
        .as_ref()
        .expect("capped")
        .start(&second.id, &second.source)
        .err();
}

#[then(expr = "the second transfer should be refused as {string}")]
fn second_refused(w: &mut MonitorWorld, code: String) {
    let refused = w.jobs.refused.clone().expect("refused");
    assert_eq!(refused.error_code(), code);
}

#[then("the second transfer should still wait")]
fn second_waits(w: &mut MonitorWorld) {
    let second = w.jobs.transfers[1].id.clone();
    assert_eq!(
        w.jobs.jobs().job(&second).expect("job").state,
        JobState::Pending
    );
}

#[then(expr = "a {string} event should show the job paused")]
fn event_paused(w: &mut MonitorWorld, name: String) {
    assert_eq!(name, topic::JOB_UPDATE);
    let id = w.jobs.job_id();
    let events = drain(&mut w.jobs);
    let paused = events
        .iter()
        .filter(|e| e.topic == topic::JOB_UPDATE && e.data["id"] == id.as_str())
        .any(|e| e.data["paused"] == true);
    assert!(paused, "{events:?}");
}

#[then("no event should carry the job's source")]
fn no_source(w: &mut MonitorWorld) {
    let source = w.jobs.stored().source;
    let events = drain(&mut w.jobs);
    for event in events {
        assert!(!event.data.to_string().contains(&source), "{event:?}");
    }
}

/// Every event received so far, kept for later steps of the scenario.
fn drain(world: &mut JobWorld) -> Vec<ApiEvent> {
    let feed = world.events.as_mut().expect("subscribed");
    let mut seen = Vec::new();
    while let Ok(event) = feed.try_recv() {
        seen.push(event);
    }
    seen
}

#[then("pausing, resuming and cancelling it again should each be refused as a conflict")]
async fn every_control_refused(w: &mut MonitorWorld) {
    for control in ["pause", "resume", "cancel"] {
        let (status, answer) = w.jobs.control(control).await;
        assert_eq!(status, StatusCode::CONFLICT, "{control}: {answer}");
        assert_eq!(answer["code"], "conflict");
    }
}
