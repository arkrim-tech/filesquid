//! Task API-F2.9: the provenance follow-up of a repair and the history backfill job
//! (API-S-028, API-S-029; ADR 040 Decisions 6 and 7).

#[path = "../support/ocfl_fixture.rs"]
mod fixture;

use filesquid_api::auth::Principal;
use filesquid_api::history::backfill::{Backfill, BACKFILL_OPERATION};
use filesquid_api::history::{journal_path, ProvenanceContext, PROV_RECORD_OPERATION};
use filesquid_api::journal::{
    JournalEntry, JournalError, JournalPage, JournalQuery, JournalStore, Level, Outcome,
    RetentionPolicy,
};
use filesquid_api::local_library::{
    RepairAnswer, RepairResult, JOURNAL_IDS_UNAVAILABLE, PROVENANCE_NOT_CONFIGURED,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::registry::{ProcessRegistry, ProcessState};
use filesquid_api::EventBus;
use filesquid_core::InstallationId;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::MAX_CHUNK_BYTES;
use filesquid_ocfl::VersionNum;
use filesquid_prov::ocfl::{missing_records, read_record};
use filesquid_prov::{Derivation, HostFacts, Record};
use fixture::{ready, v, Rig, NOW_MS};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn context() -> ProvenanceContext {
    ProvenanceContext {
        installation: InstallationId::parse(INSTALLATION).expect("installation"),
        host: HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        },
    }
}

/// `rig`'s service, recording provenance with ids from `ids`.
fn record_with(rig: &mut Rig, ids: Arc<dyn JournalStore>) {
    rig.service = rig.service.clone().with_provenance(context(), ids);
}

/// The repair of `ids` as the desktop owner.
fn repair(rig: &Rig, ids: &[&str]) -> RepairAnswer {
    ready(rig.service.repair_version_inventory(
        &Principal::Owner,
        rig.consented(ids),
        &rig.root,
        &rig.leases,
    ))
    .expect("answered")
}

/// The record of `version` of the object at `root`.
fn record(rig: &Rig, root: &str, version: &str) -> Option<Record> {
    let object = ready(Object::open(&rig.root, root)).expect("open");
    ready(read_record(&object, &v(version))).expect("read")
}

/// The journal entries the version `version` of the object at `root` carries.
fn carried(rig: &Rig, root: &str, version: &str) -> Vec<JournalEntry> {
    let object = ready(Object::open(&rig.root, root)).expect("open");
    let path = journal_path(version);
    let mut reader = object
        .stream_file(&v(version), &path, MAX_CHUNK_BYTES)
        .expect("journal file");
    let mut bytes = Vec::new();
    for _ in 0..64 {
        match ready(reader.next_chunk()).expect("chunk") {
            Some(chunk) => bytes.extend_from_slice(&chunk),
            None => break,
        }
    }
    String::from_utf8(bytes)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("entry"))
        .collect()
}

// ---- Repair follow-up ----

#[test]
fn a_repair_commits_a_provenance_record_under_the_run_id() {
    let mut rig = Rig::with_root();
    let ids = rig.journal.clone();
    record_with(&mut rig, ids);
    rig.w010_object("obj-a", "urn:uuid:a");
    let answer = repair(&rig, &["urn:uuid:a"]);
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Repaired);
    assert_eq!(one.version.as_deref(), Some("v2"), "the repaired head");
    assert_eq!(one.provenance_version.as_deref(), Some("v3"));
    assert_eq!(one.provenance_error_code, None);
    let record = record(&rig, "obj-a", "v3").expect("the repair's record");
    assert_eq!(record.activity().kind(), "repair");
    assert_eq!(record.activity().operation(), answer.run_id);
    let carried = carried(&rig, "obj-a", "v3");
    assert_eq!(carried.len(), 1);
    let id = carried[0].id.expect("allocated before the commit");
    assert_eq!(record.activity().journal_entries(), [id.to_string()]);
    let commits = rig.journaled("ocfl.commit");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].id, Some(id), "appended under the carried id");
    assert_eq!(
        commits[0].operation_id.as_deref(),
        Some(answer.run_id.as_str())
    );
    assert_eq!(commits[0].actor, "owner");
    let consent = &rig.ledger.consents()[0];
    let outcome = &rig.ledger.outcomes()[0].journal;
    for shared in [consent, outcome] {
        assert_eq!(shared.operation_id.as_deref(), Some(answer.run_id.as_str()));
    }
}

#[test]
fn without_a_provenance_context_the_missing_record_is_journaled_and_answered() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let answer = repair(&rig, &["urn:uuid:a"]);
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Repaired);
    assert_eq!(one.provenance_version, None);
    assert_eq!(
        one.provenance_error_code.as_deref(),
        Some(PROVENANCE_NOT_CONFIGURED)
    );
    let warnings = rig.journaled(PROV_RECORD_OPERATION);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].level, Level::Warn);
    assert_eq!(
        warnings[0].error_code.as_deref(),
        Some(PROVENANCE_NOT_CONFIGURED)
    );
    assert_eq!(
        warnings[0].operation_id.as_deref(),
        Some(answer.run_id.as_str())
    );
    assert_eq!(warnings[0].target_ids, ["urn:uuid:a"]);
    assert!(
        rig.journaled("ocfl.commit").is_empty(),
        "no version was made"
    );
}

/// A journal store that cannot allocate ids.
struct NoIds;

impl JournalStore for NoIds {
    fn host(&self) -> &str {
        "desktop"
    }

    fn append(&self, _entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        Err(JournalError("read-only".into()))
    }

    fn query(&self, _query: &JournalQuery) -> Result<JournalPage, JournalError> {
        Err(JournalError("read-only".into()))
    }

    fn count(&self, _query: &JournalQuery) -> Result<u64, JournalError> {
        Err(JournalError("read-only".into()))
    }

    fn apply_retention(&self, _policy: RetentionPolicy, _now_ms: i64) -> Result<u64, JournalError> {
        Err(JournalError("read-only".into()))
    }

    fn ping(&self) -> Result<(), JournalError> {
        Ok(())
    }
}

#[test]
fn a_record_that_cannot_be_written_is_journaled_as_failed_and_the_repair_stands() {
    let mut rig = Rig::with_root();
    record_with(&mut rig, Arc::new(NoIds));
    rig.w010_object("obj-a", "urn:uuid:a");
    let answer = repair(&rig, &["urn:uuid:a"]);
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Repaired);
    assert_eq!(
        one.provenance_error_code.as_deref(),
        Some(JOURNAL_IDS_UNAVAILABLE)
    );
    let failures = rig.journaled(PROV_RECORD_OPERATION);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].outcome, Outcome::Failed);
    assert_eq!(failures[0].level, Level::Error);
    assert!(record(&rig, "obj-a", "v2").is_none());
}

#[test]
fn a_refused_object_gets_no_provenance_follow_up() {
    let mut rig = Rig::with_root();
    let ids = rig.journal.clone();
    record_with(&mut rig, ids);
    rig.object("obj-b", "urn:uuid:b");
    let answer = repair(&rig, &["urn:uuid:b"]);
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Refused);
    assert_eq!(one.provenance_version, None);
    assert_eq!(one.provenance_error_code, None);
    assert!(rig.journaled(PROV_RECORD_OPERATION).is_empty());
    let json = serde_json::to_value(one).unwrap();
    assert!(
        json.get("provenance_version").is_none(),
        "absent, not null: {json}"
    );
}

// ---- Backfill ----

/// The job over `rig`'s journal, with a registry on the same queue.
fn backfill(rig: &Rig) -> (Backfill, ProcessRegistry) {
    let registry = ProcessRegistry::new(
        rig.sender(),
        EventBus::new(),
        Arc::new(Metrics::default()),
        Arc::new(|| NOW_MS),
    );
    let job = Backfill::new(
        context(),
        rig.journal.clone(),
        rig.sender(),
        registry.clone(),
    )
    .with_clock(Arc::new(|| NOW_MS + 600_000));
    (job, registry)
}

/// The versions of the object at `root` with no record.
fn missing(rig: &Rig, root: &str) -> Vec<VersionNum> {
    let object = ready(Object::open(&rig.root, root)).expect("open");
    missing_records(&object)
}

#[test]
fn a_backfill_reconstructs_every_missing_record_and_a_rerun_commits_nothing() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.object("obj-b", "urn:uuid:b");
    let (job, registry) = backfill(&rig);
    let report = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("run");
    assert_eq!(report.objects.len(), 2);
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(!report.reached_cap && !report.cancelled);
    for object in &report.objects {
        assert_eq!(object.version, "v3");
        assert_eq!(object.reconstructed, ["v1", "v2"]);
    }
    for root in ["obj-a", "obj-b"] {
        assert!(missing(&rig, root).is_empty(), "{root}");
        let v1 = record(&rig, root, "v1").expect("reconstructed");
        assert_eq!(v1.activity().derivation(), Some(Derivation::Reconstructed));
        let run = record(&rig, root, "v3").expect("the run's own record");
        assert_eq!(run.activity().kind(), "backfill");
        assert_eq!(run.activity().operation(), report.run_id);
    }
    let commits = rig.journaled("ocfl.commit");
    assert_eq!(commits.len(), 2);
    assert!(commits
        .iter()
        .all(|c| c.operation_id.as_deref() == Some(report.run_id.as_str())));
    let process = registry
        .snapshot()
        .into_iter()
        .find(|p| p.id == report.process_id)
        .expect("registered");
    assert_eq!(process.state, ProcessState::Completed);
    assert_eq!((process.done, process.total), (2, Some(2)));
    let again = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("rerun");
    assert!(again.objects.is_empty() && again.failures.is_empty());
    assert_eq!(
        rig.journaled("ocfl.commit").len(),
        2,
        "a rerun commits nothing"
    );
}

#[test]
fn a_backfill_stops_at_its_cap_and_the_next_run_continues() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.object("obj-b", "urn:uuid:b");
    let (job, _) = backfill(&rig);
    let job = job.with_max_versions(3);
    let first = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("run");
    assert!(first.reached_cap);
    let staged: usize = first.objects.iter().map(|o| o.reconstructed.len()).sum();
    assert_eq!(staged, 3);
    let second = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("run");
    assert!(!second.reached_cap);
    assert_eq!(second.objects.len(), 1);
    assert_eq!(second.objects[0].reconstructed.len(), 1);
    for root in ["obj-a", "obj-b"] {
        assert!(missing(&rig, root).is_empty(), "{root}");
    }
}

#[test]
fn a_cancelled_backfill_commits_nothing_and_ends_its_process_cancelled() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    let (job, registry) = backfill(&rig);
    let report = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(true))).expect("run");
    assert!(report.cancelled);
    assert!(report.objects.is_empty());
    assert_eq!(missing(&rig, "obj-a").len(), 2);
    let process = &registry.snapshot()[0];
    assert_eq!(process.state, ProcessState::Cancelled);
    let ends: Vec<Outcome> = rig
        .journaled(BACKFILL_OPERATION)
        .into_iter()
        .map(|e| e.outcome)
        .collect();
    assert_eq!(ends, [Outcome::Started, Outcome::Cancelled]);
}

#[test]
fn a_failing_object_is_journaled_and_the_run_goes_on() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    rig.object("obj-b", "urn:uuid:b");
    let (job, _) = backfill(&rig);
    let report = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false))).expect("run");
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].object_id.as_deref(), Some("urn:uuid:a"));
    assert_eq!(report.failures[0].error_code, "commit_failed");
    assert_eq!(report.objects.len(), 1);
    assert_eq!(report.objects[0].object_id, "urn:uuid:b");
    let failed: Vec<JournalEntry> = rig
        .journaled(BACKFILL_OPERATION)
        .into_iter()
        .filter(|e| e.outcome == Outcome::Failed)
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].target_ids, ["urn:uuid:a"]);
    assert_eq!(failed[0].error_code.as_deref(), Some("commit_failed"));
}

#[test]
fn a_backfill_without_a_storage_root_fails_its_process() {
    let rig = Rig::new();
    let (job, registry) = backfill(&rig);
    let refused = ready(job.run(&rig.root, &rig.leases, &AtomicBool::new(false)));
    assert!(refused.is_err());
    assert_eq!(registry.snapshot()[0].state, ProcessState::Failed);
    let entries = rig.journaled(BACKFILL_OPERATION);
    assert_eq!(
        entries.last().unwrap().error_code.as_deref(),
        Some("storage_unavailable")
    );
}
