//! Deterministic writer of profile `filesquid-prov/1`.
//!
//! The document is assembled from explicitly ordered members, so the output
//! never depends on a map's ordering or on Cargo features of `serde_json`.

use crate::model::{Activity, Agent, AgentKind, Entity, Record};
use crate::profile::{CONTEXT_FSQ, CONTEXT_PROV, CONTEXT_XSD, PROFILE_V1};
use crate::scalar::Timestamp;

/// One object member: name and already rendered JSON value.
type Member = (&'static str, String);

/// Render `text` as a JSON string literal.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Render members as a JSON object, in the order given.
fn object(members: &[Member]) -> String {
    let body: Vec<String> = members
        .iter()
        .map(|(name, value)| format!("{}:{value}", quote(name)))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// Render already rendered values as a JSON array.
fn array(values: &[String]) -> String {
    format!("[{}]", values.join(","))
}

/// A node reference `{"@id": id}`.
fn reference(id: &str) -> String {
    object(&[("@id", quote(id))])
}

/// A list of node references.
fn references(ids: &[String]) -> String {
    let rendered: Vec<String> = ids.iter().map(|id| reference(id)).collect();
    array(&rendered)
}

/// A typed `xsd:dateTime` literal.
fn date_time(time: &Timestamp) -> String {
    object(&[
        ("@type", quote("xsd:dateTime")),
        ("@value", quote(time.as_str())),
    ])
}

/// Push a relation list member when the list is not empty.
fn push_references(members: &mut Vec<Member>, name: &'static str, ids: &[String]) {
    if !ids.is_empty() {
        members.push((name, references(ids)));
    }
}

/// Render the activity node.
fn activity_node(activity: &Activity) -> String {
    let object_version = object(&[
        ("fsq:objectId", quote(activity.object_version().object_id())),
        ("fsq:version", quote(activity.object_version().version())),
    ]);
    let journal: Vec<String> = activity
        .journal_entries()
        .iter()
        .map(|id| quote(id))
        .collect();
    let mut members = vec![
        ("@id", quote(activity.id())),
        ("@type", quote("prov:Activity")),
        ("fsq:operation", quote(activity.operation())),
        ("fsq:kind", quote(activity.kind())),
        ("fsq:objectVersion", object_version),
        ("prov:startedAtTime", date_time(activity.started_at())),
        ("prov:endedAtTime", date_time(activity.ended_at())),
        ("fsq:journalEntry", array(&journal)),
    ];
    if let Some(message) = activity.message() {
        members.push(("fsq:message", quote(message)));
    }
    push_references(&mut members, "prov:used", activity.used());
    push_references(
        &mut members,
        "prov:wasAssociatedWith",
        activity.associated_with(),
    );
    if let Some(derivation) = activity.derivation() {
        members.push(("fsq:derivation", quote(derivation.as_str())));
    }
    if let Some(recorded_at) = activity.recorded_at() {
        members.push(("fsq:recordedAt", date_time(recorded_at)));
    }
    object(&members)
}

/// The `@type`, `fsq:agentKind` and kind-specific members of an agent.
fn agent_kind_members(kind: &AgentKind) -> Vec<Member> {
    match kind {
        AgentKind::Installation(_) => vec![
            ("@type", quote("prov:Agent")),
            ("fsq:agentKind", quote("installation")),
        ],
        AgentKind::Host(host) => vec![
            ("@type", quote("prov:SoftwareAgent")),
            ("fsq:agentKind", quote("host")),
            ("fsq:product", quote(&host.product)),
            ("fsq:distribution", quote(&host.distribution)),
            ("fsq:softwareVersion", quote(&host.version)),
        ],
        AgentKind::Tool(tool) => vec![
            ("@type", quote("prov:SoftwareAgent")),
            ("fsq:agentKind", quote("tool")),
            ("fsq:toolName", quote(&tool.name)),
            ("fsq:softwareVersion", quote(&tool.version)),
        ],
        AgentKind::Unrecorded(role) => vec![
            ("@type", quote("prov:Agent")),
            ("fsq:agentKind", quote("unrecorded")),
            ("fsq:role", quote(role.as_str())),
        ],
    }
}

/// Render an agent node.
fn agent_node(agent: &Agent) -> String {
    let mut members = vec![("@id", quote(agent.id()))];
    members.extend(agent_kind_members(agent.kind()));
    if let Some(principal) = agent.acted_on_behalf_of() {
        members.push(("prov:actedOnBehalfOf", reference(principal)));
    }
    object(&members)
}

/// Render an entity node.
fn entity_node(entity: &Entity) -> String {
    let mut members = vec![
        ("@id", quote(entity.id())),
        ("@type", quote("prov:Entity")),
        ("fsq:objectId", quote(entity.object_id())),
        ("fsq:version", quote(entity.version())),
        ("fsq:path", quote(entity.path())),
        ("fsq:sha256", quote(entity.sha256())),
    ];
    if let Some(activity) = entity.generated_by() {
        members.push(("prov:wasGeneratedBy", reference(activity)));
    }
    push_references(&mut members, "prov:wasDerivedFrom", entity.derived_from());
    if let Some(activity) = entity.invalidated_by() {
        members.push(("prov:wasInvalidatedBy", reference(activity)));
    }
    push_references(&mut members, "prov:wasAttributedTo", entity.attributed_to());
    object(&members)
}

/// The embedded `@context` of profile 1.
fn context() -> String {
    object(&[
        ("prov", quote(CONTEXT_PROV)),
        ("xsd", quote(CONTEXT_XSD)),
        ("fsq", quote(CONTEXT_FSQ)),
    ])
}

/// Serialize `record` as a `filesquid-prov/1` document ending in a newline.
pub(in crate::profile) fn write(record: &Record) -> String {
    let mut nodes = vec![activity_node(record.activity())];
    nodes.extend(record.agents().iter().map(agent_node));
    nodes.extend(record.entities().iter().map(entity_node));
    let document = object(&[
        ("@context", context()),
        ("fsq:profile", quote(PROFILE_V1)),
        ("@graph", array(&nodes)),
    ]);
    format!("{document}\n")
}
