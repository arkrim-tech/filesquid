//! Task API-F1.6: the job routes (API-S-036; NFR-003, ADR 007) through the router on the
//! desktop owner's credential. A worker the host's runner starts is paused through
//! `POST /jobs/{job}/pause`, suspends at its next checkpoint, and `POST /jobs/{job}/resume`
//! runs it again from that checkpoint: every unit of work is done exactly once.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::jobs::{Halt, Job, JobError, JobRunner, JobSpec, Jobs, MemoryJobStore};
use filesquid_api::journal::MemoryJournal;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

/// The kind of the test's jobs.
const KIND: &str = "count";

/// Units of work each job does.
const UNITS: u64 = 6;

/// The unit after which the first run waits for the test to ask for a pause.
const PAUSE_AFTER: u64 = 3;

/// The unit numbers every run did, in order.
type Done = Arc<Mutex<Vec<u64>>>;

/// A runner that does units `next..=UNITS` on a thread, checkpointing after each. After
/// [`PAUSE_AFTER`] in the first run it waits until the test has asked for a pause, and it
/// halts at the first checkpoint that brings a request still standing.
struct CountRunner {
    jobs: Mutex<Option<Jobs>>,
    done: Done,
    gate: Mutex<Option<mpsc::Receiver<()>>>,
    finished: Mutex<Option<mpsc::Sender<()>>>,
}

impl CountRunner {
    fn jobs(&self) -> Jobs {
        self.jobs.lock().unwrap().clone().expect("jobs")
    }
}

impl JobRunner for CountRunner {
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        let jobs = self.jobs();
        let (id, source) = (job.id.clone(), job.source.clone());
        let done = self.done.clone();
        let gate = self.gate.lock().unwrap().take();
        let finished = self.finished.lock().unwrap().clone().expect("sender");
        std::thread::spawn(move || {
            let mut running = jobs.start(&id, &source).expect("start");
            let first: u64 = running
                .checkpoint_text()
                .and_then(|text| text.parse().ok())
                .unwrap_or(1);
            for unit in first..=UNITS {
                done.lock().unwrap().push(unit);
                running
                    .checkpoint(&(unit + 1).to_string(), unit, Some(UNITS))
                    .expect("checkpoint");
                if unit == PAUSE_AFTER {
                    if let Some(gate) = &gate {
                        gate.recv().expect("the test asked for a pause");
                    }
                }
                if unit > PAUSE_AFTER && running.requested().is_some() {
                    match running.halt_if_requested().expect("halt") {
                        Halt::Halted(_) => {
                            finished.send(()).expect("report");
                            return;
                        }
                        Halt::Continued(kept) => running = *kept,
                    }
                }
            }
            running.complete().expect("complete");
            finished.send(()).expect("report");
        });
        Ok(())
    }
}

/// The desktop host's state with a job service, the counting runner and its token.
struct Host {
    state: ApiState,
    token: String,
    jobs: Jobs,
    runner: Arc<CountRunner>,
    _monitor: Monitor,
}

impl Host {
    fn new(gate: mpsc::Receiver<()>, finished: mpsc::Sender<()>) -> Self {
        let token = LaunchToken::generate().expect("token");
        let secret = token.secret().to_string();
        let state = ApiState::new();
        assert!(state.install_authoriser(Authoriser::desktop_owner(&token)));
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("desktop")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("desktop"),
            state.events.clone(),
        );
        let jobs = Jobs::new(
            Arc::new(MemoryJobStore::new()),
            monitor.journal(),
            monitor.registry().clone(),
            "launch-a",
        )
        .with_events(state.events.clone());
        assert!(state.install_jobs(Arc::new(jobs.clone())));
        let runner = Arc::new(CountRunner {
            jobs: Mutex::new(Some(jobs.clone())),
            done: Arc::new(Mutex::new(Vec::new())),
            gate: Mutex::new(Some(gate)),
            finished: Mutex::new(Some(finished)),
        });
        assert!(state.install_job_runner(KIND, runner.clone()));
        Self {
            state,
            token: secret,
            jobs,
            runner,
            _monitor: monitor,
        }
    }

    async fn call(&self, method: &str, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {}", self.token))
            .body(Body::empty())
            .expect("request");
        let response = router(self.state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }
}

/// Wait (at most 10 s) until the worker has started job `id`.
fn wait_running(jobs: &Jobs, id: &str) {
    for _ in 0..1000 {
        if jobs.job(id).expect("job").state == filesquid_api::jobs::JobState::Running {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the worker never started the job");
}

fn wait(finished: &mpsc::Receiver<()>) {
    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("the run ended");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_interrupted_job_resumes_without_repeating_completed_work() {
    let (open_gate, gate) = mpsc::channel();
    let (finished_tx, finished) = mpsc::channel();
    let host = Host::new(gate, finished_tx);
    let job = host
        .jobs
        .submit(JobSpec {
            kind: KIND.into(),
            operation_id: "op-count".into(),
            source: "count:6".into(),
        })
        .expect("submit");
    host.runner.resume(&job).expect("first run");
    let id = job.id.clone();
    wait_running(&host.jobs, &id);
    let (status, paused) = host.call("POST", &format!("/api/v1/jobs/{id}/pause")).await;
    assert_eq!(status, StatusCode::OK, "{paused}");
    assert_eq!(paused["job"]["control"], "pause");
    open_gate.send(()).expect("let the worker go on");
    wait(&finished);
    let (_, held) = host.call("GET", &format!("/api/v1/jobs/{id}")).await;
    assert_eq!(held["state"], "pending");
    assert_eq!(held["paused"], true);
    assert_eq!(held["done"], PAUSE_AFTER + 1);
    let (status, resumed) = host
        .call("POST", &format!("/api/v1/jobs/{id}/resume"))
        .await;
    assert_eq!(status, StatusCode::OK, "{resumed}");
    assert_eq!(resumed["scheduled"], true);
    wait(&finished);
    let (_, done) = host.call("GET", &format!("/api/v1/jobs/{id}")).await;
    assert_eq!(done["state"], "completed");
    let units = host.runner.done.lock().unwrap().clone();
    assert_eq!(units, (1..=UNITS).collect::<Vec<_>>(), "each unit once");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_routes_list_answer_and_refuse_jobs() {
    let (_open_gate, gate) = mpsc::channel();
    let (finished_tx, _finished) = mpsc::channel();
    let host = Host::new(gate, finished_tx);
    let job = host
        .jobs
        .submit(JobSpec {
            kind: KIND.into(),
            operation_id: "op-list".into(),
            source: "count:6".into(),
        })
        .expect("submit");
    let (status, page) = host.call("GET", "/api/v1/jobs?kind=count").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["jobs"][0]["id"], job.id.as_str());
    assert_eq!(page["truncated"], false);
    let (status, missing) = host.call("GET", "/api/v1/jobs/no-such-job").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["code"], "not_found");
    let (status, cancelled) = host
        .call("POST", &format!("/api/v1/jobs/{}/cancel", job.id))
        .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["job"]["state"], "cancelled");
    let (status, refused) = host
        .call("POST", &format!("/api/v1/jobs/{}/resume", job.id))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(refused["code"], "conflict");
    for query in ["state=sleeping", "limit=0", "limit=1001", "kind="] {
        let (status, bad) = host.call("GET", &format!("/api/v1/jobs?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {bad}");
    }
    let long = "k".repeat(129);
    let (status, _) = host.call("GET", &format!("/api/v1/jobs?kind={long}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, capped) = host.call("GET", "/api/v1/jobs?limit=1000").await;
    assert_eq!(status, StatusCode::OK, "{capped}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_without_a_runner_stays_pending_for_the_next_launch() {
    let (_open_gate, gate) = mpsc::channel();
    let (finished_tx, _finished) = mpsc::channel();
    let host = Host::new(gate, finished_tx);
    let job = host
        .jobs
        .submit(JobSpec {
            kind: "no-runner".into(),
            operation_id: "op-idle".into(),
            source: "idle".into(),
        })
        .expect("submit");
    host.jobs.pause(&job.id).expect("pause");
    let (status, resumed) = host
        .call("POST", &format!("/api/v1/jobs/{}/resume", job.id))
        .await;
    assert_eq!(status, StatusCode::OK, "{resumed}");
    assert_eq!(resumed["scheduled"], false);
    assert_eq!(resumed["job"]["state"], "pending");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_job_routes_need_a_credential() {
    let (_open_gate, gate) = mpsc::channel();
    let (finished_tx, _finished) = mpsc::channel();
    let host = Host::new(gate, finished_tx);
    let request = Request::builder()
        .uri("/api/v1/jobs")
        .body(Body::empty())
        .expect("request");
    let response = router(host.state.clone())
        .oneshot(request)
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
