//! PROV-S-002 / PROV-S-004: the builder turns version facts into exactly one
//! activity with its entities and agents.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_prov::{
    activity_id, build_record, entity_id, AgentKind, AgentRole, Derivation, OcflUser,
    Reconstruction, Record, VersionFacts,
};
use fixtures::*;

/// Build the standard fixture or fail the test.
fn standard_record() -> Record {
    build_record(&standard_facts()).expect("standard facts build")
}

/// Entity identifier of `path` in `version` of the standard object.
fn eid(version: &str, path: &str) -> String {
    entity_id(OBJECT, version, path)
}

#[test]
fn exactly_one_activity_named_after_operation_object_and_version() {
    let record = standard_record();
    let activity = record.activity();
    assert_eq!(
        activity.id(),
        format!("urn:filesquid:activity:{OPERATION}:{OBJECT}:v3")
    );
    assert_eq!(activity.id(), activity_id(OPERATION, OBJECT, "v3"));
    assert_eq!(activity.operation(), OPERATION);
    assert_eq!(activity.kind(), "move");
    assert_eq!(activity.object_version().object_id(), OBJECT);
    assert_eq!(activity.object_version().version(), "v3");
    assert_eq!(activity.message(), Some("filesquid.operation.v1"));
}

#[test]
fn ended_time_is_the_created_time_and_started_is_kept() {
    let record = standard_record();
    assert_eq!(
        record.activity().ended_at().as_str(),
        "2026-10-19T10:00:05Z"
    );
    assert_eq!(
        record.activity().started_at().as_str(),
        "2026-10-19T10:00:00Z"
    );
}

#[test]
fn journal_entries_are_sorted_and_deduplicated() {
    let mut facts = standard_facts();
    facts.journal_entries.push("j-1".to_string());
    let record = build_record(&facts).expect("build");
    assert_eq!(record.activity().journal_entries(), ["j-1", "j-2"]);
}

#[test]
fn changed_path_is_generated_and_derived_from_its_previous_state() {
    let record = standard_record();
    let new = record.entity(&eid("v3", "a.txt")).expect("new a.txt");
    assert_eq!(new.generated_by(), Some(record.activity().id()));
    assert_eq!(new.derived_from(), [eid("v2", "a.txt")]);
    assert_eq!(new.sha256(), digest('5'));
    let old = record.entity(&eid("v2", "a.txt")).expect("old a.txt");
    assert_eq!(old.sha256(), digest('1'));
    assert_eq!(old.version(), "v2");
    assert_eq!(old.generated_by(), None);
    assert_eq!(old.invalidated_by(), None);
}

#[test]
fn added_path_is_generated_without_derivation() {
    let record = standard_record();
    let added = record.entity(&eid("v3", ADDED_PATH)).expect("added");
    assert_eq!(added.generated_by(), Some(record.activity().id()));
    assert!(added.derived_from().is_empty());
    assert_eq!(added.path(), ADDED_PATH);
    assert_eq!(added.object_id(), OBJECT);
    assert_eq!(added.version(), "v3");
}

#[test]
fn moved_path_derives_from_the_old_paths_base_entity() {
    let record = standard_record();
    let moved = record.entity(&eid("v3", "new/c.txt")).expect("moved");
    assert_eq!(moved.derived_from(), [eid("v2", "old/c.txt")]);
    let source = record.entity(&eid("v2", "old/c.txt")).expect("source");
    assert_eq!(source.invalidated_by(), None, "a move is not a removal");
}

#[test]
fn removed_path_is_invalidated_by_the_activity() {
    let record = standard_record();
    let removed = record.entity(&eid("v2", "b.txt")).expect("removed");
    assert_eq!(removed.invalidated_by(), Some(record.activity().id()));
    assert!(record.entity(&eid("v3", "b.txt")).is_none());
}

#[test]
fn inputs_are_used_and_unchanged_paths_are_absent() {
    let record = standard_record();
    let mut expected = vec![eid("v2", "a.txt"), eid("v2", "old/c.txt")];
    expected.sort();
    assert_eq!(record.activity().used(), expected);
    assert!(record.entity(&eid("v3", "keep.txt")).is_none());
    assert!(record.entity(&eid("v2", "keep.txt")).is_none());
    assert_eq!(record.entities().len(), 6);
}

#[test]
fn generated_entities_are_attributed_to_the_installation_agent() {
    let record = standard_record();
    let installation = format!("urn:uuid:{INSTALLATION}");
    for entity in record.entities() {
        if entity.generated_by().is_some() {
            assert_eq!(entity.attributed_to(), std::slice::from_ref(&installation));
        } else {
            assert!(entity.attributed_to().is_empty());
        }
    }
}

#[test]
fn activity_is_associated_with_each_agent() {
    let record = standard_record();
    let ids: Vec<&str> = record.agents().iter().map(|a| a.id()).collect();
    assert_eq!(record.activity().associated_with(), ids);
    assert_eq!(ids.len(), 2);
}

#[test]
fn installation_is_a_plain_agent_and_host_acts_on_its_behalf() {
    let record = standard_record();
    let installation = format!("urn:uuid:{INSTALLATION}");
    let owner = record.agent(&installation).expect("installation agent");
    assert!(matches!(owner.kind(), AgentKind::Installation(_)));
    assert_eq!(owner.acted_on_behalf_of(), None);
    let host = record
        .agents()
        .iter()
        .find(|a| matches!(a.kind(), AgentKind::Host(_)))
        .expect("host agent");
    assert_eq!(
        host.id(),
        "urn:filesquid:software:FileSquid:imagesquid:0.1.0"
    );
    assert_eq!(host.acted_on_behalf_of(), Some(installation.as_str()));
    let AgentKind::Host(facts) = host.kind() else {
        unreachable!()
    };
    assert_eq!(facts, &fixtures::host());
}

#[test]
fn installation_uuid_is_rendered_lowercase() {
    let mut facts = standard_facts();
    facts.installation = Some(INSTALLATION.to_uppercase());
    let record = build_record(&facts).expect("build");
    assert!(record.agent(&format!("urn:uuid:{INSTALLATION}")).is_some());
}

#[test]
fn unobserved_agents_are_stated_as_unrecorded() {
    let mut facts = standard_facts();
    facts.installation = None;
    facts.host = None;
    let record = build_record(&facts).expect("build");
    let roles: Vec<AgentKind> = record.agents().iter().map(|a| a.kind().clone()).collect();
    assert_eq!(
        roles,
        [
            AgentKind::Unrecorded(AgentRole::Host),
            AgentKind::Unrecorded(AgentRole::Installation)
        ]
    );
    assert_eq!(
        record.agents()[1].id(),
        "urn:filesquid:agent:unrecorded:installation"
    );
    assert!(record
        .agents()
        .iter()
        .all(|a| a.acted_on_behalf_of().is_none()));
    let added = record.entity(&eid("v3", ADDED_PATH)).expect("added");
    assert_eq!(
        added.attributed_to(),
        ["urn:filesquid:agent:unrecorded:installation"]
    );
}

#[test]
fn ocfl_user_address_names_the_installation_but_its_name_is_never_recorded() {
    let mut facts = standard_facts();
    facts.installation = None;
    facts.user = Some(OcflUser {
        name: "Alice Example".to_string(),
        address: Some(format!("urn:uuid:{INSTALLATION}")),
    });
    let record = build_record(&facts).expect("build");
    assert!(record.agent(&format!("urn:uuid:{INSTALLATION}")).is_some());
    let text = filesquid_prov::to_json_ld(&record);
    assert!(!text.contains("Alice"), "no display name: {text}");
}

#[test]
fn ocfl_user_without_an_installation_urn_leaves_the_agent_unrecorded() {
    let mut facts = standard_facts();
    facts.installation = None;
    facts.user = Some(OcflUser {
        name: "Alice Example".to_string(),
        address: Some("mailto:alice@example.org".to_string()),
    });
    let record = build_record(&facts).expect("build");
    assert!(record
        .agent("urn:filesquid:agent:unrecorded:installation")
        .is_some());
    assert!(!filesquid_prov::to_json_ld(&record).contains("alice"));
}

#[test]
fn reconstruction_is_marked_with_its_own_recording_time() {
    let mut facts = standard_facts();
    facts.reconstruction = Some(Reconstruction {
        recorded_at: "2026-10-20T08:00:00Z".to_string(),
    });
    let record = build_record(&facts).expect("build");
    assert_eq!(
        record.activity().derivation(),
        Some(Derivation::Reconstructed)
    );
    assert_eq!(
        record.activity().recorded_at().map(|t| t.as_str()),
        Some("2026-10-20T08:00:00Z")
    );
    assert_eq!(Derivation::Reconstructed.as_str(), "reconstructed");
}

#[test]
fn record_paths_are_not_described_by_their_own_record() {
    let mut facts = standard_facts();
    facts
        .new_state
        .insert("provenance/v2.jsonld".to_string(), digest('9'));
    let record = build_record(&facts).expect("build");
    assert!(record
        .entities()
        .iter()
        .all(|e| !e.path().starts_with("provenance/")));
}

#[test]
fn an_empty_diff_still_records_one_activity() {
    let mut facts = standard_facts();
    facts.new_state = facts.base_state.clone();
    facts.moves.clear();
    let record = build_record(&facts).expect("provenance-only version");
    assert!(record.entities().is_empty());
    assert!(record.activity().used().is_empty());
}

/// Assert that `facts` are refused with `code`.
fn refused(facts: VersionFacts, code: &str) {
    let err = build_record(&facts).expect_err("facts must be refused");
    assert_eq!(err.code(), code, "{err}");
}

#[test]
fn invalid_facts_are_refused_with_fixed_codes() {
    let mut f = standard_facts();
    f.created = "2026-10-19T12:00:05+02:00".to_string();
    refused(f, "PROV_INVALID_FACT");
    let mut f = standard_facts();
    f.started = "2026-10-19T10:00:06Z".to_string();
    refused(f, "PROV_TIME_ORDER");
    let mut f = standard_facts();
    f.new_state.insert("x.txt".to_string(), "abc".to_string());
    refused(f, "PROV_INVALID_FACT");
    let mut f = standard_facts();
    f.installation = Some("0f8e2b9a-4c1d-1e7f-9a3b-5d6c7e8f9a0b".to_string());
    refused(f, "PROV_INVALID_FACT");
    let mut f = standard_facts();
    f.version = "3".to_string();
    refused(f, "PROV_INVALID_FACT");
    let mut f = standard_facts();
    f.new_state.insert("../escape".to_string(), digest('7'));
    refused(f, "PROV_INVALID_FACT");
    let mut f = standard_facts();
    f.object_id = "urn:uuid:has space".to_string();
    refused(f, "PROV_INVALID_FACT");
}

#[test]
fn inconsistent_moves_and_bases_are_refused() {
    let mut f = standard_facts();
    f.moves
        .insert("missing.txt".to_string(), "new/c.txt".to_string());
    refused(f, "PROV_INVALID_MOVE");
    let mut f = standard_facts();
    f.moves.insert("a.txt".to_string(), "keep.txt".to_string());
    refused(f, "PROV_INVALID_MOVE");
    let mut f = standard_facts();
    f.base_version = None;
    refused(f, "PROV_INVALID_FACT");
}

/// Facts of `v3` with the base `x.txt`, `y.txt` and the given new state and moves.
fn moves_facts(new_state: &[(&str, char)], moves: &[(&str, &str)]) -> VersionFacts {
    VersionFacts {
        base_state: state(&[("x.txt", '1'), ("y.txt", '2')]),
        new_state: state(new_state),
        moves: moves
            .iter()
            .map(|(old, new)| (old.to_string(), new.to_string()))
            .collect(),
        ..standard_facts()
    }
}

#[test]
fn chained_moves_are_accepted_and_derive_from_each_source() {
    let facts = moves_facts(
        &[("y.txt", '1'), ("z.txt", '2')],
        &[("x.txt", "y.txt"), ("y.txt", "z.txt")],
    );
    let record = build_record(&facts).expect("chained moves");
    let y = record.entity(&eid("v3", "y.txt")).expect("y");
    assert_eq!(y.derived_from(), [eid("v2", "x.txt")]);
    let z = record.entity(&eid("v3", "z.txt")).expect("z");
    assert_eq!(z.derived_from(), [eid("v2", "y.txt")]);
    assert!(record
        .entities()
        .iter()
        .all(|e| e.invalidated_by().is_none()));
}

#[test]
fn swapped_moves_are_accepted() {
    let facts = moves_facts(
        &[("x.txt", '2'), ("y.txt", '1')],
        &[("x.txt", "y.txt"), ("y.txt", "x.txt")],
    );
    let record = build_record(&facts).expect("swap");
    let x = record.entity(&eid("v3", "x.txt")).expect("x");
    assert_eq!(x.derived_from(), [eid("v2", "y.txt")]);
    let y = record.entity(&eid("v3", "y.txt")).expect("y");
    assert_eq!(y.derived_from(), [eid("v2", "x.txt")]);
}

#[test]
fn a_move_onto_a_kept_base_path_or_onto_itself_is_refused() {
    let onto_kept = moves_facts(&[("y.txt", '1')], &[("x.txt", "y.txt")]);
    refused(onto_kept, "PROV_INVALID_MOVE");
    let onto_itself = moves_facts(&[("x.txt", '1'), ("y.txt", '2')], &[("x.txt", "x.txt")]);
    refused(onto_itself, "PROV_INVALID_MOVE");
}

#[test]
fn refusal_messages_name_no_stored_value() {
    let mut f = standard_facts();
    f.object_id = "urn:SECRET value".to_string();
    let err = build_record(&f).expect_err("refused");
    assert!(!err.to_string().contains("SECRET"), "{err}");
}
