//! Versioned command and query ports owned by portable contracts.
//!
//! Hosts (Axum HTTPS and Tauri IPC) share these envelopes. UI progress is never
//! durable completion evidence.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use crate::{DomainError, OperationId};

/// Protocol schema version for command and query envelopes.
pub const PROTOCOL_SCHEMA_VERSION: u32 = 1;

/// Maximum items a portable query page MAY request (CORE-S-005: page size 1–1000).
pub const MAX_PAGE_SIZE: u32 = 1000;

/// Expected catalog or object revision for a mutation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ExpectedRevision(String);

impl ExpectedRevision {
    /// Construct a non-empty revision token.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyScope`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DomainError::EmptyScope);
        }
        Ok(Self(value))
    }

    /// Borrow the revision token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ExpectedRevision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

/// Capability scope permitted for a command or query.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OperationScope(String);

impl OperationScope {
    /// Construct a non-empty scope.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyScope`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DomainError::EmptyScope);
        }
        Ok(Self(value))
    }

    /// Borrow the scope text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OperationScope {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

/// Finite page size for a query. Zero and values above [`MAX_PAGE_SIZE`] fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct PageSize(u32);

impl PageSize {
    /// Construct a bounded page size.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidPageSize`] when `n` is 0 or greater than
    /// [`MAX_PAGE_SIZE`].
    pub fn new(n: u32) -> Result<Self, DomainError> {
        if n == 0 || n > MAX_PAGE_SIZE {
            return Err(DomainError::InvalidPageSize);
        }
        Ok(Self(n))
    }

    /// Return the page size.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for PageSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = u32::deserialize(deserializer)?;
        Self::new(n).map_err(de::Error::custom)
    }
}

/// Versioned command envelope with operation/idempotency key and expected revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    schema_version: u32,
    operation_id: OperationId,
    expected_revision: ExpectedRevision,
    scope: OperationScope,
}

impl CommandEnvelope {
    /// Construct a command envelope at [`PROTOCOL_SCHEMA_VERSION`].
    pub fn new(
        operation_id: OperationId,
        expected_revision: ExpectedRevision,
        scope: OperationScope,
    ) -> Self {
        Self {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            operation_id,
            expected_revision,
            scope,
        }
    }

    /// Return the protocol schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Return the operation/idempotency key.
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// Return the expected revision.
    pub fn expected_revision(&self) -> &ExpectedRevision {
        &self.expected_revision
    }

    /// Return the permitted scope.
    pub fn scope(&self) -> &OperationScope {
        &self.scope
    }
}

/// Versioned query envelope with a bounded page size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEnvelope {
    schema_version: u32,
    page_size: PageSize,
    scope: OperationScope,
}

impl QueryEnvelope {
    /// Construct a query envelope at [`PROTOCOL_SCHEMA_VERSION`].
    pub fn new(page_size: PageSize, scope: OperationScope) -> Self {
        Self {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            page_size,
            scope,
        }
    }

    /// Return the protocol schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Return the bounded page size.
    pub fn page_size(&self) -> PageSize {
        self.page_size
    }

    /// Return the permitted scope.
    pub fn scope(&self) -> &OperationScope {
        &self.scope
    }
}

/// Explicit result of a domain command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcomeKind {
    /// Canonical state was committed.
    Committed,
    /// Work is durable but not yet complete.
    Pending,
    /// The command could not be applied against the expected revision.
    Conflict,
    /// The command failed with a [`PortError`].
    Error,
}

/// Outcome tied to an operation id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    operation_id: OperationId,
    kind: CommandOutcomeKind,
    error: Option<PortError>,
}

impl CommandOutcome {
    /// Construct a non-error outcome. Failures use [`Self::failed`].
    pub fn new(operation_id: OperationId, kind: CommandOutcomeKind) -> Self {
        Self {
            operation_id,
            kind,
            error: None,
        }
    }

    /// Construct an error outcome that carries a [`PortError`].
    pub fn failed(operation_id: OperationId, error: PortError) -> Self {
        Self {
            operation_id,
            kind: CommandOutcomeKind::Error,
            error: Some(error),
        }
    }

    /// Return the operation id.
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// Return the outcome kind.
    pub fn kind(&self) -> CommandOutcomeKind {
        self.kind
    }

    /// Return the port error when this outcome is [`CommandOutcomeKind::Error`].
    pub fn port_error(&self) -> Option<&PortError> {
        self.error.as_ref()
    }

    /// Return whether this outcome is durable completion evidence.
    pub fn is_durable_completion(&self) -> bool {
        self.kind == CommandOutcomeKind::Committed
    }
}

/// Portable error code with retryability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    code: String,
    retryable: bool,
}

impl PortError {
    /// Construct a port error.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyName`] when `code` is empty.
    pub fn new(code: impl Into<String>, retryable: bool) -> Result<Self, DomainError> {
        let code = code.into();
        if code.is_empty() {
            return Err(DomainError::EmptyName);
        }
        Ok(Self { code, retryable })
    }

    /// Return the error code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return whether a caller MAY retry.
    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

/// UI-observed progress. This is never durable completion evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiProgress {
    operation_id: OperationId,
}

impl UiProgress {
    /// Construct UI progress for an operation.
    pub fn new(operation_id: OperationId) -> Self {
        Self { operation_id }
    }

    /// Return the operation id.
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// UI state is never durable completion evidence.
    pub fn is_durable_completion(&self) -> bool {
        false
    }
}
