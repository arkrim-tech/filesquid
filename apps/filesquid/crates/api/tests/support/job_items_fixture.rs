//! The host of the per-file progress tests (API-S-037 **Per-file progress**): a job service
//! over memory stores with the per-file port and a recording runner for the `import` kind,
//! an owner token, and sample upload jobs with three files (copied, copying, failed).

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use filesquid_api::auth::{Action, Authoriser, Policy, Principal, Resource, TokenAuthenticator};
use filesquid_api::jobs::{
    Job, JobControl, JobDestination, JobError, JobItem, JobItemState, JobRunner, JobState,
    JobStore, Jobs, MemoryJobItems, MemoryJobStore,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub const TOKEN: &str = "items-token";
pub const UPLOAD: &str = "job-upload";
pub const COLLECTION: &str = "c0ffee00-1234-4abc-8def-0123456789ab";

/// A runner that records the jobs handed to it and takes none.
#[derive(Default)]
pub struct RecordingRunner {
    pub handed: Mutex<Vec<String>>,
}

impl JobRunner for RecordingRunner {
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        self.handed.lock().expect("handed").push(job.id.clone());
        Ok(())
    }
}

/// A policy that grants everything but reading jobs.
pub struct NoJobReads;

impl Policy for NoJobReads {
    fn permits(&self, _: &Principal, action: Action, _: &Resource) -> bool {
        action != Action::ReadJobs
    }
}

/// A host with a job service over memory stores, the per-file port and a recording runner
/// for the `import` kind.
pub struct Host {
    pub state: ApiState,
    pub monitor: Monitor,
    pub store: Arc<MemoryJobStore>,
    pub items: Arc<MemoryJobItems>,
    pub runner: Arc<RecordingRunner>,
}

impl Host {
    pub fn start() -> Self {
        Self::with_policy(Arc::new(filesquid_api::auth::HostPolicy))
    }

    pub fn with_policy(policy: Arc<dyn Policy>) -> Self {
        Self::assembled(policy, true)
    }

    /// A host whose job service keeps no per-file progress (as `filesquid-server`), holding
    /// the pending job [`UPLOAD`].
    pub fn without_items() -> Self {
        let host = Self::assembled(Arc::new(filesquid_api::auth::HostPolicy), false);
        host.store.insert(&pending(UPLOAD)).expect("job");
        host
    }

    /// A host under `policy` whose job service has the per-file port when `per_file`.
    fn assembled(policy: Arc<dyn Policy>, per_file: bool) -> Self {
        let state = ApiState::new();
        let owner = TokenAuthenticator::new(TOKEN, Principal::Owner);
        state.install_authoriser(Authoriser::new(Arc::new(owner), policy));
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("host-a")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        let store = Arc::new(MemoryJobStore::new());
        let items = Arc::new(MemoryJobItems::new());
        let jobs = Jobs::new(
            store.clone(),
            monitor.journal(),
            monitor.registry().clone(),
            "launch",
        )
        .with_events(state.events.clone());
        let jobs = match per_file {
            true => jobs.with_items(items.clone()),
            false => jobs,
        };
        assert!(state.install_jobs(Arc::new(jobs)));
        let runner = Arc::new(RecordingRunner::default());
        assert!(state.install_job_runner("import", runner.clone()));
        Self {
            state,
            monitor,
            store,
            items,
            runner,
        }
    }

    /// Store upload job `id` in `state` under `control` with three files (one copied, one
    /// copying, one failed) going to the `Site photos` folder of [`COLLECTION`].
    pub fn upload_job(&self, id: &str, state: JobState, control: Option<JobControl>) {
        let mut job = pending(id);
        job.state = state;
        job.control = control;
        self.store.insert(&job).expect("job");
        let destination = JobDestination {
            collection_id: COLLECTION.into(),
            folder: Some("Site photos".into()),
        };
        self.items.put(id, Some(destination), files());
    }

    pub async fn send(&self, method: Method, path: &str, owner: bool) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if owner {
            request = request.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let request = request.body(Body::empty()).expect("request");
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
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::GET, path, true).await
    }

    pub async fn post(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::POST, path, true).await
    }

    pub fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }

    pub fn journal_text(&self) -> String {
        self.monitor.journal().flush();
        let entries = self
            .monitor
            .store()
            .query(&JournalQuery::default())
            .expect("query")
            .entries;
        serde_json::to_string(&entries).expect("json")
    }

    pub fn states(&self, job: &str) -> Vec<JobItemState> {
        self.items.all(job).iter().map(|i| i.state).collect()
    }

    pub fn handed(&self) -> Vec<String> {
        self.runner.handed.lock().expect("handed").clone()
    }
}

/// A pending `import` job `id`. Pure.
pub fn pending(id: &str) -> Job {
    Job {
        id: id.into(),
        kind: "import".into(),
        operation_id: format!("op-{id}"),
        source: "/Volumes/EOS_DIGITAL".into(),
        state: JobState::Pending,
        checkpoint: None,
        done: 1,
        total: Some(3),
        attempts: 1,
        fence: 1,
        holder: None,
        lease_expires_ms: None,
        created_ms: 1_760_000_000_000,
        updated_ms: 1_760_000_000_000,
        error_code: None,
        control: None,
        result: None,
    }
}

/// Three files of an upload: copied, copying and failed.
pub fn files() -> Vec<JobItem> {
    let file = |n: u32, name: &str, state: JobItemState, done: u64, code: Option<&str>| JobItem {
        item_id: format!("file-{n}"),
        name: name.into(),
        folder: Some("Site photos".into()),
        state,
        bytes_done: done,
        bytes_total: 6_400_000,
        error_code: code.map(str::to_string),
    };
    vec![
        file(1, "IMG_4101.JPG", JobItemState::Copied, 6_400_000, None),
        file(2, "IMG_4102.CR3", JobItemState::Copying, 3_968_000, None),
        file(
            3,
            "IMG_4099.JPG",
            JobItemState::Failed,
            0,
            Some("source_changed"),
        ),
    ]
}
