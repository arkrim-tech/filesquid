//! Unit tests for per-file progress (Task API-F2.16; API-S-037 **Per-file progress**) through
//! the router, over a job service with [`MemoryJobItems`]: an upload job's view carries its
//! destination; the items route pages at most 1,000 files with display name, destination
//! folder, state, bytes and error code and never a path; a retry of one failed file or of all
//! failed files puts them back to queued and hands a pending job to its runner; a paused job
//! is refused as `job_paused` with no change; a retry that puts nothing back changes nothing
//! and hands nothing to the runner; every file put back is journaled once as
//! `job.item_retried`; the items route needs `jobs.read`.

#[path = "../support/job_items_fixture.rs"]
mod job_items_fixture;

use axum::http::{Method, StatusCode};
use filesquid_api::auth::{Authoriser, Principal, TokenAuthenticator};
use filesquid_api::jobs::{
    JobControl, JobError, JobItemState, JobState, JobStore, Jobs, MemoryJobItems, MemoryJobStore,
    JOB_ITEM_RETRIED, JOB_REFUSED,
};
use filesquid_api::journal::{MemoryJournal, Outcome};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::ApiState;
use job_items_fixture::{
    files, pending, Host, NoJobReads, RecordingRunner, COLLECTION, TOKEN, UPLOAD,
};
use serde_json::{json, Value};
use std::sync::Arc;

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
