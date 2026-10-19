//! Step definitions for `ocfl_validation_findings_API-S-022.feature` (Task API-F2.7): OCFL
//! validation runs over a library on the in-memory backend, driven through
//! `ValidationService`, with health read from the monitor.

use crate::validation_fixture::{ready_async, ValidationRig};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::health::HealthStatus;
use filesquid_api::ocfl_validation::{FindingsStore, RunOutcome, RunRecord, RunTrigger, Severity};

/// Per-scenario state of the validation steps.
#[derive(Default)]
pub struct ValidationWorld {
    rig: Option<ValidationRig>,
    last: Option<RunRecord>,
}

impl std::fmt::Debug for ValidationWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidationWorld").finish_non_exhaustive()
    }
}

impl ValidationWorld {
    fn rig(&self) -> &ValidationRig {
        self.rig.as_ref().expect("rig")
    }
}

/// Assert `text` names no path, file or content.
fn assert_no_paths(text: &str) {
    for leak in ["stray", "content", "obj-", "/"] {
        assert!(!text.contains(leak), "{leak} leaked in {text}");
    }
}

#[given(
    "an open library with a valid object and an object with a stray file under its head version"
)]
fn library_with_e023(w: &mut MonitorWorld) {
    let rig = ValidationRig::new();
    rig.object("obj-a", "urn:uuid:a");
    rig.e023_object("obj-b", "urn:uuid:b");
    w.validation.rig = Some(rig);
}

#[given("an open library with an object whose head has no version inventory")]
fn library_with_w010(w: &mut MonitorWorld) {
    let rig = ValidationRig::new();
    rig.w010_object("obj-w", "urn:uuid:w");
    w.validation.rig = Some(rig);
}

#[given(expr = "an open library with {int} valid objects and a run cap of {int}")]
fn library_with_cap(w: &mut MonitorWorld, objects: usize, cap: u64) {
    let rig = ValidationRig::new();
    for n in 0..objects {
        rig.object(&format!("obj-{n}"), &format!("urn:x:{n}"));
    }
    rig.cap(cap);
    w.validation.rig = Some(rig);
}

#[when("the owner requests a validation run")]
fn request_run(w: &mut MonitorWorld) {
    let record = w
        .validation
        .rig()
        .run(RunTrigger::Requested, false)
        .expect("run");
    w.validation.last = Some(record);
}

#[when("the stray file is removed and the library is validated again")]
fn remove_stray(w: &mut MonitorWorld) {
    let rig = w.validation.rig();
    rig.remove("obj-b/v1/content/stray.tmp");
    rig.run(RunTrigger::Requested, true).expect("run");
}

#[when("a commit on the object with the stray file takes over an expired reservation")]
fn takeover(w: &mut MonitorWorld) {
    let rig = w.validation.rig();
    let library = rig.library();
    ready_async(
        rig.service
            .validate_after_commit(&library, "obj-b", "urn:uuid:b", true),
    )
    .expect("check");
}

#[when("the object with the stray file is deleted and the library is validated again")]
fn delete_object(w: &mut MonitorWorld) {
    let rig = w.validation.rig();
    rig.delete_object("obj-b");
    rig.run(RunTrigger::Requested, false).expect("run");
}

#[then(expr = "health should be {string} with reason {string} naming {string} in {int} object")]
fn degraded(w: &mut MonitorWorld, status: String, code: String, error: String, objects: usize) {
    let report = w.validation.rig().monitor.health_report();
    assert_eq!(
        serde_json::to_value(report.status).expect("json"),
        status.as_str()
    );
    let reason = report
        .reasons
        .iter()
        .find(|r| r.code == code)
        .expect("reason");
    assert_eq!(reason.codes, [error]);
    assert_eq!(reason.objects, Some(objects));
    assert_no_paths(&serde_json::to_string(reason).expect("json"));
}

#[then(expr = "health should be {string}")]
fn health_is(w: &mut MonitorWorld, status: String) {
    let report = w.validation.rig().monitor.health_report();
    let expected = match status.as_str() {
        "ok" => HealthStatus::Ok,
        "degraded" => HealthStatus::Degraded,
        "failing" => HealthStatus::Failing,
        other => panic!("unknown health status {other:?} in the feature"),
    };
    assert_eq!(report.status, expected, "{:?}", report.reasons);
}

#[then(expr = "one {string} entry should be journaled without paths")]
fn one_entry(w: &mut MonitorWorld, operation: String) {
    let entries = w.validation.rig().journaled(&operation);
    assert_eq!(entries.len(), 1);
    let ids = entries[0].target_ids.join(" ").replace("urn:uuid:", "");
    assert_no_paths(&ids);
}

#[then(expr = "one {string} entry for {string} should be journaled without paths")]
fn one_entry_for(w: &mut MonitorWorld, operation: String, code: String) {
    let entries: Vec<_> = w
        .validation
        .rig()
        .journaled(&operation)
        .into_iter()
        .filter(|e| e.error_code.as_deref() == Some(code.as_str()))
        .collect();
    assert_eq!(entries.len(), 1);
    assert_no_paths(&entries[0].target_ids.join(" ").replace("urn:uuid:", ""));
}

#[then(expr = "the findings should list {string} as a warning")]
fn listed_warning(w: &mut MonitorWorld, code: String) {
    let open = w
        .validation
        .rig()
        .store
        .open_findings(100)
        .expect("findings");
    let found = open.iter().find(|f| f.code == code).expect("listed");
    assert_eq!(found.severity, Severity::Warning);
}

#[then(expr = "the owner should see where the {string} finding is")]
fn owner_sees(w: &mut MonitorWorld, code: String) {
    let view = w.validation.rig().service.findings(true).expect("view");
    let found = view
        .findings
        .iter()
        .find(|f| f.code == code)
        .expect("finding");
    assert!(found.location.is_some());
}

#[then(expr = "the server administrator should see the {string} finding without its location")]
fn administrator_sees(w: &mut MonitorWorld, code: String) {
    let view = w.validation.rig().service.findings(false).expect("view");
    let found = view
        .findings
        .iter()
        .find(|f| f.code == code)
        .expect("finding");
    assert!(found.location.is_none());
}

#[then(expr = "the run should be {string} with {int} objects checked and {int} not checked")]
fn run_was(w: &mut MonitorWorld, outcome: String, checked: u64, not_checked: u64) {
    let record = w.validation.last.as_ref().expect("run");
    assert_eq!(record.outcome.as_str(), outcome);
    assert_eq!(record.outcome == RunOutcome::Capped, outcome == "capped");
    assert_eq!((record.checked, record.not_checked), (checked, not_checked));
}

#[then("the cursor should have wrapped to the first object")]
fn wrapped(w: &mut MonitorWorld) {
    let rig = w.validation.rig();
    let cursor = rig.store.cursor(&rig.library()).expect("cursor");
    assert_eq!(cursor.as_deref(), Some("urn:x:0"));
}
