//! PROV-S-003: the JSON-LD profile `filesquid-prov/1`, deterministic output
//! and the parse round trip.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_prov::{
    build_record, parse_record, to_json_ld, AgentKind, Profile, Reconstruction, CONTEXT_FSQ,
    CONTEXT_PROV, CONTEXT_XSD, PROFILE_V1,
};
use fixtures::*;
use serde_json::Value;

const OBJ1: &str = "urn:uuid:11111111-1111-4111-8111-111111111111";

/// The exact bytes the profile prescribes for a one-file first version.
fn golden() -> String {
    let inst = format!("urn:uuid:{INSTALLATION}");
    let host = "urn:filesquid:software:FileSquid:imagesquid:0.1.0";
    let act = format!("urn:filesquid:activity:op-1:{OBJ1}:v1");
    let ent = format!("urn:filesquid:entity:{OBJ1}:v1:a.txt");
    let time = r#"{"@type":"xsd:dateTime","@value":"2026-10-19T10:00:00Z"}"#;
    let sha = "a".repeat(64);
    [
        r#"{"@context":{"prov":"http://www.w3.org/ns/prov#","xsd":"http://www.w3.org/2001/XMLSchema#","fsq":"https://filesquid.tech/ns/prov/v1#"},"#.to_string(),
        r#""fsq:profile":"filesquid-prov/1","@graph":["#.to_string(),
        format!(r#"{{"@id":"{act}","@type":"prov:Activity","fsq:operation":"op-1","fsq:kind":"import","#),
        format!(r#""fsq:objectVersion":{{"fsq:objectId":"{OBJ1}","fsq:version":"v1"}},"#),
        format!(r#""prov:startedAtTime":{time},"prov:endedAtTime":{time},"fsq:journalEntry":["j-1"],"#),
        format!(r#""prov:wasAssociatedWith":[{{"@id":"{host}"}},{{"@id":"{inst}"}}]}},"#),
        format!(r#"{{"@id":"{host}","@type":"prov:SoftwareAgent","fsq:agentKind":"host","#),
        r#""fsq:product":"FileSquid","fsq:distribution":"imagesquid","fsq:softwareVersion":"0.1.0","#.to_string(),
        format!(r#""prov:actedOnBehalfOf":{{"@id":"{inst}"}}}},"#),
        format!(r#"{{"@id":"{inst}","@type":"prov:Agent","fsq:agentKind":"installation"}},"#),
        format!(r#"{{"@id":"{ent}","@type":"prov:Entity","fsq:objectId":"{OBJ1}","fsq:version":"v1","#),
        format!(r#""fsq:path":"a.txt","fsq:sha256":"{sha}","prov:wasGeneratedBy":{{"@id":"{act}"}},"#),
        format!(r#""prov:wasAttributedTo":[{{"@id":"{inst}"}}]}}]}}"#),
        "\n".to_string(),
    ]
    .concat()
}

#[test]
fn serialization_matches_the_profile_byte_for_byte() {
    let facts = first_version_facts(OBJ1, "op-1", "2026-10-19T10:00:00Z");
    let record = build_record(&facts).expect("build");
    assert_eq!(to_json_ld(&record), golden());
}

#[test]
fn equal_inputs_give_equal_bytes() {
    let one = to_json_ld(&build_record(&standard_facts()).expect("one"));
    let two = to_json_ld(&build_record(&standard_facts()).expect("two"));
    assert_eq!(one, two);
}

#[test]
fn context_and_profile_are_embedded() {
    assert_eq!(CONTEXT_PROV, "http://www.w3.org/ns/prov#");
    assert_eq!(CONTEXT_XSD, "http://www.w3.org/2001/XMLSchema#");
    assert_eq!(CONTEXT_FSQ, "https://filesquid.tech/ns/prov/v1#");
    assert_eq!(PROFILE_V1, "filesquid-prov/1");
    assert_eq!(Profile::V1.name(), PROFILE_V1);
    assert_eq!(Profile::from_name(PROFILE_V1), Some(Profile::V1));
    assert_eq!(Profile::from_name("filesquid-prov/2"), None);
    assert_eq!(Profile::CURRENT, Profile::V1);
    let doc: Value =
        serde_json::from_str(&to_json_ld(&build_record(&standard_facts()).expect("b")))
            .expect("json");
    assert_eq!(doc["@context"]["prov"], CONTEXT_PROV);
    assert_eq!(doc["@context"]["xsd"], CONTEXT_XSD);
    assert_eq!(doc["@context"]["fsq"], CONTEXT_FSQ);
    assert_eq!(doc["fsq:profile"], PROFILE_V1);
}

#[test]
fn graph_lists_activity_then_agents_then_entities_by_identifier() {
    let text = to_json_ld(&build_record(&standard_facts()).expect("build"));
    let doc: Value = serde_json::from_str(&text).expect("json");
    let graph = doc["@graph"].as_array().expect("graph");
    let types: Vec<&str> = graph.iter().map(|n| n["@type"].as_str().unwrap()).collect();
    assert_eq!(types[0], "prov:Activity");
    assert_eq!(types.iter().filter(|t| **t == "prov:Activity").count(), 1);
    let ids: Vec<&str> = graph.iter().map(|n| n["@id"].as_str().unwrap()).collect();
    let agents = &ids[1..3];
    let entities = &ids[3..];
    assert!(agents.windows(2).all(|w| w[0] < w[1]), "{agents:?}");
    assert!(entities.windows(2).all(|w| w[0] < w[1]), "{entities:?}");
    assert_eq!(entities.len(), 6);
}

#[test]
fn no_person_and_no_display_name_anywhere() {
    let text = to_json_ld(&build_record(&standard_facts()).expect("build"));
    for banned in [
        "prov:Person",
        "foaf:",
        "schema:name",
        "rdfs:label",
        "\"name\"",
    ] {
        assert!(!text.contains(banned), "{banned} in {text}");
    }
}

#[test]
fn parse_round_trip_returns_the_same_record() {
    let mut reconstructed = standard_facts();
    reconstructed.reconstruction = Some(Reconstruction {
        recorded_at: "2026-10-20T08:00:00Z".to_string(),
    });
    let mut unrecorded = standard_facts();
    unrecorded.installation = None;
    unrecorded.host = None;
    unrecorded.message = None;
    for facts in [standard_facts(), reconstructed, unrecorded] {
        let record = build_record(&facts).expect("build");
        let text = to_json_ld(&record);
        let parsed = parse_record(text.as_bytes()).expect("parse");
        assert_eq!(parsed, record);
        assert_eq!(to_json_ld(&parsed), text);
    }
}

#[test]
fn member_and_node_order_do_not_change_the_parsed_record() {
    let record = build_record(&standard_facts()).expect("build");
    let mut doc: Value = serde_json::from_str(&to_json_ld(&record)).expect("json");
    doc["@graph"].as_array_mut().expect("graph").reverse();
    // serde_json's default map sorts members, so this also reorders every member.
    let shuffled = serde_json::to_string(&doc).expect("text");
    assert_eq!(parse_record(shuffled.as_bytes()).expect("parse"), record);
}

#[test]
fn reserved_tool_agent_kind_round_trips() {
    let record = build_record(&standard_facts()).expect("build");
    let mut doc: Value = serde_json::from_str(&to_json_ld(&record)).expect("json");
    let tool_id = "urn:filesquid:tool:ocr:2.1";
    doc["@graph"]
        .as_array_mut()
        .expect("graph")
        .push(serde_json::json!({
            "@id": tool_id,
            "@type": "prov:SoftwareAgent",
            "fsq:agentKind": "tool",
            "fsq:toolName": "ocr",
            "fsq:softwareVersion": "2.1",
            "prov:actedOnBehalfOf": {"@id": format!("urn:uuid:{INSTALLATION}")}
        }));
    doc["@graph"][0]["prov:wasAssociatedWith"]
        .as_array_mut()
        .expect("associated")
        .push(serde_json::json!({"@id": tool_id}));
    let parsed = parse_record(serde_json::to_string(&doc).unwrap().as_bytes()).expect("parse");
    let tool = parsed.agent(tool_id).expect("tool agent");
    assert!(matches!(tool.kind(), AgentKind::Tool(_)));
    let again = parse_record(to_json_ld(&parsed).as_bytes()).expect("again");
    assert_eq!(again, parsed);
}
