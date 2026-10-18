//! Stand-alone FOSS metadata CRUD engine (`filesquid-metadata`,
//! import `filesquid_metadata`).
//!
//! Extracts metadata from caller-supplied bytes (for example a file read
//! through `filesquid-ocfl`), then creates, reads, updates and deletes
//! namespaced statements. FileSquid is one caller and MUST divert all metadata
//! handling here. Records persist through a pluggable [`store::MetadataStore`]:
//! `sidecar` (default, native), `ocfl` (optional feature, canonical for
//! FileSquid, `metadata.md` written through filesquid-ocfl) or a caller's own.
//!
//! The crate builds for `wasm32-unknown-unknown` with default features off.
//! Cargo features: `sidecar` (default), `ocfl`, `ocfl-fs` (native OCFL path
//! helpers) and `cli` (the native `fsquid` metadata commands).

#![deny(missing_docs)]

#[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
pub mod cli;
pub mod csv;
pub mod extract;
pub mod file_ref;
pub mod json;
pub mod markdown;
pub mod record;
pub mod store;
pub mod xmp;

use thiserror::Error;

/// Namespaces FileSquid and independent callers display on every pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataNamespace {
    /// Technical camera/file observations.
    Exif,
    /// ISO 15836-2 `/terms/` authoring keys.
    Dcterms,
    /// Schema.org overflow after confirmation.
    SchemaOrg,
}

/// Errors from the `/terms/` register.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataError {
    /// Local name is not an ISO 15836-2 `/terms/` property.
    #[error("unknown dcterms local name")]
    UnknownDctermsTerm,
    /// Collection-domain accrual property on a file.
    #[error("accrual terms MUST NOT be authored on a file")]
    AccrualOnFile,
}

const DCTERMS: &[&str] = &[
    "abstract",
    "accessRights",
    "accrualMethod",
    "accrualPeriodicity",
    "accrualPolicy",
    "alternative",
    "audience",
    "available",
    "bibliographicCitation",
    "conformsTo",
    "contributor",
    "coverage",
    "created",
    "creator",
    "date",
    "dateAccepted",
    "dateCopyrighted",
    "dateSubmitted",
    "description",
    "educationLevel",
    "extent",
    "format",
    "hasFormat",
    "hasPart",
    "hasVersion",
    "identifier",
    "instructionalMethod",
    "isFormatOf",
    "isPartOf",
    "isReferencedBy",
    "isReplacedBy",
    "isRequiredBy",
    "issued",
    "isVersionOf",
    "language",
    "license",
    "mediator",
    "medium",
    "modified",
    "provenance",
    "publisher",
    "references",
    "relation",
    "replaces",
    "requires",
    "rights",
    "rightsHolder",
    "source",
    "spatial",
    "subject",
    "tableOfContents",
    "temporal",
    "title",
    "type",
    "valid",
];

const ACCRUAL_ON_FILE: &[&str] = &["accrualMethod", "accrualPeriodicity", "accrualPolicy"];

const DC_TO_DCTERMS: &[&str] = &[
    "contributor",
    "coverage",
    "creator",
    "date",
    "description",
    "format",
    "identifier",
    "language",
    "publisher",
    "relation",
    "rights",
    "source",
    "subject",
    "title",
    "type",
];

/// Return whether `local` is an ISO 15836-2 `/terms/` property.
pub fn is_dcterms_property(local: &str) -> bool {
    DCTERMS.contains(&local)
}

/// Map a legacy `/elements/1.1/` local name onto `/terms/`.
pub fn map_dc_to_dcterms(local: &str) -> Option<&'static str> {
    DC_TO_DCTERMS.iter().copied().find(|name| *name == local)
}

/// ISO 15836-2 `/terms/` ↔ Schema.org 1:1 equivalents, as `(dcterms, schema)`.
///
/// Source: ADR 022 and the DCMI-to-Schema.org working map. Only rows marked
/// 1:1 belong here. Close matches are suggestion targets and MUST NOT
/// auto-alias, so they are deliberately absent.
pub const DCTERMS_SCHEMA_1TO1: &[(&str, &str)] = &[
    ("creator", "creator"),
    ("contributor", "contributor"),
    ("publisher", "publisher"),
    ("title", "name"),
    ("alternative", "alternateName"),
    ("description", "description"),
    ("abstract", "abstract"),
    ("identifier", "identifier"),
    ("language", "inLanguage"),
    ("license", "license"),
    ("subject", "about"),
    ("audience", "audience"),
    ("educationLevel", "educationalLevel"),
    ("spatial", "spatialCoverage"),
    ("temporal", "temporalCoverage"),
    ("created", "dateCreated"),
    ("modified", "dateModified"),
    ("issued", "datePublished"),
    ("hasPart", "hasPart"),
    ("isPartOf", "isPartOf"),
    ("references", "citation"),
    ("rightsHolder", "copyrightHolder"),
];

/// Return the 1:1 Schema.org equivalent of a `/terms/` property, if any.
pub fn dcterms_to_schema(local: &str) -> Option<&'static str> {
    DCTERMS_SCHEMA_1TO1
        .iter()
        .find(|(dcterms, _)| *dcterms == local)
        .map(|(_, schema)| *schema)
}

/// Return the 1:1 `/terms/` equivalent of a Schema.org property, if any.
pub fn schema_to_dcterms(local: &str) -> Option<&'static str> {
    DCTERMS_SCHEMA_1TO1
        .iter()
        .find(|(_, schema)| *schema == local)
        .map(|(dcterms, _)| *dcterms)
}

/// Reject unknown `/terms/` names and file-scoped accrual properties.
pub fn reject_file_dcterms_term(local: &str) -> Result<(), MetadataError> {
    if ACCRUAL_ON_FILE.contains(&local) {
        return Err(MetadataError::AccrualOnFile);
    }
    if !is_dcterms_property(local) {
        return Err(MetadataError::UnknownDctermsTerm);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_allowed() {
        assert_eq!(reject_file_dcterms_term("title"), Ok(()));
    }

    #[test]
    fn name_is_unknown() {
        assert_eq!(
            reject_file_dcterms_term("name"),
            Err(MetadataError::UnknownDctermsTerm)
        );
    }

    #[test]
    fn accrual_is_rejected_on_file() {
        assert_eq!(
            reject_file_dcterms_term("accrualMethod"),
            Err(MetadataError::AccrualOnFile)
        );
    }
}
