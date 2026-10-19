//! Writer and reader of profile `filesquid-prov/1`.
//!
//! Member order of each node, as written (optional members only when set;
//! relation lists only when not empty):
//!
//! - activity: `@id`, `@type` (`prov:Activity`), `fsq:operation`, `fsq:kind`,
//!   `fsq:objectVersion` {`fsq:objectId`, `fsq:version`}, `prov:startedAtTime`,
//!   `prov:endedAtTime`, `fsq:journalEntry` (always, possibly empty),
//!   `fsq:message`, `prov:used`, `prov:wasAssociatedWith`, `fsq:derivation`,
//!   `fsq:recordedAt`;
//! - agent: `@id`, `@type` (`prov:Agent` or `prov:SoftwareAgent`),
//!   `fsq:agentKind` (`installation`, `host`, `tool` or `unrecorded`), then
//!   `fsq:product`, `fsq:distribution`, `fsq:softwareVersion` (host),
//!   `fsq:toolName`, `fsq:softwareVersion` (tool) or `fsq:role` (unrecorded),
//!   then `prov:actedOnBehalfOf`;
//! - entity: `@id`, `@type` (`prov:Entity`), `fsq:objectId`, `fsq:version`,
//!   `fsq:path`, `fsq:sha256`, `prov:wasGeneratedBy`, `prov:wasDerivedFrom`,
//!   `prov:wasInvalidatedBy`, `prov:wasAttributedTo`.
//!
//! Times are `{"@type":"xsd:dateTime","@value":"<RFC 3339 UTC>"}` and node
//! references are `{"@id":"<identifier>"}`. The reader accepts members in any
//! order and nodes in any order, and refuses unknown members.

mod read;
mod write;

pub(super) use read::read;
pub(super) use write::write;
