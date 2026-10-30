//! Unit tests for the upload review (Task API-F2.16; API-S-036 **Review**) through the
//! router, over a starter that plans with [`PlanBook`]: the plan is answered with every field
//! the owner reviews before any byte is staged; the plan items route pages the files by base
//! name; the job starts once on the start route and a repeated start answers the same job; a
//! repeated operation ID answers its open plan, its started job or `cancelled` without a
//! dialog; an unknown, discarded or expired plan is `plan_not_found`; `DELETE` and the 1-hour
//! expiry discard the plan with no change; a changed source is `source_changed`; every plan,
//! start and discard is journaled once without a file name; and no path ever leaves the host.

#[path = "../support/import_fixture.rs"]
mod import_fixture;

use axum::http::{Method, StatusCode};
use filesquid_api::imports::{
    ImportAnswer, ImportRequest, ImportStartError, ImportStarter, PlanBook,
    IMPORT_DISCARD_OPERATION, IMPORT_PLAN_ITEMS_OPERATION, IMPORT_PLAN_OPERATION,
    IMPORT_START_OPERATION, PLAN_TTL_MS,
};
use filesquid_api::journal::Outcome;
use import_fixture::{body, body_of, scope, Host, PlanningStarter, PATH};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// A host whose starter plans, with the dialog answering `dialog` in order.
fn planning_host(dialog: Vec<bool>) -> (Host, Arc<PlanningStarter>) {
    let host = Host::start();
    let starter = PlanningStarter::new(dialog);
    assert!(host.state.install_import_starter(starter.clone()));
    (host, starter)
}

/// Plan the sample import on `host`; answer the plan ID and the answer.
async fn planned(host: &Host) -> (String, Value) {
    let (status, answer) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["import"], "planned", "{answer}");
    let plan_id = answer["plan_id"].as_str().expect("plan id").to_string();
    (plan_id, answer)
}

#[tokio::test]
async fn a_picked_source_answers_its_plan_before_any_byte_is_staged() {
    let (host, starter) = planning_host(vec![true]);
    let (plan_id, answer) = planned(&host).await;
    assert_eq!(plan_id, "plan-1");
    assert_eq!(
        answer["outcome"],
        json!({ "operation_id": "op-1", "kind": "pending", "error": null })
    );
    let plan = &answer["plan"];
    assert_eq!(plan["plan_id"], "plan-1");
    assert_eq!(plan["source"], "folder");
    assert_eq!(plan["source_volume"], "EOS_DIGITAL");
    assert_eq!(plan["files_found"], 3);
    assert_eq!(plan["files_new"], 2);
    assert_eq!(plan["files_present"], 1);
    assert_eq!(plan["bytes_to_copy"], 31_200_000);
    assert_eq!(
        plan["destination"]["collection_id"],
        import_fixture::COLLECTION
    );
    assert_eq!(plan["destination"].get("folder"), None);
    assert_eq!(plan["names_numbered"], 1);
    assert_eq!(plan["free_bytes_after"], 408_900_000_000_u64);
    assert_eq!(plan["staging_limit_bytes"], 10_737_418_240_u64);
    assert_eq!(
        plan["expires"], "2025-10-09T09:53:20.000Z",
        "one hour after the plan"
    );
    assert_eq!(answer.get("job_id"), None, "no job yet");
    assert_eq!(starter.jobs.load(Ordering::SeqCst), 0, "nothing started");
    let entries = host.journaled(IMPORT_PLAN_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].target_ids, ["plan-1"]);
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-1"));
    assert_redacted(&host, &[&answer]);
}

#[tokio::test]
async fn the_plan_items_route_pages_the_planned_files_by_base_name() {
    let (host, _) = planning_host(vec![true]);
    let (plan_id, _) = planned(&host).await;
    let path = format!("{PATH}/{plan_id}/items?limit=2");
    let (status, page) = host.send(Method::GET, &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page["items"],
        json!([
            { "item_id": "file-0", "name": "IMG_4102.CR3", "bytes": 24_800_000, "present": false, "numbered": true },
            { "item_id": "file-1", "name": "IMG_4102.JPG", "bytes": 6_400_000, "present": false, "numbered": false }
        ])
    );
    let cursor = page["next_cursor"].as_str().expect("cursor").to_string();
    let path = format!("{PATH}/{plan_id}/items?limit=2&cursor={cursor}");
    let (status, last) = host.send(Method::GET, &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{last}");
    assert_eq!(last["items"][0]["name"], "IMG_4021.JPG");
    assert_eq!(last["items"][0]["present"], true);
    assert_eq!(last["next_cursor"], Value::Null);
    let (status, all) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, true)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(all["items"].as_array().expect("items").len(), 3);
    let (status, _) = host
        .send(
            Method::GET,
            &format!("{PATH}/{plan_id}/items?limit=1001"),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a page above 1,000");
    let (status, _) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, false)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_redacted(&host, &[&page, &last, &all]);
}

#[tokio::test]
async fn start_starts_the_job_once_and_a_repeated_start_answers_the_same_job() {
    let (host, starter) = planning_host(vec![true]);
    let (plan_id, _) = planned(&host).await;
    let path = format!("{PATH}/{plan_id}/start");
    let (status, started) = host.send(Method::POST, &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(
        started,
        json!({
            "outcome": { "operation_id": "op-1", "kind": "pending", "error": null },
            "import": "started",
            "job_id": "job-1"
        })
    );
    let (status, again) = host.send(Method::POST, &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["job_id"], "job-1");
    assert_eq!(starter.jobs.load(Ordering::SeqCst), 1, "one job");
    let entries = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(entries.len(), 2, "each start journaled once");
    assert!(entries.iter().all(|e| e.outcome == Outcome::Succeeded));
    assert!(entries.iter().all(|e| e.target_ids == ["job-1"]));
    assert!(entries
        .iter()
        .all(|e| e.operation_id.as_deref() == Some("op-1")));
    let (status, items) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, true)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a started plan is no longer open: {items}"
    );
}

#[tokio::test]
async fn a_repeated_operation_answers_its_plan_its_job_or_cancelled_without_a_dialog() {
    let (host, starter) = planning_host(vec![true, false]);
    let (plan_id, first) = planned(&host).await;
    let (status, again) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, first, "the open plan again");
    assert_eq!(starter.dialogs_shown(), 1, "no second dialog");
    let path = format!("{PATH}/{plan_id}/start");
    let (status, _) = host.send(Method::POST, &path, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let (status, started) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["import"], "started");
    assert_eq!(started["job_id"], "job-1");
    let (status, cancelled) = host.post(body_of("op-2", &scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["import"], "cancelled");
    let (status, cancelled_again) = host.post(body_of("op-2", &scope()), true).await;
    assert_eq!(status, StatusCode::OK, "{cancelled_again}");
    assert_eq!(cancelled_again["import"], "cancelled");
    assert_eq!(
        starter.dialogs_shown(),
        2,
        "the cancelled operation shows no second dialog"
    );
    let plans = host.journaled(IMPORT_PLAN_OPERATION);
    assert_eq!(
        plans.len(),
        4,
        "every plan request journaled once: {plans:?}"
    );
    assert_eq!(
        plans.iter().map(|e| e.outcome).collect::<Vec<_>>(),
        [
            Outcome::Succeeded,
            Outcome::Succeeded,
            Outcome::Cancelled,
            Outcome::Cancelled
        ]
    );
    let starts = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(
        starts.len(),
        2,
        "the start and the replay of the started job"
    );
}

#[tokio::test]
async fn delete_discards_the_plan_and_changes_nothing() {
    let (host, starter) = planning_host(vec![true]);
    let (plan_id, _) = planned(&host).await;
    let path = format!("{PATH}/{plan_id}");
    let (status, answer) = host.send(Method::DELETE, &path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer, json!({ "plan_id": "plan-1", "discarded": true }));
    for (method, path) in [
        (Method::GET, format!("{PATH}/{plan_id}/items")),
        (Method::POST, format!("{PATH}/{plan_id}/start")),
        (Method::DELETE, format!("{PATH}/{plan_id}")),
    ] {
        let (status, refused) = host.send(method.clone(), &path, None, true).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {refused}");
        assert_eq!(refused["code"], "not_found");
        assert!(
            refused["message"]
                .as_str()
                .is_some_and(|m| m.contains("plan_not_found")),
            "{refused}"
        );
    }
    let (status, replayed) = host.post(body(&scope()), true).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a discarded plan's operation: {replayed}"
    );
    assert_eq!(starter.jobs.load(Ordering::SeqCst), 0, "nothing started");
    let discards = host.journaled(IMPORT_DISCARD_OPERATION);
    assert_eq!(discards.len(), 2);
    assert_eq!(discards[0].outcome, Outcome::Succeeded);
    assert_eq!(discards[0].target_ids, ["plan-1"]);
    assert_eq!(discards[1].error_code.as_deref(), Some("plan_not_found"));
    let starts = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].error_code.as_deref(), Some("plan_not_found"));
    let items = host.journaled(IMPORT_PLAN_ITEMS_OPERATION);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].error_code.as_deref(), Some("plan_not_found"));
}

#[tokio::test]
async fn a_plan_expires_after_one_hour() {
    let (host, starter) = planning_host(vec![true]);
    let (plan_id, _) = planned(&host).await;
    starter.advance(PLAN_TTL_MS - 1);
    let (status, _) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, true)
        .await;
    assert_eq!(status, StatusCode::OK, "still open a millisecond before");
    starter.advance(1);
    let (status, _) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, true)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "expired");
    let (status, refused) = host
        .send(Method::POST, &format!("{PATH}/{plan_id}/start"), None, true)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    let (status, _) = host.post(body(&scope()), true).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the operation's plan expired"
    );
    assert_eq!(starter.jobs.load(Ordering::SeqCst), 0, "nothing started");
    let starts = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(starts[0].error_code.as_deref(), Some("plan_not_found"));
}

#[tokio::test]
async fn a_changed_source_is_refused_as_source_changed_and_the_plan_stays_open() {
    let (host, starter) = planning_host(vec![true]);
    let (plan_id, _) = planned(&host).await;
    starter.change_source();
    let (status, refused) = host
        .send(Method::POST, &format!("{PATH}/{plan_id}/start"), None, true)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["code"], "conflict");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|m| m.contains("source_changed")),
        "{refused}"
    );
    assert_eq!(starter.jobs.load(Ordering::SeqCst), 0, "nothing started");
    let (status, _) = host
        .send(Method::GET, &format!("{PATH}/{plan_id}/items"), None, true)
        .await;
    assert_eq!(status, StatusCode::OK, "the plan stays open for review");
    let starts = host.journaled(IMPORT_START_OPERATION);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].outcome, Outcome::Failed);
    assert_eq!(starts[0].error_code.as_deref(), Some("source_changed"));
}

#[tokio::test]
async fn a_host_without_a_starter_answers_unavailable_on_every_review_route() {
    let host = Host::start();
    for (method, path) in [
        (Method::GET, format!("{PATH}/plan-1/items")),
        (Method::POST, format!("{PATH}/plan-1/start")),
        (Method::DELETE, format!("{PATH}/plan-1")),
    ] {
        let (status, answer) = host.send(method.clone(), &path, None, true).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {answer}"
        );
    }
}

#[test]
fn the_plan_book_forgets_the_oldest_plan_at_its_cap() {
    let book = PlanBook::new();
    let request: ImportRequest = serde_json::from_value(body(&scope())).expect("request");
    for n in 0..=filesquid_api::imports::MAX_REMEMBERED_PLANS {
        let mut request = request.clone();
        request.envelope = filesquid_core::CommandEnvelope::new(
            filesquid_core::OperationId::new(format!("op-{n}")).expect("op"),
            filesquid_core::ExpectedRevision::new("r1").expect("revision"),
            filesquid_core::OperationScope::new(scope()).expect("scope"),
        );
        book.open(
            &format!("plan-{n}"),
            &request,
            PlanningStarter::facts(),
            Vec::new(),
            "fp",
            n as i64,
        );
    }
    assert!(!book.is_open("plan-0", 0), "the oldest was forgotten");
    assert!(book.is_open("plan-1", 1));
}

/// The sample import request under operation `op`.
fn request_of(op: &str) -> ImportRequest {
    serde_json::from_value(body_of(op, &scope())).expect("request")
}

#[test]
fn the_plan_book_submits_one_job_per_plan_and_never_restarts_a_discarded_plan() {
    let book = PlanBook::new();
    let facts = PlanningStarter::facts();
    book.open(
        "plan-a",
        &request_of("op-a"),
        facts.clone(),
        Vec::new(),
        "fp",
        0,
    );
    let refused = book.start("plan-a", "fp", 1, |_, _| Err(ImportStartError::Unavailable));
    assert_eq!(refused, Err(ImportStartError::Unavailable));
    assert!(
        book.is_open("plan-a", 1),
        "a failed submission leaves the plan open"
    );
    let submitted = std::sync::atomic::AtomicUsize::new(0);
    let submit = |op: &str, _: &ImportRequest| {
        submitted.fetch_add(1, Ordering::SeqCst);
        Ok(format!("job-of-{op}"))
    };
    let first = book.start("plan-a", "fp", 2, submit).expect("started");
    let again = book.start("plan-a", "fp", 3, submit).expect("the same job");
    assert_eq!(first.job_id.as_deref(), Some("job-of-op-a"));
    assert_eq!(again, first);
    assert_eq!(submitted.load(Ordering::SeqCst), 1, "one job per plan");
    book.open("plan-b", &request_of("op-b"), facts, Vec::new(), "fp", 0);
    book.discard("plan-b", 1).expect("discarded");
    let refused = book.start("plan-b", "fp", 2, submit);
    assert_eq!(refused, Err(ImportStartError::PlanNotFound));
    assert_eq!(
        submitted.load(Ordering::SeqCst),
        1,
        "a discarded plan submits nothing"
    );
}

#[test]
fn the_plan_book_forgets_the_oldest_cancelled_operation_first() {
    let book = PlanBook::new();
    let cap = filesquid_api::imports::MAX_REMEMBERED_PLANS;
    for n in 0..cap {
        book.cancelled(&request_of(&format!("op-{n}")));
    }
    book.cancelled(&request_of("op-0"));
    let replayed = book.replay(&request_of("op-1"), 0).expect("replay");
    assert!(
        replayed.is_some(),
        "re-cancelling a known operation forgets no other"
    );
    book.cancelled(&request_of(&format!("op-{cap}")));
    assert_eq!(
        book.replay(&request_of("op-0"), 0),
        Ok(None),
        "the oldest went first"
    );
    for n in 1..=cap {
        let replayed = book
            .replay(&request_of(&format!("op-{n}")), 0)
            .expect("replay");
        assert!(replayed.is_some(), "op-{n} is remembered");
    }
}

#[tokio::test]
async fn a_second_request_of_an_operation_in_flight_is_dialog_open_and_opens_no_dialog() {
    let host = Host::start();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let starter = Arc::new(HeldStarter {
        released: std::sync::Mutex::new(released),
        dialogs: std::sync::atomic::AtomicUsize::new(0),
    });
    assert!(host.state.install_import_starter(starter.clone()));
    let first = {
        let state = host.state.clone();
        let token = host.token.clone();
        tokio::spawn(async move { import_fixture::post_as(state, &token, body(&scope())).await })
    };
    for _ in 0..200 {
        if starter.dialogs.load(Ordering::SeqCst) == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(
        starter.dialogs.load(Ordering::SeqCst),
        1,
        "the first dialog is open"
    );
    let (status, refused) = host.post(body(&scope()), true).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    release.send(()).expect("release");
    let (status, answer) = first.await.expect("first");
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["import"], "cancelled");
    assert_eq!(starter.dialogs.load(Ordering::SeqCst), 1, "one dialog only");
    let refusals = host.journaled(IMPORT_PLAN_OPERATION);
    assert!(refusals
        .iter()
        .any(|e| e.error_code.as_deref() == Some("dialog_open")));
}

/// A starter whose dialog stays open until the test releases it, then is closed.
struct HeldStarter {
    released: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    dialogs: std::sync::atomic::AtomicUsize,
}

impl ImportStarter for HeldStarter {
    fn replay(&self, _request: &ImportRequest) -> Result<Option<ImportAnswer>, ImportStartError> {
        Ok(None)
    }

    fn plan(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        self.dialogs.fetch_add(1, Ordering::SeqCst);
        let released = self.released.lock().expect("released");
        released
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ImportStartError::Unavailable)?;
        Ok(ImportAnswer::cancelled(request.envelope.operation_id()))
    }
}

/// Assert no answer and no journal entry carries a picked path or the volume's folder.
fn assert_redacted(host: &Host, answers: &[&Value]) {
    for answer in answers {
        let text = answer.to_string();
        assert!(!text.contains("/Volumes"), "a path leaked: {text}");
        assert!(!text.contains("DCIM"), "a folder leaked: {text}");
    }
    let journal = host.journal_text();
    assert!(!journal.contains("/Volumes"), "a path was journaled");
    assert!(!journal.contains("IMG_4"), "a file name was journaled");
    assert!(!journal.contains("EOS_DIGITAL"), "the volume was journaled");
}
