//! Versioned operation schemas of the API contract (API-S-002, CORE-S-005).
//!
//! The envelopes are owned by `filesquid-core` and re-exported here so hosts and route
//! handlers reach them through the API. The contract documents them as
//! `components.schemas.{CommandEnvelope, QueryEnvelope, CommandOutcome, CommandOutcomeKind,
//! PortError}` and the reusable query parameters `PageSize` (`limit`, 1 to
//! [`MAX_PAGE_SIZE`]) and `Cursor`; the contract drift test serializes these Rust types and
//! checks them against those schemas.
//!
//! Wire convention for operation routes:
//!
//! - A command (mutation) sends a [`CommandEnvelope`] as the `envelope` member of its JSON
//!   request body: `operation_id` is the idempotency key, `expected_revision` the revision
//!   the caller last read, `scope` the permitted scope. It answers with a [`CommandOutcome`].
//! - A query (list) takes the `limit` and `cursor` query parameters. The handler turns
//!   `limit` into the [`QueryEnvelope`] page size; a `limit` outside 1 to [`MAX_PAGE_SIZE`] is
//!   an `invalid_request` error. `cursor` is not part of the envelope (CORE-S-005 defines the
//!   envelope as schema version, scope and page size): it is an opaque paging position the
//!   handler passes to the module next to the envelope, and each page answers with the
//!   cursor of the next page.
//! - Failures answer with [`crate::ApiError`], whose `retryable` flag mirrors
//!   [`PortError::retryable`] for module errors.

pub use filesquid_core::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationId,
    OperationScope, PageSize, PortError, QueryEnvelope, MAX_PAGE_SIZE, PROTOCOL_SCHEMA_VERSION,
};
