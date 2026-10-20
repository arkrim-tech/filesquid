//! Namespaced statements and a metadata record.

use crate::namespace::NamespaceRegistry;
use crate::{reject_file_dcterms_term, MetadataError, MetadataNamespace};

/// One populated key/value pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    /// Namespace (`exif`, `dcterms`, `schema` or a registered extension).
    pub namespace: MetadataNamespace,
    /// Local name (`title`, `GPSLatitude`).
    pub term: String,
    /// Literal value.
    pub value: String,
}

impl Statement {
    /// Prefix used in the UI, CSV, and CLI (`dcterms`, or the extension
    /// prefix).
    pub fn prefix(&self) -> &str {
        self.namespace.prefix()
    }

    /// `prefix:term`.
    pub fn curie(&self) -> String {
        format!("{}:{}", self.prefix(), self.term)
    }
}

/// Canonical record stored as `metadata.md`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    /// Populated statements only.
    pub statements: Vec<Statement>,
    /// Optional Markdown body (long-form notes).
    pub body: String,
}

impl Record {
    /// Insert or replace a statement after register checks. Without a
    /// registry no extension prefix is registered, so an
    /// [`Extension`](MetadataNamespace::Extension) statement is refused; use
    /// [`set_registered`](Self::set_registered).
    ///
    /// # Errors
    ///
    /// The `/terms/` register's refusal, or
    /// [`MetadataError::UnregisteredNamespace`] naming an extension prefix.
    pub fn set(
        &mut self,
        namespace: MetadataNamespace,
        term: &str,
        value: impl Into<String>,
    ) -> Result<(), MetadataError> {
        self.set_registered(&NamespaceRegistry::default(), namespace, term, value)
    }

    /// Insert or replace a statement after the `/terms/` register checks
    /// and, for an extension namespace, the `registry` checks (MD-S-012).
    ///
    /// # Errors
    ///
    /// The `/terms/` register's refusal,
    /// [`MetadataError::UnregisteredNamespace`] naming an unregistered
    /// prefix, or [`MetadataError::UnknownExtensionTerm`].
    pub fn set_registered(
        &mut self,
        registry: &NamespaceRegistry,
        namespace: MetadataNamespace,
        term: &str,
        value: impl Into<String>,
    ) -> Result<(), MetadataError> {
        match &namespace {
            MetadataNamespace::Dcterms => reject_file_dcterms_term(term)?,
            MetadataNamespace::Extension(prefix) => registry.check(prefix, term)?,
            MetadataNamespace::Exif | MetadataNamespace::SchemaOrg => {}
        }
        self.insert(namespace, term, value.into());
        Ok(())
    }

    /// Insert or replace a statement without checks: a statement read back
    /// from a canonical record, which keeps what it holds.
    pub(crate) fn insert(&mut self, namespace: MetadataNamespace, term: &str, value: String) {
        if let Some(existing) = self
            .statements
            .iter_mut()
            .find(|s| s.namespace == namespace && s.term == term)
        {
            existing.value = value;
            return;
        }
        self.statements.push(Statement {
            namespace,
            term: term.to_string(),
            value,
        });
    }

    /// Find a statement by CURIE.
    pub fn get(&self, curie: &str) -> Option<&Statement> {
        self.statements.iter().find(|s| s.curie() == curie)
    }
}

/// Parse `exif:GPSLatitude` / `dcterms:title` / `schema:certification`.
/// Without a registry no extension prefix is registered.
///
/// # Errors
///
/// As [`parse_curie_with`].
pub fn parse_curie(curie: &str) -> Result<(MetadataNamespace, &str), MetadataError> {
    parse_curie_with(curie, &NamespaceRegistry::default())
}

/// Parse a CURIE against the built-in namespaces and `registry`: `dc:` maps
/// onto `dcterms:`, `dcterms:` terms pass the `/terms/` register and an
/// extension prefix resolves to its registered namespace, never `dcterms:`.
///
/// # Errors
///
/// [`MetadataError::UnknownDctermsTerm`] for text that is not `prefix:term`
/// or a refused `/terms/` name, [`MetadataError::AccrualOnFile`],
/// [`MetadataError::UnregisteredNamespace`] naming an unknown prefix, or
/// [`MetadataError::UnknownExtensionTerm`].
pub fn parse_curie_with<'c>(
    curie: &'c str,
    registry: &NamespaceRegistry,
) -> Result<(MetadataNamespace, &'c str), MetadataError> {
    let Some((prefix, term)) = curie.split_once(':') else {
        return Err(MetadataError::UnknownDctermsTerm);
    };
    if term.is_empty() {
        return Err(MetadataError::UnknownDctermsTerm);
    }
    resolve_term(prefix, term, registry)
}

/// Resolve `prefix` and `term` as [`parse_curie_with`] does once the CURIE is
/// split.
pub(crate) fn resolve_term<'t>(
    prefix: &str,
    term: &'t str,
    registry: &NamespaceRegistry,
) -> Result<(MetadataNamespace, &'t str), MetadataError> {
    let namespace = match prefix {
        "exif" => MetadataNamespace::Exif,
        "schema" => MetadataNamespace::SchemaOrg,
        "dcterms" | "dc" => return parse_dcterms(prefix, term),
        other => {
            registry.check(other, term)?;
            MetadataNamespace::Extension(other.to_string())
        }
    };
    Ok((namespace, term))
}

/// A `dcterms:` or `dc:` term, mapped and checked against the register.
fn parse_dcterms<'t>(
    prefix: &str,
    term: &'t str,
) -> Result<(MetadataNamespace, &'t str), MetadataError> {
    let term = match prefix {
        "dc" => crate::map_dc_to_dcterms(term).ok_or(MetadataError::UnknownDctermsTerm)?,
        _ => term,
    };
    reject_file_dcterms_term(term)?;
    Ok((MetadataNamespace::Dcterms, term))
}
