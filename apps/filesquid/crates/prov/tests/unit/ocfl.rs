//! The `ocfl` feature (PV-0.2; PROV-S-006, OCFL-S-024; PRD F-067, F-042;
//! ADR 040 Decisions 3 and 7): records as `provenance/<version>.jsonld` in
//! the version they describe, written through the change-model hook,
//! regenerated on rebase, read back, listed, and reconstructed from
//! inventories for versions that have none.

use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{complete_immediately, MemoryBackend};
use filesquid_ocfl::write::{rfc3339_utc, LeaseConfig, LeaseStore, SystemClock};
use filesquid_ocfl::VersionNum;
use filesquid_prov::ocfl::{
    list_records, missing_records, read_record, reconstruct, record_path, stage_reconstructed,
    OperationFacts, RecordError, RecordHook,
};
use filesquid_prov::{to_json_ld, AgentKind, Derivation, HostFacts, Record};

const OBJECT_ID: &str = "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";
const ROOT: &str = "objects/a";
const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// Storage root, leases and staging for one test.
struct Rig {
    storage: MemoryBackend,
    leases: LeaseStore<MemoryBackend, SystemClock>,
    staging: MemoryBackend,
}

fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("ready")
}

fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

impl Rig {
    fn new() -> Self {
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            SystemClock,
            LeaseConfig::default(),
        )
        .expect("leases");
        Self {
            storage: MemoryBackend::new(),
            leases,
            staging: MemoryBackend::new(),
        }
    }

    fn model(
        &self,
        holder: &str,
    ) -> ChangeModel<'_, MemoryBackend, LeaseStore<MemoryBackend, SystemClock>, MemoryBackend> {
        let staging = StagingArea::new(&self.staging, &format!("changes/{holder}")).unwrap();
        ChangeModel::new(
            &self.storage,
            &self.leases,
            holder,
            ROOT,
            OBJECT_ID,
            staging,
        )
    }

    fn object(&self) -> Object<'_, MemoryBackend> {
        ready(Object::open(&self.storage, ROOT)).expect("open")
    }

    /// Commit `files` without a record, as before the history existed.
    fn commit_plain(&self, files: &[(&str, &[u8])], meta: VersionMeta) -> VersionNum {
        let m = self.model("plain");
        for (path, bytes) in files {
            ready(m.stage_bytes(path, bytes)).unwrap();
        }
        ready(m.commit(meta)).unwrap().version
    }
}

fn created(n: u64) -> String {
    rfc3339_utc(1_791_590_400_000 + n * 60_000)
}

fn operation(id: &str, kind: &str) -> OperationFacts {
    OperationFacts {
        operation_id: id.to_string(),
        kind: kind.to_string(),
        started: created(0),
        installation: Some(INSTALLATION.to_string()),
        host: Some(HostFacts {
            product: "filesquid".into(),
            distribution: "filesquid".into(),
            version: "0.1.0".into(),
        }),
        journal_entries: vec!["17".into()],
        ..OperationFacts::default()
    }
}

#[test]
fn a_commit_writes_its_record_named_after_its_version() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let m = rig.model("writer");
    ready(m.stage_bytes("a.txt", b"alpha two")).unwrap();
    let hook = RecordHook::new(operation("op-1", "metadata.edit"));
    let receipt = ready(m.commit_with(VersionMeta::new(created(2)), Some(&hook))).unwrap();
    assert_eq!(receipt.version, v("v2"));
    let object = rig.object();
    let state = object.state(&v("v2")).unwrap();
    assert!(state.get(&record_path("v2")).is_some());
    let record = ready(read_record(&object, &v("v2")))
        .unwrap()
        .expect("record");
    let activity = record.activity();
    assert_eq!(activity.operation(), "op-1");
    assert_eq!(activity.kind(), "metadata.edit");
    assert_eq!(activity.ended_at().as_str(), receipt.created);
    assert_eq!(activity.object_version().version(), "v2");
    assert_eq!(activity.journal_entries(), ["17"]);
    let changed = record
        .entities()
        .iter()
        .find(|e| e.path() == "a.txt" && e.version() == "v2")
        .expect("generated a.txt");
    assert!(changed.derived_from().iter().any(|id| id.contains(":v1:")));
    assert!(
        record
            .entities()
            .iter()
            .all(|e| !e.path().starts_with("provenance/")),
        "a record never describes records"
    );
}

#[test]
fn a_rebased_commit_regenerates_its_record_for_the_new_version() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let mine = rig.model("mine");
    ready(mine.stage_bytes("b.txt", b"bravo")).unwrap();
    rig.commit_plain(&[("c.txt", b"charlie")], VersionMeta::new(created(2)));
    let hook = RecordHook::new(operation("op-2", "import"));
    let receipt = ready(mine.commit_with(VersionMeta::new(created(3)), Some(&hook))).unwrap();
    assert_eq!(receipt.version, v("v3"));
    let record = ready(read_record(&rig.object(), &v("v3")))
        .unwrap()
        .expect("record");
    assert_eq!(record.activity().object_version().version(), "v3");
    let state = rig.object().state(&v("v3")).unwrap();
    assert!(state.get(&record_path("v3")).is_some());
    assert!(state.get(&record_path("v2")).is_none());
}

#[test]
fn records_are_listed_and_a_version_without_one_reads_as_none() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let m = rig.model("writer");
    ready(m.stage_bytes("b.txt", b"bravo")).unwrap();
    let hook = RecordHook::new(operation("op-1", "import"));
    ready(m.commit_with(VersionMeta::new(created(2)), Some(&hook))).unwrap();
    let object = rig.object();
    assert_eq!(list_records(&object), [v("v2")]);
    assert_eq!(missing_records(&object), [v("v1")]);
    assert!(ready(read_record(&object, &v("v1"))).unwrap().is_none());
}

#[test]
fn a_reconstructed_record_invents_no_agent_and_reads_the_operation_message() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let message = r#"filesquid.operation.v1 {"operation_id":"op-7","kind":"rename"}"#;
    let user = User {
        name: "FileSquid installation".into(),
        address: Some(format!("urn:uuid:{INSTALLATION}")),
    };
    let meta = VersionMeta::new(created(2))
        .with_message(message)
        .with_user(user);
    rig.commit_plain(&[("b.txt", b"bravo")], meta);
    let object = rig.object();
    let recorded_at = created(9);
    let first = reconstruct(object.inventory(), &v("v1"), &recorded_at).unwrap();
    assert_eq!(
        first.activity().derivation(),
        Some(Derivation::Reconstructed)
    );
    assert_eq!(
        first.activity().recorded_at().unwrap().as_str(),
        recorded_at
    );
    assert_eq!(first.activity().kind(), "unknown");
    assert!(
        first
            .agents()
            .iter()
            .all(|a| matches!(a.kind(), AgentKind::Unrecorded(_))),
        "no agent the inventory did not record"
    );
    let second = reconstruct(object.inventory(), &v("v2"), &recorded_at).unwrap();
    assert_eq!(second.activity().operation(), "op-7");
    assert_eq!(second.activity().kind(), "rename");
    assert!(second
        .agents()
        .iter()
        .any(|a| a.id() == format!("urn:uuid:{INSTALLATION}")));
    assert_eq!(second.activity().ended_at().as_str(), created(2));
}

#[test]
fn backfill_stages_every_missing_record_and_commits_them_in_one_version() {
    let rig = Rig::new();
    for n in 1..=3u64 {
        let name = format!("f{n}.txt");
        rig.commit_plain(
            &[(name.as_str(), b"x" as &[u8])],
            VersionMeta::new(created(n)),
        );
    }
    let m = rig.model("backfill");
    let staged = ready(stage_reconstructed(&m, &rig.object(), &created(9), 100)).unwrap();
    assert_eq!(staged, [v("v1"), v("v2"), v("v3")]);
    let hook = RecordHook::new(operation("run-1", "backfill"));
    let receipt = ready(m.commit_with(VersionMeta::new(created(10)), Some(&hook))).unwrap();
    assert_eq!(receipt.version, v("v4"));
    let object = rig.object();
    assert!(missing_records(&object).is_empty());
    for name in ["v1", "v2", "v3"] {
        let record = ready(read_record(&object, &v(name)))
            .unwrap()
            .expect("record");
        assert_eq!(
            record.activity().derivation(),
            Some(Derivation::Reconstructed)
        );
        assert_eq!(record.activity().object_version().version(), name);
    }
    let run = ready(read_record(&object, &v("v4")))
        .unwrap()
        .expect("run record");
    assert_eq!(run.activity().kind(), "backfill");
    let again = ready(stage_reconstructed(&m, &object, &created(11), 100)).unwrap();
    assert!(again.is_empty(), "idempotent: nothing left to reconstruct");
}

#[test]
fn backfill_is_capped_per_run() {
    let rig = Rig::new();
    for n in 1..=3u64 {
        let name = format!("f{n}.txt");
        rig.commit_plain(
            &[(name.as_str(), b"x" as &[u8])],
            VersionMeta::new(created(n)),
        );
    }
    let m = rig.model("backfill");
    let staged = ready(stage_reconstructed(&m, &rig.object(), &created(9), 2)).unwrap();
    assert_eq!(staged, [v("v1"), v("v2")]);
}

#[test]
fn record_paths_follow_the_version_name() {
    assert_eq!(record_path("v3"), "provenance/v3.jsonld");
}

#[test]
fn a_record_path_in_the_head_never_answers_for_an_uncommitted_version() {
    let rig = Rig::new();
    let first = rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let stray = reconstruct(rig.object().inventory(), &first, &created(9)).unwrap();
    let bytes = to_json_ld(&stray).into_bytes();
    let path = record_path("v9");
    rig.commit_plain(
        &[(path.as_str(), bytes.as_slice())],
        VersionMeta::new(created(2)),
    );
    let object = rig.object();
    let err = ready(read_record(&object, &v("v9"))).expect_err("v9 is not committed");
    assert!(matches!(err, RecordError::Read(_)), "{err}");
}

/// Reconstruct `v1` of an object whose only version carries `message`.
fn reconstruct_with_message(message: &str) -> Record {
    let rig = Rig::new();
    let meta = VersionMeta::new(created(1)).with_message(message);
    let version = rig.commit_plain(&[("a.txt", b"alpha")], meta);
    reconstruct(rig.object().inventory(), &version, &created(9)).expect("reconstruct")
}

#[test]
fn malformed_operation_fields_fall_back_to_the_placeholders() {
    let record = reconstruct_with_message(
        r#"filesquid.operation.v1 {"operation_id":"has space","kind":"Not a kind!"}"#,
    );
    assert_eq!(record.activity().operation(), "unrecorded");
    assert_eq!(record.activity().kind(), "unknown");
    let record = reconstruct_with_message(r#"filesquid.operation.v1 {"operation_id":"","kind":7}"#);
    assert_eq!(record.activity().operation(), "unrecorded");
    assert_eq!(record.activity().kind(), "unknown");
}

#[test]
fn a_catalog_message_without_a_kind_keeps_its_operation_id() {
    let message = r#"filesquid.operation.v1 {"operation_id":"op-8","revision":"12","created_id":"c-1","deletion":{"deleted_at":"2026-10-19T10:00:00Z"}}"#;
    let record = reconstruct_with_message(message);
    assert_eq!(record.activity().operation(), "op-8");
    assert_eq!(record.activity().kind(), "unknown");
}

#[test]
fn backfill_refuses_an_object_its_change_model_does_not_commit_to() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let staging = StagingArea::new(&rig.staging, "changes/other").unwrap();
    let other = ChangeModel::new(
        &rig.storage,
        &rig.leases,
        "other",
        "objects/b",
        "urn:uuid:7a2e3d4c-5b6f-4071-9b8c-0d1e2f3a4b5c",
        staging,
    );
    let err = ready(stage_reconstructed(&other, &rig.object(), &created(9), 10))
        .expect_err("another object");
    assert!(
        matches!(err, RecordError::Change(ChangeError::WrongObject { .. })),
        "{err}"
    );
}

#[test]
fn a_record_a_revert_removed_from_the_head_is_not_reconstructed() {
    let rig = Rig::new();
    rig.commit_plain(&[("a.txt", b"alpha")], VersionMeta::new(created(1)));
    let m = rig.model("writer");
    ready(m.stage_bytes("b.txt", b"bravo")).unwrap();
    let hook = RecordHook::new(operation("op-2", "import"));
    ready(m.commit_with(VersionMeta::new(created(2)), Some(&hook))).unwrap();
    ready(m.revert(&v("v1"), VersionMeta::new(created(3)))).unwrap();
    let object = rig.object();
    assert!(ready(read_record(&object, &v("v2"))).unwrap().is_some());
    assert_eq!(
        missing_records(&object),
        [v("v1"), v("v3")],
        "v2 keeps its own record"
    );
    assert_eq!(
        list_records(&object),
        [v("v2")],
        "history still lists the record of v2"
    );
}
