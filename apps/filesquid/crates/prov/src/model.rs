//! The PROV-O model (PROV-S-002): one [`Record`] holds exactly one
//! `prov:Activity`, its `prov:Agent`s and its `prov:Entity`s.
//!
//! Relations live on the node PROV-O puts them on: `prov:used` and
//! `prov:wasAssociatedWith` on the activity; `prov:wasGeneratedBy`,
//! `prov:wasDerivedFrom`, `prov:wasInvalidatedBy` and `prov:wasAttributedTo`
//! on entities; `prov:actedOnBehalfOf` on software agents. Every relation
//! holds identifiers of nodes in the same record.
//!
//! There is no `prov:Person`: the owner is a plain `prov:Agent` named by the
//! installation UUID, and no agent carries a person identity or display name.
//! Records are made only by [`crate::build_record`] or [`crate::parse_record`],
//! both of which check every identifier and reference, so a `Record` value
//! always satisfies the profile.

use crate::scalar::{InstallationId, Sha256Digest, Timestamp};

/// A FileSquid host as a `prov:SoftwareAgent`: product, distribution and
/// version, and never a host name.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostFacts {
    /// Product name, for example `FileSquid`.
    pub product: String,
    /// Distribution the host runs, for example `filesquid` or `imagesquid`.
    pub distribution: String,
    /// Software version of the host, for example `0.1.0`.
    pub version: String,
}

/// A tool or model `prov:SoftwareAgent`. Reserved for later AI actions: the
/// profile reads and writes it, but [`crate::build_record`] never produces it.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolFacts {
    /// Tool or model name.
    pub name: String,
    /// Tool or model version.
    pub version: String,
}

/// The role an unrecorded agent stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentRole {
    /// The owner's installation was not observed.
    Installation,
    /// The FileSquid host was not observed.
    Host,
}

impl AgentRole {
    /// The role as the profile spells it (`installation` or `host`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installation => "installation",
            Self::Host => "host",
        }
    }

    /// The role named `text`, if the profile knows it.
    pub(crate) fn from_name(text: &str) -> Option<Self> {
        match text {
            "installation" => Some(Self::Installation),
            "host" => Some(Self::Host),
            _ => None,
        }
    }
}

/// What kind of agent a [`Agent`] is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentKind {
    /// The owner's FileSquid installation: a plain `prov:Agent` `urn:uuid:<uuid>`.
    Installation(InstallationId),
    /// The FileSquid host: a `prov:SoftwareAgent` acting on the installation's behalf.
    Host(HostFacts),
    /// A tool or model `prov:SoftwareAgent` (reserved; never built yet).
    Tool(ToolFacts),
    /// An agent the caller did not observe, stated as unrecorded, never guessed.
    Unrecorded(AgentRole),
}

/// A `prov:Agent` (or `prov:SoftwareAgent`) of a record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Agent {
    pub(crate) id: String,
    pub(crate) kind: AgentKind,
    pub(crate) acted_on_behalf_of: Option<String>,
}

impl Agent {
    /// The agent identifier (see [`crate::ids`] for the forms).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What kind of agent this is.
    pub fn kind(&self) -> &AgentKind {
        &self.kind
    }

    /// `prov:actedOnBehalfOf`: the agent this software agent acted for.
    pub fn acted_on_behalf_of(&self) -> Option<&str> {
        self.acted_on_behalf_of.as_deref()
    }
}

/// `fsq:derivation`: how a record came to be, when not written at commit time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Derivation {
    /// Reconstructed later from OCFL inventories (backfill).
    Reconstructed,
}

impl Derivation {
    /// The value as the profile spells it (`reconstructed`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reconstructed => "reconstructed",
        }
    }

    /// The derivation named `text`, if the profile knows it.
    pub(crate) fn from_name(text: &str) -> Option<Self> {
        (text == "reconstructed").then_some(Self::Reconstructed)
    }
}

/// `fsq:objectVersion`: the OCFL object and version an activity committed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectVersion {
    pub(crate) object_id: String,
    pub(crate) version: String,
}

impl ObjectVersion {
    /// The OCFL object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The OCFL version name as the inventory spells it, for example `v3`.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// The one `prov:Activity` of a record: one committed operation and OCFL version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Activity {
    pub(crate) id: String,
    pub(crate) operation: String,
    pub(crate) kind: String,
    pub(crate) object_version: ObjectVersion,
    pub(crate) started_at: Timestamp,
    pub(crate) ended_at: Timestamp,
    pub(crate) journal_entries: Vec<String>,
    pub(crate) message: Option<String>,
    pub(crate) used: Vec<String>,
    pub(crate) associated_with: Vec<String>,
    pub(crate) derivation: Option<Derivation>,
    pub(crate) recorded_at: Option<Timestamp>,
}

impl Activity {
    /// `urn:filesquid:activity:<operation-id>:<object-id>:<version>`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// `fsq:operation`: the operation identifier, shared by every version one operation commits.
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// `fsq:kind`: the operation kind, for example `import` or `metadata.edit`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// `fsq:objectVersion`: the OCFL object and version this activity committed.
    pub fn object_version(&self) -> &ObjectVersion {
        &self.object_version
    }

    /// `prov:startedAtTime` (RFC 3339 UTC).
    pub fn started_at(&self) -> &Timestamp {
        &self.started_at
    }

    /// `prov:endedAtTime` (RFC 3339 UTC): the inventory `created` time of the version.
    pub fn ended_at(&self) -> &Timestamp {
        &self.ended_at
    }

    /// `fsq:journalEntry`: opaque journal entry identifiers, sorted.
    pub fn journal_entries(&self) -> &[String] {
        &self.journal_entries
    }

    /// `fsq:message`: the OCFL version message, when the caller supplied one.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// `prov:used`: identifiers of the entities this activity used, sorted.
    pub fn used(&self) -> &[String] {
        &self.used
    }

    /// `prov:wasAssociatedWith`: identifiers of every agent of the record, sorted.
    pub fn associated_with(&self) -> &[String] {
        &self.associated_with
    }

    /// `fsq:derivation`: set when the record was reconstructed after the fact.
    pub fn derivation(&self) -> Option<Derivation> {
        self.derivation
    }

    /// `fsq:recordedAt`: when a reconstructed record was written.
    pub fn recorded_at(&self) -> Option<&Timestamp> {
        self.recorded_at.as_ref()
    }
}

/// A `prov:Entity`: the state of one logical path in one OCFL object version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) id: String,
    pub(crate) object_id: String,
    pub(crate) version: String,
    pub(crate) path: String,
    pub(crate) sha256: Sha256Digest,
    pub(crate) generated_by: Option<String>,
    pub(crate) derived_from: Vec<String>,
    pub(crate) invalidated_by: Option<String>,
    pub(crate) attributed_to: Vec<String>,
}

impl Entity {
    /// `urn:filesquid:entity:<object-id>:<version>:<percent-encoded logical path>`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// `fsq:objectId`: the OCFL object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// `fsq:version`: the OCFL version this state belongs to.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// `fsq:path`: the logical path, not encoded.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// `fsq:sha256`: the SHA-256 digest of the content, lower-case hexadecimal.
    pub fn sha256(&self) -> &str {
        self.sha256.as_str()
    }

    /// `prov:wasGeneratedBy`: the activity that generated this state.
    pub fn generated_by(&self) -> Option<&str> {
        self.generated_by.as_deref()
    }

    /// `prov:wasDerivedFrom`: identifiers of the states this one derives from, sorted.
    pub fn derived_from(&self) -> &[String] {
        &self.derived_from
    }

    /// `prov:wasInvalidatedBy`: the activity that removed this state.
    pub fn invalidated_by(&self) -> Option<&str> {
        self.invalidated_by.as_deref()
    }

    /// `prov:wasAttributedTo`: identifiers of the agents this state is attributed to, sorted.
    pub fn attributed_to(&self) -> &[String] {
        &self.attributed_to
    }
}

/// One PROV-O record: exactly one activity, its agents and its entities, each
/// list sorted by identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Record {
    pub(crate) activity: Activity,
    pub(crate) agents: Vec<Agent>,
    pub(crate) entities: Vec<Entity>,
}

impl Record {
    /// The one activity of the record.
    pub fn activity(&self) -> &Activity {
        &self.activity
    }

    /// Every agent, sorted by identifier.
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    /// Every entity, sorted by identifier.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// The agent with identifier `id`.
    pub fn agent(&self, id: &str) -> Option<&Agent> {
        let found = self.agents.binary_search_by(|a| a.id.as_str().cmp(id));
        found.ok().map(|at| &self.agents[at])
    }

    /// The entity with identifier `id`.
    pub fn entity(&self, id: &str) -> Option<&Entity> {
        let found = self.entities.binary_search_by(|e| e.id.as_str().cmp(id));
        found.ok().map(|at| &self.entities[at])
    }
}
