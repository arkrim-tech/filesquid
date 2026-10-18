//! The error for OCFL files that do not follow the specification.

use thiserror::Error;

/// An OCFL file (inventory, sidecar or NAMASTE declaration) that cannot be
/// accepted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FormatError {
    /// The input is larger than the parser's safety bound. This is a limit of
    /// this engine, not an OCFL rule, so it has no validation code.
    #[error("{what} is {size} bytes, above the parser's bound of {limit} bytes")]
    TooLarge {
        /// What was too large.
        what: &'static str,
        /// Its size in bytes.
        size: usize,
        /// The bound that applies.
        limit: usize,
    },
    /// The input breaks an OCFL 1.1 rule.
    #[error("{code}{}: {reason}", location(at))]
    Invalid {
        /// The OCFL 1.1 validation code of the rule, such as `E036`.
        code: &'static str,
        /// Where the fault is: a JSON pointer into an inventory, or a file
        /// name. Empty for the whole input.
        at: String,
        /// What is wrong, for people.
        reason: String,
    },
}

/// ` at "<where>"` for a located fault, nothing for the whole input.
fn location(at: &str) -> String {
    if at.is_empty() {
        return String::new();
    }
    format!(" at {at:?}")
}

impl FormatError {
    /// The OCFL 1.1 validation code (`E###`), or `None` for a safety bound.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Invalid { code, .. } => Some(code),
            Self::TooLarge { .. } => None,
        }
    }

    /// A rule violation with `code` at `at`.
    pub(crate) fn invalid(
        code: &'static str,
        at: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Invalid {
            code,
            at: at.into(),
            reason: reason.into(),
        }
    }
}
