//! The result of validating one object: every broken rule as a [`Finding`]
//! with its OCFL 1.1 validation code (OCFL-S-009 (d)).

use std::collections::BTreeSet;
use std::fmt;

use crate::inventory::FormatError;
use crate::read::ReadError;

/// One broken OCFL 1.1 rule: an error (`E###`, a MUST) or a warning
/// (`W###`, a SHOULD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The OCFL 1.1 validation code, such as `E092` or `W010`.
    pub code: &'static str,
    /// Where the fault is: the storage key of the file or directory, or the
    /// key and JSON pointer of an inventory value.
    pub at: String,
    /// What is wrong, for people.
    pub message: String,
}

impl Finding {
    /// Whether this finding is a warning (`W###`) rather than an error.
    pub fn is_warning(&self) -> bool {
        self.code.starts_with('W')
    }
}

impl fmt::Display for Finding {
    /// `CODE at "where": message`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:?}: {}", self.code, self.at, self.message)
    }
}

/// Every error and warning found in one object, in the order the
/// validator met them. An object is valid when it has no errors; warnings
/// never make it invalid.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    object: String,
    errors: Vec<Finding>,
    warnings: Vec<Finding>,
}

impl ValidationReport {
    /// An empty report for the object whose root is `object`.
    pub(crate) fn new(object: &str) -> Self {
        Self {
            object: object.to_string(),
            ..Self::default()
        }
    }

    /// The object root that was validated, as a key prefix.
    pub fn object(&self) -> &str {
        &self.object
    }

    /// Whether the object has no errors.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// The errors (`E###`), in the order found.
    pub fn errors(&self) -> &[Finding] {
        &self.errors
    }

    /// The warnings (`W###`), in the order found.
    pub fn warnings(&self) -> &[Finding] {
        &self.warnings
    }

    /// The distinct error codes.
    pub fn error_codes(&self) -> BTreeSet<&'static str> {
        self.errors.iter().map(|finding| finding.code).collect()
    }

    /// The distinct warning codes.
    pub fn warning_codes(&self) -> BTreeSet<&'static str> {
        self.warnings.iter().map(|finding| finding.code).collect()
    }

    /// Record a broken rule; a `W` code is a warning, any other an error.
    pub(crate) fn add(
        &mut self,
        code: &'static str,
        at: impl Into<String>,
        message: impl Into<String>,
    ) {
        let finding = Finding {
            code,
            at: at.into(),
            message: message.into(),
        };
        tracing::debug!(code, at = %finding.at, "OCFL validation finding");
        let list = if finding.is_warning() {
            &mut self.warnings
        } else {
            &mut self.errors
        };
        list.push(finding);
    }

    /// Record a refusal that carries an OCFL code as a finding. Anything
    /// else (an engine bound, a backend failure, an unsupported version)
    /// means the object could not be validated, so it is returned.
    ///
    /// # Errors
    ///
    /// `err` itself unless it is a [`FormatError::Invalid`].
    pub(crate) fn record(&mut self, err: ReadError) -> Result<(), ReadError> {
        match err {
            ReadError::Format(FormatError::Invalid { code, at, reason }) => {
                self.add(code, at, reason);
                Ok(())
            }
            other => Err(other),
        }
    }
}
