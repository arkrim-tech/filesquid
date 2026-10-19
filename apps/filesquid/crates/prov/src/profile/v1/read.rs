//! Reader of profile `filesquid-prov/1`.
//!
//! Every node is read member by member through [`Node`], which remembers the
//! members it consumed, so an unknown member is refused. Every refusal is a
//! [`ProvError::InvalidNode`] with a fixed reason that names no stored value.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::builder::RECORD_PREFIX;
use crate::error::ProvError;
use crate::graph::assemble;
use crate::model::{
    Activity, Agent, AgentKind, AgentRole, Derivation, Entity, HostFacts, ObjectVersion, Record,
    ToolFacts,
};
use crate::profile::{CONTEXT_FSQ, CONTEXT_PROV, CONTEXT_XSD, MAX_GRAPH_NODES};
use crate::scalar::{
    check_identifier, check_kind, check_label, check_path, check_version, InstallationId,
    Sha256Digest, Timestamp, MAX_MESSAGE_BYTES,
};

/// Result of reading part of a document.
type Read<T> = Result<T, ProvError>;

/// A profile refusal with a fixed `reason`.
fn invalid(reason: &'static str) -> ProvError {
    ProvError::InvalidNode { reason }
}

/// Fail with `reason` unless `ok`.
fn ensure(ok: bool, reason: &'static str) -> Read<()> {
    ok.then_some(()).ok_or(invalid(reason))
}

/// A JSON object being read, with the members consumed so far.
struct Node<'a> {
    map: &'a Map<String, Value>,
    seen: BTreeSet<&'static str>,
}

impl<'a> Node<'a> {
    /// Start reading `value`, which must be a JSON object.
    fn new(value: &'a Value, reason: &'static str) -> Read<Self> {
        let map = value.as_object().ok_or(invalid(reason))?;
        Ok(Self {
            map,
            seen: BTreeSet::new(),
        })
    }

    /// Consume member `key`, if present.
    fn take(&mut self, key: &'static str) -> Option<&'a Value> {
        self.seen.insert(key);
        self.map.get(key)
    }

    /// Consume an optional string member.
    fn opt_string(&mut self, key: &'static str, reason: &'static str) -> Read<Option<&'a str>> {
        self.take(key)
            .map(|value| value.as_str().ok_or(invalid(reason)))
            .transpose()
    }

    /// Consume a required string member.
    fn string(&mut self, key: &'static str, reason: &'static str) -> Read<&'a str> {
        self.opt_string(key, reason)?.ok_or(invalid(reason))
    }

    /// Consume an optional node reference `{"@id": "..."}`.
    fn opt_reference(&mut self, key: &'static str, reason: &'static str) -> Read<Option<String>> {
        self.take(key)
            .map(|value| reference(value, reason))
            .transpose()
    }

    /// Consume an optional array (absent means empty) and read each item.
    fn list<T>(
        &mut self,
        key: &'static str,
        reason: &'static str,
        item: impl Fn(&Value) -> Read<T>,
    ) -> Read<Vec<T>> {
        let Some(value) = self.take(key) else {
            return Ok(Vec::new());
        };
        let items = value.as_array().ok_or(invalid(reason))?;
        ensure(items.len() <= MAX_GRAPH_NODES, reason)?;
        items.iter().map(item).collect()
    }

    /// Consume a list of node references.
    fn references(&mut self, key: &'static str, reason: &'static str) -> Read<Vec<String>> {
        self.list(key, reason, |value| reference(value, reason))
    }

    /// Consume an optional typed `xsd:dateTime` literal.
    fn opt_time(&mut self, key: &'static str, reason: &'static str) -> Read<Option<Timestamp>> {
        self.take(key)
            .map(|value| date_time(value, reason))
            .transpose()
    }

    /// Consume a required typed `xsd:dateTime` literal.
    fn time(&mut self, key: &'static str, reason: &'static str) -> Read<Timestamp> {
        self.opt_time(key, reason)?.ok_or(invalid(reason))
    }

    /// Refuse any member that was not consumed.
    fn finish(self) -> Read<()> {
        let unknown = self.map.keys().any(|key| !self.seen.contains(key.as_str()));
        ensure(
            !unknown,
            "a node holds a member the profile does not define",
        )
    }
}

/// Read a node reference `{"@id": "<identifier>"}` and nothing else.
fn reference(value: &Value, reason: &'static str) -> Read<String> {
    let mut node = Node::new(value, reason)?;
    let id = node.string("@id", reason)?.to_string();
    node.finish()?;
    Ok(id)
}

/// Read `{"@type": "xsd:dateTime", "@value": "<RFC 3339 UTC>"}`.
fn date_time(value: &Value, reason: &'static str) -> Read<Timestamp> {
    let mut node = Node::new(value, reason)?;
    ensure(node.string("@type", reason)? == "xsd:dateTime", reason)?;
    let time = Timestamp::parse(node.string("@value", reason)?).map_err(|_| invalid(reason))?;
    node.finish()?;
    Ok(time)
}

/// Read a validated identifier string.
fn identifier(value: &Value, reason: &'static str) -> Read<String> {
    let text = value.as_str().ok_or(invalid(reason))?;
    ensure(check_identifier(text), reason)?;
    Ok(text.to_string())
}

/// Check the document members and the embedded context.
fn check_document(root: &Map<String, Value>) -> Read<()> {
    let members = ["@context", "fsq:profile", "@graph"];
    let known = root.len() == members.len() && members.iter().all(|m| root.contains_key(*m));
    ensure(
        known,
        "the document members are not @context, fsq:profile and @graph",
    )?;
    let mut context = Node::new(&root["@context"], "the @context is not an object")?;
    let expected = [
        ("prov", CONTEXT_PROV),
        ("xsd", CONTEXT_XSD),
        ("fsq", CONTEXT_FSQ),
    ];
    for (prefix, iri) in expected {
        let found = context.string(prefix, "the @context does not map the profile prefixes")?;
        ensure(found == iri, "the @context maps a prefix to another IRI")?;
    }
    context.finish()
}

/// Nodes of a graph, split by type.
#[derive(Default)]
struct Nodes {
    activities: Vec<Activity>,
    agents: Vec<Agent>,
    entities: Vec<Entity>,
}

/// Read every node of the `@graph` into its type's list.
fn read_graph(graph: &Value) -> Read<Nodes> {
    let items = graph.as_array().ok_or(invalid("@graph is not an array"))?;
    ensure(
        items.len() <= MAX_GRAPH_NODES,
        "@graph holds too many nodes",
    )?;
    let mut nodes = Nodes::default();
    for item in items {
        let kind = item
            .get("@type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "prov:Activity" => nodes.activities.push(read_activity(item)?),
            "prov:Agent" | "prov:SoftwareAgent" => nodes.agents.push(read_agent(item)?),
            "prov:Entity" => nodes.entities.push(read_entity(item)?),
            _ => return Err(invalid("a node has a type the profile does not define")),
        }
    }
    Ok(nodes)
}

/// Read a `filesquid-prov/1` document whose profile member was already checked.
pub(in crate::profile) fn read(root: &Map<String, Value>) -> Result<Record, ProvError> {
    check_document(root)?;
    let mut nodes = read_graph(&root["@graph"])?;
    ensure(
        nodes.activities.len() == 1,
        "the graph does not hold exactly one activity",
    )?;
    let activity = nodes.activities.remove(0);
    assemble(activity, nodes.agents, nodes.entities).map_err(invalid)
}

/// Read `fsq:objectVersion`.
fn read_object_version(value: &Value) -> Read<ObjectVersion> {
    let reason = "fsq:objectVersion is malformed";
    let mut node = Node::new(value, reason)?;
    let object_id = identifier(node.take("fsq:objectId").unwrap_or(&Value::Null), reason)?;
    let version = node.string("fsq:version", reason)?;
    ensure(check_version(version), reason)?;
    let version = version.to_string();
    node.finish()?;
    Ok(ObjectVersion { object_id, version })
}

/// Read the optional `fsq:message`, `fsq:derivation` and `fsq:recordedAt` members.
fn read_activity_extras(
    node: &mut Node<'_>,
) -> Read<(Option<String>, Option<Derivation>, Option<Timestamp>)> {
    let message = node.opt_string("fsq:message", "fsq:message is malformed")?;
    ensure(
        message.is_none_or(|m| m.len() <= MAX_MESSAGE_BYTES),
        "fsq:message is too long",
    )?;
    let derivation = node
        .opt_string("fsq:derivation", "fsq:derivation is malformed")?
        .map(|text| Derivation::from_name(text).ok_or(invalid("fsq:derivation is unknown")))
        .transpose()?;
    let recorded_at = node.opt_time(
        "fsq:recordedAt",
        "fsq:recordedAt is not an RFC 3339 UTC time",
    )?;
    Ok((message.map(str::to_string), derivation, recorded_at))
}

/// Read the activity node.
fn read_activity(value: &Value) -> Read<Activity> {
    let mut node = Node::new(value, "a node is not an object")?;
    node.take("@type");
    let id = node.string("@id", "the activity has no @id")?.to_string();
    let operation = identifier(
        node.take("fsq:operation").unwrap_or(&Value::Null),
        "fsq:operation is malformed",
    )?;
    let kind = node.string("fsq:kind", "fsq:kind is malformed")?;
    ensure(check_kind(kind), "fsq:kind is malformed")?;
    let kind = kind.to_string();
    let object_version =
        read_object_version(node.take("fsq:objectVersion").unwrap_or(&Value::Null))?;
    let started_at = node.time(
        "prov:startedAtTime",
        "prov:startedAtTime is not an RFC 3339 UTC time",
    )?;
    let ended_at = node.time(
        "prov:endedAtTime",
        "prov:endedAtTime is not an RFC 3339 UTC time",
    )?;
    let journal_reason = "fsq:journalEntry is malformed";
    ensure(node.map.contains_key("fsq:journalEntry"), journal_reason)?;
    let journal_entries = node.list("fsq:journalEntry", journal_reason, |v| {
        identifier(v, journal_reason)
    })?;
    let (message, derivation, recorded_at) = read_activity_extras(&mut node)?;
    let used = node.references("prov:used", "prov:used is malformed")?;
    let associated_with = node.references(
        "prov:wasAssociatedWith",
        "prov:wasAssociatedWith is malformed",
    )?;
    node.finish()?;
    Ok(Activity {
        id,
        operation,
        kind,
        object_version,
        started_at,
        ended_at,
        journal_entries,
        message,
        used,
        associated_with,
        derivation,
        recorded_at,
    })
}

/// Read a software label member.
fn label(node: &mut Node<'_>, key: &'static str) -> Read<String> {
    let reason = "a software agent label is malformed";
    let text = node.string(key, reason)?;
    ensure(check_label(text), reason)?;
    Ok(text.to_string())
}

/// Read the kind-specific members of an agent of `@type` `node_type` and `fsq:agentKind` `kind`.
fn read_agent_kind(node: &mut Node<'_>, id: &str, node_type: &str, kind: &str) -> Read<AgentKind> {
    let software = node_type == "prov:SoftwareAgent";
    match (kind, software) {
        ("installation", false) => {
            let uuid = id
                .strip_prefix("urn:uuid:")
                .ok_or(invalid("an installation agent is not urn:uuid"))?;
            let id = InstallationId::parse(uuid)
                .map_err(|_| invalid("an installation agent is not a UUID v4"))?;
            Ok(AgentKind::Installation(id))
        }
        ("host", true) => Ok(AgentKind::Host(HostFacts {
            product: label(node, "fsq:product")?,
            distribution: label(node, "fsq:distribution")?,
            version: label(node, "fsq:softwareVersion")?,
        })),
        ("tool", true) => Ok(AgentKind::Tool(ToolFacts {
            name: label(node, "fsq:toolName")?,
            version: label(node, "fsq:softwareVersion")?,
        })),
        ("unrecorded", false) => {
            let role = node.string("fsq:role", "fsq:role is malformed")?;
            AgentRole::from_name(role)
                .map(AgentKind::Unrecorded)
                .ok_or(invalid("fsq:role is unknown"))
        }
        _ => Err(invalid("fsq:agentKind does not match the agent type")),
    }
}

/// Read an agent node.
fn read_agent(value: &Value) -> Read<Agent> {
    let mut node = Node::new(value, "a node is not an object")?;
    let id = node.string("@id", "an agent has no @id")?.to_string();
    let node_type = node.string("@type", "an agent has no @type")?;
    let kind = node.string("fsq:agentKind", "an agent has no fsq:agentKind")?;
    let kind = read_agent_kind(&mut node, &id, node_type, kind)?;
    let acted_on_behalf_of =
        node.opt_reference("prov:actedOnBehalfOf", "prov:actedOnBehalfOf is malformed")?;
    node.finish()?;
    Ok(Agent {
        id,
        kind,
        acted_on_behalf_of,
    })
}

/// Read an entity node.
fn read_entity(value: &Value) -> Read<Entity> {
    let mut node = Node::new(value, "a node is not an object")?;
    node.take("@type");
    let id = node.string("@id", "an entity has no @id")?.to_string();
    let object_id = identifier(
        node.take("fsq:objectId").unwrap_or(&Value::Null),
        "fsq:objectId is malformed",
    )?;
    let version = node.string("fsq:version", "fsq:version is malformed")?;
    ensure(check_version(version), "fsq:version is malformed")?;
    let path = node.string("fsq:path", "fsq:path is malformed")?;
    ensure(check_path(path), "fsq:path is malformed")?;
    ensure(
        !path.starts_with(RECORD_PREFIX),
        "an entity describes a record path",
    )?;
    let sha256 = Sha256Digest::parse(node.string("fsq:sha256", "fsq:sha256 is malformed")?)
        .map_err(|_| invalid("fsq:sha256 is not a SHA-256 digest"))?;
    let entity = Entity {
        id,
        object_id,
        version: version.to_string(),
        path: path.to_string(),
        sha256,
        generated_by: node
            .opt_reference("prov:wasGeneratedBy", "prov:wasGeneratedBy is malformed")?,
        derived_from: node.references("prov:wasDerivedFrom", "prov:wasDerivedFrom is malformed")?,
        invalidated_by: node.opt_reference(
            "prov:wasInvalidatedBy",
            "prov:wasInvalidatedBy is malformed",
        )?,
        attributed_to: node
            .references("prov:wasAttributedTo", "prov:wasAttributedTo is malformed")?,
    };
    node.finish()?;
    Ok(entity)
}
