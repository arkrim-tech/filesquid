//! Unit tests for per-file progress (Task API-F2.16; API-S-037 **Per-file progress**) through
//! the router, over a job service with [`MemoryJobItems`]: an upload job's view carries its
//! destination; the items route pages at most 1,000 files with display name, destination
//! folder, state, bytes and error code and never a path; a retry of one failed file or of all
//! failed files puts them back to queued and hands a pending job to its runner; a paused job
//! is refused as `job_paused` with no change; a retry that puts nothing back changes nothing
//! and hands nothing to the runner; every file put back is journaled once as
//! `job.item_retried`; the items route needs `jobs.read`.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use filesquid_api::auth::{Action, Authoriser, Policy, Principal, Resource, TokenAuthenticator};
use filesquid_api::jobs::{
    Job, JobControl, JobDestination, JobError, JobItem, JobItemState, JobRunner, JobState,
    JobStore, Jobs, MemoryJobItems, MemoryJobStore, JOB_ITEM_RETRIED, JOB_REFUSED,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const TOKEN: &str = "items-token";
const UPLOAD: &str = "job-upload";
const COLLECTION: &str = "c0ffee00-1234-4abc-8def-0123456789ab";

/// A runner that records the jobs handed to it and takes none.
#[derive(Default)]
struct RecordingRunner {
    handed: Mutex<Vec<String>>,
}

impl JobRunner for RecordingRunner {
    fn resume(&self, job: &Job) -> Result<(), JobError> {
        self.handed.lock().expect("handed").push(job.id.clone());
        Ok(())
    }
}

/// A policy that grants everything but reading jobs.
struct NoJobReads;

impl Policy for NoJobReads {
    fn permits(&self, _: &Principal, action: Action, _: &Resource) -> bool {
        action != Action::ReadJobs
    }
}

/// A host with a job service over memory stores, the per-file port and a recording runner
/// for the `import` kind.
struct Host {
    state: ApiState,
    monitor: Monitor,
    store: Arc<MemoryJobStore>,
    items: Arc<MemoryJobItems>,
    runner: Arc<RecordingRunner>,
}

impl Host {
    fn start() -> Self {
        Self::with_policy(Arc::new(filesquid_api::auth::HostPolicy))
    }

    fn with_policy(policy: Arc<dyn Policy>) -> Self {
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
        .with_events(state.events.clone())
        .with_items(items.clone());
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

    /// Store a pending upload job with three files: one copied, one copying, one failed.
    fn upload_job(&self, id: &str, state: JobState, control: Option<JobControl>) {
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

    async fn send(&self, method: Method, path: &str, owner: bool) -> (StatusCode, Value) {
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

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::GET, path, true).await
    }

    async fn post(&self, path: &str) -> (StatusCode, Value) {
        self.send(Method::POST, path, true).await
    }

    fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.to_string()),
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }

    fn journal_text(&self) -> String {
        self.monitor.journal().flush();
        let entries = self
            .monitor
            .store()
            .query(&JournalQuery::default())
            .expect("query")
            .entries;
        serde_json::to_string(&entries).expect("json")
    }

    fn states(&self, job: &str) -> Vec<JobItemState> {
        self.items.all(job).iter().map(|i| i.state).collect()
    }

    fn handed(&self) -> Vec<String> {
        self.runner.handed.lock().expect("handed").clone()
    }
}

/// A pending `import` job `id`. Pure.
fn pending(id: &str) -> Job {
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
fn files() -> Vec<JobItem> {
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

#[tokio::test]
async fn an_upload_job_s_view_and_listing_carry_its_destination() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Pending, None);
    host.store.insert(&pending("job-other")).expect("job");
    let (status, view) = host.get(&format!("/api/v1/jobs/{UPLOAD}")).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(
        view["destination"],
        json!({ "collection_id": COLLECTION, "folder": "Site photos" })
    );
    let (status, other) = host.get("/api/v1/jobs/job-other").await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_eq!(other.get("destination"), None, "a job without files");
    let (status, page) = host.get("/api/v1/jobs").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let listed = page["jobs"]
        .as_array()
        .expect("jobs")
        .iter()
        .find(|j| j["id"] == UPLOAD)
        .expect("the upload");
    assert_eq!(listed["destination"]["collection_id"], COLLECTION);
    assert_eq!(listed["result"], Value::Null);
}

#[tokio::test]
async fn the_items_route_pages_the_files_with_their_state_and_never_a_path() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Running, None);
    let (status, page) = host
        .get(&format!("/api/v1/jobs/{UPLOAD}/items?limit=2"))
        .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page["items"],
        json!([
            { "item_id": "file-1", "name": "IMG_4101.JPG", "folder": "Site photos", "state": "copied", "bytes_done": 6_400_000, "bytes_total": 6_400_000 },
            { "item_id": "file-2", "name": "IMG_4102.CR3", "folder": "Site photos", "state": "copying", "bytes_done": 3_968_000, "bytes_total": 6_400_000 }
        ])
    );
    let cursor = page["next_cursor"].as_str().expect("cursor").to_string();
    let (status, last) = host
        .get(&format!(
            "/api/v1/jobs/{UPLOAD}/items?limit=2&cursor={cursor}"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{last}");
    assert_eq!(
        last["items"],
        json!([{ "item_id": "file-3", "name": "IMG_4099.JPG", "folder": "Site photos", "state": "failed", "bytes_done": 0, "bytes_total": 6_400_000, "error_code": "source_changed" }])
    );
    assert_eq!(last["next_cursor"], Value::Null);
    assert!(!page.to_string().contains("/Volumes"), "no path: {page}");
    let (status, _) = host
        .get(&format!("/api/v1/jobs/{UPLOAD}/items?limit=1001"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a page above 1,000");
    let (status, _) = host
        .get(&format!("/api/v1/jobs/{UPLOAD}/items?order=asc"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "an unknown field");
    let (status, _) = host.get("/api/v1/jobs/job-unknown/items").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let refused = host.journaled(JOB_REFUSED);
    assert_eq!(refused.len(), 3);
    assert_eq!(refused[2].error_code.as_deref(), Some("not_found"));
    let (status, _) = host
        .send(Method::GET, &format!("/api/v1/jobs/{UPLOAD}/items"), false)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_items_route_needs_jobs_read() {
    let host = Host::with_policy(Arc::new(NoJobReads));
    host.upload_job(UPLOAD, JobState::Running, None);
    let (status, _) = host.get(&format!("/api/v1/jobs/{UPLOAD}/items")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    assert_eq!(status, StatusCode::OK, "jobs.manage is still granted");
}

#[tokio::test]
async fn a_retry_of_one_failed_file_queues_it_and_hands_the_pending_job_to_its_runner() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Pending, None);
    let (status, answer) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/items/file-3/retry"))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["scheduled"], true);
    assert_eq!(answer["job"]["id"], UPLOAD);
    assert_eq!(answer["job"]["destination"]["collection_id"], COLLECTION);
    assert_eq!(
        host.states(UPLOAD),
        [
            JobItemState::Copied,
            JobItemState::Copying,
            JobItemState::Queued
        ]
    );
    let queued = &host.items.all(UPLOAD)[2];
    assert_eq!(queued.bytes_done, 0);
    assert_eq!(queued.error_code, None);
    assert_eq!(host.handed(), [UPLOAD]);
    let retried = host.journaled(JOB_ITEM_RETRIED);
    assert_eq!(retried.len(), 1);
    assert_eq!(retried[0].target_ids, [UPLOAD, "file-3"]);
    assert_eq!(retried[0].operation_id.as_deref(), Some("op-job-upload"));
    assert!(
        !host.journal_text().contains("IMG_4"),
        "no file name journaled"
    );
    let (status, again) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/items/file-3/retry"))
        .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(
        again["scheduled"], false,
        "a repeated retry hands nothing over"
    );
    assert_eq!(host.handed(), [UPLOAD], "handed once");
    assert_eq!(host.journaled(JOB_ITEM_RETRIED).len(), 1, "journaled once");
}

#[tokio::test]
async fn retry_failed_queues_every_failed_file_and_leaves_the_others() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Pending, None);
    host.items.set(
        UPLOAD,
        "file-1",
        JobItemState::Failed,
        0,
        Some("size_mismatch"),
    );
    let (status, answer) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["scheduled"], true);
    assert_eq!(
        host.states(UPLOAD),
        [
            JobItemState::Queued,
            JobItemState::Copying,
            JobItemState::Queued
        ]
    );
    let retried = host.journaled(JOB_ITEM_RETRIED);
    assert_eq!(retried.len(), 2, "once per file put back");
    assert_eq!(retried[0].target_ids, [UPLOAD, "file-1"]);
    assert_eq!(retried[1].target_ids, [UPLOAD, "file-3"]);
    assert_eq!(host.handed(), [UPLOAD]);
    let (status, nothing) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    assert_eq!(status, StatusCode::OK, "{nothing}");
    assert_eq!(nothing["scheduled"], false);
    assert_eq!(host.handed(), [UPLOAD], "nothing more handed over");
    assert_eq!(host.journaled(JOB_ITEM_RETRIED).len(), 2);
}

#[tokio::test]
async fn a_running_job_s_files_are_queued_without_handing_the_job_over() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Running, None);
    let (status, answer) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(
        answer["scheduled"], false,
        "its worker picks the queued files up"
    );
    assert_eq!(host.states(UPLOAD)[2], JobItemState::Queued);
    assert!(host.handed().is_empty());
}

#[tokio::test]
async fn a_paused_job_refuses_a_retry_with_job_paused_and_nothing_changes() {
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Pending, Some(JobControl::Pause));
    for path in [
        format!("/api/v1/jobs/{UPLOAD}/items/file-3/retry"),
        format!("/api/v1/jobs/{UPLOAD}/retry-failed"),
    ] {
        let (status, refused) = host.post(&path).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {refused}");
        assert_eq!(refused["code"], "conflict");
    }
    assert_eq!(host.states(UPLOAD)[2], JobItemState::Failed, "unchanged");
    assert!(host.handed().is_empty());
    let refused = host.journaled(JOB_REFUSED);
    assert_eq!(refused.len(), 2);
    assert!(refused
        .iter()
        .all(|e| e.error_code.as_deref() == Some("job_paused")));
    assert!(refused
        .iter()
        .all(|e| e.operation_id.as_deref() == Some("op-job-upload")));
    assert!(host.journaled(JOB_ITEM_RETRIED).is_empty());
}

#[tokio::test]
async fn a_file_that_is_not_failed_an_unknown_item_and_a_finished_job_are_handled_as_the_rule_says()
{
    let host = Host::start();
    host.upload_job(UPLOAD, JobState::Pending, None);
    let (status, answer) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/items/file-1/retry"))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["scheduled"], false, "a copied file is left as it is");
    assert_eq!(host.states(UPLOAD)[0], JobItemState::Copied);
    let (status, refused) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/items/file-9/retry"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    assert_eq!(
        host.journaled(JOB_REFUSED)[0].error_code.as_deref(),
        Some("not_found")
    );
    host.upload_job("job-done", JobState::Completed, None);
    let (status, refused) = host.post("/api/v1/jobs/job-done/retry-failed").await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(
        host.journaled(JOB_REFUSED)[1].error_code.as_deref(),
        Some("job_finished")
    );
    host.upload_job("job-stopping", JobState::Running, Some(JobControl::Cancel));
    let (status, refused) = host.post("/api/v1/jobs/job-stopping/retry-failed").await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(
        host.journaled(JOB_REFUSED)[2].error_code.as_deref(),
        Some("cancel_requested")
    );
    assert!(host.handed().is_empty());
    let (status, _) = host
        .send(
            Method::POST,
            &format!("/api/v1/jobs/{UPLOAD}/retry-failed"),
            false,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_host_without_the_per_file_port_answers_unavailable() {
    let state = ApiState::new();
    let owner = TokenAuthenticator::new(TOKEN, Principal::Owner);
    state.install_authoriser(Authoriser::new(
        Arc::new(owner),
        Arc::new(filesquid_api::auth::HostPolicy),
    ));
    let monitor = Monitor::start(
        Arc::new(MemoryJournal::new("host-b")),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("host-b"),
        state.events.clone(),
    );
    state.install_monitor(monitor.clone());
    let store = Arc::new(MemoryJobStore::new());
    store.insert(&pending(UPLOAD)).expect("job");
    let jobs = Jobs::new(
        store,
        monitor.journal(),
        monitor.registry().clone(),
        "launch",
    );
    assert!(state.install_jobs(Arc::new(jobs)));
    let host = Host {
        state,
        monitor,
        store: Arc::new(MemoryJobStore::new()),
        items: Arc::new(MemoryJobItems::new()),
        runner: Arc::new(RecordingRunner::default()),
    };
    let (status, _) = host.get(&format!("/api/v1/jobs/{UPLOAD}/items")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = host
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, view) = host.get(&format!("/api/v1/jobs/{UPLOAD}")).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view.get("destination"), None);
}

#[test]
fn the_reference_port_pages_and_requeues_as_the_rule_says() {
    use filesquid_api::jobs::{page_of_items, requeue, JobItems};
    let items = MemoryJobItems::new();
    items.put("job", None, files());
    assert_eq!(
        items.retry("job", Some("file-2")).expect("retry"),
        Vec::<String>::new()
    );
    assert_eq!(items.retry("job", None).expect("retry"), ["file-3"]);
    assert_eq!(
        items.retry("job", None).expect("retry"),
        Vec::<String>::new()
    );
    assert_eq!(items.retry("job", Some("file-9")), Err(JobError::NotFound));
    assert_eq!(items.retry("other", None), Err(JobError::NotFound));
    let page = page_of_items(&files(), Some("x"), 5);
    assert_eq!(
        page.items.len(),
        3,
        "a cursor that is no position starts first"
    );
    assert_eq!(page.next_cursor, None);
    let page = page_of_items(&files(), Some("9"), 5);
    assert!(page.items.is_empty(), "a cursor past the end");
    let mut all = files();
    assert_eq!(requeue(&mut all, None), ["file-3"]);
    assert_eq!(all[2].state, JobItemState::Queued);
    let _ = Outcome::Info;
}
