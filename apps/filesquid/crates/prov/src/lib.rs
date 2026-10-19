//! # filesquid-prov
//!
//! The `prov` module of FileSquid: a W3C PROV-O history of OCFL versions and
//! changes in the versioned JSON-LD profile `filesquid-prov/1`. It is a
//! stand-alone crate: it depends on no other FileSquid crate, links no host,
//! database or media library, does no filesystem or network I/O, and builds
//! for `wasm32-unknown-unknown`, so preservation programmes can use it
//! without FileSquid.
//!
//! ## What it holds
//!
//! - [`model`]: one [`Record`] per committed version, with exactly one
//!   `prov:Activity`, its `prov:Agent`s and its `prov:Entity`s and the
//!   relations `prov:used`, `prov:wasGeneratedBy`, `prov:wasDerivedFrom`,
//!   `prov:wasInvalidatedBy`, `prov:wasAssociatedWith`,
//!   `prov:wasAttributedTo` and `prov:actedOnBehalfOf`. The owner is a plain
//!   `prov:Agent` named by an installation UUID (never a `prov:Person`); the
//!   FileSquid host is a `prov:SoftwareAgent` acting on its behalf; an agent
//!   the caller did not observe is stated as unrecorded.
//! - [`builder`]: [`build_record`] turns caller-supplied [`VersionFacts`] (the
//!   logical-state diff of an OCFL version and who committed it) into a record.
//! - [`profile`]: [`to_json_ld`] writes the deterministic JSON-LD document and
//!   [`parse_record`] reads it back under fixed bounds (1 MiB, depth 16, no
//!   repeated keys, known profile, valid nodes) with fixed error codes.
//! - [`index`]: [`HistoryIndex`] answers the activities of an object, an
//!   entity, an operation, an agent or a time range in bounded pages.
//!
//! ## Example
//!
//! ```
//! use filesquid_prov::{build_record, parse_record, to_json_ld, VersionFacts};
//!
//! let facts = VersionFacts {
//!     object_id: "urn:uuid:11111111-1111-4111-8111-111111111111".into(),
//!     version: "v1".into(),
//!     created: "2026-10-19T10:00:01Z".into(),
//!     started: "2026-10-19T10:00:00Z".into(),
//!     operation_id: "op-1".into(),
//!     kind: "import".into(),
//!     new_state: [("photo.jpg".to_string(), "a".repeat(64))].into(),
//!     installation: Some("0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b".into()),
//!     ..VersionFacts::default()
//! };
//! let record = build_record(&facts)?;
//! let text = to_json_ld(&record);
//! assert_eq!(parse_record(text.as_bytes())?, record);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![deny(missing_docs)]

pub mod builder;
pub mod error;
mod graph;
pub mod ids;
pub mod index;
pub mod model;
pub mod profile;
pub mod scalar;

pub use builder::{build_record, OcflUser, Reconstruction, VersionFacts, RECORD_PREFIX};
pub use error::{BuildError, IndexError, InvalidValue, ProvError};
pub use ids::{activity_id, entity_id, percent_encode};
pub use index::{Cursor, HistoryIndex, HistoryQuery, Page, PageRequest, MAX_PAGE_SIZE};
pub use model::{
    Activity, Agent, AgentKind, AgentRole, Derivation, Entity, HostFacts, ObjectVersion, Record,
    ToolFacts,
};
pub use profile::{
    parse_record, to_json_ld, Profile, CONTEXT_FSQ, CONTEXT_PROV, CONTEXT_XSD, MAX_DEPTH,
    MAX_GRAPH_NODES, MAX_RECORD_BYTES, PROFILE_V1,
};
pub use scalar::{InstallationId, Sha256Digest, Timestamp};
