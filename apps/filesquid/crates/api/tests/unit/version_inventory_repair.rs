//! Task CL-1.1: the consented version-inventory repair command and its candidate query
//! (API-S-023, OCFL-S-020, OCFL-S-018, OCFL-S-002; owner decision 2026-10-18).

#[path = "../support/ocfl_fixture.rs"]
mod fixture;

use filesquid_api::auth::Principal;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::local_library::{
    RepairResult, CONSENT_STATEMENT, CONSENT_STATEMENT_ID, MAX_REPAIR_OBJECTS, REPAIR_OPERATION,
};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::{CommitError, VersionReservations};
use filesquid_ocfl::DigestAlgorithm;
use fixture::{library, ready, Rig};

/// The repair as the desktop owner.
fn repair_as_owner(
    rig: &Rig,
    request: filesquid_api::local_library::RepairRequest,
) -> Result<
    filesquid_api::local_library::RepairAnswer,
    filesquid_api::local_library::LocalLibraryError,
> {
    ready(
        rig.service
            .repair_version_inventory(&Principal::Owner, request, &rig.root, &rig.leases),
    )
}

/// The outcome rows the ledger recorded (consent rows excluded).
fn outcome_rows(rig: &Rig) -> Vec<JournalEntry> {
    rig.ledger
        .outcomes()
        .into_iter()
        .map(|record| record.journal)
        .collect()
}

/// A v3 commit on `root`, as the next writer would make it.
fn commit_v3(rig: &Rig, root: &str, object_id: &str) -> Result<(), CommitError> {
    let mut draft = rig.draft(object_id, &format!("{root}-v3"), &[("c.txt", b"charlie")]);
    draft.keep("a.txt", DigestAlgorithm::Sha256.digest_hex(b"alpha"));
    draft.keep("b.txt", DigestAlgorithm::Sha256.digest_hex(b"bravo"));
    rig.commit(root, Some("v2"), &draft).map(|_| ())
}

#[test]
fn a_consented_repair_by_the_owner_makes_a_sha256_object_committable() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let refused = commit_v3(&rig, "obj-a", "urn:uuid:a").expect_err("W010 stays closed");
    assert!(
        matches!(refused, CommitError::MissingVersionInventory { .. }),
        "{refused:?}"
    );
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("repair");
    assert_eq!(answer.library_id, library());
    assert_eq!(answer.objects.len(), 1);
    let one = &answer.objects[0];
    assert_eq!(one.result, RepairResult::Repaired);
    assert_eq!(one.version.as_deref(), Some("v2"));
    assert_eq!(one.error_code, None);
    assert_eq!(
        rig.bytes("obj-a/v2/inventory.json"),
        rig.bytes("obj-a/inventory.json")
    );
    assert_eq!(
        rig.bytes("obj-a/v2/inventory.json.sha256"),
        rig.bytes("obj-a/inventory.json.sha256")
    );
    rig.clock.advance(120_000);
    commit_v3(&rig, "obj-a", "urn:uuid:a").expect("the repaired object is committable");
}

#[test]
fn consent_and_one_outcome_row_per_object_are_journaled_without_paths() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    rig.object("obj-b", "urn:uuid:b");
    repair_as_owner(&rig, rig.consented(&["urn:uuid:a", "urn:uuid:b"])).expect("answered");
    let consents = rig.ledger.consents();
    assert_eq!(consents.len(), 1);
    assert_eq!(consents[0].operation, REPAIR_OPERATION);
    assert_eq!(consents[0].outcome, Outcome::Started);
    assert_eq!(consents[0].actor, "owner");
    assert_eq!(
        consents[0].target_ids,
        [library().as_str(), "urn:uuid:a", "urn:uuid:b"]
    );
    let rows = outcome_rows(&rig);
    assert_eq!(rows.len(), 2, "one outcome row per object");
    assert!(rows.iter().all(|r| r.operation == REPAIR_OPERATION));
    assert_eq!(rows[0].outcome, Outcome::Succeeded);
    assert_eq!(rows[0].target_ids, [library().as_str(), "urn:uuid:a", "v2"]);
    assert_eq!(rows[0].error_code, None);
    assert_eq!(rows[1].outcome, Outcome::Failed);
    assert_eq!(rows[1].error_code.as_deref(), Some("nothing_to_repair"));
    for row in rows.iter().chain(&consents) {
        let text = serde_json::to_string(row).expect("json");
        assert!(
            !text.contains("obj-") && !text.contains("inventory.json"),
            "{text}"
        );
    }
    let records = rig.ledger.outcomes();
    let receipt = records[0]
        .receipt
        .as_ref()
        .expect("receipt in the same record");
    assert_eq!(receipt.object_id, "urn:uuid:a");
    assert_eq!(receipt.version, "v2");
    assert!(records[1].receipt.is_none());
}

#[test]
fn a_head_that_already_has_both_files_is_refused_with_nothing_to_repair() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    let writes = rig.root.writes();
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(answer.objects[0].result, RepairResult::Refused);
    assert_eq!(
        answer.objects[0].error_code.as_deref(),
        Some("nothing_to_repair")
    );
    assert_eq!(rig.root.writes(), writes, "nothing written");
}

#[test]
fn a_second_repair_is_refused_with_nothing_to_repair() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("first");
    let again = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("second");
    assert_eq!(
        again.objects[0].error_code.as_deref(),
        Some("nothing_to_repair")
    );
}

#[test]
fn an_interrupted_repair_is_resumed() {
    let rig = Rig::with_root();
    rig.interrupted_repair("obj-a", "urn:uuid:a");
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("repair");
    assert_eq!(answer.objects[0].result, RepairResult::Repaired);
    assert!(rig.stored("obj-a/v2/inventory.json.sha256"));
    let receipt = rig.ledger.outcomes()[0].receipt.clone().expect("receipt");
    assert_eq!(receipt.written, ["v2/inventory.json.sha256"]);
}

#[test]
fn the_command_is_refused_without_consent_and_writes_nothing() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let writes = rig.root.writes();
    let mut cases = Vec::new();
    let mut none = rig.consented(&["urn:uuid:a"]);
    none.consent = None;
    cases.push((none, "consent_required"));
    let mut unconfirmed = rig.consented(&["urn:uuid:a"]);
    unconfirmed.consent.as_mut().expect("consent").confirmed = false;
    cases.push((unconfirmed, "consent_required"));
    let mut other_objects = rig.consented(&["urn:uuid:a"]);
    other_objects.consent.as_mut().expect("consent").object_ids = vec!["urn:uuid:z".into()];
    cases.push((other_objects, "consent_mismatch"));
    let mut stale = rig.consented(&["urn:uuid:a"]);
    stale.consent.as_mut().expect("consent").statement_id = "old".into();
    cases.push((stale, "consent_mismatch"));
    let mut before_the_history_version = rig.consented(&["urn:uuid:a"]);
    before_the_history_version
        .consent
        .as_mut()
        .expect("consent")
        .statement_id = "version_inventory_repair.v1".into();
    cases.push((before_the_history_version, "consent_mismatch"));
    for (request, code) in cases {
        let err = repair_as_owner(&rig, request).expect_err("refused");
        assert_eq!(err.error_code(), code);
    }
    assert_eq!(rig.root.writes(), writes, "nothing written");
    assert!(rig.ledger.consents().is_empty() && rig.ledger.outcomes().is_empty());
    let refusals = rig.journaled(REPAIR_OPERATION);
    assert_eq!(refusals.len(), 5, "every refusal is journaled");
    assert!(refusals.iter().all(|e| e.outcome == Outcome::Failed));
}

#[test]
fn the_command_is_refused_for_another_caller() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let writes = rig.root.writes();
    let err = ready(rig.service.repair_version_inventory(
        &Principal::Administrator,
        rig.consented(&["urn:uuid:a"]),
        &rig.root,
        &rig.leases,
    ))
    .expect_err("refused");
    assert_eq!(err.error_code(), "forbidden");
    assert_eq!(rig.root.writes(), writes);
    let refusals = rig.journaled(REPAIR_OPERATION);
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].actor, "administrator");
    assert_eq!(refusals[0].error_code.as_deref(), Some("forbidden"));
}

#[test]
fn object_lists_are_bounded_and_distinct() {
    let rig = Rig::with_root();
    let empty = rig.consented(&[]);
    assert_eq!(
        repair_as_owner(&rig, empty).unwrap_err().error_code(),
        "invalid_objects"
    );
    let repeated = rig.consented(&["urn:uuid:a", "urn:uuid:a"]);
    assert_eq!(
        repair_as_owner(&rig, repeated).unwrap_err().error_code(),
        "invalid_objects"
    );
    let ids: Vec<String> = (0..=MAX_REPAIR_OBJECTS)
        .map(|n| format!("urn:x:{n}"))
        .collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let too_many = rig.consented(&refs);
    assert_eq!(
        repair_as_owner(&rig, too_many).unwrap_err().error_code(),
        "invalid_objects"
    );
}

#[test]
fn an_unknown_object_is_refused_per_object() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a", "urn:uuid:missing"]))
        .expect("answered");
    assert_eq!(answer.objects[0].result, RepairResult::Repaired);
    assert_eq!(
        answer.objects[1].error_code.as_deref(),
        Some("object_not_found")
    );
    assert_eq!(answer.objects[1].version, None);
}

#[test]
fn an_unrecordable_consent_repairs_nothing() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    rig.ledger.fail_consents(true);
    let writes = rig.root.writes();
    let err = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect_err("refused");
    assert_eq!(err.error_code(), "unavailable");
    assert!(err.retryable());
    assert_eq!(rig.root.writes(), writes);
    assert_eq!(
        rig.journaled(REPAIR_OPERATION).len(),
        1,
        "the refusal is journaled"
    );
}

#[test]
fn an_unrecordable_outcome_still_reaches_the_journal() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    rig.ledger.fail_outcomes(true);
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(answer.objects[0].result, RepairResult::Repaired);
    let fallback = rig.journaled(REPAIR_OPERATION);
    assert_eq!(fallback.len(), 1, "no logging gap");
    assert_eq!(fallback[0].outcome, Outcome::Succeeded);
    assert_eq!(
        fallback[0].target_ids,
        [library().as_str(), "urn:uuid:a", "v2"]
    );
}

#[test]
fn a_held_reservation_is_a_retryable_refusal() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let held = ready(VersionReservations::reserve(
        &rig.leases,
        "urn:uuid:a",
        &fixture::v("v3"),
        "other-writer",
    ))
    .expect("held by another writer");
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(
        answer.objects[0].error_code.as_deref(),
        Some("reservation_held")
    );
    assert!(answer.objects[0].retryable);
    assert!(!rig.stored("obj-a/v2/inventory.json"));
    drop(held);
}

#[test]
fn candidates_say_whether_the_repair_opens_the_object_to_commits() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    let found = ready(
        rig.service
            .repair_candidates("owner", &library(), &rig.root),
    )
    .expect("query");
    assert!(
        found.candidates[0].committable_after_repair,
        "sha256 objects become committable"
    );
}

#[test]
fn the_candidate_query_lists_heads_without_their_version_inventory() {
    let rig = Rig::with_root();
    rig.object("obj-a", "urn:uuid:a");
    rig.w010_object("obj-b", "urn:uuid:b");
    rig.interrupted_repair("obj-c", "urn:uuid:c");
    let writes = rig.root.writes();
    let found = ready(
        rig.service
            .repair_candidates("owner", &library(), &rig.root),
    )
    .expect("query");
    assert_eq!(
        rig.root.writes(),
        writes,
        "the query never writes or repairs"
    );
    assert!(!found.truncated);
    let ids: Vec<&str> = found
        .candidates
        .iter()
        .map(|c| c.object_id.as_str())
        .collect();
    assert_eq!(ids, ["urn:uuid:b", "urn:uuid:c"]);
    assert_eq!(found.candidates[0].head, "v2");
    assert!(!found.candidates[0].interrupted);
    assert!(found.candidates[1].interrupted);
    assert_eq!(found.candidates[0].digest_algorithm, "sha256");
}

#[test]
fn the_consent_statement_id_is_pinned() {
    assert_eq!(CONSENT_STATEMENT_ID, "version_inventory_repair.v2");
}

#[test]
fn a_damaged_interrupted_repair_is_refused_without_writing_the_sidecar() {
    let rig = Rig::with_root();
    rig.interrupted_repair("obj-a", "urn:uuid:a");
    ready(
        rig.root
            .put(&fixture::key("obj-a/v2/content/b.txt"), b"tampered"),
    )
    .expect("tamper");
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(
        answer.objects[0].error_code.as_deref(),
        Some("object_invalid")
    );
    assert_eq!(answer.objects[0].version.as_deref(), Some("v2"));
    assert!(
        !rig.stored("obj-a/v2/inventory.json.sha256"),
        "no sidecar written"
    );
}

#[test]
fn an_unlisted_file_in_the_head_is_neither_offered_nor_repaired() {
    let rig = Rig::with_root();
    rig.interrupted_repair("obj-a", "urn:uuid:a");
    ready(rig.root.put(&fixture::key("obj-a/v2/stray.tmp"), b"junk")).expect("stray");
    let found = ready(
        rig.service
            .repair_candidates("owner", &library(), &rig.root),
    )
    .expect("query");
    assert!(found.candidates.is_empty(), "{found:?}");
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(answer.objects[0].result, RepairResult::Refused);
    assert!(
        !rig.stored("obj-a/v2/inventory.json.sha256"),
        "no sidecar written"
    );
}

#[test]
fn a_root_sidecar_that_does_not_commit_the_root_inventory_is_refused() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    ready(rig.root.put(
        &fixture::key("obj-a/inventory.json.sha256"),
        b"0000  inventory.json\n",
    ))
    .expect("bad sidecar");
    let writes = rig.root.writes();
    let answer = repair_as_owner(&rig, rig.consented(&["urn:uuid:a"])).expect("answered");
    assert_eq!(answer.objects[0].result, RepairResult::Refused);
    assert_eq!(rig.root.writes(), writes, "nothing written");
}

#[test]
fn the_consent_statement_names_what_changes() {
    for phrase in [
        "one version inventory and its sidecar",
        "copies of the root inventory",
        "latest version directory",
        "a new version recording the repair will be committed to each object's history",
        "no file content changes",
    ] {
        assert!(CONSENT_STATEMENT.contains(phrase), "{phrase}");
    }
}

#[test]
fn open_copy_and_rebuild_never_reach_the_repair() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    ready(rig.service.open_root("owner", &library(), &rig.root)).expect("open");
    let dest = filesquid_ocfl::storage::MemoryBackend::new();
    ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect("copy");
    ready(rig.service.rebuild("owner", &library(), &rig.root)).expect("rebuild");
    ready(
        rig.service
            .repair_candidates("owner", &library(), &rig.root),
    )
    .expect("candidates");
    assert!(!rig.stored("obj-a/v2/inventory.json"));
    assert!(rig.ledger.consents().is_empty() && rig.ledger.outcomes().is_empty());
    assert!(rig.journaled(REPAIR_OPERATION).is_empty());
}

/// API-F2.8 (ADR 040 Decision 6): the consent, every outcome and the
/// answer share one repair run ID as their operation ID, and an outcome
/// names the object version it repaired.
#[test]
fn consent_and_outcomes_share_one_repair_run_id() {
    let rig = Rig::with_root();
    rig.w010_object("obj-a", "urn:uuid:a");
    rig.object("obj-b", "urn:uuid:b");
    let answer =
        repair_as_owner(&rig, rig.consented(&["urn:uuid:a", "urn:uuid:b"])).expect("answered");
    assert!(
        filesquid_api::journal::is_opaque(&answer.run_id),
        "{}",
        answer.run_id
    );
    let consents = rig.ledger.consents();
    assert_eq!(
        consents[0].operation_id.as_deref(),
        Some(answer.run_id.as_str())
    );
    let rows = outcome_rows(&rig);
    for row in &rows {
        assert_eq!(row.operation_id.as_deref(), Some(answer.run_id.as_str()));
    }
    assert_eq!(rows[0].object_ref.as_deref(), Some("urn:uuid:a@v2"));
    assert_eq!(rows[1].outcome, Outcome::Failed);
    assert_eq!(
        rows[1].object_ref, None,
        "a refused object stays journal-only (review round 1)"
    );
    assert!(rows[1].target_ids.iter().any(|t| t == "v2"));
    let again = repair_as_owner(&rig, rig.consented(&["urn:uuid:b"])).expect("answered again");
    assert_ne!(again.run_id, answer.run_id, "each consent is its own run");
}
