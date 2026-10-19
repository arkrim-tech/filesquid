//! Task API-F2.7: OCFL validation findings reach health and the Monitor (API-S-022,
//! OCFL-S-018; F-061, F-042). Real OCFL roots on the in-memory backend and, for the E023
//! case, on a real directory through `FsBackend`.

#[path = "../support/ocfl_fixture.rs"]
mod fixture;

#[path = "../support/validation_fixture.rs"]
mod vfix;

use filesquid_api::health::HealthStatus;
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::ocfl_validation::{
    FindingsStore, MemoryFindingsStore, OcflValidation, RunOutcome, RunRequest, RunTrigger,
    Severity, ValidationConfig, ValidationService, CONFIGURE_OPERATION, DEFAULT_INTERVAL_MS,
    DEFAULT_OBJECT_CAP, FINDING_CLOSED_OPERATION, FINDING_OPERATION, RUN_OPERATION,
};
use filesquid_ocfl::storage::StorageBackend;
use fixture::library;
use std::sync::{Arc, Mutex};
use vfix::{ValidationRig, WEEK_MS};

/// Strings that must never reach the journal or a health reason (logical paths, content
/// paths, object roots, file names, content).
const LEAKS: [&str; 6] = ["stray", "a.txt", "content", "obj-", "/", "fenced-out"];

/// Assert `text` carries none of [`LEAKS`].
fn assert_redacted(text: &str) {
    for leak in LEAKS {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
}

/// The open error findings of `store`.
fn open_errors(store: &dyn FindingsStore) -> Vec<filesquid_api::ocfl_validation::Finding> {
    store
        .open_findings(100)
        .expect("findings")
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect()
}

/// The `ocfl_validation_errors` reason of the current health, if any.
fn ocfl_reason(rig: &ValidationRig) -> Option<filesquid_api::health::HealthReason> {
    rig.monitor
        .health_report()
        .reasons
        .into_iter()
        .find(|r| r.code == "ocfl_validation_errors")
}

#[test]
fn defaults_are_seven_days_and_one_hundred_thousand_objects() {
    assert_eq!(DEFAULT_INTERVAL_MS, WEEK_MS);
    assert_eq!(DEFAULT_OBJECT_CAP, 100_000);
    let config = ValidationConfig::default();
    assert_eq!((config.interval_ms, config.object_cap), (WEEK_MS, 100_000));
}

#[test]
fn an_e023_object_degrades_health_until_it_is_fixed_and_revalidated() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    rig.e023_object("obj-b", "urn:uuid:b");
    let record = rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(record.outcome, RunOutcome::Completed);
    assert_eq!((record.checked, record.not_checked), (2, 0));
    let health = rig.monitor.health_report();
    assert_eq!(health.status, HealthStatus::Degraded);
    let reason = ocfl_reason(&rig).expect("reason");
    assert_eq!(reason.codes, ["E023"]);
    assert_eq!(reason.objects, Some(1));
    let journaled = journaled_errors(&rig);
    assert_eq!(journaled.len(), 1, "one error finding journaled");
    assert_eq!(journaled[0].error_code.as_deref(), Some("E023"));
    assert_eq!(journaled[0].level, Level::Warn);
    assert_every_open_finding_journaled_once(&rig);
    assert_eq!(
        journaled[0].target_ids,
        [library().as_str(), "urn:uuid:b", "v1"]
    );
    rig.remove("obj-b/v1/content/stray.tmp");
    rig.run(RunTrigger::Requested, true).expect("revalidate");
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Ok);
    assert!(ocfl_reason(&rig).is_none());
    let closed = rig.journaled(FINDING_CLOSED_OPERATION);
    assert_eq!(closed.len(), 1, "only the E023 closed; the warnings stay");
    assert_eq!(closed[0].error_code.as_deref(), Some("E023"));
    assert_eq!(journaled_errors(&rig).len(), 1, "not journaled twice");
    assert_every_open_finding_journaled_once(&rig);
}

#[test]
fn a_finding_stays_open_until_a_later_validation_no_longer_reports_it() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, false).expect("first");
    rig.run(RunTrigger::Requested, true).expect("second");
    let open = open_errors(rig.store.as_ref());
    assert_eq!(
        open.len(),
        1,
        "still open after a second run that reports it"
    );
    assert_eq!(journaled_errors(&rig).len(), 1, "journaled when opened");
    assert_every_open_finding_journaled_once(&rig);
}

#[test]
fn a_w010_object_is_listed_as_a_warning_and_leaves_health_ok() {
    let rig = ValidationRig::new();
    rig.w010_object("obj-w", "urn:uuid:w");
    rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Ok);
    let view = rig.service.findings(true).expect("view");
    let w010: Vec<_> = view.findings.iter().filter(|f| f.code == "W010").collect();
    assert_eq!(w010.len(), 1);
    assert_eq!(w010[0].severity, Severity::Warning);
    let journaled: Vec<_> = rig
        .journaled(FINDING_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.as_deref() == Some("W010"))
        .collect();
    assert_eq!(journaled.len(), 1, "a warning is journaled like an error");
    assert_eq!(journaled[0].level, Level::Info);
    assert_eq!(
        journaled[0].target_ids[..2],
        [library().as_str(), "urn:uuid:w"]
    );
    assert_every_open_finding_journaled_once(&rig);
}

#[test]
fn a_run_over_many_objects_journals_one_entry_per_finding_with_no_loss() {
    let rig = ValidationRig::with_queue(2);
    for n in 0..40 {
        rig.object(&format!("obj-{n:02}"), &format!("urn:uuid:many-{n:02}"));
    }
    rig.w010_object("obj-w", "urn:uuid:w");
    let record = rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(record.checked, 41);
    let open = rig.store.open_findings(10_000).expect("findings");
    let warnings = open
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    assert!(warnings >= 41, "every object has a warning: {warnings}");
    assert_every_open_finding_journaled_once(&rig);
}

/// The journaled error findings (`E###`).
fn journaled_errors(rig: &ValidationRig) -> Vec<JournalEntry> {
    rig.journaled(FINDING_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.as_deref().is_some_and(|c| c.starts_with('E')))
        .collect()
}

/// Every open finding, error or warning, has exactly one `ocfl.validation.finding` entry
/// with its object and code, and no entry names a finding that is not open or closed.
fn assert_every_open_finding_journaled_once(rig: &ValidationRig) {
    let mut open: Vec<(String, String)> = rig
        .store
        .open_findings(10_000)
        .expect("findings")
        .into_iter()
        .map(|f| (f.object_id, f.code))
        .collect();
    let mut opened: Vec<(String, String)> = rig
        .journaled(FINDING_OPERATION)
        .into_iter()
        .map(|e| (e.target_ids[1].clone(), e.error_code.unwrap_or_default()))
        .collect();
    let mut closed: Vec<(String, String)> = rig
        .journaled(FINDING_CLOSED_OPERATION)
        .into_iter()
        .map(|e| (e.target_ids[1].clone(), e.error_code.unwrap_or_default()))
        .collect();
    open.append(&mut closed);
    open.sort();
    opened.sort();
    assert_eq!(opened, open, "one entry per finding, none lost");
}

#[test]
fn a_takeover_commit_triggers_validation_of_that_object() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    vfix::ready_async(
        rig.service
            .validate_after_commit(&library(), "obj-b", "urn:uuid:b", false),
    )
    .expect("plain commit");
    assert!(
        rig.store.open_findings(100).expect("findings").is_empty(),
        "no takeover, no check"
    );
    vfix::ready_async(
        rig.service
            .validate_after_commit(&library(), "obj-b", "urn:uuid:b", true),
    )
    .expect("takeover commit");
    let open = open_errors(rig.store.as_ref());
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].code, "E023");
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Degraded);
}

#[test]
fn locations_are_shown_only_to_a_caller_who_may_read_the_object() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, false).expect("run");
    let reader = rig.service.findings(true).expect("owner view");
    let e023 = reader
        .findings
        .iter()
        .find(|f| f.code == "E023")
        .expect("E023");
    assert!(e023
        .location
        .as_deref()
        .is_some_and(|l| l.contains("stray")));
    let other = rig.service.findings(false).expect("administrator view");
    assert!(other.findings.iter().all(|f| f.location.is_none()));
    assert!(other.findings.iter().any(|f| f.code == "E023"));
}

#[test]
fn journal_rows_and_health_reasons_carry_no_path_title_or_content() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, false).expect("run");
    rig.delete_object("obj-b");
    rig.run(RunTrigger::Requested, false).expect("close");
    for entry in rig.journal() {
        let ids = entry.target_ids.join(" ").replace("urn:uuid:", "");
        assert_redacted(&format!(
            "{} {} {:?}",
            entry.operation, ids, entry.error_code
        ));
    }
    rig.e023_object("obj-c", "urn:uuid:c");
    rig.run(RunTrigger::Requested, true).expect("reopen");
    let reason = ocfl_reason(&rig).expect("reason");
    assert_redacted(&serde_json::to_string(&reason).expect("json"));
}

#[test]
fn a_run_stops_at_its_cap_and_reports_what_it_did_not_check() {
    let rig = ValidationRig::new();
    for n in 0..3 {
        rig.object(&format!("obj-{n}"), &format!("urn:x:{n}"));
    }
    rig.cap(2);
    let record = rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(record.outcome, RunOutcome::Capped);
    assert_eq!((record.checked, record.not_checked), (2, 1));
    let runs = rig.journaled(RUN_OPERATION);
    assert_eq!(
        runs.last().expect("run journaled").outcome,
        Outcome::Succeeded
    );
}

#[test]
fn a_run_registers_in_the_process_registry_with_progress() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    rig.run(RunTrigger::Requested, false).expect("run");
    let processes = rig.monitor.registry().snapshot();
    let run = processes
        .iter()
        .find(|p| p.kind == "ocfl.validation")
        .expect("registered");
    assert_eq!(run.done, 1);
    assert_eq!(run.total, Some(1));
}

#[test]
fn a_run_never_starts_twice_at_once_for_one_library() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let second = Arc::new(Mutex::new(None));
    let (service, slot) = (rig.service.clone(), second.clone());
    rig.root.on_read(
        "obj-a/v1/content/a.txt",
        Box::new(move || {
            let id = library();
            let run = service.run(&id, RunTrigger::Requested, false, "owner");
            *slot.lock().expect("lock") = Some(vfix::ready_async(run));
        }),
    );
    rig.run(RunTrigger::Requested, false).expect("first run");
    let refused = second
        .lock()
        .expect("lock")
        .take()
        .expect("second run tried");
    assert_eq!(
        refused.expect_err("refused").error_code(),
        "already_running"
    );
    let refusals: Vec<_> = rig
        .journaled(RUN_OPERATION)
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(refusals.len(), 1, "the refusal is journaled");
    assert_eq!(refusals[0].error_code.as_deref(), Some("already_running"));
    rig.run(RunTrigger::Requested, false).expect("free again");
}

#[test]
fn a_cancelled_run_stops_and_reports_the_rest() {
    let rig = ValidationRig::new();
    for n in 0..3 {
        rig.object(&format!("obj-{n}"), &format!("urn:x:{n}"));
    }
    let service = rig.service.clone();
    rig.root.on_read(
        "obj-0/v1/content/a.txt",
        Box::new(move || assert!(service.cancel(&library()), "a run is going")),
    );
    let record = rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(record.outcome, RunOutcome::Cancelled);
    assert_eq!((record.checked, record.not_checked), (1, 2));
    assert!(!rig.service.cancel(&library()), "nothing left to cancel");
}

#[test]
fn the_findings_view_reports_the_last_run_and_the_next_scheduled_run() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let before = rig.service.findings(true).expect("view");
    let lib = &before.libraries[0];
    assert!(lib.last_run.is_none());
    let first_next = lib.next_run.clone().expect("a next run is scheduled");
    rig.advance(1_000);
    rig.run(RunTrigger::Requested, false).expect("run");
    let after = rig.service.findings(true).expect("view");
    let lib = &after.libraries[0];
    let last = lib.last_run.as_ref().expect("last run");
    assert_eq!((last.checked, last.not_checked), (1, 0));
    assert_eq!(last.outcome, RunOutcome::Completed);
    assert!(last.started.ends_with('Z') && last.ended.ends_with('Z'));
    assert_ne!(
        lib.next_run.as_deref(),
        Some(first_next.as_str()),
        "the next run moved"
    );
}

#[test]
fn a_library_is_due_one_interval_after_it_was_opened() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    assert!(
        rig.service.due_libraries().is_empty(),
        "not due right after opening"
    );
    rig.advance(WEEK_MS);
    assert_eq!(rig.service.due_libraries(), [library()]);
}

#[tokio::test]
async fn the_scheduler_tick_starts_a_scheduled_run_when_one_is_due() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    OcflValidation::tick(&rig.service);
    assert!(
        rig.store.last_run(&library()).expect("store").is_none(),
        "nothing due yet"
    );
    rig.advance(WEEK_MS);
    OcflValidation::tick(&rig.service);
    for _ in 0..200 {
        if rig.store.last_run(&library()).expect("store").is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let run = rig.store.last_run(&library()).expect("store").expect("ran");
    assert_eq!(run.trigger, RunTrigger::Scheduled);
    assert!(
        rig.service.due_libraries().is_empty(),
        "the next run is a week later"
    );
}

#[test]
fn capped_runs_resume_from_the_cursor_in_object_id_order_and_wrap() {
    let rig = ValidationRig::new();
    for n in [3, 1, 4, 2] {
        rig.object(&format!("obj-{n}"), &format!("urn:x:{n}"));
    }
    rig.cap(3);
    rig.run(RunTrigger::Scheduled, false).expect("run 1");
    assert_eq!(
        rig.store.cursor(&library()).expect("cursor").as_deref(),
        Some("urn:x:3")
    );
    let restarted = ValidationRig::with_store(rig.store.clone(), rig.root.clone());
    restarted.cap(3);
    restarted.remove("obj-4/0=ocfl_object_1.1");
    restarted.delete_object("obj-4");
    restarted.object("obj-0", "urn:x:0");
    let record = restarted.run(RunTrigger::Scheduled, false).expect("run 2");
    assert_eq!(
        record.checked, 3,
        "the deleted object ahead is skipped, the pass wraps"
    );
    assert_eq!(
        restarted
            .store
            .cursor(&library())
            .expect("cursor")
            .as_deref(),
        Some("urn:x:2"),
        "run 2 found nothing after urn:x:3, wrapped and checked urn:x:0 to urn:x:2"
    );
}

#[test]
fn an_on_request_run_continues_from_the_cursor_unless_asked_to_start_over() {
    let rig = ValidationRig::new();
    for n in 1..=4 {
        rig.object(&format!("obj-{n}"), &format!("urn:x:{n}"));
    }
    rig.cap(2);
    rig.run(RunTrigger::Requested, false).expect("run 1");
    rig.run(RunTrigger::Requested, false).expect("run 2");
    assert_eq!(
        rig.store.cursor(&library()).expect("cursor").as_deref(),
        Some("urn:x:4")
    );
    rig.run(RunTrigger::Requested, true)
        .expect("from the start");
    assert_eq!(
        rig.store.cursor(&library()).expect("cursor").as_deref(),
        Some("urn:x:2")
    );
}

#[test]
fn deleting_an_object_with_an_open_e023_closes_its_finding() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Degraded);
    rig.delete_object("obj-b");
    rig.run(RunTrigger::Requested, false).expect("run");
    let closed: Vec<_> = rig
        .journaled(FINDING_CLOSED_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.as_deref() == Some("E023"))
        .collect();
    assert_eq!(closed.len(), 1);
    assert_eq!(
        closed[0].target_ids,
        [library().as_str(), "urn:uuid:b", "v1"]
    );
    assert!(
        rig.store.open_findings(100).expect("findings").is_empty(),
        "warnings close too"
    );
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Ok);
}

#[tokio::test]
async fn a_change_in_the_open_findings_is_pushed_on_health_change() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    let mut events = rig.events();
    rig.service
        .run(&library(), RunTrigger::Requested, false, "owner")
        .await
        .expect("run");
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let event = events.next().await.expect("event");
            if event.topic == "health.change" {
                return event;
            }
        }
    })
    .await
    .expect("health.change pushed");
    assert!(event.data.to_string().contains("ocfl_validation_errors"));
}

#[test]
fn the_configuration_is_bounded_and_journaled() {
    let rig = ValidationRig::new();
    let zero = ValidationConfig {
        interval_ms: 0,
        object_cap: 1,
    };
    assert_eq!(
        rig.service
            .configure("owner", zero)
            .unwrap_err()
            .error_code(),
        "invalid_config"
    );
    assert_eq!(
        rig.service.config(),
        ValidationConfig::default(),
        "unchanged"
    );
    let none = ValidationConfig {
        interval_ms: WEEK_MS,
        object_cap: 0,
    };
    assert_eq!(
        rig.service
            .configure("owner", none)
            .unwrap_err()
            .error_code(),
        "invalid_config"
    );
    assert_eq!(
        rig.service.config(),
        ValidationConfig::default(),
        "unchanged"
    );
    rig.cap(5);
    assert_eq!(rig.service.config().object_cap, 5);
    let journaled = rig.journaled(CONFIGURE_OPERATION);
    let outcomes: Vec<Outcome> = journaled.iter().map(|e| e.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Failed, Outcome::Failed, Outcome::Succeeded]
    );
    assert_eq!(journaled[0].error_code.as_deref(), Some("invalid_config"));
    assert_eq!(journaled[2].actor, "owner");
}

#[test]
fn start_runs_refuses_an_unknown_library_and_a_busy_one() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let unknown = RunRequest {
        library_id: Some(filesquid_core::LibraryId::new("other").expect("id")),
        from_start: false,
    };
    let err = rig
        .service
        .start_runs(unknown, "owner")
        .expect_err("unknown");
    assert_eq!(err.error_code(), "not_found");
    let refused = Arc::new(Mutex::new(None));
    let (service, slot) = (rig.service.clone(), refused.clone());
    rig.root.on_read(
        "obj-a/v1/content/a.txt",
        Box::new(move || {
            let busy = RunRequest {
                library_id: Some(library()),
                from_start: false,
            };
            *slot.lock().expect("lock") = Some(service.start_runs(busy, "owner"));
        }),
    );
    rig.run(RunTrigger::Requested, false).expect("run");
    let busy = refused.lock().expect("lock").take().expect("tried");
    assert_eq!(busy.expect_err("busy").error_code(), "already_running");
}

#[test]
fn an_e023_object_on_a_real_directory_is_found() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("api-f2.7-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = filesquid_ocfl::storage::FsBackend::new(dir.join("root"), dir.join("staging"))
        .expect("fs backend");
    let rig = ValidationRig::new();
    let store: Arc<dyn FindingsStore> = Arc::new(MemoryFindingsStore::new());
    let service = ValidationService::new(store.clone(), rig.monitor.clone());
    let fs = Arc::new(backend);
    fixture::ready(fs.put(&fixture::key("0=ocfl_1.1"), b"ocfl_1.1\n")).expect("declare");
    let leases = filesquid_ocfl::write::LeaseStore::new(
        filesquid_ocfl::storage::MemoryBackend::new(),
        "leases",
        fixture::ManualClock::at(fixture::NOW_MS as u64),
        filesquid_ocfl::write::LeaseConfig::default(),
    )
    .expect("leases");
    let staging = filesquid_ocfl::storage::MemoryBackend::new();
    let staged = fixture::key("stage/a.txt");
    fixture::ready(staging.put(&staged, b"alpha")).expect("stage");
    let mut draft = filesquid_ocfl::write::VersionDraft::new("urn:uuid:fs", "2026-10-18T00:00:00Z");
    draft.stage("a.txt", staged);
    let writer = filesquid_ocfl::write::ObjectWriter::new(&*fs, &leases, "w");
    fixture::ready(writer.commit("obj-fs", None, &staging, &draft)).expect("commit");
    fixture::ready(fs.put(&fixture::key("obj-fs/v1/content/stray.tmp"), b"x")).expect("stray");
    service.open_library(library(), fs);
    vfix::ready_async(service.run(&library(), RunTrigger::Requested, false, "owner")).expect("run");
    let open = open_errors(store.as_ref());
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].code, "E023");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- The routes, through the router ----

/// Call `method path` with `token` (none for an anonymous call) and `body`; answer the
/// status and JSON body.
async fn call_route(
    state: &filesquid_api::ApiState,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: &str,
) -> (u16, serde_json::Value) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let request = request
        .body(axum::body::Body::from(body.to_string()))
        .expect("request");
    let response = filesquid_api::router(state.clone())
        .oneshot(request)
        .await
        .expect("answer");
    let status = response.status().as_u16();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Path of the findings route.
const FINDINGS: &str = "/api/v1/admin/ocfl/findings";

/// Path of the run route.
const RUNS: &str = "/api/v1/admin/ocfl/validation-runs";

#[tokio::test]
async fn the_owner_sees_locations_and_the_administrator_does_not() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.service
        .run(&library(), RunTrigger::Requested, false, "owner")
        .await
        .expect("run");
    let token = filesquid_api::auth::LaunchToken::generate().expect("token");
    let owner = rig.state.clone();
    owner.install_authoriser(filesquid_api::auth::Authoriser::desktop_owner(&token));
    let (status, body) = call_route(&owner, "GET", FINDINGS, Some(token.secret()), "").await;
    assert_eq!(status, 200, "{body}");
    assert!(
        body.to_string().contains("stray.tmp"),
        "the owner sees where"
    );
    let admin_rig = ValidationRig::with_store(rig.store.clone(), rig.root.clone());
    admin_rig
        .state
        .install_authoriser(filesquid_api::auth::Authoriser::server_administrator(
            "admin",
        ));
    let (status, body) = call_route(&admin_rig.state, "GET", FINDINGS, Some("admin"), "").await;
    assert_eq!(status, 200, "{body}");
    assert!(body.to_string().contains("E023"));
    assert!(
        !body.to_string().contains("stray"),
        "no location for the administrator"
    );
}

#[tokio::test]
async fn anyone_else_is_refused_and_the_refusal_is_journaled() {
    let rig = ValidationRig::new();
    rig.state
        .install_authoriser(filesquid_api::auth::Authoriser::server_administrator(
            "admin",
        ));
    let (status, _) = call_route(&rig.state, "GET", FINDINGS, None, "").await;
    assert_eq!(status, 401);
    let (status, _) = call_route(&rig.state, "POST", RUNS, Some("guess"), "").await;
    assert_eq!(status, 403);
    let denials: Vec<_> = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == filesquid_api::auth::DENIED_OPERATION)
        .collect();
    assert_eq!(denials.len(), 2, "both refusals are journaled");
}

#[tokio::test]
async fn a_run_request_starts_runs_and_a_malformed_one_is_refused() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.state
        .install_authoriser(filesquid_api::auth::Authoriser::server_administrator(
            "admin",
        ));
    let (status, body) = call_route(&rig.state, "POST", RUNS, Some("admin"), "{\"from\":1}").await;
    assert_eq!(status, 400, "{body}");
    let refused = rig.journaled(RUN_OPERATION);
    assert_eq!(
        refused.last().expect("journaled").error_code.as_deref(),
        Some("invalid_request")
    );
    let (status, body) = call_route(&rig.state, "POST", RUNS, Some("admin"), "").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["started"], serde_json::json!([library().as_str()]));
    for _ in 0..200 {
        if rig.store.last_run(&library()).expect("store").is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        rig.store.last_run(&library()).expect("store").is_some(),
        "the run finished"
    );
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Degraded);
}

/// A findings store that fails every call, as an unreachable operational store does.
struct FailingStore;

impl FindingsStore for FailingStore {
    fn replace_findings(
        &self,
        _: &filesquid_core::LibraryId,
        _: &str,
        _: Vec<filesquid_api::ocfl_validation::Finding>,
    ) -> Result<
        filesquid_api::ocfl_validation::FindingChange,
        filesquid_api::ocfl_validation::StoreError,
    > {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn objects_with_findings(
        &self,
        _: &filesquid_core::LibraryId,
    ) -> Result<Vec<String>, filesquid_api::ocfl_validation::StoreError> {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn open_findings(
        &self,
        _: usize,
    ) -> Result<
        Vec<filesquid_api::ocfl_validation::Finding>,
        filesquid_api::ocfl_validation::StoreError,
    > {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn error_summary(
        &self,
    ) -> Result<
        filesquid_api::ocfl_validation::ErrorSummary,
        filesquid_api::ocfl_validation::StoreError,
    > {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn cursor(
        &self,
        _: &filesquid_core::LibraryId,
    ) -> Result<Option<String>, filesquid_api::ocfl_validation::StoreError> {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn set_cursor(
        &self,
        _: &filesquid_core::LibraryId,
        _: Option<&str>,
    ) -> Result<(), filesquid_api::ocfl_validation::StoreError> {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn record_run(
        &self,
        _: &filesquid_api::ocfl_validation::RunRecord,
    ) -> Result<(), filesquid_api::ocfl_validation::StoreError> {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
    fn last_run(
        &self,
        _: &filesquid_core::LibraryId,
    ) -> Result<
        Option<filesquid_api::ocfl_validation::RunRecord>,
        filesquid_api::ocfl_validation::StoreError,
    > {
        Err(filesquid_api::ocfl_validation::StoreError::unavailable(
            "down",
        ))
    }
}

#[test]
fn a_failing_findings_store_fails_health_and_the_run() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let monitor = filesquid_api::monitor::Monitor::start(
        Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
        Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        filesquid_api::monitor::MonitorConfig::new("desktop"),
        filesquid_api::EventBus::new(),
    );
    let service: ValidationService<fixture::CountingBackend> =
        ValidationService::new(Arc::new(FailingStore), monitor.clone());
    service.open_library(library(), rig.root.clone());
    let health = monitor.health_report();
    assert!(
        health.reasons.iter().any(|r| r.code == "store_unreachable"),
        "{health:?}"
    );
    let record = vfix::ready_async(service.run(&library(), RunTrigger::Requested, false, "owner"))
        .expect("answered");
    assert_eq!(record.outcome, RunOutcome::Failed);
    assert_eq!(record.error_code.as_deref(), Some("unavailable"));
    monitor.shutdown();
}

/// A memory store whose cursor moves fail after `ok` successful moves, and whose run records
/// fail when `records_fail` is set.
struct FlakyCursor {
    inner: MemoryFindingsStore,
    ok: std::sync::atomic::AtomicUsize,
    records_fail: bool,
}

impl FindingsStore for FlakyCursor {
    fn replace_findings(
        &self,
        l: &filesquid_core::LibraryId,
        o: &str,
        f: Vec<filesquid_api::ocfl_validation::Finding>,
    ) -> Result<
        filesquid_api::ocfl_validation::FindingChange,
        filesquid_api::ocfl_validation::StoreError,
    > {
        self.inner.replace_findings(l, o, f)
    }
    fn objects_with_findings(
        &self,
        l: &filesquid_core::LibraryId,
    ) -> Result<Vec<String>, filesquid_api::ocfl_validation::StoreError> {
        self.inner.objects_with_findings(l)
    }
    fn open_findings(
        &self,
        n: usize,
    ) -> Result<
        Vec<filesquid_api::ocfl_validation::Finding>,
        filesquid_api::ocfl_validation::StoreError,
    > {
        self.inner.open_findings(n)
    }
    fn error_summary(
        &self,
    ) -> Result<
        filesquid_api::ocfl_validation::ErrorSummary,
        filesquid_api::ocfl_validation::StoreError,
    > {
        self.inner.error_summary()
    }
    fn cursor(
        &self,
        l: &filesquid_core::LibraryId,
    ) -> Result<Option<String>, filesquid_api::ocfl_validation::StoreError> {
        self.inner.cursor(l)
    }
    fn set_cursor(
        &self,
        l: &filesquid_core::LibraryId,
        c: Option<&str>,
    ) -> Result<(), filesquid_api::ocfl_validation::StoreError> {
        let left = self.ok.load(std::sync::atomic::Ordering::SeqCst);
        if left == 0 {
            return Err(filesquid_api::ocfl_validation::StoreError::unavailable(
                "busy",
            ));
        }
        self.ok.store(left - 1, std::sync::atomic::Ordering::SeqCst);
        self.inner.set_cursor(l, c)
    }
    fn record_run(
        &self,
        r: &filesquid_api::ocfl_validation::RunRecord,
    ) -> Result<(), filesquid_api::ocfl_validation::StoreError> {
        if self.records_fail {
            return Err(filesquid_api::ocfl_validation::StoreError::unavailable(
                "busy",
            ));
        }
        self.inner.record_run(r)
    }
    fn last_run(
        &self,
        l: &filesquid_core::LibraryId,
    ) -> Result<
        Option<filesquid_api::ocfl_validation::RunRecord>,
        filesquid_api::ocfl_validation::StoreError,
    > {
        self.inner.last_run(l)
    }
}

#[test]
fn a_run_stopped_by_an_outage_reports_what_it_checked_and_publishes_health() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-0", "urn:x:0");
    rig.object("obj-1", "urn:x:1");
    rig.object("obj-2", "urn:x:2");
    let store = Arc::new(FlakyCursor {
        inner: MemoryFindingsStore::new(),
        ok: std::sync::atomic::AtomicUsize::new(1),
        records_fail: false,
    });
    let monitor = filesquid_api::monitor::Monitor::start(
        Arc::new(filesquid_api::journal::MemoryJournal::new("desktop")),
        Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        filesquid_api::monitor::MonitorConfig::new("desktop"),
        rig.state.events.clone(),
    );
    let service: ValidationService<fixture::CountingBackend> =
        ValidationService::new(store.clone(), monitor.clone());
    service.open_library(library(), rig.root.clone());
    let mut events = rig.events();
    let record = vfix::ready_async(service.run(&library(), RunTrigger::Requested, false, "owner"))
        .expect("answered");
    assert_eq!(record.outcome, RunOutcome::Failed);
    assert_eq!((record.checked, record.not_checked), (1, 2));
    assert_eq!(record.error_code.as_deref(), Some("unavailable"));
    assert_eq!(
        monitor.health_report().status,
        HealthStatus::Degraded,
        "E023 was stored"
    );
    let mut topics = Vec::new();
    for _ in 0..100 {
        match vfix::ready_async(events.next()) {
            Some(event) => topics.push(event.topic),
            None => break,
        }
        if topics.iter().any(|t| t == "health.change") {
            break;
        }
    }
    assert!(topics.iter().any(|t| t == "health.change"), "{topics:?}");
    monitor.shutdown();
}

#[test]
fn shutting_down_cancels_runs_and_refuses_new_ones() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let service = rig.service.clone();
    rig.root.on_read(
        "obj-a/v1/content/a.txt",
        Box::new(move || {
            assert!(
                !service.shut_down(std::time::Duration::from_millis(40)),
                "the run is going"
            );
        }),
    );
    let record = rig.run(RunTrigger::Requested, false).expect("run");
    assert_eq!(
        record.outcome,
        RunOutcome::Completed,
        "the object being checked finishes"
    );
    let refused = rig.run(RunTrigger::Requested, false).expect_err("stopped");
    assert_eq!(refused.error_code(), "unavailable");
    assert!(
        rig.service.shut_down(std::time::Duration::from_millis(40)),
        "nothing left"
    );
}

#[test]
fn a_takeover_check_that_cannot_start_is_journaled() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    OcflValidation::on_commit(&rig.service, &library(), "obj-b", "urn:uuid:b", true);
    let refused = rig.journaled(RUN_OPERATION);
    assert_eq!(
        refused.len(),
        1,
        "no runtime: the skipped check is journaled"
    );
    assert_eq!(refused[0].error_code.as_deref(), Some("unavailable"));
    OcflValidation::on_commit(&rig.service, &library(), "obj-b", "urn:uuid:b", false);
    assert_eq!(
        rig.journaled(RUN_OPERATION).len(),
        1,
        "no takeover, nothing to do"
    );
}

#[test]
fn a_takeover_check_reads_only_the_named_object() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.object("obj-c", "urn:uuid:c");
    vfix::ready_async(
        rig.service
            .validate_after_commit(&library(), "obj-c", "urn:uuid:b", true),
    )
    .expect("another object at that root");
    assert!(
        rig.store.open_findings(100).expect("findings").is_empty(),
        "left alone"
    );
}

#[tokio::test]
async fn the_settings_route_changes_the_schedule_and_refuses_bad_values() {
    let rig = ValidationRig::new();
    rig.state
        .install_authoriser(filesquid_api::auth::Authoriser::server_administrator(
            "admin",
        ));
    let path = "/api/v1/admin/ocfl/validation-settings";
    let body = format!("{{\"interval_ms\":{},\"object_cap\":7}}", 2 * WEEK_MS);
    let (status, answer) = call_route(&rig.state, "PUT", path, Some("admin"), &body).await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["object_cap"], 7);
    let (status, view) = call_route(&rig.state, "GET", FINDINGS, Some("admin"), "").await;
    assert_eq!(status, 200);
    assert_eq!(view["settings"]["interval_ms"], 2 * WEEK_MS);
    let (status, _) = call_route(&rig.state, "PUT", path, Some("admin"), "{}").await;
    assert_eq!(status, 400);
    let (status, _) = call_route(
        &rig.state,
        "PUT",
        path,
        Some("admin"),
        "{\"interval_ms\":1,\"object_cap\":1}",
    )
    .await;
    assert_eq!(status, 400);
    let refusals: Vec<_> = rig
        .journaled(CONFIGURE_OPERATION)
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(refusals.len(), 2, "both refusals are journaled");
}

#[test]
fn a_findings_change_journals_a_health_transition_once() {
    let rig = ValidationRig::new();
    rig.monitor.tick();
    let before = rig.journaled("health.changed").len();
    rig.monitor.publish_health();
    assert_eq!(
        rig.journaled("health.changed").len(),
        before,
        "no transition, no entry"
    );
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, false).expect("run");
    let after = rig.journaled("health.changed");
    assert_eq!(after.len(), before + 1, "ok to degraded journaled once");
    assert_eq!(
        after.last().expect("entry").error_code.as_deref(),
        Some("ocfl_validation_errors")
    );
    rig.monitor.tick();
    assert_eq!(
        rig.journaled("health.changed").len(),
        before + 1,
        "the tick sees no new transition"
    );
}

#[test]
fn a_takeover_check_on_a_damaged_object_opens_an_error_finding() {
    let rig = ValidationRig::new();
    rig.object("obj-b", "urn:uuid:b");
    rig.remove("obj-b/inventory.json");
    vfix::ready_async(
        rig.service
            .validate_after_commit(&library(), "obj-b", "urn:uuid:b", true),
    )
    .expect("checked");
    let open = open_errors(rig.store.as_ref());
    assert!(!open.is_empty(), "the damage is a finding");
    assert!(
        open[0].object_id.starts_with("root:"),
        "{:?}",
        open[0].object_id
    );
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Degraded);
}

#[test]
fn a_run_whose_record_cannot_be_stored_is_reported_as_failed() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    let store = Arc::new(FlakyCursor {
        inner: MemoryFindingsStore::new(),
        ok: std::sync::atomic::AtomicUsize::new(usize::MAX),
        records_fail: true,
    });
    let service: ValidationService<fixture::CountingBackend> =
        ValidationService::new(store, rig.monitor.clone());
    let other = filesquid_core::LibraryId::new("lib-records").expect("id");
    service.open_library(other.clone(), rig.root.clone());
    let record = vfix::ready_async(service.run(&other, RunTrigger::Requested, false, "owner"))
        .expect("answered");
    assert_eq!(
        record.outcome,
        RunOutcome::Failed,
        "never reported as a success"
    );
    assert_eq!(record.checked, 1);
    assert_eq!(record.error_code.as_deref(), Some("unavailable"));
    let runs = rig.journaled(RUN_OPERATION);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].outcome, Outcome::Failed);
    assert_eq!(runs[0].error_code.as_deref(), Some("unavailable"));
}

#[test]
fn two_roots_that_declare_one_identifier_keep_separate_findings() {
    let rig = ValidationRig::new();
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.object("obj-c", "urn:uuid:b");
    for _ in 0..2 {
        let record = rig.run(RunTrigger::Requested, true).expect("run");
        assert_eq!((record.checked, record.not_checked), (2, 0));
    }
    let open = open_errors(rig.store.as_ref());
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(journaled_e023(&rig, FINDING_OPERATION), 1, "opened once");
    assert!(
        rig.journaled(FINDING_CLOSED_OPERATION).is_empty(),
        "the two roots never overwrite each other's findings"
    );
    let warnings_first = rig.journaled(FINDING_OPERATION).len();
    rig.run(RunTrigger::Requested, true).expect("third run");
    assert_eq!(
        rig.journaled(FINDING_OPERATION).len(),
        warnings_first,
        "a later run opens nothing again"
    );
}

#[test]
fn an_object_deleted_after_the_run_indexed_it_has_its_findings_closed() {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    rig.e023_object("obj-b", "urn:uuid:b");
    rig.run(RunTrigger::Requested, true).expect("first run");
    assert_eq!(open_errors(rig.store.as_ref()).len(), 1);
    let root = rig.root.clone();
    rig.root.on_read(
        "obj-a/v1/content/a.txt",
        Box::new(move || {
            let page = fixture::ready(root.list("obj-b/", None, 1_000)).expect("list");
            for k in page.keys {
                fixture::ready(root.delete(&k)).expect("delete");
            }
        }),
    );
    let record = rig.run(RunTrigger::Requested, true).expect("second run");
    assert_eq!(record.outcome, RunOutcome::Completed);
    assert!(
        open_errors(rig.store.as_ref()).is_empty(),
        "no missing-declaration finding"
    );
    let opened_before = rig.journaled(FINDING_OPERATION).len();
    assert_eq!(
        journaled_e023(&rig, FINDING_OPERATION),
        1,
        "nothing new opened"
    );
    assert_eq!(
        journaled_e023(&rig, FINDING_CLOSED_OPERATION),
        1,
        "E023 closed"
    );
    rig.run(RunTrigger::Requested, true).expect("third run");
    assert_eq!(
        rig.journaled(FINDING_OPERATION).len(),
        opened_before,
        "the gone object opens no finding"
    );
    assert_eq!(rig.monitor.health_report().status, HealthStatus::Ok);
}

/// How many `operation` entries of `rig`'s journal carry `E023`.
fn journaled_e023(rig: &ValidationRig, operation: &str) -> usize {
    rig.journaled(operation)
        .iter()
        .filter(|e| e.error_code.as_deref() == Some("E023"))
        .count()
}
