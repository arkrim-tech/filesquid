//! Assembly and validation of a [`Record`] graph (PROV-S-002, PROV-S-003).
//!
//! The builder and every profile reader hand their nodes to [`assemble`],
//! which puts lists in canonical order and checks every identifier form and
//! every reference. Errors are fixed reasons that name no stored value.

use std::collections::BTreeSet;

use crate::ids::{activity_id, entity_id, host_agent_id, tool_agent_id, unrecorded_agent_id};
use crate::model::{Activity, Agent, AgentKind, AgentRole, Entity, Record};

/// Outcome of a graph check: `Err` holds a fixed reason.
type Check = Result<(), &'static str>;

/// Sort and deduplicate a list of identifiers in place.
fn canonical(list: &mut Vec<String>) {
    list.sort();
    list.dedup();
}

/// Put every list of the graph in canonical order.
fn canonicalize(activity: &mut Activity, agents: &mut [Agent], entities: &mut [Entity]) {
    canonical(&mut activity.journal_entries);
    canonical(&mut activity.used);
    canonical(&mut activity.associated_with);
    for entity in entities.iter_mut() {
        canonical(&mut entity.derived_from);
        canonical(&mut entity.attributed_to);
    }
    agents.sort_by(|a, b| a.id.cmp(&b.id));
    entities.sort_by(|a, b| a.id.cmp(&b.id));
}

/// Build a record from its nodes after checking that it satisfies the profile.
///
/// # Errors
/// A fixed reason when an identifier does not match its fields, a reference
/// does not resolve, or a rule of the profile is broken.
pub(crate) fn assemble(
    mut activity: Activity,
    mut agents: Vec<Agent>,
    mut entities: Vec<Entity>,
) -> Result<Record, &'static str> {
    canonicalize(&mut activity, &mut agents, &mut entities);
    check_activity(&activity)?;
    check_agents(&agents)?;
    check_entities(&activity, &entities)?;
    let agent_ids: BTreeSet<&str> = agents.iter().map(|a| a.id.as_str()).collect();
    let entity_ids: BTreeSet<&str> = entities.iter().map(|e| e.id.as_str()).collect();
    check_references(&activity, &agents, &entities, &agent_ids, &entity_ids)?;
    check_associations(&activity, &agents)?;
    check_hosts(&agents)?;
    Ok(Record {
        activity,
        agents,
        entities,
    })
}

/// Check the activity's identifier, times and derivation marker.
fn check_activity(activity: &Activity) -> Check {
    let object = &activity.object_version;
    let expected = activity_id(&activity.operation, &object.object_id, &object.version);
    if activity.id != expected {
        return Err("the activity identifier does not match its operation, object and version");
    }
    if activity.started_at.instant() > activity.ended_at.instant() {
        return Err("the activity starts after it ends");
    }
    if activity.derivation.is_some() != activity.recorded_at.is_some() {
        return Err("fsq:derivation and fsq:recordedAt must appear together");
    }
    Ok(())
}

/// The identifier the profile prescribes for an agent of `kind`.
fn expected_agent_id(kind: &AgentKind) -> String {
    match kind {
        AgentKind::Installation(id) => id.urn(),
        AgentKind::Host(host) => host_agent_id(host),
        AgentKind::Tool(tool) => tool_agent_id(tool),
        AgentKind::Unrecorded(role) => unrecorded_agent_id(*role),
    }
}

/// Check agent identifiers, uniqueness and who may act on whose behalf.
fn check_agents(agents: &[Agent]) -> Check {
    if agents.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err("two agents share one identifier");
    }
    for agent in agents {
        if agent.id != expected_agent_id(&agent.kind) {
            return Err("an agent identifier does not match its kind and facts");
        }
        let software = matches!(agent.kind, AgentKind::Host(_) | AgentKind::Tool(_));
        if agent.acted_on_behalf_of.is_some() && !software {
            return Err("only a software agent acts on behalf of another agent");
        }
    }
    Ok(())
}

/// Check entity identifiers, uniqueness and links to the activity.
fn check_entities(activity: &Activity, entities: &[Entity]) -> Check {
    if entities.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err("two entities share one identifier");
    }
    let object = &activity.object_version;
    for entity in entities {
        if entity.id != entity_id(&entity.object_id, &entity.version, &entity.path) {
            return Err("an entity identifier does not match its object, version and path");
        }
        let links = [&entity.generated_by, &entity.invalidated_by];
        if links
            .iter()
            .any(|l| l.as_ref().is_some_and(|id| *id != activity.id))
        {
            return Err("an entity names an activity that is not the record's activity");
        }
        let foreign = entity.object_id != object.object_id || entity.version != object.version;
        if entity.generated_by.is_some() && foreign {
            return Err("a generated entity is not in the committed object version");
        }
        if entity.invalidated_by.is_some() && !foreign {
            return Err("an invalidated entity is in the committed object version");
        }
    }
    Ok(())
}

/// Check that the activity is associated with every agent of the record.
fn check_associations(activity: &Activity, agents: &[Agent]) -> Check {
    let associated: BTreeSet<&str> = activity
        .associated_with
        .iter()
        .map(String::as_str)
        .collect();
    if agents
        .iter()
        .all(|agent| associated.contains(agent.id.as_str()))
    {
        return Ok(());
    }
    Err("an agent is not associated with the record's activity")
}

/// Whether `agent` stands for the installation, recorded or unrecorded.
fn is_installation(agent: &Agent) -> bool {
    matches!(
        agent.kind,
        AgentKind::Installation(_) | AgentKind::Unrecorded(AgentRole::Installation)
    )
}

/// Check that every host acts on behalf of the installation agent (recorded
/// or unrecorded). A record without a host, such as a reconstructed one,
/// passes. References were already resolved by [`check_references`].
fn check_hosts(agents: &[Agent]) -> Check {
    let on_behalf_of_installation = |host: &Agent| {
        let target = host.acted_on_behalf_of.as_deref();
        agents
            .iter()
            .any(|agent| Some(agent.id.as_str()) == target && is_installation(agent))
    };
    let hosts_ok = agents
        .iter()
        .filter(|agent| matches!(agent.kind, AgentKind::Host(_)))
        .all(on_behalf_of_installation);
    match hosts_ok {
        true => Ok(()),
        false => Err("a host does not act on behalf of the installation agent"),
    }
}

/// Check that every relation names a node of the right type in the record.
fn check_references(
    activity: &Activity,
    agents: &[Agent],
    entities: &[Entity],
    agent_ids: &BTreeSet<&str>,
    entity_ids: &BTreeSet<&str>,
) -> Check {
    let resolves =
        |ids: &[String], set: &BTreeSet<&str>| ids.iter().all(|id| set.contains(id.as_str()));
    if !resolves(&activity.used, entity_ids) || !resolves(&activity.associated_with, agent_ids) {
        return Err("an activity relation names a node that is not in the record");
    }
    let entities_resolve = entities.iter().all(|entity| {
        resolves(&entity.derived_from, entity_ids) && resolves(&entity.attributed_to, agent_ids)
    });
    if !entities_resolve {
        return Err("an entity relation names a node that is not in the record");
    }
    let agents_resolve = agents.iter().all(|agent| {
        agent
            .acted_on_behalf_of
            .as_deref()
            .is_none_or(|id| agent_ids.contains(id) && id != agent.id)
    });
    if !agents_resolve {
        return Err("prov:actedOnBehalfOf names an agent that is not in the record");
    }
    Ok(())
}
