//! The namespace registry (MD-S-012): extension namespaces beside the
//! built-in `dcterms:`, `exif:` and `schema:` ([ADR 022](../../../../../../resources/architecture/022-dublin-core-descriptive-metadata.md)).
//!
//! An owner, a distribution or a module registers an extension namespace: a
//! prefix, an IRI and an optional term list (for example RiC-O). An owner may
//! also declare a custom namespace whose keys are free key:value pairs. A
//! registration never redefines `dcterms:`, `dc:`, `schema:` or `exif:`, a
//! prefix maps to exactly one IRI, and a namespace with a term list rejects
//! unknown terms. Statements check their prefix here
//! ([`Record::set_registered`](crate::record::Record::set_registered)); a
//! prefix that is not registered is rejected by name.
//!
//! The registry is pure data. It persists as its own Markdown record,
//! [`NAMESPACES_MD`] by default, through the canonical store
//! ([`OcflStore::commit_registry_with`](crate::store::ocfl::OcflStore::commit_registry_with)),
//! so a rebuilt index recovers it. The crate ships no term list of its own:
//! a distribution registers the vocabulary it needs (RiC-O for Arkrim).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MetadataError;

/// The default logical path of the registry record.
pub const NAMESPACES_MD: &str = "namespaces.md";

/// The schema identifier of the registry record.
pub const REGISTRY_SCHEMA: &str = "filesquid.namespaces.v1";

/// Most namespaces one registry holds.
pub const MAX_REGISTRATIONS: usize = 1024;

/// Most terms one term list holds.
pub const MAX_TERMS: usize = 10_000;

/// Longest prefix, in bytes.
const MAX_PREFIX_BYTES: usize = 64;

/// Longest IRI, in bytes.
const MAX_IRI_BYTES: usize = 2048;

/// Longest term, in bytes.
const MAX_TERM_BYTES: usize = 255;

/// The built-in prefixes a registration MUST NOT redefine.
const BUILT_IN_PREFIXES: &[&str] = &["dcterms", "dc", "schema", "exif"];

/// Keys the record formats use themselves: the Schema.org map of a file
/// record, the file-record header and `import` list, and this registry's
/// own list. A namespace map under one of them would collide with the format.
const FORMAT_KEYS: &[&str] = &[
    "schemaorg",
    "import",
    "namespaces",
    "original_name",
    "name_normalization",
    "display_name",
    "extension",
    "format",
    "media_type",
];

/// The IRIs of the built-in namespaces: registering one under another
/// prefix would redefine that namespace.
const BUILT_IN_IRIS: &[&str] = &[
    "http://purl.org/dc/terms/",
    "http://purl.org/dc/elements/1.1/",
    "https://schema.org/",
    "http://schema.org/",
    "http://ns.adobe.com/exif/1.0/",
];

/// What a registered namespace is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceKind {
    /// An extension namespace with an IRI and an optional term list.
    Extension,
    /// An owner-declared custom namespace of free key:value pairs.
    Custom,
}

/// Who registered a namespace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Registrant {
    /// The library's owner.
    Owner,
    /// A distribution, by name (for example `arkrim`).
    Distribution(String),
    /// A module, by crate name.
    Module(String),
}

/// One registered namespace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    /// The prefix statements carry (`rico`).
    prefix: String,
    /// The namespace IRI.
    iri: String,
    /// Extension or custom.
    kind: NamespaceKind,
    /// Who registered it.
    registrant: Registrant,
    /// The allowed terms; `None` accepts any valid term name.
    terms: Option<Vec<String>>,
}

impl Registration {
    /// An extension namespace registered by `registrant`, with an optional
    /// term list. [`NamespaceRegistry::register`] checks it.
    pub fn extension(
        prefix: &str,
        iri: &str,
        terms: Option<Vec<String>>,
        registrant: Registrant,
    ) -> Self {
        Self {
            prefix: prefix.to_string(),
            iri: iri.to_string(),
            kind: NamespaceKind::Extension,
            registrant,
            terms,
        }
    }

    /// A custom namespace the owner declares: free keys, no term list.
    pub fn custom(prefix: &str, iri: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            iri: iri.to_string(),
            kind: NamespaceKind::Custom,
            registrant: Registrant::Owner,
            terms: None,
        }
    }

    /// The prefix.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The namespace IRI.
    pub fn iri(&self) -> &str {
        &self.iri
    }

    /// Extension or custom.
    pub fn kind(&self) -> NamespaceKind {
        self.kind
    }

    /// Who registered it.
    pub fn registrant(&self) -> &Registrant {
        &self.registrant
    }

    /// The term list, if the namespace has one.
    pub fn terms(&self) -> Option<&[String]> {
        self.terms.as_deref()
    }

    /// Check every rule a registration keeps on its own.
    fn validate(&self) -> Result<(), RegistryError> {
        check_prefix(&self.prefix)?;
        check_iri(&self.iri)?;
        if self.kind == NamespaceKind::Custom && self.registrant != Registrant::Owner {
            return Err(RegistryError::CustomNotOwner(self.prefix.clone()));
        }
        if self.kind == NamespaceKind::Custom && self.terms.is_some() {
            return Err(RegistryError::CustomWithTerms(self.prefix.clone()));
        }
        self.terms
            .as_deref()
            .map_or(Ok(()), |terms| check_terms(&self.prefix, terms))
    }

    /// Whether `term` may be used in this namespace.
    fn allows(&self, term: &str) -> bool {
        valid_term(term)
            && self
                .terms
                .as_ref()
                .is_none_or(|terms| terms.iter().any(|t| t == term))
    }
}

/// A refused registration or registry record.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    /// The prefix is a built-in namespace or a key of the record format.
    #[error("the prefix {0} is reserved and cannot be registered")]
    ReservedPrefix(String),
    /// The IRI is a built-in namespace's IRI.
    #[error("the IRI {0} is a built-in namespace and cannot be registered again")]
    ReservedIri(String),
    /// The prefix is not 1 to 64 bytes of `[a-z][A-Za-z0-9_-]*`, or starts
    /// with `xml`.
    #[error("the prefix {0:?} is not a valid namespace prefix")]
    InvalidPrefix(String),
    /// The IRI is empty, too long, has no scheme or holds a space or control
    /// character.
    #[error("the IRI is not a valid absolute IRI of at most 2,048 bytes")]
    InvalidIri,
    /// A term is not 1 to 255 bytes of `[A-Za-z_][A-Za-z0-9_.-]*`.
    #[error("the term {0:?} is not a valid term name")]
    InvalidTerm(String),
    /// A term list names a term twice.
    #[error("the term {0} is listed twice")]
    DuplicateTerm(String),
    /// The prefix is registered with another IRI.
    #[error("the prefix {prefix} is already registered with the IRI {iri}")]
    PrefixTaken {
        /// The prefix.
        prefix: String,
        /// The IRI it keeps.
        iri: String,
    },
    /// The IRI is registered under another prefix.
    #[error("the IRI {iri} is already registered as {prefix}")]
    IriTaken {
        /// The IRI.
        iri: String,
        /// The prefix that holds it.
        prefix: String,
    },
    /// A re-registration changes extension into custom or back.
    #[error("the namespace {0} cannot change between extension and custom")]
    KindChanged(String),
    /// A re-registration or save drops terms the namespace accepted.
    #[error("the namespace {0} cannot drop terms it accepted")]
    TermsRemoved(String),
    /// A save drops a registration.
    #[error("the namespace {0} cannot be unregistered")]
    RegistrationRemoved(String),
    /// A custom namespace is declared only by the owner.
    #[error("only the owner may declare the custom namespace {0}")]
    CustomNotOwner(String),
    /// A custom namespace has free keys, never a term list.
    #[error("the custom namespace {0} cannot have a term list")]
    CustomWithTerms(String),
    /// The registry already holds [`MAX_REGISTRATIONS`] namespaces.
    #[error("a registry holds at most 1,024 namespaces")]
    TooManyRegistrations,
    /// A term list exceeds [`MAX_TERMS`].
    #[error("the term list of {0} exceeds 10,000 terms")]
    TooManyTerms(String),
    /// The registry record cannot be read.
    #[error("malformed namespace registry: {0}")]
    Malformed(&'static str),
}

/// The registered extension and custom namespaces of one repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamespaceRegistry {
    /// Registrations in registration order.
    registrations: Vec<Registration>,
}

impl NamespaceRegistry {
    /// Register `registration`. `Ok(true)` when the registry changed;
    /// `Ok(false)` for a registration it already holds. An existing prefix
    /// keeps its IRI and kind and may only add terms to its term list.
    ///
    /// # Errors
    ///
    /// The [`RegistryError`] of the rule the registration breaks; the
    /// registry is unchanged.
    pub fn register(&mut self, registration: Registration) -> Result<bool, RegistryError> {
        registration.validate()?;
        let Some(index) = self.position(&registration.prefix) else {
            self.insert_new(registration)?;
            return Ok(true);
        };
        let existing = &self.registrations[index];
        check_kept(existing, &registration)?;
        if existing.terms == registration.terms {
            return Ok(false);
        }
        self.registrations[index].terms = registration.terms;
        Ok(true)
    }

    /// The registration of `prefix`.
    pub fn get(&self, prefix: &str) -> Option<&Registration> {
        self.registrations.iter().find(|r| r.prefix == prefix)
    }

    /// The prefix registered for `iri`.
    pub fn prefix_of_iri(&self, iri: &str) -> Option<&str> {
        self.registrations
            .iter()
            .find(|r| r.iri == iri)
            .map(|r| r.prefix.as_str())
    }

    /// Every registration, in registration order.
    pub fn registrations(&self) -> &[Registration] {
        &self.registrations
    }

    /// Check that `prefix:term` is a registered namespace and term.
    ///
    /// # Errors
    ///
    /// [`MetadataError::UnregisteredNamespace`] naming the prefix, or
    /// [`MetadataError::UnknownExtensionTerm`] for a term outside the term
    /// list or not a valid term name.
    pub fn check(&self, prefix: &str, term: &str) -> Result<(), MetadataError> {
        let registration = self
            .get(prefix)
            .ok_or_else(|| MetadataError::UnregisteredNamespace(prefix.to_string()))?;
        if registration.allows(term) {
            return Ok(());
        }
        Err(MetadataError::UnknownExtensionTerm {
            prefix: prefix.to_string(),
            term: term.to_string(),
        })
    }

    /// The append-only rule of a registry save: every registration of
    /// `base` is still here with its IRI and kind, and every term it
    /// accepted.
    ///
    /// # Errors
    ///
    /// [`RegistryError::RegistrationRemoved`], [`RegistryError::PrefixTaken`],
    /// [`RegistryError::KindChanged`] or [`RegistryError::TermsRemoved`].
    pub fn check_extends(&self, base: &NamespaceRegistry) -> Result<(), RegistryError> {
        for old in &base.registrations {
            let new = self
                .get(&old.prefix)
                .ok_or_else(|| RegistryError::RegistrationRemoved(old.prefix.clone()))?;
            check_kept(old, new)?;
        }
        Ok(())
    }

    /// Render the registry record: front matter with the schema identifier
    /// and one JSON registration per line.
    pub fn to_markdown(&self) -> String {
        let mut out = format!("---\nschema: {REGISTRY_SCHEMA}\nnamespaces:\n");
        for registration in &self.registrations {
            out.push_str("  - ");
            out.push_str(
                &serde_json::to_string(registration)
                    .expect("a registration of strings always serializes"),
            );
            out.push('\n');
        }
        out.push_str("---\n");
        out
    }

    /// Parse a registry record written by [`to_markdown`](Self::to_markdown),
    /// checking every registration again.
    ///
    /// # Errors
    ///
    /// [`RegistryError::Malformed`] for text that is not a registry record,
    /// a duplicate prefix, or the error of a rule a stored registration breaks.
    pub fn from_markdown(text: &str) -> Result<Self, RegistryError> {
        let mut registry = Self::default();
        for line in entry_lines(text)? {
            let json = line.strip_prefix("  - ").ok_or(RegistryError::Malformed(
                "a line after `namespaces:` is not an entry",
            ))?;
            let registration: Registration = serde_json::from_str(json)
                .map_err(|_| RegistryError::Malformed("a registration is not valid"))?;
            registration.validate()?;
            if registry.get(&registration.prefix).is_some() {
                return Err(RegistryError::Malformed("a prefix is registered twice"));
            }
            registry.insert_new(registration)?;
        }
        Ok(registry)
    }

    /// The index of `prefix`'s registration.
    fn position(&self, prefix: &str) -> Option<usize> {
        self.registrations.iter().position(|r| r.prefix == prefix)
    }

    /// Add a valid registration of a new prefix.
    fn insert_new(&mut self, registration: Registration) -> Result<(), RegistryError> {
        if let Some(prefix) = self.prefix_of_iri(&registration.iri) {
            return Err(RegistryError::IriTaken {
                iri: registration.iri.clone(),
                prefix: prefix.to_string(),
            });
        }
        if self.registrations.len() >= MAX_REGISTRATIONS {
            return Err(RegistryError::TooManyRegistrations);
        }
        self.registrations.push(registration);
        Ok(())
    }
}

/// The entry lines of a registry record: the lines after `namespaces:` in
/// its front matter, at most [`MAX_REGISTRATIONS`] of them.
fn entry_lines(text: &str) -> Result<Vec<&str>, RegistryError> {
    let front = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("---\n").map(|(front, _)| front))
        .ok_or(RegistryError::Malformed("no front matter"))?;
    let mut lines = front.lines();
    if lines.next() != Some(&format!("schema: {REGISTRY_SCHEMA}")) {
        return Err(RegistryError::Malformed(
            "not a filesquid.namespaces.v1 record",
        ));
    }
    if lines.next() != Some("namespaces:") {
        return Err(RegistryError::Malformed("no `namespaces:` list"));
    }
    let entries: Vec<&str> = lines.collect();
    if entries.len() > MAX_REGISTRATIONS {
        return Err(RegistryError::TooManyRegistrations);
    }
    Ok(entries)
}

/// `new` keeps what `old` registered: its IRI, its kind and every term it
/// accepted.
fn check_kept(old: &Registration, new: &Registration) -> Result<(), RegistryError> {
    if new.iri != old.iri {
        return Err(RegistryError::PrefixTaken {
            prefix: old.prefix.clone(),
            iri: old.iri.clone(),
        });
    }
    if new.kind != old.kind {
        return Err(RegistryError::KindChanged(old.prefix.clone()));
    }
    let Some(old_terms) = &old.terms else {
        return match new.terms {
            None => Ok(()),
            Some(_) => Err(RegistryError::TermsRemoved(old.prefix.clone())),
        };
    };
    if old_terms.iter().all(|term| new.allows(term)) {
        return Ok(());
    }
    Err(RegistryError::TermsRemoved(old.prefix.clone()))
}

/// A prefix that is not reserved and is 1 to 64 bytes of
/// `[a-z][A-Za-z0-9_-]*`, not starting with `xml` in any case.
fn check_prefix(prefix: &str) -> Result<(), RegistryError> {
    if BUILT_IN_PREFIXES.contains(&prefix) || FORMAT_KEYS.contains(&prefix) {
        return Err(RegistryError::ReservedPrefix(prefix.to_string()));
    }
    let bytes = prefix.as_bytes();
    let well_formed = (1..=MAX_PREFIX_BYTES).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
        && !prefix.to_ascii_lowercase().starts_with("xml");
    if well_formed {
        return Ok(());
    }
    Err(RegistryError::InvalidPrefix(prefix.to_string()))
}

/// An absolute IRI of at most 2,048 bytes that is not a built-in namespace.
fn check_iri(iri: &str) -> Result<(), RegistryError> {
    if BUILT_IN_IRIS.contains(&iri) {
        return Err(RegistryError::ReservedIri(iri.to_string()));
    }
    let scheme_ok = iri.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    let chars_ok = !iri
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '<' | '>'));
    if scheme_ok && chars_ok && iri.len() <= MAX_IRI_BYTES {
        return Ok(());
    }
    Err(RegistryError::InvalidIri)
}

/// A term list of at most [`MAX_TERMS`] distinct, valid term names.
fn check_terms(prefix: &str, terms: &[String]) -> Result<(), RegistryError> {
    if terms.len() > MAX_TERMS {
        return Err(RegistryError::TooManyTerms(prefix.to_string()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for term in terms {
        if !valid_term(term) {
            return Err(RegistryError::InvalidTerm(term.clone()));
        }
        if !seen.insert(term.as_str()) {
            return Err(RegistryError::DuplicateTerm(term.clone()));
        }
    }
    Ok(())
}

/// A term name that is a YAML key and an XML element name: 1 to 255 bytes
/// of `[A-Za-z_][A-Za-z0-9_.-]*`.
pub fn valid_term(term: &str) -> bool {
    let bytes = term.as_bytes();
    (1..=MAX_TERM_BYTES).contains(&bytes.len())
        && (bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// Whether `key` could be a registered prefix: well formed and not
/// reserved. `metadata.md` parsing keeps such a map as extension statements.
pub fn is_extension_prefix(key: &str) -> bool {
    check_prefix(key).is_ok()
}
