//! Step definitions for `upload_review_and_per_file_progress_API-S-036.feature` (Task
//! API-F2.16; API-S-036 **Review**, API-S-037 **Per-file progress**): a desktop owner's
//! router over a starter that plans with a [`PlanBook`] (the import fixture's
//! [`PlanningStarter`]) and over a job service with the per-file port (the job items
//! fixture), every request sent through the router.

use crate::import_fixture::{body, scope, Host, PlanningStarter, PATH};
use crate::job_items_fixture::{self, UPLOAD};
use crate::MonitorWorld;
use axum::http::{Method, StatusCode};
use cucumber::{given, then, when};
use filesquid_api::imports::{
    IMPORT_DISCARD_OPERATION, IMPORT_PLAN_OPERATION, IMPORT_START_OPERATION, PLAN_TTL_MS,
};
use filesquid_api::jobs::{
    JobControl, JobItemState, JobState, JobStore, JOB_ITEM_RETRIED, JOB_REFUSED,
};
use filesquid_api::journal::Outcome;
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Per-scenario state of the import steps.
#[derive(Default)]
pub struct ImportsWorld {
    host: Option<Host>,
    starter: Option<Arc<PlanningStarter>>,
    plan_id: Option<String>,
    answers: Vec<(StatusCode, Value)>,
    jobs: Option<job_items_fixture::Host>,
}

impl std::fmt::Debug for ImportsWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportsWorld")
            .field("plan_id", &self.plan_id)
            .finish_non_exhaustive()
    }
}

impl ImportsWorld {
    fn host(&self) -> &Host {
        self.host.as_ref().expect("a planning host")
    }

    fn starter(&self) -> &PlanningStarter {
        self.starter.as_ref().expect("a planning starter")
    }

    fn plan_id(&self) -> &str {
        self.plan_id.as_deref().expect("a plan")
    }

    fn last(&self) -> &(StatusCode, Value) {
        self.answers.last().expect("an answer")
    }

    fn jobs(&self) -> &job_items_fixture::Host {
        self.jobs.as_ref().expect("a job host")
    }
}

#[given("a desktop host whose import dialog plans what the owner picks")]
fn planning_host(w: &mut MonitorWorld) {
    let host = Host::start();
    let starter = PlanningStarter::new(vec![true, true]);
    assert!(host.state.install_import_starter(starter.clone()));
    w.imports.host = Some(host);
    w.imports.starter = Some(starter);
}

#[when("the owner asks to upload a folder into a collection")]
#[given("the owner has asked to upload a folder into a collection")]
async fn plan(w: &mut MonitorWorld) {
    let (status, answer) = w.imports.host().post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    w.imports.plan_id = answer["plan_id"].as_str().map(str::to_string);
    w.imports.answers.push((status, answer));
}

#[when("the same request is sent again")]
async fn plan_again(w: &mut MonitorWorld) {
    let (status, answer) = w.imports.host().post(body(&scope()), true).await;
    w.imports.answers.push((status, answer));
}

#[then(
    expr = "the review should show {int} files found, {int} new and {int} already in this library"
)]
fn review_counts(w: &mut MonitorWorld, found: u64, new: u64, present: u64) {
    let (status, answer) = w.imports.last();
    assert_eq!(*status, StatusCode::OK, "{answer}");
    assert_eq!(answer["import"], "planned");
    let plan = &answer["plan"];
    assert_eq!(plan["files_found"], found);
    assert_eq!(plan["files_new"], new);
    assert_eq!(plan["files_present"], present);
    assert_eq!(plan["bytes_to_copy"], 31_200_000);
}

#[then("the review should name the destination, the numbered names, the free bytes after the upload and the staging limit")]
fn review_fields(w: &mut MonitorWorld) {
    let plan = &w.imports.last().1["plan"];
    assert_eq!(
        plan["destination"]["collection_id"],
        crate::import_fixture::COLLECTION
    );
    assert_eq!(plan["names_numbered"], 1);
    assert_eq!(plan["free_bytes_after"], 408_900_000_000_u64);
    assert_eq!(plan["staging_limit_bytes"], 10_737_418_240_u64);
    assert!(plan["expires"].as_str().is_some_and(|e| e.ends_with('Z')));
}

#[then("no job should have started")]
fn nothing_started(w: &mut MonitorWorld) {
    assert_eq!(w.imports.starter().jobs.load(Ordering::SeqCst), 0);
    assert!(w
        .imports
        .host()
        .journaled(IMPORT_START_OPERATION)
        .is_empty());
}

#[then("the plan request should be journaled once with the plan and no file name")]
fn plan_journaled(w: &mut MonitorWorld) {
    let entries = w.imports.host().journaled(IMPORT_PLAN_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].target_ids, [w.imports.plan_id()]);
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
    assert_no_leak(w);
}

#[when(expr = "the owner reads the planned files {int} at a time")]
async fn read_items(w: &mut MonitorWorld, limit: usize) {
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let mut path = format!("{PATH}/{}/items?limit={limit}", w.imports.plan_id());
        if let Some(cursor) = &cursor {
            path.push_str(&format!("&cursor={cursor}"));
        }
        let (status, page) = w.imports.host().send(Method::GET, &path, None, true).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        cursor = page["next_cursor"].as_str().map(str::to_string);
        w.imports.answers.push((status, page));
        if cursor.is_none() {
            return;
        }
    }
    panic!("the pages never ended");
}

#[then("the files should be listed by base name with their size and whether they are new")]
fn items_listed(w: &mut MonitorWorld) {
    let pages = w.imports.answers.iter().skip(1);
    let items: Vec<&Value> = pages
        .flat_map(|(_, page)| page["items"].as_array().expect("items"))
        .collect();
    let names: Vec<&str> = items
        .iter()
        .map(|i| i["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["IMG_4102.CR3", "IMG_4102.JPG", "IMG_4021.JPG"]);
    assert_eq!(items[0]["bytes"], 24_800_000);
    assert_eq!(items[0]["present"], false);
    assert_eq!(items[2]["present"], true);
    assert_eq!(w.imports.answers.len(), 3, "two pages of two then one");
}

#[then("no path should appear in any answer")]
fn no_path(w: &mut MonitorWorld) {
    for (_, answer) in &w.imports.answers {
        let text = answer.to_string();
        assert!(
            !text.contains("/Volumes") && !text.contains("DCIM"),
            "{text}"
        );
    }
    assert_no_leak(w);
}

#[when("the owner confirms the upload twice")]
async fn confirm_twice(w: &mut MonitorWorld) {
    confirm(w).await;
    confirm(w).await;
}

#[when("the owner confirms the upload")]
async fn confirm(w: &mut MonitorWorld) {
    let path = format!("{PATH}/{}/start", w.imports.plan_id());
    let answer = w.imports.host().send(Method::POST, &path, None, true).await;
    w.imports.answers.push(answer);
}

#[then("one import job should have started and both answers should name it")]
fn one_job(w: &mut MonitorWorld) {
    assert_eq!(w.imports.starter().jobs.load(Ordering::SeqCst), 1);
    let starts: Vec<&(StatusCode, Value)> = w.imports.answers.iter().skip(1).collect();
    assert_eq!(starts.len(), 2);
    for (status, answer) in starts {
        assert_eq!(*status, StatusCode::OK, "{answer}");
        assert_eq!(answer["import"], "started");
        assert_eq!(answer["job_id"], "job-1");
    }
}

#[then("each start should be journaled once with the job")]
fn starts_journaled(w: &mut MonitorWorld) {
    let entries = w.imports.host().journaled(IMPORT_START_OPERATION);
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|e| e.outcome == Outcome::Succeeded && e.target_ids == ["job-1"]));
    assert_no_leak(w);
}

#[then("the open plan should be answered again")]
fn same_plan(w: &mut MonitorWorld) {
    let first = &w.imports.answers[0].1;
    let (status, again) = w.imports.last();
    assert_eq!(*status, StatusCode::OK, "{again}");
    assert_eq!(again, first);
}

#[then("the dialog should have been shown once")]
fn one_dialog(w: &mut MonitorWorld) {
    assert_eq!(w.imports.starter().dialogs_shown(), 1);
}

#[when("the owner discards the plan")]
async fn discard(w: &mut MonitorWorld) {
    let path = format!("{PATH}/{}", w.imports.plan_id());
    let (status, answer) = w
        .imports
        .host()
        .send(Method::DELETE, &path, None, true)
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["discarded"], true);
    w.imports.answers.push((status, answer));
}

#[when("an hour passes")]
fn hour_passes(w: &mut MonitorWorld) {
    w.imports.starter().advance(PLAN_TTL_MS);
}

#[then("the plan should be gone and no job should have started")]
async fn plan_gone(w: &mut MonitorWorld) {
    let plan = w.imports.plan_id().to_string();
    for (method, path) in [
        (Method::GET, format!("{PATH}/{plan}/items")),
        (Method::POST, format!("{PATH}/{plan}/start")),
    ] {
        let (status, answer) = w.imports.host().send(method, &path, None, true).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{answer}");
        assert!(answer["message"]
            .as_str()
            .is_some_and(|m| m.contains("plan_not_found")));
    }
    assert_eq!(w.imports.starter().jobs.load(Ordering::SeqCst), 0);
}

#[then("the discard should be journaled once with the plan")]
fn discard_journaled(w: &mut MonitorWorld) {
    let entries = w.imports.host().journaled(IMPORT_DISCARD_OPERATION);
    let succeeded: Vec<_> = entries
        .iter()
        .filter(|e| e.outcome == Outcome::Succeeded)
        .collect();
    assert_eq!(succeeded.len(), 1);
    assert_eq!(succeeded[0].target_ids, [w.imports.plan_id()]);
    assert_no_leak(w);
}

#[given("the picked files changed since the plan")]
fn source_changed(w: &mut MonitorWorld) {
    w.imports.starter().change_source();
}

#[then(expr = "the upload should be refused as {string}")]
fn refused(w: &mut MonitorWorld, code: String) {
    let (status, answer) = w.imports.last();
    assert_eq!(*status, StatusCode::CONFLICT, "{answer}");
    assert!(answer["message"]
        .as_str()
        .is_some_and(|m| m.contains(&code)));
    let starts = w.imports.host().journaled(IMPORT_START_OPERATION);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].error_code.as_deref(), Some(code.as_str()));
}

#[then("the plan should still be open")]
async fn still_open(w: &mut MonitorWorld) {
    let path = format!("{PATH}/{}/items", w.imports.plan_id());
    let (status, _) = w.imports.host().send(Method::GET, &path, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(w.imports.starter().jobs.load(Ordering::SeqCst), 0);
}

/// Assert the journal names no path, no volume and no file.
fn assert_no_leak(w: &MonitorWorld) {
    let journal = w.imports.host().journal_text();
    assert!(!journal.contains("/Volumes"), "a path was journaled");
    assert!(!journal.contains("IMG_4"), "a file name was journaled");
    assert!(!journal.contains("EOS_DIGITAL"), "the volume was journaled");
}

#[given("an upload job with a copied, a copying and a failed file")]
fn upload_job(w: &mut MonitorWorld) {
    let jobs = job_items_fixture::Host::start();
    jobs.upload_job(UPLOAD, JobState::Pending, None);
    w.imports.jobs = Some(jobs);
}

#[given("the job is paused")]
fn paused(w: &mut MonitorWorld) {
    let jobs = w.imports.jobs();
    jobs.store
        .update(UPLOAD, &mut |mut job| {
            job.control = Some(JobControl::Pause);
            Ok(job)
        })
        .expect("paused");
}

#[when("the owner reads the job and its files")]
async fn read_job(w: &mut MonitorWorld) {
    let jobs = w.imports.jobs();
    let view = jobs.get(&format!("/api/v1/jobs/{UPLOAD}")).await;
    let items = jobs.get(&format!("/api/v1/jobs/{UPLOAD}/items")).await;
    w.imports.answers.push(view);
    w.imports.answers.push(items);
}

#[then("the job should name its destination collection and folder")]
fn job_destination(w: &mut MonitorWorld) {
    let (status, view) = &w.imports.answers[0];
    assert_eq!(*status, StatusCode::OK, "{view}");
    assert_eq!(
        view["destination"]["collection_id"],
        job_items_fixture::COLLECTION
    );
    assert_eq!(view["destination"]["folder"], "Site photos");
}

#[then("the files should show their name, folder, state and bytes and never a path")]
fn files_shown(w: &mut MonitorWorld) {
    let (status, page) = &w.imports.answers[1];
    assert_eq!(*status, StatusCode::OK, "{page}");
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["name"], "IMG_4101.JPG");
    assert_eq!(items[0]["folder"], "Site photos");
    assert_eq!(items[0]["state"], "copied");
    assert_eq!(items[1]["state"], "copying");
    assert_eq!(items[1]["bytes_done"], 3_968_000);
    assert_eq!(items[1]["bytes_total"], 6_400_000);
    assert_eq!(items[2]["state"], "failed");
    assert_eq!(items[2]["error_code"], "source_changed");
    assert!(!page.to_string().contains("/Volumes"), "{page}");
}

#[when("the owner retries the failed file")]
async fn retry_file(w: &mut MonitorWorld) {
    let answer = w
        .imports
        .jobs()
        .post(&format!("/api/v1/jobs/{UPLOAD}/items/file-3/retry"))
        .await;
    w.imports.answers.push(answer);
}

#[when("the owner retries all failed files")]
async fn retry_all(w: &mut MonitorWorld) {
    let answer = w
        .imports
        .jobs()
        .post(&format!("/api/v1/jobs/{UPLOAD}/retry-failed"))
        .await;
    w.imports.answers.push(answer);
}

#[then("the file should be queued again and the job handed to its runner")]
fn requeued(w: &mut MonitorWorld) {
    let (status, answer) = w.imports.last();
    assert_eq!(*status, StatusCode::OK, "{answer}");
    assert_eq!(answer["scheduled"], true);
    let jobs = w.imports.jobs();
    assert_eq!(jobs.states(UPLOAD)[2], JobItemState::Queued);
    assert_eq!(jobs.handed(), [UPLOAD]);
}

#[then("the retry should be journaled once for that file without its name")]
fn retry_journaled(w: &mut MonitorWorld) {
    let jobs = w.imports.jobs();
    let entries = jobs.journaled(JOB_ITEM_RETRIED);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].target_ids, [UPLOAD, "file-3"]);
    assert!(!jobs.journal_text().contains("IMG_4"));
}

#[then("nothing should change and nothing should be handed to the runner")]
fn nothing_changed(w: &mut MonitorWorld) {
    let (status, answer) = w.imports.last();
    assert_eq!(*status, StatusCode::OK, "{answer}");
    assert_eq!(answer["scheduled"], false);
    let jobs = w.imports.jobs();
    assert_eq!(jobs.handed(), [UPLOAD], "handed once in all");
    assert_eq!(jobs.journaled(JOB_ITEM_RETRIED).len(), 1);
}

#[then(expr = "the retry should be refused as {string} and the file should stay failed")]
fn retry_refused(w: &mut MonitorWorld, code: String) {
    let (status, answer) = w.imports.last();
    assert_eq!(*status, StatusCode::CONFLICT, "{answer}");
    let jobs = w.imports.jobs();
    assert_eq!(jobs.states(UPLOAD)[2], JobItemState::Failed);
    assert!(jobs.handed().is_empty());
    let refused = jobs.journaled(JOB_REFUSED);
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].error_code.as_deref(), Some(code.as_str()));
}
